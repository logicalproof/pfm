use std::process::Command;

/// Check if groot CLI is available
pub fn is_available() -> bool {
    Command::new("which")
        .arg("groot")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create a worktree via groot (best-effort)
pub fn create_worktree(branch: &str) -> Result<String, String> {
    let output = Command::new("groot")
        .args(["plant", "--branch", branch])
        .output()
        .map_err(|e| format!("failed to run groot: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Attach to a groot grove/tree
#[allow(dead_code)]
pub fn attach(name: &str) -> Result<(), String> {
    let status = Command::new("groot")
        .args(["attach", name])
        .status()
        .map_err(|e| format!("failed to run groot attach: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err("groot attach failed".into())
    }
}
