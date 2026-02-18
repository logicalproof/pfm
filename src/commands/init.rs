use crate::config::{PfmConfig, write_config};
use crate::templates;
use std::fs;
use std::path::Path;

pub fn run(base: &Path) -> Result<(), String> {
    let pfm_dir = base.join(".pfm");

    // Create directory structure
    let dirs = [
        pfm_dir.clone(),
        pfm_dir.join("roles"),
        pfm_dir.join("work"),
        pfm_dir.join("templates"),
        pfm_dir.join("runtime"),
    ];

    for dir in &dirs {
        fs::create_dir_all(dir)
            .map_err(|e| format!("failed to create {}: {}", dir.display(), e))?;
    }

    // Write config.json if missing
    let config_path = pfm_dir.join("config.json");
    if !config_path.exists() {
        let config = PfmConfig::default();
        write_config(&config_path, &config)?;
        println!("  created {}", config_path.display());
    } else {
        println!("  exists  {}", config_path.display());
    }

    // Write templates if missing
    for (filename, content) in templates::work_templates() {
        let path = pfm_dir.join("templates").join(filename);
        if !path.exists() {
            fs::write(&path, content)
                .map_err(|e| format!("failed to write {}: {}", path.display(), e))?;
            println!("  created {}", path.display());
        } else {
            println!("  exists  {}", path.display());
        }
    }

    // Write role specs if missing
    for (filename, content) in templates::role_specs() {
        let path = pfm_dir.join("roles").join(filename);
        if !path.exists() {
            fs::write(&path, content)
                .map_err(|e| format!("failed to write {}: {}", path.display(), e))?;
            println!("  created {}", path.display());
        } else {
            println!("  exists  {}", path.display());
        }
    }

    // Write .gitignore for runtime dir
    let gitignore_path = pfm_dir.join("runtime").join(".gitignore");
    if !gitignore_path.exists() {
        fs::write(&gitignore_path, "*\n!.gitignore\n")
            .map_err(|e| format!("failed to write .gitignore: {}", e))?;
    }

    println!("\npfm initialized at {}", pfm_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_init_creates_structure() {
        let dir = tempdir().unwrap();
        run(dir.path()).unwrap();

        assert!(dir.path().join(".pfm").exists());
        assert!(dir.path().join(".pfm/roles").exists());
        assert!(dir.path().join(".pfm/work").exists());
        assert!(dir.path().join(".pfm/templates").exists());
        assert!(dir.path().join(".pfm/runtime").exists());
        assert!(dir.path().join(".pfm/config.json").exists());
    }

    #[test]
    fn test_init_creates_templates() {
        let dir = tempdir().unwrap();
        run(dir.path()).unwrap();

        for (filename, _) in templates::work_templates() {
            assert!(dir.path().join(".pfm/templates").join(filename).exists());
        }
    }

    #[test]
    fn test_init_creates_role_specs() {
        let dir = tempdir().unwrap();
        run(dir.path()).unwrap();

        for (filename, _) in templates::role_specs() {
            assert!(dir.path().join(".pfm/roles").join(filename).exists());
        }
    }

    #[test]
    fn test_init_idempotent() {
        let dir = tempdir().unwrap();
        run(dir.path()).unwrap();
        // Running again should not fail
        run(dir.path()).unwrap();
        assert!(dir.path().join(".pfm/config.json").exists());
    }

    #[test]
    fn test_init_config_valid_json() {
        let dir = tempdir().unwrap();
        run(dir.path()).unwrap();
        let config = crate::config::read_config(&dir.path().join(".pfm/config.json")).unwrap();
        assert_eq!(config.default_stack, "rails");
    }
}
