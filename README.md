# PFM — Production Flow Manager

A CLI control plane that orchestrates Claude Code role agents through a file-driven, git-backed pipeline. PFM sequences specialized AI agents (PRD writer, planner, test author, implementer, reviewer, QA, etc.) through deterministic gates, storing all state as JSON files in your repo.

**The core idea:** instead of one long Claude session that loses context, PFM breaks work into discrete roles with explicit handoffs. Each agent reads the previous agent's output, does its job, writes a handoff note, and exits. PFM manages the sequencing, failure recovery, and audit trail.

## Why PFM?

- **Persistence over memory** — All state lives in `.pfm/` as files, not in model context. Agents can crash and resume.
- **Deterministic completion** — A role is "done" only when its gate is terminal AND a handoff file exists. No guessing.
- **Automatic rerouting** — Test failures restart implementation. Review changes requested loops back. QA failures trigger a fix cycle.
- **Audit trail** — Every command, output, and agent start/stop is logged to `runlog.md`.
- **Stack-aware** — Ships with verify/security command presets for Rails, React Native, and Node/Ruby CLIs.

## Install

```bash
# From source
cargo install --path .

# Verify
pfm --version
```

Requires [Rust](https://rustup.rs/) and [Claude Code](https://docs.anthropic.com/en/docs/claude-code) (`claude` CLI on PATH).

## Quick Start

```bash
# 1. Initialize PFM in your repo
pfm init

# 2. Create a work item
pfm work new "Add user authentication" --id FEAT-auth --stack rails

# 3. Check the initial state
pfm status FEAT-auth
#  [  ] prd         todo
#  [  ] plan        todo
#  [  ] env         todo
#  ...

# 4. Run the full pipeline — PFM starts each role agent in sequence
pfm run FEAT-auth

# Or run up to a specific gate
pfm run FEAT-auth --to impl

# Or start a single role agent manually
pfm agent start prd FEAT-auth

# 5. Check verification at any time
pfm check FEAT-auth

# 6. List all work items
pfm work list
```

## Pipeline

PFM runs 8 gates in fixed order. Each gate is owned by a specialized role agent:

```
prd ─→ plan ─→ env ─→ tests ─→ impl ─→ review_security ─→ qa ─→ git
```

| Gate | Role | What it does |
|------|------|-------------|
| `prd` | prd | Generates PRD and acceptance criteria from the work title |
| `plan` | orchestrator | Creates implementation plan and task breakdown |
| `env` | env | Sets up branch, worktree, installs dependencies |
| `tests` | test | Writes tests before implementation (TDD red phase) |
| `impl` | implementation | Implements code to make tests pass |
| `review_security` | review_security | Code review + security scan |
| `qa` | qa | Validates against acceptance criteria |
| `git` | git | Commits, pushes, creates PR |

### Gate Statuses

Each gate can be in one of these states:

| Status | Meaning |
|--------|---------|
| `todo` | Not started |
| `in_progress` | Agent is running |
| `pass` | Complete and successful |
| `fail` | Complete but failed |
| `changes_requested` | Review requested changes (review_security only) |

### Reroute Rules

When a gate fails, PFM automatically reroutes:

- **`tests=fail`** → restart implementation agent
- **`review_security=changes_requested`** → restart implementation agent
- **`qa=fail`** → restart implementation agent, then re-run tests and qa

### Completion Signals

A gate is considered complete when BOTH conditions are met:
1. Its status in `state.json` is terminal (`pass`, `fail`, or `changes_requested`)
2. A handoff file exists in `handoffs/` for that role, newer than the agent start time

This prevents false positives from stale state.

## Commands

### `pfm init`

Creates the `.pfm/` directory structure with default config, templates, and role specs. Safe to run multiple times (idempotent).

### `pfm work new "<title>" [--id FEAT-...] [--stack rails|react_native|cli_node|cli_ruby]`

Creates a new work item:
- Copies templates into `.pfm/work/<id>/`
- Seeds `state.json` with verify/security commands from the selected stack
- Creates a git branch `pfm/<id>`
- Attempts Groot worktree creation (best-effort)

If `--id` is omitted, generates one from the title (e.g., "Add login page" → `FEAT-add-login-page`).

### `pfm work list`

Lists all work items with ID, status, owner, and title.

### `pfm status <work_id>`

Shows detailed view: all gate statuses with visual indicators, workspace info, configured commands, and notes.

```
Gates:
  [OK] prd                  pass
  [OK] plan                 pass
  [>>] env                  in_progress
  [  ] tests                todo
  [XX] impl                 fail
  [CR] review_security      changes_requested
```

### `pfm agent start <role> <work_id>`

Starts a Claude Code agent for the specified role:
- Renders a bootstrap prompt with the role spec path, work directory, and hard requirements
- Prefers tmux sessions when available; falls back to direct `claude --print` execution
- Sets the gate to `in_progress` and logs the start to `runlog.md`

### `pfm agent nudge <role> <work_id>`

Sends a resume message to a running tmux agent session. If no session exists, prints the prompt for manual paste.

### `pfm check <work_id>`

Runs the `verify` and `security` commands from `state.json`:
- Executes in the worktree directory if configured
- Logs full command output to `runlog.md`
- Updates the `tests` gate to `pass` or `fail`

### `pfm run <work_id> [--to <gate>] [--mode classic|teams]`

Orchestrates the full pipeline:
- Determines the next non-pass gate
- Starts the corresponding role agent
- Polls for completion (gate terminal + handoff file)
- Auto-runs `pfm check` after tests/impl gates
- Applies reroute rules on failures
- Stops at `--to` gate if specified

Teams mode (experimental): falls back to classic if agent teams aren't available.

## Directory Layout

```
.pfm/
├── config.json                 # Stack-specific verify/security commands
├── roles/                      # Role spec markdowns (8 files)
│   ├── prd.md
│   ├── orchestrator.md
│   ├── env.md
│   ├── test.md
│   ├── implementation.md
│   ├── review_security.md
│   ├── qa.md
│   └── git.md
├── templates/                  # Work item file templates
│   ├── state.json
│   ├── prd.md
│   ├── acceptance.md
│   ├── plan.md
│   ├── tasks.md
│   ├── runlog.md
│   └── qa.md
├── work/
│   └── <WORK_ID>/
│       ├── state.json          # Gate statuses, commands, workspace pointers
│       ├── prd.md              # Product requirements
│       ├── acceptance.md       # Acceptance criteria
│       ├── plan.md             # Implementation plan
│       ├── tasks.md            # Task breakdown
│       ├── runlog.md           # Audit log of all commands and agent runs
│       ├── qa.md               # QA report
│       ├── handoffs/           # Role handoff notes (timestamped)
│       └── artifacts/          # Build/test artifacts
└── runtime/                    # Ephemeral pointers (gitignored)
```

## State Schema

Each work item's `state.json`:

```json
{
  "id": "FEAT-auth",
  "title": "Add user authentication",
  "repo": "myapp",
  "branch": "pfm/FEAT-auth",
  "status": "in_progress",
  "owner": "implementation",
  "updated_at": "2026-02-18T12:00:00+00:00",
  "gates": {
    "prd": "pass",
    "plan": "pass",
    "env": "pass",
    "tests": "pass",
    "impl": "in_progress",
    "review_security": "todo",
    "qa": "todo",
    "git": "todo"
  },
  "commands": {
    "verify": "bundle exec rspec",
    "security": "bundle exec brakeman -q",
    "qa_smoke": ""
  },
  "workspace": {
    "worktree": "",
    "tmux_session": "pfm-FEAT-auth-implementation",
    "container": ""
  },
  "notes": []
}
```

## Configuration

`.pfm/config.json` defines stack presets:

```json
{
  "default_stack": "rails",
  "stacks": {
    "rails": {
      "verify": "bundle exec rspec",
      "security": "bundle exec brakeman -q"
    },
    "react_native": {
      "verify": "npm test",
      "security": "npm audit"
    },
    "cli_node": {
      "verify": "npm test",
      "security": "npm audit"
    },
    "cli_ruby": {
      "verify": "bundle exec rspec",
      "security": "bundle exec brakeman -q"
    }
  }
}
```

## Architecture

PFM is Groot-adjacent: Groot manages worktrees/tmux/containers; PFM manages persistence (git-backed work ledger) and orchestration (gates + agent runs).

Key design principles:
- **File-driven** — All state is JSON on disk. No database, no model memory.
- **Sessions are runtime views** — Persistence is the source of truth.
- **Pluggable adapters** — Groot and tmux integration via clean adapter interfaces, not hardcoded assumptions.
- **Deterministic** — Completion requires both gate update AND handoff file. No race conditions.

## Groot Integration

When `groot` is available on PATH, PFM will:
- Attempt worktree creation via `groot plant` on `pfm work new`
- Use existing Groot sessions for agent execution when available

This is best-effort — PFM works fine without Groot installed.

## Development

```bash
cargo test              # Run all 42 unit tests
cargo build             # Build debug binary
cargo install --path .  # Install to ~/.cargo/bin
```

## License

MIT
