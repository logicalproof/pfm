use std::process::Command;

/// Check if tmux is available
pub fn is_available() -> bool {
    Command::new("which")
        .arg("tmux")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a tmux session exists
pub fn session_exists(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create a new tmux session running a command
pub fn new_session(name: &str, working_dir: &str, command: &str) -> Result<(), String> {
    let status = Command::new("tmux")
        .args([
            "new-session", "-d", "-s", name, "-c", working_dir, command,
        ])
        .status()
        .map_err(|e| format!("failed to create tmux session: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("tmux new-session failed".into())
    }
}

/// Send keys to an existing tmux session
pub fn send_keys(session: &str, keys: &str) -> Result<(), String> {
    let status = Command::new("tmux")
        .args(["send-keys", "-t", session, keys, "Enter"])
        .status()
        .map_err(|e| format!("failed to send keys to tmux: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("tmux send-keys failed".into())
    }
}
