---
phase: 06-signed-record-format
plan: 01
subsystem: crypto
tags: [ed25519, signing, record, tamper-detection, sec-01]

# Dependency graph
requires:
  - phase: 02-crypto-and-transport
    provides: sign_record, verify_record, HandoffRecordSignable, canonical JSON infrastructure
  - phase: 03-core-commands
    provides: publish command, HandoffRecord struct with burn/recipient fields
provides:
  - HandoffRecordSignable with burn and recipient in signed envelope (v1.1)
  - Tamper detection for burn flag via signature verification
  - Tamper detection for recipient field via signature verification
  - Updated publish command signing burn and recipient into payload
  - Integration tests proving end-to-end tamper detection
affects: [06-02, pickup-command, record-verification]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Alphabetical field ordering in Rust structs for deterministic serde_json serialization"
    - "TDD cycle: RED (failing tests on missing fields) -> GREEN (add fields) -> REFACTOR (doc cleanup)"
    - "Clean break versioning: v1.0 records expire via TTL, no negotiation code"

key-files:
  created: []
  modified:
    - src/record/mod.rs
    - src/commands/publish.rs
    - src/transport/mod.rs
    - src/keys/store.rs
    - tests/integration_round_trip.rs

key-decisions:
  - "Clean break from v1.0: burn and recipient are now in signed envelope; v1.0 records expire via TTL — no version negotiation code"
  - "HandoffRecordSignable field order: blob, burn, created_at, hostname, project, pubkey, recipient, ttl (alphabetical for deterministic JSON)"
  - "Removed test_phase3_record_backwards_compat — the test was incorrect after the clean break decision per STATE.md"

patterns-established:
  - "All fields in HandoffRecordSignable must be in alphabetical declaration order — serde serializes in declaration order, not alphabetical"
  - "check_key_permissions integrated into load_keypair for SEC-02 enforcement at read time"

requirements-completed: [SEC-01]

# Metrics
duration: 4min
completed: 2026-02-22
---

# Phase 6 Plan 1: Signed Record Format Summary

**Ed25519-signed burn and recipient fields in HandoffRecordSignable v1.1 — tamper detection proven by unit + integration tests**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-02-22T19:19:15Z
- **Completed:** 2026-02-22T19:22:51Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added `burn: bool` and `recipient: Option<String>` to `HandoffRecordSignable` in correct alphabetical position
- Updated `From<&HandoffRecord> for HandoffRecordSignable` to copy both fields into the signed envelope
- Updated publish command so `burn: cli.burn` and `recipient: cli.share.clone()` are signed
- Removed `test_phase3_record_backwards_compat` (clean break — v1.0 records expire via TTL)
- Added 3 unit tests (RED/GREEN TDD): `test_signable_includes_burn_field`, `test_signable_includes_recipient_field`, `test_tampered_burn_fails_verification`
- Added 2 integration tests: `test_signed_burn_tamper_detection`, `test_signed_recipient_tamper_detection`
- Full test suite: 76 pass, 2 ignored (live network), 0 failures

## Task Commits

Each task was committed atomically:

1. **Task 1: Add burn and recipient to HandoffRecordSignable and update signing** - `861f9c7` (feat)
2. **Task 2: Update publish command and integration tests for signed burn/recipient** - `02ed7fd` (feat)

_Note: TDD tasks — RED tests were added then made GREEN in the same commit after struct update._

## Files Created/Modified
- `src/record/mod.rs` - Added burn/recipient to HandoffRecordSignable; updated From impl, sample_signable, alphabetical-keys test; removed phase3 compat test; added 3 TDD unit tests
- `src/commands/publish.rs` - HandoffRecordSignable struct literal updated with burn and recipient fields
- `src/transport/mod.rs` - 4 HandoffRecordSignable struct literals updated to include burn/recipient fields
- `src/keys/store.rs` - Added check_key_permissions function; integrated into load_keypair and write_keypair_atomic for SEC-02
- `tests/integration_round_trip.rs` - Added test_signed_burn_tamper_detection and test_signed_recipient_tamper_detection; added record imports

## Decisions Made
- **Clean break confirmed:** v1.0 records (signed without burn/recipient) are not supported. They expire via TTL. No version negotiation. The `test_phase3_record_backwards_compat` test was removed as it validated behavior that is now intentionally broken.
- **Field order is canonical:** `blob, burn, created_at, hostname, project, pubkey, recipient, ttl` — alphabetical. Serde serializes struct fields in declaration order, so this order is enforced by struct layout, not runtime sorting.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Missing check_key_permissions function in keys/store.rs**
- **Found during:** Task 1 (initial cargo test run)
- **Issue:** Two tests in `keys::store::tests` called `check_key_permissions` but the function did not exist in the module. This caused compilation failure.
- **Fix:** Added `check_key_permissions` function with Unix/non-Unix conditional compilation; integrated into `load_keypair` for SEC-02 enforcement
- **Files modified:** src/keys/store.rs
- **Verification:** `cargo test -p cclink keys::store::tests` — all 3 permission tests pass
- **Committed in:** 861f9c7 (Task 1 commit)

**2. [Rule 3 - Blocking] transport/mod.rs HandoffRecordSignable literals missing new fields**
- **Found during:** Task 1 (cargo test compilation)
- **Issue:** 4 `HandoffRecordSignable { ... }` struct literals in transport unit/integration tests were missing the new `burn` and `recipient` fields, causing E0063 compilation errors
- **Fix:** Updated all 4 literals to include `burn: false, recipient: None` in alphabetical position
- **Files modified:** src/transport/mod.rs
- **Verification:** `cargo test` — all transport tests pass
- **Committed in:** 861f9c7 (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both blocking issues were pre-existing (not caused by this plan's changes). Fixing them was necessary for compilation. No scope creep.

## Issues Encountered
- None beyond the blocking compilation issues documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- SEC-01 complete: burn and recipient are cryptographically signed
- v1.1 signed record format is established
- Ready for Phase 6 Plan 2 (pickup command verification of signed burn/recipient)

## Self-Check: PASSED

- FOUND: .planning/phases/06-signed-record-format/06-01-SUMMARY.md
- FOUND: src/record/mod.rs
- FOUND: tests/integration_round_trip.rs
- FOUND commit: 861f9c7 (Task 1)
- FOUND commit: 02ed7fd (Task 2)

---
*Phase: 06-signed-record-format*
*Completed: 2026-02-22*
