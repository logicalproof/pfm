use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackConfig {
    pub verify: String,
    pub security: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PfmConfig {
    pub default_stack: String,
    pub stacks: HashMap<String, StackConfig>,
}

impl Default for PfmConfig {
    fn default() -> Self {
        let mut stacks = HashMap::new();
        stacks.insert("rails".into(), StackConfig {
            verify: "bundle exec rspec".into(),
            security: "bundle exec brakeman -q".into(),
        });
        stacks.insert("react_native".into(), StackConfig {
            verify: "npm test".into(),
            security: "npm audit".into(),
        });
        stacks.insert("cli_node".into(), StackConfig {
            verify: "npm test".into(),
            security: "npm audit".into(),
        });
        stacks.insert("cli_ruby".into(), StackConfig {
            verify: "bundle exec rspec".into(),
            security: "bundle exec brakeman -q".into(),
        });
        stacks.insert("rust".into(), StackConfig {
            verify: "cargo test".into(),
            security: "cargo audit".into(),
        });
        PfmConfig {
            default_stack: "rails".into(),
            stacks,
        }
    }
}

pub fn read_config(path: &Path) -> Result<PfmConfig, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}

pub fn write_config(path: &Path, config: &PfmConfig) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("failed to serialize config: {}", e))?;
    fs::write(path, content)
        .map_err(|e| format!("failed to write {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_has_all_stacks() {
        let config = PfmConfig::default();
        assert!(config.stacks.contains_key("rails"));
        assert!(config.stacks.contains_key("react_native"));
        assert!(config.stacks.contains_key("cli_node"));
        assert!(config.stacks.contains_key("cli_ruby"));
        assert!(config.stacks.contains_key("rust"));
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = PfmConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: PfmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.default_stack, "rails");
        assert_eq!(parsed.stacks["rails"].verify, "bundle exec rspec");
    }

    #[test]
    fn test_config_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let config = PfmConfig::default();
        write_config(&path, &config).unwrap();
        let loaded = read_config(&path).unwrap();
        assert_eq!(loaded.default_stack, config.default_stack);
        assert_eq!(loaded.stacks.len(), config.stacks.len());
    }
}
