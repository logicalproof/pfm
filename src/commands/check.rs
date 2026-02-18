use crate::state::{GateStatus, read_state, write_state};
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub fn run(base: &Path, work_id: &str) -> Result<(), String> {
    let work_dir = base.join(".pfm/work").join(work_id);
    if !work_dir.exists() {
        return Err(format!("work item {} not found", work_id));
    }

    let state_path = work_dir.join("state.json");
    let state = read_state(&state_path)?;

    // Determine working directory
    let cwd = if !state.workspace.worktree.is_empty() {
        state.workspace.worktree.clone()
    } else {
        base.to_string_lossy().to_string()
    };

    let mut all_passed = true;

    // Run verify command
    if !state.commands.verify.is_empty() {
        println!("running verify: {}", state.commands.verify);
        let (success, output) = run_command(&state.commands.verify, &cwd)?;
        append_to_runlog(
            &work_dir,
            &format!(
                "\n## Check: verify — {}\n\nCommand: `{}`\nResult: {}\n\n```\n{}\n```\n",
                Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                state.commands.verify,
                if success { "PASS" } else { "FAIL" },
                output.chars().take(2000).collect::<String>(),
            ),
        )?;
        if success {
            println!("  verify: PASS");
        } else {
            println!("  verify: FAIL");
            all_passed = false;
        }
    } else {
        println!("  verify: (no command configured)");
    }

    // Run security command
    if !state.commands.security.is_empty() {
        println!("running security: {}", state.commands.security);
        let (success, output) = run_command(&state.commands.security, &cwd)?;
        append_to_runlog(
            &work_dir,
            &format!(
                "\n## Check: security — {}\n\nCommand: `{}`\nResult: {}\n\n```\n{}\n```\n",
                Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                state.commands.security,
                if success { "PASS" } else { "FAIL" },
                output.chars().take(2000).collect::<String>(),
            ),
        )?;
        if success {
            println!("  security: PASS");
        } else {
            println!("  security: FAIL");
            all_passed = false;
        }
    } else {
        println!("  security: (no command configured)");
    }

    // Update tests gate based on verify result
    let mut state = read_state(&state_path)?;
    state.gates.set(
        "tests",
        if all_passed {
            GateStatus::Pass
        } else {
            GateStatus::Fail
        },
    );
    state.touch();
    write_state(&state_path, &state)?;

    if all_passed {
        println!("\nall checks passed — tests gate set to pass");
    } else {
        println!("\nchecks failed — tests gate set to fail");
    }

    Ok(())
}

fn run_command(cmd: &str, cwd: &str) -> Result<(bool, String), String> {
    let output = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to run command '{}': {}", cmd, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    Ok((output.status.success(), combined))
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
