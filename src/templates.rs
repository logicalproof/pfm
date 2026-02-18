/// Template content for work item files

pub const STATE_JSON: &str = r#"{
  "id": "",
  "title": "",
  "repo": "",
  "branch": "",
  "status": "in_progress",
  "owner": "prd",
  "updated_at": "",
  "gates": {
    "prd": "todo",
    "plan": "todo",
    "env": "todo",
    "tests": "todo",
    "impl": "todo",
    "review_security": "todo",
    "qa": "todo",
    "git": "todo"
  },
  "commands": {
    "verify": "",
    "security": "",
    "qa_smoke": ""
  },
  "workspace": {
    "worktree": "",
    "tmux_session": "",
    "container": ""
  },
  "notes": []
}"#;

pub const PRD_MD: &str = r#"# Product Requirements Document

## Work ID: {WORK_ID}
## Title: {TITLE}

## Problem Statement

<!-- Describe the problem this work addresses -->

## Requirements

<!-- List functional and non-functional requirements -->

## Success Criteria

<!-- Define measurable success criteria -->

## Out of Scope

<!-- List items explicitly not included -->
"#;

pub const ACCEPTANCE_MD: &str = r#"# Acceptance Criteria

## Work ID: {WORK_ID}

## Criteria

- [ ] Criterion 1
- [ ] Criterion 2

## Verification Steps

1. Step 1
2. Step 2
"#;

pub const PLAN_MD: &str = r#"# Implementation Plan

## Work ID: {WORK_ID}

## Overview

<!-- High-level approach -->

## Tasks

<!-- Ordered list of implementation tasks -->

## Dependencies

<!-- External dependencies or blockers -->

## Risks

<!-- Known risks and mitigations -->
"#;

pub const TASKS_MD: &str = r#"# Task Breakdown

## Work ID: {WORK_ID}

## Tasks

- [ ] Task 1
- [ ] Task 2

## Notes

<!-- Additional context for task execution -->
"#;

pub const RUNLOG_MD: &str = r#"# Run Log

## Work ID: {WORK_ID}

<!-- Automated entries will be appended below -->
"#;

pub const QA_MD: &str = r#"# QA Report

## Work ID: {WORK_ID}

## Test Results

<!-- QA findings will be recorded here -->

## Issues Found

<!-- List any issues discovered during QA -->
"#;

pub const ROLE_PRD: &str = r#"# Role: PRD Agent

## Purpose
Generate a complete Product Requirements Document from the work item title and any existing context.

## Inputs
- `state.json` — read work item metadata
- Any existing `prd.md` content
- User-provided context in notes

## Actions
1. Read the work title and notes from state.json
2. Generate a comprehensive PRD in `prd.md`
3. Generate acceptance criteria in `acceptance.md`
4. Update gate `prd` to `pass` in state.json
5. Write handoff note

## Gate Owned
`prd`

## Stop Condition
- `prd.md` and `acceptance.md` are complete
- Gate `prd` = `pass`
- Handoff note written
"#;

pub const ROLE_ORCHESTRATOR: &str = r#"# Role: Orchestrator Agent

## Purpose
Create an implementation plan and task breakdown from the PRD.

## Inputs
- `prd.md` — requirements
- `acceptance.md` — acceptance criteria
- `state.json` — work metadata

## Actions
1. Read PRD and acceptance criteria
2. Create `plan.md` with implementation approach
3. Break down into tasks in `tasks.md`
4. Update gate `plan` to `pass` in state.json
5. Write handoff note

## Gate Owned
`plan`

## Stop Condition
- `plan.md` and `tasks.md` are complete
- Gate `plan` = `pass`
- Handoff note written
"#;

pub const ROLE_ENV: &str = r#"# Role: Environment Agent

## Purpose
Set up the development environment (branch, worktree, dependencies).

## Inputs
- `state.json` — branch name, workspace config
- `plan.md` — dependency requirements

## Actions
1. Create/verify git branch
2. Set up worktree if using Groot
3. Install dependencies as specified in plan
4. Verify environment is functional
5. Update gate `env` to `pass` in state.json
6. Write handoff note

## Gate Owned
`env`

## Stop Condition
- Environment is ready and verified
- Gate `env` = `pass`
- Handoff note written
"#;

pub const ROLE_TEST: &str = r#"# Role: Test Agent

## Purpose
Write tests based on the plan and acceptance criteria BEFORE implementation.

## Inputs
- `plan.md` — implementation plan
- `tasks.md` — task breakdown
- `acceptance.md` — acceptance criteria

## Actions
1. Read plan and acceptance criteria
2. Write test files (unit, integration as appropriate)
3. Verify tests exist and fail (red phase of TDD)
4. Update gate `tests` to `pass` in state.json
5. Write handoff note

## Gate Owned
`tests`

## Stop Condition
- Tests are written and properly failing
- Gate `tests` = `pass`
- Handoff note written
"#;

pub const ROLE_IMPLEMENTATION: &str = r#"# Role: Implementation Agent

## Purpose
Implement the feature to make tests pass.

## Inputs
- `plan.md` — implementation plan
- `tasks.md` — task breakdown
- Test files from test agent

## Actions
1. Read plan and tasks
2. Implement code changes per the plan
3. Run tests to verify they pass
4. Update gate `impl` to `pass` in state.json
5. Write handoff note

## Gate Owned
`impl`

## Stop Condition
- All tests pass
- Implementation matches plan
- Gate `impl` = `pass`
- Handoff note written
"#;

pub const ROLE_REVIEW_SECURITY: &str = r#"# Role: Review & Security Agent

## Purpose
Review code quality and run security checks.

## Inputs
- Implementation diff
- `commands.security` from state.json
- `acceptance.md` — acceptance criteria

## Actions
1. Review all changed files for quality issues
2. Run security command from state.json
3. Check for common vulnerabilities (OWASP top 10)
4. If issues found, set gate to `changes_requested` and detail in handoff
5. If clean, set gate to `pass`
6. Write handoff note

## Gate Owned
`review_security`

## Stop Condition
- Review is complete
- Security scan has run
- Gate `review_security` = `pass` or `changes_requested`
- Handoff note written
"#;

pub const ROLE_QA: &str = r#"# Role: QA Agent

## Purpose
Validate the implementation against acceptance criteria.

## Inputs
- `acceptance.md` — acceptance criteria
- `commands.qa_smoke` from state.json
- Implementation artifacts

## Actions
1. Read acceptance criteria
2. Run smoke tests if configured
3. Verify each acceptance criterion
4. Record results in `qa.md`
5. Update gate `qa` to `pass` or `fail`
6. Write handoff note

## Gate Owned
`qa`

## Stop Condition
- All acceptance criteria verified
- QA report written
- Gate `qa` = `pass` or `fail`
- Handoff note written
"#;

pub const ROLE_GIT: &str = r#"# Role: Git Agent

## Purpose
Finalize the branch: clean up, create commit, prepare for merge.

## Inputs
- `state.json` — branch info
- All work artifacts

## Actions
1. Verify all prior gates are `pass`
2. Stage and commit changes with descriptive message
3. Push branch to remote
4. Create PR if configured
5. Update gate `git` to `pass`
6. Set work status to `done`
7. Write handoff note

## Gate Owned
`git`

## Stop Condition
- Branch is pushed and PR created (if applicable)
- Gate `git` = `pass`
- Work status = `done`
- Handoff note written
"#;

/// Returns (filename, content) pairs for all role specs
pub fn role_specs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("prd.md", ROLE_PRD),
        ("orchestrator.md", ROLE_ORCHESTRATOR),
        ("env.md", ROLE_ENV),
        ("test.md", ROLE_TEST),
        ("implementation.md", ROLE_IMPLEMENTATION),
        ("review_security.md", ROLE_REVIEW_SECURITY),
        ("qa.md", ROLE_QA),
        ("git.md", ROLE_GIT),
    ]
}

/// Returns (filename, content) pairs for work item templates
pub fn work_templates() -> Vec<(&'static str, &'static str)> {
    vec![
        ("state.json", STATE_JSON),
        ("prd.md", PRD_MD),
        ("acceptance.md", ACCEPTANCE_MD),
        ("plan.md", PLAN_MD),
        ("tasks.md", TASKS_MD),
        ("runlog.md", RUNLOG_MD),
        ("qa.md", QA_MD),
    ]
}
