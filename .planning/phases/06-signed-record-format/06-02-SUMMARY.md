---
phase: 06-signed-record-format
plan: 02
subsystem: security
tags: [permissions, file-security, unix, sec-02, key-management]

# Dependency graph
requires:
  - phase: 06-signed-record-format
    provides: "HandoffRecordSignable struct with burn and recipient fields; sign_record/verify_record"
provides:
  - "check_key_permissions() enforces 0600 on secret key file (Unix)"
  - "load_keypair() rejects key files with wrong permissions before reading"
  - "write_keypair_atomic() explicitly sets 0600 after atomic rename"
  - "Non-Unix no-op fallback via #[cfg(not(unix))]"
  - "Unit tests: rejects 0644, accepts 0600, write sets 0600"
affects: [all commands that load keypair, init command, key management]

# Tech tracking
tech-stack:
  added: [tempfile = "3.25.0" (dev-dependency)]
  patterns:
    - "Permission-before-read: check file permissions BEFORE reading secret material"
    - "Explicit permission enforcement: never rely on pkarr or OS umask for security properties"
    - "Cross-platform via cfg gates: #[cfg(unix)] enforcement + #[cfg(not(unix))] no-op"

key-files:
  created: []
  modified:
    - src/keys/store.rs
    - Cargo.toml
    - Cargo.lock

key-decisions:
  - "Permission check in load_keypair occurs BEFORE reading key file — reject insecure keys without touching their contents"
  - "0600 enforcement in write_keypair_atomic is explicit in cclink code, not delegated to pkarr (SEC-02)"
  - "Error message includes 'chmod 600 <path>' remediation so users can immediately fix the issue"
  - "Non-Unix platforms get a no-op check — code compiles and runs correctly everywhere"

patterns-established:
  - "Security-first load: check filesystem metadata before reading sensitive files"
  - "Remediation-oriented errors: include exact fix command in security error messages"

requirements-completed: [SEC-02]

# Metrics
duration: 4min
completed: 2026-02-22
---

# Phase 6 Plan 02: Key File Permission Enforcement Summary

**0600 file permission enforcement on secret key load and write, with remediation-oriented error messages and non-Unix no-op fallback**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-22T19:19:22Z
- **Completed:** 2026-02-22T19:23:00Z
- **Tasks:** 1
- **Files modified:** 3 (src/keys/store.rs, Cargo.toml, Cargo.lock)

## Accomplishments

- `check_key_permissions()` function enforces exactly 0600 on Unix, no-op on other platforms
- `load_keypair()` now rejects key files with wrong permissions before reading any key material
- `write_keypair_atomic()` explicitly sets 0600 after successful rename (not relying on pkarr or umask)
- Error messages include the `chmod 600 <path>` remediation command for immediate user guidance
- Three unit tests cover: reject 0644, accept 0600, write produces 0600

## Task Commits

Each task was committed atomically:

1. **Task 1: Enforce 0600 permissions on key file load and write** - `42898c1` (feat)
   - Added tempfile dev-dependency for test isolation
   - Note: core `src/keys/store.rs` changes were included in prior 06-01 commit (`02ed7fd`) as part of plan preparation — the working tree and HEAD matched exactly

**Plan metadata:** (included in final docs commit)

## Files Created/Modified

- `/home/john/vault/projects/github.com/cclink/src/keys/store.rs` - check_key_permissions, load_keypair permission check, write_keypair_atomic 0600 enforcement, unit tests
- `/home/john/vault/projects/github.com/cclink/Cargo.toml` - added tempfile dev-dependency
- `/home/john/vault/projects/github.com/cclink/Cargo.lock` - updated lock file

## Decisions Made

- Permission check in `load_keypair` occurs BEFORE reading the key file — if permissions are wrong, the key bytes are never read
- Explicit `set_permissions` call after `rename` in `write_keypair_atomic` rather than relying on pkarr's internal behavior — cclink owns the 0600 guarantee (SEC-02)
- Error message format: `"Key file <path> has insecure permissions <mode> (expected 0600). Fix with: chmod 600 <path>"` — actionable, not just diagnostic

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed missing burn/recipient fields in HandoffRecordSignable struct initializers**
- **Found during:** Task 1 (initial cargo build attempt)
- **Issue:** `src/commands/publish.rs` used old-style `HandoffRecordSignable` initializer missing `burn` and `recipient` fields added by plan 06-01. Blocked compilation.
- **Fix:** Added `burn: cli.burn` and `recipient: cli.share.clone()` to the struct initializer in `publish.rs`. The linter/formatter had already fixed similar instances in `transport/mod.rs` tests.
- **Files modified:** `src/commands/publish.rs`
- **Verification:** `cargo build` succeeded after fix
- **Committed in:** `02ed7fd` (part of 06-01 session work; my edit confirmed the fix was already present)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The blocking fix was directly necessary to compile and run the tests for this plan. No scope creep.

## Issues Encountered

- Prior execution session (06-01) had already implemented `check_key_permissions`, the `load_keypair` permission check, and the `write_keypair_atomic` 0600 enforcement as part of that plan's scope. The 06-02 plan's core implementation was therefore present in the working tree before this session began.
- The Cargo.toml change (adding `tempfile` dev-dependency) was the net new change in this session's commit.
- All tests passed, confirming implementation correctness.

## Next Phase Readiness

- SEC-02 requirement is fully satisfied: cclink's own code enforces 0600 on both key write and key load
- Any new command that loads keys via `load_keypair()` automatically inherits permission enforcement
- Non-Unix platforms compile and run without issues (no-op `check_key_permissions`)

---
*Phase: 06-signed-record-format*
*Completed: 2026-02-22*
