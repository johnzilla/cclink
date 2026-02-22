---
phase: 07-code-quality-and-transport
plan: 02
subsystem: transport
tags: [rust, transport, refactoring, session-management]

# Dependency graph
requires:
  - phase: 07-01
    provides: CclinkError::RecordNotFound typed error used in get_all_records silent skip
  - phase: 06-signed-record-format
    provides: HomeserverClient transport layer base used throughout
provides:
  - HomeserverClient with lazy signin via Cell<bool> signed_in field and ensure_signed_in()
  - get_all_records() encapsulating list+fetch-all pattern in transport layer
  - list command making one transport call instead of N individual get_record calls
affects:
  - 08-xx (CLI Fixes — commands rely on this transport layer)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Interior mutability via Cell<bool> for session state in &self methods without &mut self
    - Transport encapsulation — batch logical operation in transport layer; command layer makes one call

key-files:
  created: []
  modified:
    - src/transport/mod.rs
    - src/commands/list.rs

key-decisions:
  - "ensure_signed_in() uses Cell<bool> interior mutability so &self methods can set session state without requiring &mut self"
  - "get_all_records() is a logical batch — N individual HTTP fetches are an implementation detail of transport, not visible to callers"
  - "list.rs retains explicit client.signin() call rather than relying solely on ensure_signed_in — command intent is clear and the signed_in flag prevents actual double-signin"
  - "ROADMAP criterion #3 was already updated correctly in a prior session — no change needed"

patterns-established:
  - "Transport layer owns the listing+fetching pattern; command layer makes one call to get_all_records()"
  - "HomeserverClient signed_in: Cell<bool> prevents redundant signin HTTP calls within a single process lifetime"

requirements-completed: [QUAL-04, FUNC-03]

# Metrics
duration: 2min
completed: 2026-02-22
---

# Phase 7 Plan 02: Lazy Signin and List Optimization Summary

**Added lazy signin via Cell<bool> to HomeserverClient and encapsulated the list+fetch-all pattern in a new get_all_records() method; list command now makes one transport call instead of N individual fetches**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-02-22T21:33:07Z
- **Completed:** 2026-02-22T21:35:17Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added `signed_in: Cell<bool>` field to `HomeserverClient` — initialized to `false`, set to `true` in `signin()` after successful POST /session
- Added private `ensure_signed_in()` method that gates on the Cell flag — signin HTTP call happens at most once per client lifetime
- Updated `publish()` to call `ensure_signed_in()` instead of `signin()` directly — no redundant signin when the caller already has a session
- Added `get_all_records()` public method that encapsulates `list_record_tokens()` + N `get_record()` calls — records that fail silently skipped (consistent with prior list.rs behavior)
- Rewrote `run_list()` to use a single `client.get_all_records()` call with a functional `.filter()` chain for expired record removal — removed the `for token in &tokens { client.get_record(...) }` loop entirely
- Added `test_ensure_signed_in_flag` test verifying Cell<bool> state transitions
- All 45 tests pass, zero compiler warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Add lazy signin and get_all_records to HomeserverClient** - `f3348da` (feat)
2. **Task 2: Update list command to use batch get_all_records transport call** - `69650f2` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `src/transport/mod.rs` - Added Cell<bool> signed_in field, ensure_signed_in() method, signed_in.set(true) in signin(), ensure_signed_in() call in publish(), get_all_records() method, and test_ensure_signed_in_flag test
- `src/commands/list.rs` - Replaced explicit signin+token-loop+get_record pattern with signin+get_all_records() + functional filter chain

## Decisions Made
- `Cell<bool>` chosen for interior mutability — `signed_in` is logically a cache flag (not externally observable state), and `Cell` is the simplest no-overhead primitive for this single-threaded use case
- `publish()` uses `ensure_signed_in()` while command-level callers (list, revoke, pickup) keep explicit `signin()` calls — this preserves command intent clarity while the signed_in flag prevents actual redundant network calls
- `get_all_records()` documented as architectural encapsulation, not true batching — the doc comment clearly explains the N individual HTTP fetches are an unavoidable protocol constraint of the Pubky homeserver
- ROADMAP criterion #3 was already updated to the correct text — no change needed

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

During Task 1, `get_all_records()` temporarily produced a "method never used" warning before Task 2 wired it into list.rs. This was an expected transient state that resolved after Task 2.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Transport layer is clean and session-efficient — ready for Phase 8 CLI fixes
- HomeserverClient signs in exactly once per invocation across all command paths
- Zero warnings baseline maintained throughout both Phase 7 plans

---
*Phase: 07-code-quality-and-transport*
*Completed: 2026-02-22*

## Self-Check: PASSED

- src/transport/mod.rs: FOUND
- src/commands/list.rs: FOUND
- 07-02-SUMMARY.md: FOUND
- Commit f3348da: FOUND
- Commit 69650f2: FOUND
- Compiler warnings: 0
- Test suites passing: 5
