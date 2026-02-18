use crate::config::read_config;
use crate::state::{Commands, WorkState, write_state};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Create a new work item
pub fn new_work(
    base: &Path,
    title: &str,
    id: Option<&str>,
    stack: Option<&str>,
) -> Result<String, String> {
    let pfm_dir = base.join(".pfm");
    if !pfm_dir.exists() {
        return Err("not initialized — run `pfm init` first".into());
    }

    // Generate ID if not provided
    let work_id = match id {
        Some(id) => id.to_string(),
        None => {
            let short = title
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == ' ')
                .collect::<String>()
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join("-")
                .to_lowercase();
            format!("FEAT-{}", if short.is_empty() { "work".to_string() } else { short })
        }
    };

    let work_dir = pfm_dir.join("work").join(&work_id);
    if work_dir.exists() {
        return Err(format!("work item {} already exists", work_id));
    }

    // Read config for stack commands
    let config = read_config(&pfm_dir.join("config.json"))?;
    let detected = detect_stack(base);
    let stack_name = stack
        .or(detected.as_deref())
        .unwrap_or(&config.default_stack);
    let stack_config = config.stacks.get(stack_name)
        .ok_or_else(|| format!("unknown stack: {}", stack_name))?;

    let commands = Commands {
        verify: stack_config.verify.clone(),
        security: stack_config.security.clone(),
        qa_smoke: String::new(),
    };

    // Detect repo name
    let repo = detect_repo_name(base);

    // Create work directory and subdirs
    fs::create_dir_all(work_dir.join("handoffs"))
        .map_err(|e| format!("failed to create work dir: {}", e))?;
    fs::create_dir_all(work_dir.join("artifacts"))
        .map_err(|e| format!("failed to create artifacts dir: {}", e))?;

    // Write state.json
    let state = WorkState::new(&work_id, title, &repo, commands);
    write_state(&work_dir.join("state.json"), &state)?;

    // Copy templates (with placeholder replacement)
    let templates_dir = pfm_dir.join("templates");
    let template_files = ["prd.md", "acceptance.md", "plan.md", "tasks.md", "runlog.md", "qa.md"];
    for filename in &template_files {
        let template_path = templates_dir.join(filename);
        if template_path.exists() {
            let content = fs::read_to_string(&template_path)
                .map_err(|e| format!("failed to read template {}: {}", filename, e))?;
            let content = content
                .replace("{WORK_ID}", &work_id)
                .replace("{TITLE}", title);
            fs::write(work_dir.join(filename), content)
                .map_err(|e| format!("failed to write {}: {}", filename, e))?;
        }
    }

    // Create git branch (best-effort)
    let branch = format!("pfm/{}", work_id);
    let _ = create_branch(base, &branch);

    // Try groot worktree (best-effort)
    if crate::adapters::groot::is_available() {
        match crate::adapters::groot::create_worktree(&branch) {
            Ok(path) => println!("  groot worktree: {}", path),
            Err(e) => println!("  groot worktree skipped: {}", e),
        }
    }

    let how = if stack.is_some() {
        "specified"
    } else if detected.is_some() {
        "detected"
    } else {
        "default"
    };
    println!("created work item: {}", work_id);
    println!("  directory: {}", work_dir.display());
    println!("  branch: {}", branch);
    println!("  stack: {} ({})", stack_name, how);

    Ok(work_id)
}

/// List all work items
pub fn list_work(base: &Path) -> Result<(), String> {
    let work_dir = base.join(".pfm/work");
    if !work_dir.exists() {
        println!("no work items found");
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(&work_dir)
        .map_err(|e| format!("failed to read work dir: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    if entries.is_empty() {
        println!("no work items found");
        return Ok(());
    }

    entries.sort_by_key(|e| e.file_name());

    println!("{:<20} {:<15} {:<15} {}", "ID", "STATUS", "OWNER", "TITLE");
    println!("{}", "-".repeat(70));

    for entry in entries {
        let state_path = entry.path().join("state.json");
        if state_path.exists() {
            match crate::state::read_state(&state_path) {
                Ok(state) => {
                    println!(
                        "{:<20} {:<15} {:<15} {}",
                        state.id, state.status, state.owner, state.title
                    );
                }
                Err(_) => {
                    println!(
                        "{:<20} {:<15} {:<15} {}",
                        entry.file_name().to_string_lossy(),
                        "???",
                        "???",
                        "(invalid state.json)"
                    );
                }
            }
        }
    }

    Ok(())
}

/// Auto-detect stack from repo contents.
/// Checks for marker files in priority order:
///   1. Gemfile + config/routes.rb (or bin/rails) → rails
///   2. package.json with react-native dep → react_native
///   3. package.json → cli_node
///   4. Gemfile → cli_ruby
fn detect_stack(base: &Path) -> Option<String> {
    let has_gemfile = base.join("Gemfile").exists();
    let has_package_json = base.join("package.json").exists();
    let has_rails = base.join("config/routes.rb").exists()
        || base.join("bin/rails").exists()
        || base.join("config/application.rb").exists();

    if has_gemfile && has_rails {
        return Some("rails".into());
    }

    if has_package_json {
        // Check for react-native in package.json
        if let Ok(content) = fs::read_to_string(base.join("package.json")) {
            if content.contains("react-native") {
                return Some("react_native".into());
            }
        }
        return Some("cli_node".into());
    }

    if has_gemfile {
        return Some("cli_ruby".into());
    }

    None
}

fn detect_repo_name(base: &Path) -> String {
    Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(base)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let url = String::from_utf8_lossy(&o.stdout).trim().to_string();
                url.rsplit('/').next().map(|s| s.trim_end_matches(".git").to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            base.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into())
        })
}

fn create_branch(base: &Path, branch: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(["branch", branch])
        .current_dir(base)
        .output()
        .map_err(|e| format!("failed to run git branch: {}", e))?;

    if output.status.success() {
        println!("  created branch: {}", branch);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already exists") {
            println!("  branch exists: {}", branch);
            Ok(())
        } else {
            Err(format!("git branch failed: {}", stderr.trim()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init;
    use tempfile::tempdir;

    fn init_test_repo(dir: &Path) {
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir)
            .output()
            .unwrap();
        init::run(dir).unwrap();
    }

    #[test]
    fn test_new_work_creates_directory() {
        let dir = tempdir().unwrap();
        init_test_repo(dir.path());
        let id = new_work(dir.path(), "Test feature", Some("FEAT-001"), None).unwrap();
        assert_eq!(id, "FEAT-001");
        assert!(dir.path().join(".pfm/work/FEAT-001/state.json").exists());
        assert!(dir.path().join(".pfm/work/FEAT-001/prd.md").exists());
        assert!(dir.path().join(".pfm/work/FEAT-001/handoffs").exists());
        assert!(dir.path().join(".pfm/work/FEAT-001/artifacts").exists());
    }

    #[test]
    fn test_new_work_state_has_correct_values() {
        let dir = tempdir().unwrap();
        init_test_repo(dir.path());
        new_work(dir.path(), "My feature", Some("FEAT-002"), Some("rails")).unwrap();
        let state = crate::state::read_state(
            &dir.path().join(".pfm/work/FEAT-002/state.json"),
        ).unwrap();
        assert_eq!(state.id, "FEAT-002");
        assert_eq!(state.title, "My feature");
        assert_eq!(state.branch, "pfm/FEAT-002");
        assert_eq!(state.commands.verify, "bundle exec rspec");
    }

    #[test]
    fn test_new_work_auto_id() {
        let dir = tempdir().unwrap();
        init_test_repo(dir.path());
        let id = new_work(dir.path(), "Add login page", None, None).unwrap();
        assert_eq!(id, "FEAT-add-login-page");
    }

    #[test]
    fn test_new_work_duplicate_fails() {
        let dir = tempdir().unwrap();
        init_test_repo(dir.path());
        new_work(dir.path(), "Test", Some("FEAT-DUP"), None).unwrap();
        let result = new_work(dir.path(), "Test", Some("FEAT-DUP"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_work_unknown_stack_fails() {
        let dir = tempdir().unwrap();
        init_test_repo(dir.path());
        let result = new_work(dir.path(), "Test", Some("FEAT-X"), Some("unknown_stack"));
        assert!(result.is_err());
    }

    #[test]
    fn test_new_work_without_init_fails() {
        let dir = tempdir().unwrap();
        let result = new_work(dir.path(), "Test", Some("FEAT-X"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_stack_rails() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Gemfile"), "gem 'rails'").unwrap();
        fs::create_dir_all(dir.path().join("config")).unwrap();
        fs::write(dir.path().join("config/routes.rb"), "").unwrap();
        assert_eq!(detect_stack(dir.path()), Some("rails".into()));
    }

    #[test]
    fn test_detect_stack_rails_via_bin() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Gemfile"), "gem 'rails'").unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        fs::write(dir.path().join("bin/rails"), "").unwrap();
        assert_eq!(detect_stack(dir.path()), Some("rails".into()));
    }

    #[test]
    fn test_detect_stack_react_native() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies":{"react-native":"0.72"}}"#,
        ).unwrap();
        assert_eq!(detect_stack(dir.path()), Some("react_native".into()));
    }

    #[test]
    fn test_detect_stack_cli_node() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"dependencies":{"express":"4"}}"#,
        ).unwrap();
        assert_eq!(detect_stack(dir.path()), Some("cli_node".into()));
    }

    #[test]
    fn test_detect_stack_cli_ruby() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Gemfile"), "gem 'thor'").unwrap();
        assert_eq!(detect_stack(dir.path()), Some("cli_ruby".into()));
    }

    #[test]
    fn test_detect_stack_unknown() {
        let dir = tempdir().unwrap();
        assert_eq!(detect_stack(dir.path()), None);
    }

    #[test]
    fn test_detect_stack_explicit_overrides() {
        let dir = tempdir().unwrap();
        init_test_repo(dir.path());
        // Repo has no marker files, so detection returns None → falls back to default (rails)
        // But explicit --stack cli_node should win
        new_work(dir.path(), "Test", Some("FEAT-EXPLICIT"), Some("cli_node")).unwrap();
        let state = crate::state::read_state(
            &dir.path().join(".pfm/work/FEAT-EXPLICIT/state.json"),
        ).unwrap();
        assert_eq!(state.commands.verify, "npm test");
    }
}
