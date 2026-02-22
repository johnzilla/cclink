---
phase: 01-foundation-and-key-management
plan: 01
subsystem: cli
tags: [rust, pkarr, ed25519, clap, anyhow, thiserror, dirs, keypair, atomic-write]

# Dependency graph
requires: []
provides:
  - cclink binary (compilable Rust CLI)
  - PKARR Ed25519 keypair generation via Keypair::random()
  - Atomic key storage at ~/.pubky/secret_key (0600 permissions)
  - Key import from file path and stdin (hex format)
  - Overwrite protection with fingerprint display
  - Homeserver persistence at ~/.pubky/cclink_homeserver
  - Key store module with all path resolution functions
  - CLI structure with Init(InitArgs) and Whoami subcommands
affects: [02-whoami-and-identity, 03-session-publish-and-pickup, all-phases]

# Tech tracking
tech-stack:
  added:
    - pkarr 5.0.3 (features = ["keys"]) — Ed25519 keypair generation/file I/O
    - clap 4.5 with derive feature — CLI argument parsing
    - anyhow 1.0 — error propagation with context chaining
    - thiserror 2.0 — typed domain error enum
    - dirs 5.0 — cross-platform home directory resolution
  patterns:
    - Atomic write pattern: write_secret_key_file to temp path + std::fs::rename
    - Module structure: keys/{store,fingerprint}, commands/{init,whoami}
    - Error hierarchy: CclinkError thiserror enum + anyhow for propagation
    - Stdin detection via IsTerminal trait for non-interactive mode handling
    - Temp file validation pattern for stdin hex import

key-files:
  created:
    - Cargo.toml
    - src/main.rs
    - src/cli.rs
    - src/error.rs
    - src/keys/mod.rs
    - src/keys/store.rs
    - src/keys/fingerprint.rs
    - src/commands/mod.rs
    - src/commands/init.rs
    - src/commands/whoami.rs

key-decisions:
  - "pkarr 5.0.3 requires features = ['keys'] when default-features = false to access Keypair and PublicKey types"
  - "Stdin import uses temp file + from_secret_key_file to avoid ed25519_dalek::SecretKey type ambiguity"
  - "Homeserver stored as plain text at ~/.pubky/cclink_homeserver — separate from secret key, no atomic write needed"
  - "Only secret key file stored on disk; public key derived in memory from loaded keypair to avoid sync issues"

patterns-established:
  - "Atomic write: keypair.write_secret_key_file(&tmp_path) then std::fs::rename(&tmp, &dest) — always same directory"
  - "Overwrite guard: check keypair_exists() before write, load existing for fingerprint display, detect non-interactive stdin"
  - "Validation before write: all import paths validate key data before any write to final destination"

requirements-completed: [KEY-01, KEY-03, KEY-04]

# Metrics
duration: 3min
completed: 2026-02-21
---

# Phase 1 Plan 01: Foundation and Key Management Summary

**cclink Rust CLI scaffold with pkarr Ed25519 keypair generation, atomic key storage at ~/.pubky/secret_key, and import from file/stdin with overwrite protection**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-21T23:16:21Z
- **Completed:** 2026-02-21T23:19:05Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Full cclink binary compiles with pkarr 5.0.3, clap 4.5, anyhow, thiserror, dirs
- `cclink init` generates Ed25519/PKARR keypair and atomically stores at `~/.pubky/secret_key` with 0600 permissions
- Import from file path and stdin both work, validating key format before any disk write
- Overwrite protection prompts with existing key fingerprint, detects non-interactive stdin gracefully
- All error paths (empty stdin, invalid hex, wrong length, nonexistent file) produce clear messages

## Task Commits

Each task was committed atomically:

1. **Task 1: Scaffold Rust project with CLI skeleton, error types, and key store module** - `c381479` (feat)
2. **Task 2: Implement cclink init — generate keypair, atomic write, overwrite guard, import from file and stdin** - `fa7e478` (feat)

**Plan metadata:** (docs commit — see below)

## Files Created/Modified
- `Cargo.toml` — Package manifest with pkarr 5.0.3 (keys feature), clap 4.5, anyhow, thiserror, dirs
- `src/main.rs` — CLI entry point with Cli::parse() and subcommand dispatch
- `src/cli.rs` — Clap derive structs: Cli, Commands enum, InitArgs (--import, --homeserver, --yes)
- `src/error.rs` — CclinkError thiserror enum: NoKeypairFound, InvalidKeyFormat, KeyCorrupted, AtomicWriteFailed, HomeDirNotFound
- `src/keys/store.rs` — Key storage: path resolution, atomic write (temp+rename), load, overwrite guard
- `src/keys/fingerprint.rs` — short_fingerprint: first 8 chars of public key z32 encoding
- `src/keys/mod.rs` — Re-exports store and fingerprint modules
- `src/commands/mod.rs` — Declares init and whoami submodules
- `src/commands/init.rs` — Full cclink init implementation with all edge cases
- `src/commands/whoami.rs` — Placeholder for Plan 02

## Decisions Made
- Added `features = ["keys"]` to pkarr dependency — Keypair and PublicKey are gated behind this feature in pkarr 5.0.3 with default-features = false (discovered during compilation)
- Stdin import uses temp file approach (write hex to temp, use from_secret_key_file) to avoid ed25519_dalek::SecretKey type complexity — same code path as file import, maximum reuse
- Detected non-interactive stdin via IsTerminal trait — non-interactive mode with existing key and no --yes flag aborts with helpful message rather than hanging
- Only secret key stored on disk; public key always derived from loaded keypair (avoids sync issues)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added features = ["keys"] to pkarr dependency**
- **Found during:** Task 1 (scaffold and cargo check)
- **Issue:** pkarr 5.0.3 with `default-features = false` gates `Keypair` and `PublicKey` behind a `keys` feature flag — compiler errors `E0433` and `E0425`
- **Fix:** Changed Cargo.toml from `pkarr = { version = "5.0.3", default-features = false }` to `pkarr = { version = "5.0.3", default-features = false, features = ["keys"] }`
- **Files modified:** Cargo.toml
- **Verification:** cargo check and cargo build both succeed
- **Committed in:** c381479 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking dependency feature flag)
**Impact on plan:** Necessary fix; plan anticipated this possibility and provided explicit guidance. No scope creep.

## Issues Encountered
- Pitfall 8 from research (from_secret_key type ambiguity) was avoided by using the temp-file approach for stdin import as recommended in the plan

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- cclink binary compiles and runs
- Key store functions ready: keypair_exists, load_keypair, read_homeserver available for Plan 02 (whoami)
- CLI structure ready for additional commands
- Blocker: None

---
*Phase: 01-foundation-and-key-management*
*Completed: 2026-02-21*

## Self-Check: PASSED

- All 10 source files found on disk
- Task 1 commit c381479 verified in git log
- Task 2 commit fa7e478 verified in git log
