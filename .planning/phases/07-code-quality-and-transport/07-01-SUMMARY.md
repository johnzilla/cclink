---
phase: 07-code-quality-and-transport
plan: 01
subsystem: api
tags: [rust, error-handling, refactoring, utilities]

# Dependency graph
requires:
  - phase: 06-signed-record-format
    provides: CclinkError type and transport layer used as base for typed error enrichment
provides:
  - Canonical human_duration utility in src/util.rs shared across all commands
  - CclinkError::RecordNotFound typed variant replacing string-based 404 detection
  - Lean CclinkError enum with dead variants removed (zero compiler warnings)
affects:
  - 07-02 (transport improvements that use the same error types and transport module)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Typed error downcast via e.downcast_ref::<CclinkError>() for structured 404 detection
    - Single-source utility functions in src/util.rs imported via use crate::util::fn_name

key-files:
  created:
    - src/util.rs
  modified:
    - src/error.rs
    - src/transport/mod.rs
    - src/commands/pickup.rs
    - src/commands/list.rs
    - src/main.rs
    - src/lib.rs

key-decisions:
  - "CclinkError::RecordNotFound carries no payload — URL context added by caller via anyhow context chain"
  - "Dead variants removed without deprecation period — no external API surface, safe immediate removal"
  - "util.rs exposed as pub mod util in lib.rs for future integration test access"

patterns-established:
  - "Shared utilities live in src/util.rs and are imported via use crate::util::fn_name"
  - "HTTP 404 from transport layer surfaces as CclinkError::RecordNotFound, detected via downcast_ref in retry loops"

requirements-completed: [QUAL-01, QUAL-02, QUAL-03]

# Metrics
duration: 2min
completed: 2026-02-22
---

# Phase 7 Plan 01: Code Quality and Transport Summary

**Eliminated human_duration duplication across two commands, removed 5 dead CclinkError variants, and replaced fragile string-matching 404 detection with typed CclinkError::RecordNotFound + downcast_ref throughout the pickup retry loop**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-02-22T21:29:11Z
- **Completed:** 2026-02-22T21:31:00Z
- **Tasks:** 2
- **Files modified:** 6 (plus 1 created)

## Accomplishments
- Created `src/util.rs` as canonical home for `human_duration` — single definition, tests moved from pickup.rs and list.rs, both commands now import from crate::util
- Restructured `CclinkError`: removed 5 dead variants (InvalidKeyFormat, KeyCorrupted, RecordDeserializationFailed, HandoffExpired, NetworkRetryExhausted), added `RecordNotFound` — binary compiles with zero warnings
- Updated `transport/mod.rs` `get_bytes()` to return `CclinkError::RecordNotFound.into()` on HTTP 404 instead of a string-based anyhow error
- Updated all three 404-detection sites in `pickup.rs` retry loop from `msg.contains("not found") || msg.contains("404")` to typed `e.downcast_ref::<CclinkError>().map_or(false, |ce| matches!(ce, CclinkError::RecordNotFound))`
- All 77 tests pass, zero compiler warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Extract human_duration to shared util module and restructure CclinkError** - `122b387` (refactor)
2. **Task 2: Replace stringly-typed 404 detection with typed CclinkError::RecordNotFound** - `dfbb960` (refactor)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `src/util.rs` - New shared utility module with pub fn human_duration and its tests
- `src/error.rs` - CclinkError with 5 dead variants removed, RecordNotFound added (6 total variants)
- `src/transport/mod.rs` - get_bytes() returns CclinkError::RecordNotFound on 404
- `src/commands/pickup.rs` - Removed inline human_duration, added use crate::util::human_duration, replaced 3 string-matching sites with typed downcast
- `src/commands/list.rs` - Removed inline human_duration and its tests, added use crate::util::human_duration
- `src/main.rs` - Added mod util; declaration
- `src/lib.rs` - Added pub mod util; declaration

## Decisions Made
- `RecordNotFound` carries no payload — the surrounding anyhow context already contains the URL when needed; a bare variant is cleaner and sufficient
- Dead variants removed immediately without deprecation — this is a private binary crate with no external API consumers
- `pub mod util` in lib.rs ensures integration tests can access utilities if needed in future plans

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. The `RecordNotFound` variant produced a "never constructed" warning after Task 1 (expected — Task 2 had not yet added the usage). After Task 2 the warning disappeared.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Error types are clean and typed — ready for 07-02 transport improvements
- Shared utility module in place for any future common functions
- Zero warnings baseline established for the codebase

---
*Phase: 07-code-quality-and-transport*
*Completed: 2026-02-22*

## Self-Check: PASSED

- src/util.rs: FOUND
- src/error.rs: FOUND
- 07-01-SUMMARY.md: FOUND
- Commit 122b387: FOUND
- Commit dfbb960: FOUND
- Compiler warnings: 0
