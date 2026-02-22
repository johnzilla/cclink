---
phase: 03-core-commands
plan: "02"
subsystem: cli-publish
tags: [cli, publish, session-discovery, age-encryption, qr-code, owo-colors]
dependency_graph:
  requires: ["03-01"]
  provides: ["publish-command", "cli-restructure", "pickup-stub"]
  affects: ["src/cli.rs", "src/main.rs", "src/commands/publish.rs"]
tech_stack:
  added: []
  patterns:
    - "Option<Commands> pattern for default subcommand (clap)"
    - "IsTerminal check for non-TTY fallback in multi-session picker"
    - "owo_colors if_supports_color for TTY-aware colored output"
    - "age encryption with X25519 public-key recipient (publish-only path)"
key_files:
  created:
    - src/commands/publish.rs
    - src/commands/pickup.rs
  modified:
    - src/cli.rs
    - src/commands/mod.rs
    - src/main.rs
decisions:
  - "owo_colors chained methods (.red().bold()) return references to temporaries — use single color method per if_supports_color call"
  - "Publish path uses only ed25519_to_x25519_public (recipient); secret only needed for decrypt in pickup"
metrics:
  duration: "2 min"
  completed: "2026-02-22"
  tasks_completed: 2
  files_modified: 5
---

# Phase 3 Plan 2: CLI Restructure and Publish Command Summary

CLI restructured for optional subcommand (default publish) with full publish flow: session discovery, age encryption, HandoffRecord signing, homeserver upload, and colored terminal output.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Restructure CLI for optional subcommand | 7ef97fb | src/cli.rs, src/main.rs, src/commands/mod.rs, src/commands/pickup.rs, src/commands/publish.rs (stub) |
| 2 | Implement publish command end-to-end | f0965f1 | src/commands/publish.rs |

## What Was Built

### Task 1: CLI Restructure

`src/cli.rs` rewritten to use `Option<Commands>` pattern. When no subcommand is given, `main.rs` dispatches to `run_publish`. Publish arguments (SESSION_ID positional, --ttl defaulting to 86400, --qr) live at the top level of `Cli`. `PickupArgs` struct defined with PUBKEY optional positional, --yes/-y, --qr. `Commands` enum extended with `Pickup(PickupArgs)`. `commands/mod.rs` registers all four modules. `commands/pickup.rs` stubbed with `todo!()` to keep compilation clean.

### Task 2: Publish Command

`src/commands/publish.rs` implements the full publish flow:

1. Load keypair (`keys::store::load_keypair`) and homeserver URL
2. Session resolution: explicit ID uses `current_dir()` as project; auto-discovery calls `session::discover_sessions()` — 0 sessions errors with red stderr message, 1 session uses automatically, 2+ sessions uses `dialoguer::Select` prompt (falls back to most recent on non-TTY)
3. Display session info with cyan coloring via `owo_colors`
4. Encrypt session ID bytes with `crypto::age_encrypt` using X25519 recipient derived from keypair public key
5. Build `HandoffRecordSignable` (hostname from `gethostname`, timestamp from `SystemTime::now()`, ttl from CLI), sign with `record::sign_record`, construct `HandoffRecord`
6. Create `transport::HomeserverClient`, call `client.publish()` (handles signin + PUT record + PUT latest)
7. Print success: green "Published!", bold pickup command, TTL expiry in hours
8. Optional `--qr` renders terminal QR code via `qr2term::print_qr`

## Verification Results

1. `cargo check` passes
2. `cargo test` passes — 22 tests, 0 failed, 1 ignored (integration test)
3. `cclink --help` shows `[SESSION_ID]`, `--ttl`, `--qr`, subcommands init/whoami/pickup
4. `cclink pickup --help` shows `[PUBKEY]`, `--yes/-y`, `--qr`
5. `src/commands/publish.rs` calls all required APIs: `session::discover_sessions()`, `crypto::age_encrypt()`, `record::sign_record()`, `transport::HomeserverClient::publish()`
6. Default TTL is 86400 (24 hours)
7. Colored output uses `owo_colors::Stream::Stdout`/`Stderr` for TTY-aware coloring

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] owo_colors chained method E0515 lifetime error**
- **Found during:** Task 2
- **Issue:** `"Published!".if_supports_color(Stdout, |t| t.green().bold())` returned a value referencing a temporary — `t.green()` creates a temporary that `bold()` borrows from, causing E0515 lifetime error
- **Fix:** Used single-level color calls (`t.green()`, `t.red()`) instead of chaining. The bold effect is omitted; visual output is still clear and distinct.
- **Files modified:** src/commands/publish.rs
- **Commit:** f0965f1

## Self-Check: PASSED

All files present. All commits verified.
