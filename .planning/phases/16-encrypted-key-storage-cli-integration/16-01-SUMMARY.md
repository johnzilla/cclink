---
phase: 16-encrypted-key-storage-cli-integration
plan: 01
subsystem: keys
tags: [pkarr, keypair, encryption, CCLINKEK, argon2, age, zeroize, atomic-write]

# Dependency graph
requires:
  - phase: 15-encrypted-key-crypto-layer
    provides: encrypt_key_envelope and decrypt_key_envelope crypto primitives
provides:
  - write_encrypted_keypair_atomic (pub, dead_code until Plan 02)
  - load_keypair with CCLINKEK format detection
  - load_plaintext_keypair (private helper, backward-compat hex path)
  - load_encrypted_keypair (private, interactive passphrase prompt)
  - load_encrypted_keypair_with_passphrase (private, testable core)
affects: [16-encrypted-key-storage-cli-integration Plan 02, any phase calling load_keypair]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Binary format detection via starts_with(b"CCLINKEK") in load_keypair
    - Testable core pattern: load_encrypted_keypair_with_passphrase has no I/O; interactive wrapper converts Err to exit(1)
    - Atomic write with pre-rename 0600 permissions to minimize insecure window

key-files:
  created: []
  modified:
    - src/keys/store.rs

key-decisions:
  - "Testable core pattern: load_encrypted_keypair_with_passphrase returns Err for wrong passphrase; interactive wrapper load_encrypted_keypair converts Err to eprintln+exit(1) — enables both test assertions and production UX"
  - "write_encrypted_keypair_atomic writes raw bytes verbatim (not hex); sets 0600 before rename (minimize insecure window) and after rename (defense-in-depth)"
  - "load_keypair reads with std::fs::read (Vec<u8>) not read_to_string — CCLINKEK envelopes are binary and not valid UTF-8"
  - "load_plaintext_keypair wraps String in Zeroizing::new so heap buffer zeroed on drop; no change from previous hex-decode approach"

patterns-established:
  - "Testable core / interactive wrapper split: pure function returns Result, I/O wrapper handles UX and exit"
  - "Binary format detection before dispatch: read raw bytes, check magic, branch to format-specific loader"

requirements-completed: [KEYS-03, KEYS-04, KEYS-06]

# Metrics
duration: 3min
completed: 2026-02-24
---

# Phase 16 Plan 01: Encrypted Key Store Functions Summary

**CCLINKEK binary envelope read/write store layer with format-detecting load_keypair, atomic 0600 writes, and testable passphrase-decrypt core**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-24T17:44:00Z
- **Completed:** 2026-02-24T17:47:00Z
- **Tasks:** 2 (RED + GREEN TDD)
- **Files modified:** 1

## Accomplishments

- Implemented `write_encrypted_keypair_atomic` that writes CCLINKEK binary envelope verbatim with 0600 permissions (pre-rename + post-rename)
- Refactored `load_keypair` with transparent format detection: CCLINKEK magic bytes branch to encrypted path, otherwise falls through to plaintext hex (full backward compat)
- Implemented `load_encrypted_keypair_with_passphrase` as testable core (no I/O) and `load_encrypted_keypair` as interactive wrapper (dialoguer prompt + exit(1) on wrong passphrase)
- Added `load_plaintext_keypair` extracting existing hex-decode logic into a named private function
- 6 new TDD tests all pass; 9 total store tests pass; full suite (81 tests) passes

## Task Commits

1. **Task 1: RED — Add failing tests** - `fc2944d` (test)
2. **Task 2: GREEN — Implement store functions** - `0c10d98` (feat)

## Files Created/Modified

- `src/keys/store.rs` - Added write_encrypted_keypair_atomic, refactored load_keypair with format detection, added load_plaintext_keypair / load_encrypted_keypair / load_encrypted_keypair_with_passphrase

## Decisions Made

- Testable core pattern: `load_encrypted_keypair_with_passphrase` returns `Result` with no I/O; `load_encrypted_keypair` wraps it and converts `Err` to `eprintln!("Wrong passphrase"); std::process::exit(1)`. This enables test assertions on the `Err` path while keeping production UX aligned with KEYS-04.
- `write_encrypted_keypair_atomic` sets 0600 on temp file BEFORE rename (minimizes insecure window) and again after rename (defense-in-depth), matching the existing `write_keypair_atomic` pattern.
- `load_keypair` switches from `read_to_string` to `std::fs::read` (raw bytes) since CCLINKEK envelopes are binary and not valid UTF-8.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Redundant closure in map_err**
- **Found during:** Task 2 (GREEN — implement store functions)
- **Issue:** `std::fs::write(&tmp, envelope).map_err(|e| CclinkError::AtomicWriteFailed(e))` triggers clippy `redundant_closure`
- **Fix:** Changed to `.map_err(CclinkError::AtomicWriteFailed)` (tuple variant as function pointer)
- **Files modified:** src/keys/store.rs
- **Verification:** `cargo clippy --all-targets -- -D warnings` passes
- **Committed in:** `0c10d98` (Task 2 commit)

**2. [Rule 1 - Bug] Formatting deviations from rustfmt**
- **Found during:** Task 2 (GREEN — implement store functions)
- **Issue:** `cargo fmt --check` reported multiple line-length reformats in new functions and tests
- **Fix:** Ran `cargo fmt` to apply canonical formatting
- **Files modified:** src/keys/store.rs
- **Verification:** `cargo fmt --check` passes
- **Committed in:** `0c10d98` (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 — code correctness/style)
**Impact on plan:** Both auto-fixes are minor style corrections required for clippy/fmt compliance. No behavioral or scope changes.

## Issues Encountered

None - plan executed cleanly with only minor clippy and fmt fixes.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Store layer complete: `write_encrypted_keypair_atomic` and format-detecting `load_keypair` are ready for Plan 02 to wire into `run_init` and `run_pickup`
- `write_encrypted_keypair_atomic` marked `#[allow(dead_code)]` until Plan 02 calls it from `run_init`
- Concern from STATE.md: validate non-interactive terminal guard behavior with piped invocations (e.g., `cclink publish < /dev/null`) during integration testing in Plan 02

## Self-Check: PASSED

- FOUND: `.planning/phases/16-encrypted-key-storage-cli-integration/16-01-SUMMARY.md`
- FOUND: `src/keys/store.rs`
- FOUND: `fc2944d` (test(16-01): RED commit)
- FOUND: `0c10d98` (feat(16-01): GREEN commit)

---
*Phase: 16-encrypted-key-storage-cli-integration*
*Completed: 2026-02-24*
