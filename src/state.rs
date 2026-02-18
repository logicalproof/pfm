use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Gate statuses for each pipeline phase
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Todo,
    InProgress,
    Pass,
    Fail,
    ChangesRequested,
}

impl std::fmt::Display for GateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GateStatus::Todo => write!(f, "todo"),
            GateStatus::InProgress => write!(f, "in_progress"),
            GateStatus::Pass => write!(f, "pass"),
            GateStatus::Fail => write!(f, "fail"),
            GateStatus::ChangesRequested => write!(f, "changes_requested"),
        }
    }
}

impl GateStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, GateStatus::Pass | GateStatus::Fail | GateStatus::ChangesRequested)
    }
}

/// Work item status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    InProgress,
    Blocked,
    Done,
}

impl std::fmt::Display for WorkStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkStatus::InProgress => write!(f, "in_progress"),
            WorkStatus::Blocked => write!(f, "blocked"),
            WorkStatus::Done => write!(f, "done"),
        }
    }
}

/// Role names that own work
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Prd,
    Orchestrator,
    Env,
    Test,
    Implementation,
    ReviewSecurity,
    Qa,
    Git,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Prd => write!(f, "prd"),
            Role::Orchestrator => write!(f, "orchestrator"),
            Role::Env => write!(f, "env"),
            Role::Test => write!(f, "test"),
            Role::Implementation => write!(f, "implementation"),
            Role::ReviewSecurity => write!(f, "review_security"),
            Role::Qa => write!(f, "qa"),
            Role::Git => write!(f, "git"),
        }
    }
}

impl std::str::FromStr for Role {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "prd" => Ok(Role::Prd),
            "orchestrator" => Ok(Role::Orchestrator),
            "env" => Ok(Role::Env),
            "test" => Ok(Role::Test),
            "implementation" => Ok(Role::Implementation),
            "review_security" => Ok(Role::ReviewSecurity),
            "qa" => Ok(Role::Qa),
            "git" => Ok(Role::Git),
            _ => Err(format!("unknown role: {s}")),
        }
    }
}

/// Gate names in pipeline order
pub const GATE_ORDER: &[&str] = &[
    "prd",
    "plan",
    "env",
    "tests",
    "impl",
    "review_security",
    "qa",
    "git",
];

/// Map gate name to the role that owns it
pub fn gate_to_role(gate: &str) -> Option<Role> {
    match gate {
        "prd" => Some(Role::Prd),
        "plan" => Some(Role::Orchestrator),
        "env" => Some(Role::Env),
        "tests" => Some(Role::Test),
        "impl" => Some(Role::Implementation),
        "review_security" => Some(Role::ReviewSecurity),
        "qa" => Some(Role::Qa),
        "git" => Some(Role::Git),
        _ => None,
    }
}

/// Map role to its owned gate
pub fn role_to_gate(role: &Role) -> &'static str {
    match role {
        Role::Prd => "prd",
        Role::Orchestrator => "plan",
        Role::Env => "env",
        Role::Test => "tests",
        Role::Implementation => "impl",
        Role::ReviewSecurity => "review_security",
        Role::Qa => "qa",
        Role::Git => "git",
    }
}

/// All gates initialized to Todo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gates {
    pub prd: GateStatus,
    pub plan: GateStatus,
    pub env: GateStatus,
    pub tests: GateStatus,
    #[serde(rename = "impl")]
    pub impl_: GateStatus,
    pub review_security: GateStatus,
    pub qa: GateStatus,
    pub git: GateStatus,
}

impl Default for Gates {
    fn default() -> Self {
        Gates {
            prd: GateStatus::Todo,
            plan: GateStatus::Todo,
            env: GateStatus::Todo,
            tests: GateStatus::Todo,
            impl_: GateStatus::Todo,
            review_security: GateStatus::Todo,
            qa: GateStatus::Todo,
            git: GateStatus::Todo,
        }
    }
}

impl Gates {
    pub fn get(&self, gate: &str) -> Option<&GateStatus> {
        match gate {
            "prd" => Some(&self.prd),
            "plan" => Some(&self.plan),
            "env" => Some(&self.env),
            "tests" => Some(&self.tests),
            "impl" => Some(&self.impl_),
            "review_security" => Some(&self.review_security),
            "qa" => Some(&self.qa),
            "git" => Some(&self.git),
            _ => None,
        }
    }

    pub fn set(&mut self, gate: &str, status: GateStatus) -> bool {
        match gate {
            "prd" => self.prd = status,
            "plan" => self.plan = status,
            "env" => self.env = status,
            "tests" => self.tests = status,
            "impl" => self.impl_ = status,
            "review_security" => self.review_security = status,
            "qa" => self.qa = status,
            "git" => self.git = status,
            _ => return false,
        }
        true
    }
}

/// Commands to run for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commands {
    pub verify: String,
    pub security: String,
    #[serde(default)]
    pub qa_smoke: String,
}

impl Default for Commands {
    fn default() -> Self {
        Commands {
            verify: String::new(),
            security: String::new(),
            qa_smoke: String::new(),
        }
    }
}

/// Workspace pointers (runtime, optional)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(default)]
    pub worktree: String,
    #[serde(default)]
    pub tmux_session: String,
    #[serde(default)]
    pub container: String,
}

/// The main state file for a work item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkState {
    pub id: String,
    pub title: String,
    pub repo: String,
    pub branch: String,
    pub status: WorkStatus,
    pub owner: Role,
    pub updated_at: String,
    pub gates: Gates,
    pub commands: Commands,
    pub workspace: Workspace,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl WorkState {
    pub fn new(id: &str, title: &str, repo: &str, commands: Commands) -> Self {
        WorkState {
            id: id.to_string(),
            title: title.to_string(),
            repo: repo.to_string(),
            branch: format!("pfm/{}", id),
            status: WorkStatus::InProgress,
            owner: Role::Prd,
            updated_at: Utc::now().to_rfc3339(),
            gates: Gates::default(),
            commands,
            workspace: Workspace::default(),
            notes: vec![],
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now().to_rfc3339();
    }

    /// Find next gate that isn't pass, in pipeline order
    #[allow(dead_code)]
    pub fn next_pending_gate(&self) -> Option<&'static str> {
        for gate_name in GATE_ORDER {
            if let Some(status) = self.gates.get(gate_name) {
                if *status != GateStatus::Pass {
                    return Some(gate_name);
                }
            }
        }
        None
    }
}

/// Read state from a JSON file
pub fn read_state(path: &Path) -> Result<WorkState, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}

/// Write state to a JSON file (pretty-printed)
pub fn write_state(path: &Path, state: &WorkState) -> Result<(), String> {
    let content = serde_json::to_string_pretty(state)
        .map_err(|e| format!("failed to serialize state: {}", e))?;
    fs::write(path, content)
        .map_err(|e| format!("failed to write {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_gates_all_todo() {
        let gates = Gates::default();
        for gate_name in GATE_ORDER {
            assert_eq!(*gates.get(gate_name).unwrap(), GateStatus::Todo);
        }
    }

    #[test]
    fn test_gate_set_and_get() {
        let mut gates = Gates::default();
        assert!(gates.set("prd", GateStatus::Pass));
        assert_eq!(*gates.get("prd").unwrap(), GateStatus::Pass);
        assert_eq!(*gates.get("plan").unwrap(), GateStatus::Todo);
    }

    #[test]
    fn test_gate_set_invalid() {
        let mut gates = Gates::default();
        assert!(!gates.set("nonexistent", GateStatus::Pass));
    }

    #[test]
    fn test_gate_get_invalid() {
        let gates = Gates::default();
        assert!(gates.get("nonexistent").is_none());
    }

    #[test]
    fn test_gate_status_terminal() {
        assert!(!GateStatus::Todo.is_terminal());
        assert!(!GateStatus::InProgress.is_terminal());
        assert!(GateStatus::Pass.is_terminal());
        assert!(GateStatus::Fail.is_terminal());
        assert!(GateStatus::ChangesRequested.is_terminal());
    }

    #[test]
    fn test_gate_order_length() {
        assert_eq!(GATE_ORDER.len(), 8);
    }

    #[test]
    fn test_gate_to_role_mapping() {
        assert_eq!(gate_to_role("prd"), Some(Role::Prd));
        assert_eq!(gate_to_role("plan"), Some(Role::Orchestrator));
        assert_eq!(gate_to_role("env"), Some(Role::Env));
        assert_eq!(gate_to_role("tests"), Some(Role::Test));
        assert_eq!(gate_to_role("impl"), Some(Role::Implementation));
        assert_eq!(gate_to_role("review_security"), Some(Role::ReviewSecurity));
        assert_eq!(gate_to_role("qa"), Some(Role::Qa));
        assert_eq!(gate_to_role("git"), Some(Role::Git));
        assert_eq!(gate_to_role("nonexistent"), None);
    }

    #[test]
    fn test_role_to_gate_roundtrip() {
        for gate_name in GATE_ORDER {
            let role = gate_to_role(gate_name).unwrap();
            assert_eq!(role_to_gate(&role), *gate_name);
        }
    }

    #[test]
    fn test_work_state_new() {
        let state = WorkState::new("FEAT-001", "Test feature", "myrepo", Commands::default());
        assert_eq!(state.id, "FEAT-001");
        assert_eq!(state.branch, "pfm/FEAT-001");
        assert_eq!(state.status, WorkStatus::InProgress);
        assert_eq!(state.owner, Role::Prd);
    }

    #[test]
    fn test_next_pending_gate_all_todo() {
        let state = WorkState::new("FEAT-001", "Test", "repo", Commands::default());
        assert_eq!(state.next_pending_gate(), Some("prd"));
    }

    #[test]
    fn test_next_pending_gate_some_passed() {
        let mut state = WorkState::new("FEAT-001", "Test", "repo", Commands::default());
        state.gates.prd = GateStatus::Pass;
        state.gates.plan = GateStatus::Pass;
        assert_eq!(state.next_pending_gate(), Some("env"));
    }

    #[test]
    fn test_next_pending_gate_all_passed() {
        let mut state = WorkState::new("FEAT-001", "Test", "repo", Commands::default());
        state.gates.prd = GateStatus::Pass;
        state.gates.plan = GateStatus::Pass;
        state.gates.env = GateStatus::Pass;
        state.gates.tests = GateStatus::Pass;
        state.gates.impl_ = GateStatus::Pass;
        state.gates.review_security = GateStatus::Pass;
        state.gates.qa = GateStatus::Pass;
        state.gates.git = GateStatus::Pass;
        assert_eq!(state.next_pending_gate(), None);
    }

    #[test]
    fn test_state_serialization_roundtrip() {
        let state = WorkState::new("FEAT-001", "Test feature", "myrepo", Commands {
            verify: "cargo test".into(),
            security: "cargo audit".into(),
            qa_smoke: "".into(),
        });
        let json = serde_json::to_string_pretty(&state).unwrap();
        let parsed: WorkState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "FEAT-001");
        assert_eq!(parsed.commands.verify, "cargo test");
        assert_eq!(*parsed.gates.get("prd").unwrap(), GateStatus::Todo);
    }

    #[test]
    fn test_state_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = WorkState::new("FEAT-002", "File test", "repo", Commands::default());
        write_state(&path, &state).unwrap();
        let loaded = read_state(&path).unwrap();
        assert_eq!(loaded.id, "FEAT-002");
        assert_eq!(loaded.title, "File test");
    }

    #[test]
    fn test_role_display_and_parse() {
        let roles = vec![
            Role::Prd, Role::Orchestrator, Role::Env, Role::Test,
            Role::Implementation, Role::ReviewSecurity, Role::Qa, Role::Git,
        ];
        for role in roles {
            let s = role.to_string();
            let parsed: Role = s.parse().unwrap();
            assert_eq!(parsed, role);
        }
    }

    #[test]
    fn test_impl_field_serializes_as_impl() {
        let state = WorkState::new("FEAT-001", "Test", "repo", Commands::default());
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"impl\""));
        assert!(!json.contains("\"impl_\""));
    }
}
