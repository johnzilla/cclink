---
phase: 08-cli-fixes-and-documentation
plan: 01
subsystem: cli
tags: [cli, ux, documentation]
dependency_graph:
  requires: []
  provides: [FUNC-01, FUNC-02, DOCS-01]
  affects: [src/cli.rs, src/commands/publish.rs, cclink-prd.md]
tech_stack:
  added: []
  patterns: [clap-conflicts_with]
key_files:
  created: []
  modified:
    - src/cli.rs
    - src/commands/publish.rs
    - cclink-prd.md
decisions:
  - "--burn and --share mutual exclusion implemented via clap conflicts_with, not runtime validation"
  - "Self-publish message shows 'cclink pickup' with no token; QR section retains token for concrete identifier"
  - "PRD updated only for ~/.cclink -> ~/.pubky/secret_key; other stale references left intentionally (historical planning doc)"
metrics:
  duration: "52 seconds"
  completed: "2026-02-22"
  tasks: 3
  files: 3
---

# Phase 8 Plan 01: CLI Fixes and Documentation Summary

**One-liner:** Clap conflicts_with gates --burn/--share, self-publish guides users to `cclink pickup`, PRD key paths corrected to ~/.pubky/secret_key.

## What Was Built

Three targeted fixes to clean up CLI surface honesty, success message accuracy, and documentation correctness.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Add --burn / --share mutual exclusion in clap | fdd83ea | src/cli.rs |
| 2 | Fix self-publish success message to show cclink pickup | 00116da | src/commands/publish.rs |
| 3 | Update PRD key storage paths from ~/.cclink/ to ~/.pubky/ | a4b8249 | cclink-prd.md |

## Key Changes

### Task 1 — src/cli.rs
Added `conflicts_with = "share"` to the `burn` field's `#[arg]` attribute. Clap now rejects `cclink --burn --share <pubkey>` at parse time with exit code 2 before any code in main() executes. No runtime validation was added.

### Task 2 — src/commands/publish.rs
Self-publish branch (the `else` arm) changed from `format!("cclink pickup {}", token)` to the literal string `"cclink pickup"`. The `token` argument is not needed because self-pickup resolves via the `latest` pointer. The shared-publish branch and the QR code section are unchanged.

### Task 3 — cclink-prd.md
Two lines updated:
- Line 148 (Phase 1 deliverables): `~/.cclink/keys` → `~/.pubky/secret_key`
- Line 213 (Security Model table): `~/.cclink/keys` → `~/.pubky/secret_key`
Zero `~/.cclink` references remain in the file.

## Verification Results

1. `cargo build` — succeeded with zero warnings
2. `./target/debug/cclink --burn --share abc123` — exits with code 2 and clap conflict error
3. `grep -c '~/.cclink' cclink-prd.md` — returns 0
4. `grep 'cclink pickup' src/commands/publish.rs` — self-publish branch shows no token interpolation

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- src/cli.rs: contains `conflicts_with = "share"` on burn field
- src/commands/publish.rs: self-publish branch prints `"cclink pickup"` without token
- cclink-prd.md: zero `~/.cclink` references; two `~/.pubky/secret_key` references present
- Commits fdd83ea, 00116da, a4b8249: all exist in git log
