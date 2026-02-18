use crate::state::{self, GateStatus, Role, read_state, gate_to_role, GATE_ORDER};
use chrono::Utc;
use std::env;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Run mode
#[derive(Debug, Clone, PartialEq)]
pub enum RunMode {
    Auto,
    Classic,
    Teams,
}

impl RunMode {
    /// Resolve Auto to a concrete mode by checking the environment
    fn resolve(&self) -> RunMode {
        match self {
            RunMode::Auto => {
                if agent_teams_available() {
                    println!("agent teams enabled — using teams mode");
                    RunMode::Teams
                } else {
                    RunMode::Classic
                }
            }
            other => other.clone(),
        }
    }
}

fn agent_teams_available() -> bool {
    env::var("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

impl std::str::FromStr for RunMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(RunMode::Auto),
            "classic" => Ok(RunMode::Classic),
            "teams" => Ok(RunMode::Teams),
            _ => Err(format!("unknown mode: {} (use 'auto', 'classic', or 'teams')", s)),
        }
    }
}

/// Run the pipeline for a work item
pub fn run(
    base: &Path,
    work_id: &str,
    to_gate: Option<&str>,
    mode: RunMode,
) -> Result<(), String> {
    let work_dir = base.join(".pfm/work").join(work_id);
    if !work_dir.exists() {
        return Err(format!("work item {} not found", work_id));
    }

    // Validate --to gate if provided
    if let Some(gate) = to_gate {
        if !GATE_ORDER.contains(&gate) {
            return Err(format!("unknown gate: {} (valid: {:?})", gate, GATE_ORDER));
        }
    }

    let mode = mode.resolve();

    match mode {
        RunMode::Teams => return run_teams(base, work_id, to_gate),
        _ => {}
    }

    println!("starting pipeline for {} (classic mode)", work_id);
    println!();

    loop {
        let state = read_state(&work_dir.join("state.json"))?;

        // Find next gate to process
        let next_gate = match determine_next_gate(&state) {
            Some(gate) => gate,
            None => {
                println!("all gates passed — work item complete!");
                return Ok(());
            }
        };

        // Check if we've reached the target gate (already passed)
        if let Some(target) = to_gate {
            if gate_index(target) < gate_index(next_gate) {
                println!("reached target gate '{}' — stopping", target);
                return Ok(());
            }
        }

        let role = gate_to_role(next_gate)
            .ok_or_else(|| format!("no role for gate: {}", next_gate))?;

        println!("=== gate: {} | role: {} ===", next_gate, role);

        // Start the agent — runs interactively, blocks until user exits
        crate::commands::agent::start(base, &role, work_id)?;

        // Agent session ended — check what happened
        println!();
        let state = read_state(&work_dir.join("state.json"))?;
        let gate_status = state.gates.get(next_gate).cloned().unwrap_or(GateStatus::Todo);

        println!("gate '{}' = {}", next_gate, gate_status);

        if !gate_status.is_terminal() {
            println!("agent exited but gate '{}' is still {} — not complete", next_gate, gate_status);
            println!("  restart with: pfm agent start {} {}", role, work_id);
            return Ok(());
        }

        // Auto-run check after tests/impl gates
        if next_gate == "tests" || next_gate == "impl" {
            println!("running automatic checks...");
            let _ = crate::commands::check::run(base, work_id);
        }

        // Handle reroute rules
        let state = read_state(&work_dir.join("state.json"))?;
        match apply_reroute_rules(&state, next_gate) {
            RerouteAction::Continue => {}
            RerouteAction::RestartRole(role) => {
                println!("rerouting to {} due to gate failure", role);
                crate::commands::agent::start(base, &role, work_id)?;
                continue;
            }
            RerouteAction::NeedHuman(msg) => {
                println!("human intervention needed: {}", msg);
                return Ok(());
            }
        }

        // Check if we've reached the --to target
        if let Some(target) = to_gate {
            if next_gate == target {
                println!("reached target gate '{}' — stopping", target);
                return Ok(());
            }
        }

        println!();
    }
}

/// Run pipeline using Claude Code agent teams.
/// Starts a single lead session that spawns teammates for each remaining role.
fn run_teams(base: &Path, work_id: &str, to_gate: Option<&str>) -> Result<(), String> {
    let work_dir = base.join(".pfm/work").join(work_id);
    let state = read_state(&work_dir.join("state.json"))?;

    // Collect the gates/roles that still need to run
    let mut remaining_roles: Vec<(&str, Role)> = Vec::new();
    for gate_name in GATE_ORDER {
        if let Some(status) = state.gates.get(gate_name) {
            if *status != GateStatus::Pass {
                if let Some(role) = gate_to_role(gate_name) {
                    remaining_roles.push((gate_name, role));
                }
            }
        }
        if let Some(target) = to_gate {
            if *gate_name == target {
                break;
            }
        }
    }

    if remaining_roles.is_empty() {
        println!("all gates passed — work item complete!");
        return Ok(());
    }

    let roles_dir = base.join(".pfm/roles");
    let role_list: Vec<String> = remaining_roles
        .iter()
        .map(|(gate, role)| {
            format!(
                "- **{}** (gate: `{}`): role spec at `{}`",
                role,
                gate,
                roles_dir.join(format!("{}.md", role)).display()
            )
        })
        .collect();

    let prompt = format!(
        r#"You are the PFM orchestrator lead agent running in teams mode.

## Work Item
- ID: {work_id}
- Directory: {work_dir}
- State: {work_dir}/state.json

## Your Job
Spawn a teammate for each role below. Each teammate must:
1. Read their role spec and follow it exactly
2. Read {work_dir}/state.json and {work_dir}/tasks.md before starting
3. Read the most recent file in {work_dir}/handoffs/ for context from prior roles
4. Update ONLY their own gate in {work_dir}/state.json
5. Log commands and outputs in {work_dir}/runlog.md
6. Write a handoff note to {work_dir}/handoffs/{{TIMESTAMP}}-{{ROLE}}.md when done

## Roles to Spawn (in order)
{roles}

## Sequencing Rules
- Roles must execute in the order listed above
- Each role should wait for the prior role's gate to be `pass` before starting
- After `tests` or `impl` gates complete, run the verify command: `{verify}`
- After `impl` gate, run the security command: `{security}`

## Reroute Rules
- If `tests` gate = `fail` → have the implementation teammate fix and retry
- If `review_security` gate = `changes_requested` → have the implementation teammate fix and retry
- If `qa` gate = `fail` → have the implementation teammate fix, then re-run tests and qa

## Completion
When all gates are `pass` (or you reach the target gate), set work status to `done` in state.json.

Start now by creating the team and spawning the first role."#,
        work_id = work_id,
        work_dir = work_dir.display(),
        roles = role_list.join("\n"),
        verify = state.commands.verify,
        security = state.commands.security,
    );

    // Log the teams run start
    let now = Utc::now();
    let log_entry = format!(
        "\n## Teams Run Start: {} — {}\n\nRoles: {}\n",
        now.format("%Y-%m-%d %H:%M:%S UTC"),
        work_id,
        remaining_roles
            .iter()
            .map(|(_, r)| r.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    );
    let runlog_path = work_dir.join("runlog.md");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&runlog_path)
        .map_err(|e| format!("failed to open runlog: {}", e))?;
    std::io::Write::write_all(&mut file, log_entry.as_bytes())
        .map_err(|e| format!("failed to write runlog: {}", e))?;

    println!("starting pipeline for {} (teams mode)", work_id);
    println!(
        "  roles: {}",
        remaining_roles
            .iter()
            .map(|(_, r)| r.to_string())
            .collect::<Vec<_>>()
            .join(" → ")
    );
    println!();

    // Determine working directory
    let cwd = if !state.workspace.worktree.is_empty() {
        state.workspace.worktree.clone()
    } else {
        base.to_string_lossy().to_string()
    };

    // Try tmux first, fall back to direct
    let session_name = format!("pfm-{}-lead", work_id);
    if crate::adapters::tmux::is_available() {
        let claude_cmd = format!("claude --print \"{}\"", prompt.replace('"', "\\\""));
        match crate::adapters::tmux::new_session(&session_name, &cwd, &claude_cmd) {
            Ok(()) => {
                println!("started lead agent in tmux session: {}", session_name);
                println!("  attach with: tmux attach -t {}", session_name);
                println!();

                // Poll for completion of all remaining gates
                let start_time = Utc::now();
                let target = to_gate.unwrap_or(*GATE_ORDER.last().unwrap());
                return wait_for_all_gates(base, work_id, &remaining_roles, target, start_time);
            }
            Err(e) => {
                println!("tmux unavailable ({}), running directly...", e);
            }
        }
    }

    // Direct execution
    println!("starting lead agent...");
    println!("---");

    let status = std::process::Command::new("claude")
        .args(["--print", &prompt])
        .current_dir(&cwd)
        .status()
        .map_err(|e| format!("failed to start claude: {}", e))?;

    if !status.success() {
        return Err(format!("lead agent exited with status: {}", status));
    }

    println!("lead agent finished — checking final gate statuses...");
    let final_state = read_state(&work_dir.join("state.json"))?;
    print_gate_summary(&final_state);

    Ok(())
}

fn wait_for_all_gates(
    base: &Path,
    work_id: &str,
    remaining_roles: &[(&str, Role)],
    target_gate: &str,
    _start_time: chrono::DateTime<Utc>,
) -> Result<(), String> {
    let work_dir = base.join(".pfm/work").join(work_id);
    let state_path = work_dir.join("state.json");

    let max_polls = 360; // 30 minutes at 5s intervals
    for i in 0..max_polls {
        let state = read_state(&state_path)?;

        // Check if all remaining gates up to target are terminal
        let all_done = remaining_roles.iter().all(|(gate_name, _)| {
            if gate_index(gate_name) > gate_index(target_gate) {
                return true; // past target, don't care
            }
            state
                .gates
                .get(gate_name)
                .map(|s| *s == GateStatus::Pass)
                .unwrap_or(false)
        });

        if all_done {
            println!("all target gates passed!");
            print_gate_summary(&state);
            return Ok(());
        }

        // Check for hard failures that need human intervention
        for (gate_name, _) in remaining_roles {
            if let Some(status) = state.gates.get(gate_name) {
                if *status == GateStatus::Fail
                    && *gate_name != "tests"
                    && *gate_name != "qa"
                {
                    // Non-reroutable failure
                    if *gate_name != "review_security" {
                        println!("gate '{}' failed — teams agent should handle rerouting", gate_name);
                    }
                }
            }
        }

        if i > 0 && i % 12 == 0 {
            let state = read_state(&state_path)?;
            let progress: Vec<String> = remaining_roles
                .iter()
                .filter(|(gate_name, _)| gate_index(gate_name) <= gate_index(target_gate))
                .map(|(gate_name, _)| {
                    let status = state
                        .gates
                        .get(gate_name)
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "?".into());
                    format!("{}={}", gate_name, status)
                })
                .collect();
            println!("  progress ({}s): {}", i * 5, progress.join("  "));
        }

        thread::sleep(Duration::from_secs(5));
    }

    println!("timed out waiting for teams completion");
    let state = read_state(&state_path)?;
    print_gate_summary(&state);
    Ok(())
}

fn print_gate_summary(state: &state::WorkState) {
    println!();
    for gate_name in GATE_ORDER {
        if let Some(status) = state.gates.get(gate_name) {
            let icon = match status {
                GateStatus::Pass => "OK",
                GateStatus::Fail => "XX",
                GateStatus::InProgress => ">>",
                GateStatus::ChangesRequested => "CR",
                GateStatus::Todo => "  ",
            };
            println!("  [{}] {:<20} {}", icon, gate_name, status);
        }
    }
}

/// Determine the next gate, considering failures and reroute needs
fn determine_next_gate(state: &state::WorkState) -> Option<&'static str> {
    for gate_name in GATE_ORDER {
        let status = state.gates.get(gate_name)?;
        match status {
            GateStatus::Pass => continue,
            _ => return Some(gate_name),
        }
    }
    None
}

enum RerouteAction {
    Continue,
    RestartRole(Role),
    NeedHuman(String),
}

fn apply_reroute_rules(state: &state::WorkState, gate: &str) -> RerouteAction {
    let status = match state.gates.get(gate) {
        Some(s) => s,
        None => return RerouteAction::Continue,
    };

    match (gate, status) {
        // tests=fail => start implementation
        ("tests", GateStatus::Fail) => {
            RerouteAction::RestartRole(Role::Implementation)
        }
        // review_security=changes_requested => start implementation
        ("review_security", GateStatus::ChangesRequested) => {
            RerouteAction::RestartRole(Role::Implementation)
        }
        // qa=fail => start implementation (will re-run tests and qa)
        ("qa", GateStatus::Fail) => {
            RerouteAction::RestartRole(Role::Implementation)
        }
        // Any other failure that isn't handled
        (_, GateStatus::Fail) => {
            RerouteAction::NeedHuman(format!("gate '{}' failed", gate))
        }
        _ => RerouteAction::Continue,
    }
}

/// Wait for completion: gate is terminal AND handoff file exists (used by teams mode polling)
#[allow(dead_code)]
fn wait_for_completion(
    base: &Path,
    work_id: &str,
    gate: &str,
    role: &Role,
    start_time: chrono::DateTime<Utc>,
) -> Result<bool, String> {
    let work_dir = base.join(".pfm/work").join(work_id);
    let state_path = work_dir.join("state.json");
    let handoffs_dir = work_dir.join("handoffs");
    let role_name = role.to_string();

    // Poll for completion (max ~10 minutes with 5-second intervals)
    let max_polls = 120;
    for i in 0..max_polls {
        // Check gate status
        let state = read_state(&state_path)?;
        let gate_status = state.gates.get(gate).cloned().unwrap_or(GateStatus::Todo);

        if gate_status.is_terminal() {
            // Check for handoff file newer than start time
            if has_recent_handoff(&handoffs_dir, &role_name, start_time) {
                return Ok(true);
            }
        }

        // If this is the first check after agent exited (non-tmux mode),
        // the agent already ran synchronously, so check immediately
        if i == 0 && !crate::adapters::tmux::is_available() {
            // Agent ran synchronously — if gate is terminal, accept it
            // even without handoff (agent might not have written one)
            if gate_status.is_terminal() {
                return Ok(true);
            }
            // Agent ran but didn't update gate — not complete
            return Ok(false);
        }

        if i > 0 && i % 12 == 0 {
            println!("  waiting for {} agent to complete... ({}s)", role, i * 5);
        }

        thread::sleep(Duration::from_secs(5));
    }

    Ok(false)
}

#[allow(dead_code)]
fn has_recent_handoff(
    handoffs_dir: &Path,
    role_name: &str,
    after: chrono::DateTime<Utc>,
) -> bool {
    let entries = match fs::read_dir(handoffs_dir) {
        Ok(e) => e,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.contains(role_name) && name.ends_with(".md") {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    let modified_dt: chrono::DateTime<Utc> = modified.into();
                    if modified_dt > after {
                        return true;
                    }
                }
            }
        }
    }

    false
}

fn gate_index(gate: &str) -> usize {
    GATE_ORDER.iter().position(|g| *g == gate).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{WorkState, Commands, GateStatus};

    fn make_state() -> WorkState {
        WorkState::new("FEAT-001", "Test", "repo", Commands::default())
    }

    #[test]
    fn test_determine_next_gate_all_todo() {
        let state = make_state();
        assert_eq!(determine_next_gate(&state), Some("prd"));
    }

    #[test]
    fn test_determine_next_gate_partial_progress() {
        let mut state = make_state();
        state.gates.prd = GateStatus::Pass;
        state.gates.plan = GateStatus::Pass;
        state.gates.env = GateStatus::Pass;
        assert_eq!(determine_next_gate(&state), Some("tests"));
    }

    #[test]
    fn test_determine_next_gate_all_pass() {
        let mut state = make_state();
        state.gates.prd = GateStatus::Pass;
        state.gates.plan = GateStatus::Pass;
        state.gates.env = GateStatus::Pass;
        state.gates.tests = GateStatus::Pass;
        state.gates.impl_ = GateStatus::Pass;
        state.gates.review_security = GateStatus::Pass;
        state.gates.qa = GateStatus::Pass;
        state.gates.git = GateStatus::Pass;
        assert_eq!(determine_next_gate(&state), None);
    }

    #[test]
    fn test_determine_next_gate_failed_gate() {
        let mut state = make_state();
        state.gates.prd = GateStatus::Pass;
        state.gates.plan = GateStatus::Fail;
        assert_eq!(determine_next_gate(&state), Some("plan"));
    }

    #[test]
    fn test_reroute_tests_fail() {
        let mut state = make_state();
        state.gates.tests = GateStatus::Fail;
        match apply_reroute_rules(&state, "tests") {
            RerouteAction::RestartRole(Role::Implementation) => {}
            _ => panic!("expected RestartRole(Implementation)"),
        }
    }

    #[test]
    fn test_reroute_review_changes_requested() {
        let mut state = make_state();
        state.gates.review_security = GateStatus::ChangesRequested;
        match apply_reroute_rules(&state, "review_security") {
            RerouteAction::RestartRole(Role::Implementation) => {}
            _ => panic!("expected RestartRole(Implementation)"),
        }
    }

    #[test]
    fn test_reroute_qa_fail() {
        let mut state = make_state();
        state.gates.qa = GateStatus::Fail;
        match apply_reroute_rules(&state, "qa") {
            RerouteAction::RestartRole(Role::Implementation) => {}
            _ => panic!("expected RestartRole(Implementation)"),
        }
    }

    #[test]
    fn test_reroute_pass_continues() {
        let mut state = make_state();
        state.gates.prd = GateStatus::Pass;
        match apply_reroute_rules(&state, "prd") {
            RerouteAction::Continue => {}
            _ => panic!("expected Continue"),
        }
    }

    #[test]
    fn test_gate_index() {
        assert_eq!(gate_index("prd"), 0);
        assert_eq!(gate_index("git"), 7);
        assert_eq!(gate_index("nonexistent"), usize::MAX);
    }

    #[test]
    fn test_run_mode_parse() {
        assert_eq!("auto".parse::<RunMode>().unwrap(), RunMode::Auto);
        assert_eq!("classic".parse::<RunMode>().unwrap(), RunMode::Classic);
        assert_eq!("teams".parse::<RunMode>().unwrap(), RunMode::Teams);
        assert!("invalid".parse::<RunMode>().is_err());
    }

    #[test]
    fn test_auto_resolves_to_classic_without_env() {
        // Ensure env var is not set in test
        env::remove_var("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS");
        assert_eq!(RunMode::Auto.resolve(), RunMode::Classic);
    }

    #[test]
    fn test_auto_resolves_to_teams_with_env() {
        env::set_var("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "1");
        assert_eq!(RunMode::Auto.resolve(), RunMode::Teams);
        env::remove_var("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS");
    }

    #[test]
    fn test_explicit_mode_not_overridden() {
        env::set_var("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS", "1");
        assert_eq!(RunMode::Classic.resolve(), RunMode::Classic);
        env::remove_var("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS");
    }
}
