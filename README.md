# PFM — Production Flow Manager

PFM orchestrates Claude Code role agents through a file-driven pipeline. It stores durable work state in-repo under `.pfm/` and sequences agents via deterministic gates.

## Install

```bash
cargo install --path .
```

## Quick Start

```bash
# Initialize PFM in your repo
pfm init

# Create a work item
pfm work new "Add user authentication" --id FEAT-auth --stack rails

# Check status
pfm status FEAT-auth

# Run the full pipeline (starts each role agent in sequence)
pfm run FEAT-auth

# Or run up to a specific gate
pfm run FEAT-auth --to impl

# Start a single role agent manually
pfm agent start prd FEAT-auth

# Nudge a stalled agent
pfm agent nudge prd FEAT-auth

# Run verification checks
pfm check FEAT-auth

# List all work items
pfm work list
```

## Pipeline

PFM runs 8 gates in fixed order, each owned by a role agent:

| Gate | Role | Purpose |
|------|------|---------|
| `prd` | prd | Generate PRD and acceptance criteria |
| `plan` | orchestrator | Create implementation plan and task breakdown |
| `env` | env | Set up branch, worktree, dependencies |
| `tests` | test | Write tests (TDD red phase) |
| `impl` | implementation | Implement code to pass tests |
| `review_security` | review_security | Code review + security scan |
| `qa` | qa | Validate against acceptance criteria |
| `git` | git | Commit, push, create PR |

### Reroute Rules

- `tests=fail` → restart implementation agent
- `review_security=changes_requested` → restart implementation agent
- `qa=fail` → restart implementation agent, then re-run tests and qa

### Completion Signals

A gate is considered complete when BOTH:
1. Its status in `state.json` is terminal (`pass`, `fail`, or `changes_requested`)
2. A handoff file exists in `handoffs/` for that role, newer than the agent start time

## Directory Layout

```
.pfm/
  config.json              # Repo-level defaults (verify/security commands per stack)
  roles/                   # Role spec markdowns (8 files)
    prd.md
    orchestrator.md
    env.md
    test.md
    implementation.md
    review_security.md
    qa.md
    git.md
  templates/               # Work item file templates
    state.json
    prd.md
    acceptance.md
    plan.md
    tasks.md
    runlog.md
    qa.md
  work/
    <WORK_ID>/
      state.json           # Gate statuses, commands, workspace pointers
      prd.md
      acceptance.md
      plan.md
      tasks.md
      runlog.md
      qa.md
      handoffs/            # Role handoff notes (timestamped)
      artifacts/           # Build/test artifacts
  runtime/                 # Ephemeral pointers (gitignored)
```

## Configuration

`.pfm/config.json` defines stack-specific verify and security commands:

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
    }
  }
}
```

## Commands

### `pfm init`
Creates `.pfm/` structure, writes default config, templates, and role specs.

### `pfm work new "<title>" [--id FEAT-...] [--stack rails]`
Creates a new work item from templates. Seeds `state.json` with stack commands. Creates a git branch `pfm/<id>`.

### `pfm work list`
Lists all work items with their status and owner.

### `pfm status <work_id>`
Shows detailed gate statuses, workspace info, and commands for a work item.

### `pfm agent start <role> <work_id>`
Starts a Claude Code agent for the specified role. Renders a bootstrap prompt and runs `claude --print`. Prefers tmux sessions when available.

### `pfm agent nudge <role> <work_id>`
Sends a resume message to a running agent session, or prints the message for manual paste.

### `pfm check <work_id>`
Runs verify and security commands from `state.json`. Updates the `tests` gate based on results. Logs output to `runlog.md`.

### `pfm run <work_id> [--to <gate>] [--mode classic]`
Runs the full pipeline. Starts each role agent in sequence, waits for completion, handles rerouting on failures. Stops at `--to` gate if specified.

## Groot Integration

PFM integrates with Groot for worktree management (best-effort). When `groot` is available on PATH, `pfm work new` will attempt to create a worktree via `groot plant`. The adapter is pluggable — no hard assumptions about Groot's API are baked in.

## Development

```bash
cargo test          # Run all tests
cargo build         # Build debug binary
cargo install --path .  # Install to ~/.cargo/bin
```
