use crate::state::{self, read_state, GATE_ORDER};
use std::path::Path;

/// Show status for a specific work item
pub fn show(base: &Path, work_id: &str) -> Result<(), String> {
    let work_dir = base.join(".pfm/work").join(work_id);
    if !work_dir.exists() {
        return Err(format!("work item {} not found", work_id));
    }

    let state = read_state(&work_dir.join("state.json"))?;

    println!("Work Item: {}", state.id);
    println!("Title:     {}", state.title);
    println!("Repo:      {}", state.repo);
    println!("Branch:    {}", state.branch);
    println!("Status:    {}", state.status);
    println!("Owner:     {}", state.owner);
    println!("Updated:   {}", state.updated_at);
    println!();

    println!("Gates:");
    for gate_name in GATE_ORDER {
        if let Some(status) = state.gates.get(gate_name) {
            let indicator = match status {
                state::GateStatus::Todo => "  ",
                state::GateStatus::InProgress => ">>",
                state::GateStatus::Pass => "OK",
                state::GateStatus::Fail => "XX",
                state::GateStatus::ChangesRequested => "CR",
            };
            println!("  [{}] {:<20} {}", indicator, gate_name, status);
        }
    }

    if !state.workspace.worktree.is_empty()
        || !state.workspace.tmux_session.is_empty()
        || !state.workspace.container.is_empty()
    {
        println!();
        println!("Workspace:");
        if !state.workspace.worktree.is_empty() {
            println!("  worktree: {}", state.workspace.worktree);
        }
        if !state.workspace.tmux_session.is_empty() {
            println!("  tmux:     {}", state.workspace.tmux_session);
        }
        if !state.workspace.container.is_empty() {
            println!("  container: {}", state.workspace.container);
        }
    }

    if !state.commands.verify.is_empty() || !state.commands.security.is_empty() {
        println!();
        println!("Commands:");
        if !state.commands.verify.is_empty() {
            println!("  verify:   {}", state.commands.verify);
        }
        if !state.commands.security.is_empty() {
            println!("  security: {}", state.commands.security);
        }
        if !state.commands.qa_smoke.is_empty() {
            println!("  qa_smoke: {}", state.commands.qa_smoke);
        }
    }

    if !state.notes.is_empty() {
        println!();
        println!("Notes:");
        for note in &state.notes {
            println!("  - {}", note);
        }
    }

    Ok(())
}
