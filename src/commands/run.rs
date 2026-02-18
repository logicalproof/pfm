use crate::state::{self, GateStatus, Role, read_state, gate_to_role, GATE_ORDER};
use chrono::Utc;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Run mode
#[derive(Debug, Clone, PartialEq)]
pub enum RunMode {
    Classic,
    Teams,
}

impl std::str::FromStr for RunMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "classic" => Ok(RunMode::Classic),
            "teams" => Ok(RunMode::Teams),
            _ => Err(format!("unknown mode: {} (use 'classic' or 'teams')", s)),
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

    if mode == RunMode::Teams {
        println!("teams mode not yet available, falling back to classic mode");
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

        // Start the agent
        crate::commands::agent::start(base, &role, work_id)?;

        // Wait for completion signals
        let start_time = Utc::now();
        let completed = wait_for_completion(base, work_id, next_gate, &role, start_time)?;

        if !completed {
            println!("agent did not complete — manual intervention may be needed");
            println!("  resume with: pfm agent nudge {} {}", role, work_id);
            return Ok(());
        }

        // Re-read state after agent completion
        let state = read_state(&work_dir.join("state.json"))?;
        let gate_status = state.gates.get(next_gate).cloned().unwrap_or(GateStatus::Todo);

        println!("gate '{}' = {}", next_gate, gate_status);

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

/// Wait for completion: gate is terminal AND handoff file exists
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
        assert_eq!("classic".parse::<RunMode>().unwrap(), RunMode::Classic);
        assert_eq!("teams".parse::<RunMode>().unwrap(), RunMode::Teams);
        assert!("invalid".parse::<RunMode>().is_err());
    }
}
