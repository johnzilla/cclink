---
phase: 16-encrypted-key-storage-cli-integration
plan: 02
subsystem: cli
tags: [clap, dialoguer, zeroize, argon2, age, encryption, keypair]

# Dependency graph
requires:
  - phase: 16-01
    provides: write_encrypted_keypair_atomic and CCLINKEK envelope format wired into store layer
  - phase: 15-01
    provides: encrypt_key_envelope and decrypt_key_envelope crypto functions

provides:
  - cclink init with interactive passphrase prompt and CCLINKEK encrypted key write (default path)
  - cclink init --no-passphrase for plaintext hex key write (v1.2-compatible)
  - --no-passphrase CLI flag on InitArgs in src/cli.rs
  - Non-interactive terminal guard in run_init (bails with clear error)
  - Passphrase min-8-char validation with eprintln+exit(1) pattern
  - (encrypted) label in overwrite prompt for existing CCLINKEK files
  - No stale #[allow(dead_code)] on any active crypto or store functions

affects: [cclink-init, encrypted-key-storage, cli-integration, user-experience]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Encrypt-or-plaintext branching in run_init controlled by args.no_passphrase flag
    - dialoguer::Password::with_confirmation for passphrase entry with match verification
    - Zeroizing::new() wrapping dialoguer interact() return value — heap buffer wiped on drop
    - eprintln! + process::exit(1) for passphrase validation errors (avoids "Error: Error:" double prefix)
    - Binary magic byte detection (b"CCLINKEK") before identifier fallback in prompt_overwrite

key-files:
  created: []
  modified:
    - src/cli.rs
    - src/commands/init.rs
    - src/crypto/mod.rs
    - src/keys/store.rs

key-decisions:
  - "eprintln!+exit(1) for passphrase-too-short error (not anyhow::bail!) — consistent with Phase 13 PIN validation pattern, avoids double Error: prefix"
  - "Passphrase length validated AFTER double-entry confirmation — user sees the mistake once, not re-prompted"
  - "Import path (--import) flows through the same Step 5 branching — --no-passphrase controls the output regardless of source"

patterns-established:
  - "Zeroizing<String> wrapper at dialoguer::interact() call site — no bare String copy escapes scope"
  - "Binary magic detection (starts_with) before pkarr::Keypair::from_secret_key_file fallback in overwrite identifier logic"

requirements-completed: [KEYS-01, KEYS-02]

# Metrics
duration: 4min
completed: 2026-02-24
---

# Phase 16 Plan 02: Encrypted Key Storage CLI Integration Summary

**`cclink init` now prompts for a passphrase by default (dialoguer with confirmation) and writes a CCLINKEK-encrypted key file; `--no-passphrase` writes a plaintext hex file for v1.2 compatibility**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-24T17:49:56Z
- **Completed:** 2026-02-24T17:54:03Z
- **Tasks:** 1
- **Files modified:** 4

## Accomplishments
- `cclink init` default path now prompts for passphrase with confirmation and writes encrypted CCLINKEK file via `encrypt_key_envelope` + `write_encrypted_keypair_atomic`
- `cclink init --no-passphrase` skips prompt and writes v1.2-compatible plaintext hex file
- Non-interactive terminal without `--no-passphrase` bails with "Use --no-passphrase for non-interactive init"
- Passphrase shorter than 8 characters rejected with eprintln+exit(1) (no double "Error:" prefix)
- Overwrite prompt shows `(encrypted)` for existing CCLINKEK files instead of `(unreadable)`
- All stale `#[allow(dead_code)]` removed from crypto constants, `key_derive_key`, `encrypt_key_envelope`, `decrypt_key_envelope`, and `write_encrypted_keypair_atomic`

## Task Commits

Each task was committed atomically:

1. **Task 1: Add --no-passphrase CLI flag and passphrase prompt flow in run_init** - `4a4445f` (feat)

**Plan metadata:** (see final commit below)

## Files Created/Modified
- `src/cli.rs` - Added `no_passphrase: bool` field to `InitArgs`
- `src/commands/init.rs` - Replaced single `write_keypair_atomic` call with encrypt-or-plaintext branching; added `Zeroizing` import; updated success output labels; updated `prompt_overwrite` with CCLINKEK magic detection
- `src/crypto/mod.rs` - Removed all stale `#[allow(dead_code)]` from CCLINKEK constants, `key_derive_key`, `encrypt_key_envelope`, and `decrypt_key_envelope`
- `src/keys/store.rs` - Removed stale `#[allow(dead_code)]` from `write_encrypted_keypair_atomic`

## Decisions Made
- Used `eprintln!` + `std::process::exit(1)` for the passphrase-too-short error (consistent with Phase 13 PIN validation) rather than `anyhow::bail!` to avoid the "Error: Error:" double-prefix rendering
- Passphrase length check fires after the `with_confirmation` double-entry so the user only sees the "too short" error once, not before confirming
- `--import` path flows through the same Step 5 branching as generated keys — `--no-passphrase` controls whether the *output* is encrypted, regardless of source

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Removed stale #[allow(dead_code)] from crypto/store symbols**
- **Found during:** Task 1 (post-implementation clippy check)
- **Issue:** The plan specified "Run cargo clippy -- -D warnings and...verify that #[allow(dead_code)] on encrypt_key_envelope can now be removed." Removed all stale annotations on constants, key_derive_key, encrypt_key_envelope, decrypt_key_envelope, and write_encrypted_keypair_atomic since all are now reachable from init.rs or store.rs.
- **Fix:** Removed 10 `#[allow(dead_code)]` annotations across crypto/mod.rs and keys/store.rs
- **Files modified:** src/crypto/mod.rs, src/keys/store.rs
- **Verification:** `cargo clippy --all-targets -- -D warnings` passes with no warnings after removal
- **Committed in:** 4a4445f (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (missing critical — cleanup specified in plan action)
**Impact on plan:** Cleanup was explicitly called for in the task action; no scope creep.

## Issues Encountered
None — plan executed cleanly on first attempt.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 16 is now complete: both the store layer (16-01) and CLI integration (16-02) are done
- v1.3 encrypted key storage feature is fully implemented end-to-end
- Users upgrading from v1.2 will see a passphrase prompt on next `cclink init`; existing hex key files load transparently via the format-detection logic from 16-01

---
*Phase: 16-encrypted-key-storage-cli-integration*
*Completed: 2026-02-24*
