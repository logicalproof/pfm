use crate::state::{Role, read_state, write_state, role_to_gate, GateStatus};
use chrono::Utc;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

/// Render the bootstrap prompt for a role agent
pub fn render_bootstrap_prompt(role: &Role, work_dir: &Path, pfm_base: &Path) -> String {
    let role_name = role.to_string();
    let role_spec_path = pfm_base
        .join(".pfm/roles")
        .join(format!("{}.md", role_name));
    let work_dir_str = work_dir.display();
    let role_spec_str = role_spec_path.display();

    format!(
        r#"You are acting as the {role_name} agent.
Read and follow your role spec exactly: {role_spec_str}
Your assigned work item directory is: {work_dir_str}
Start by reading:
1) {work_dir_str}/state.json
2) {work_dir_str}/tasks.md
3) The most recent file in {work_dir_str}/handoffs/ (if any)

Hard requirements:
- Update only the gate you own in state.json (do not modify other gates).
- Log commands, outputs, and failures in {work_dir_str}/runlog.md.
- When finished, write a handoff note to {work_dir_str}/handoffs/{{TIMESTAMP}}-{role_name}.md using the standard format.
- Stop when your role spec stop condition is met."#
    )
}

/// Start a role agent for a work item
pub fn start(base: &Path, role: &Role, work_id: &str) -> Result<(), String> {
    let work_dir = base.join(".pfm/work").join(work_id);
    if !work_dir.exists() {
        return Err(format!("work item {} not found", work_id));
    }

    // Ensure handoffs dir exists
    let handoffs_dir = work_dir.join("handoffs");
    fs::create_dir_all(&handoffs_dir)
        .map_err(|e| format!("failed to create handoffs dir: {}", e))?;

    // Update state: set owner and gate to in_progress
    let state_path = work_dir.join("state.json");
    let mut state = read_state(&state_path)?;
    let gate = role_to_gate(role);
    state.gates.set(gate, GateStatus::InProgress);
    state.owner = role.clone();
    state.touch();
    write_state(&state_path, &state)?;

    // Render bootstrap prompt
    let prompt = render_bootstrap_prompt(role, &work_dir, base);

    // Log agent start
    let now = Utc::now();
    let log_entry = format!(
        "\n## Agent Start: {} — {}\n\nRole: {}\nGate: {}\n",
        now.format("%Y-%m-%d %H:%M:%S UTC"),
        role,
        role,
        gate,
    );
    append_to_runlog(&work_dir, &log_entry)?;

    // Determine working directory (prefer worktree if set)
    let cwd = if !state.workspace.worktree.is_empty() {
        state.workspace.worktree.clone()
    } else {
        base.to_string_lossy().to_string()
    };

    // Try to start in tmux session, fall back to direct execution
    let session_name = format!("pfm-{}-{}", work_id, role);

    if crate::adapters::tmux::is_available() {
        let claude_cmd = format!("claude --print \"{}\"", prompt.replace('"', "\\\""));
        match crate::adapters::tmux::new_session(&session_name, &cwd, &claude_cmd) {
            Ok(()) => {
                println!("started {} agent in tmux session: {}", role, session_name);

                // Update workspace
                let mut state = read_state(&state_path)?;
                state.workspace.tmux_session = session_name;
                state.touch();
                write_state(&state_path, &state)?;

                return Ok(());
            }
            Err(e) => {
                println!("tmux unavailable ({}), running directly...", e);
            }
        }
    }

    // Direct execution: run claude with the prompt
    println!("starting {} agent for {}...", role, work_id);
    println!("---");

    let status = Command::new("claude")
        .args(["--print", &prompt])
        .current_dir(&cwd)
        .status()
        .map_err(|e| format!("failed to start claude: {}", e))?;

    if !status.success() {
        let log_entry = format!(
            "\n## Agent Exit (non-zero): {} — {}\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            role,
        );
        append_to_runlog(&work_dir, &log_entry)?;
        return Err(format!("claude exited with status: {}", status));
    }

    let log_entry = format!(
        "\n## Agent Complete: {} — {}\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        role,
    );
    append_to_runlog(&work_dir, &log_entry)?;

    Ok(())
}

/// Send a nudge/resume message to a running agent
pub fn nudge(base: &Path, role: &Role, work_id: &str) -> Result<(), String> {
    let work_dir = base.join(".pfm/work").join(work_id);
    if !work_dir.exists() {
        return Err(format!("work item {} not found", work_id));
    }

    let state = read_state(&work_dir.join("state.json"))?;
    let session_name = if !state.workspace.tmux_session.is_empty() {
        state.workspace.tmux_session.clone()
    } else {
        format!("pfm-{}-{}", work_id, role)
    };

    let gate = role_to_gate(role);
    let nudge_msg = format!(
        "Resume your work. Check {}/state.json for current state. \
         Your gate is '{}'. Complete your role spec requirements and write a handoff note.",
        work_dir.display(),
        gate,
    );

    if crate::adapters::tmux::session_exists(&session_name) {
        crate::adapters::tmux::send_keys(&session_name, &nudge_msg)?;
        println!("nudged {} agent in session {}", role, session_name);
    } else {
        println!("no active session found for {} agent", role);
        println!("paste this prompt to resume manually:\n");
        println!("{}", nudge_msg);
    }

    Ok(())
}

fn append_to_runlog(work_dir: &Path, entry: &str) -> Result<(), String> {
    let runlog_path = work_dir.join("runlog.md");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&runlog_path)
        .map_err(|e| format!("failed to open runlog: {}", e))?;
    file.write_all(entry.as_bytes())
        .map_err(|e| format!("failed to write runlog: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Role;
    use tempfile::tempdir;

    #[test]
    fn test_render_bootstrap_prompt_contains_role() {
        let dir = tempdir().unwrap();
        let work_dir = dir.path().join("work/FEAT-001");
        let prompt = render_bootstrap_prompt(&Role::Prd, &work_dir, dir.path());
        assert!(prompt.contains("prd agent"));
        assert!(prompt.contains("state.json"));
        assert!(prompt.contains("handoffs"));
        assert!(prompt.contains("role spec"));
    }

    #[test]
    fn test_render_bootstrap_prompt_all_roles() {
        let dir = tempdir().unwrap();
        let work_dir = dir.path().join("work/FEAT-001");
        let roles = vec![
            Role::Prd, Role::Orchestrator, Role::Env, Role::Test,
            Role::Implementation, Role::ReviewSecurity, Role::Qa, Role::Git,
        ];
        for role in roles {
            let prompt = render_bootstrap_prompt(&role, &work_dir, dir.path());
            assert!(prompt.contains(&role.to_string()));
        }
    }
}
