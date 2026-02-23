---
phase: 11-prerequisites
plan: 02
subsystem: infra
tags: [cargo-audit, backon, backoff, rustsec, retry, exponential-backoff, security]

# Dependency graph
requires:
  - phase: 11-prerequisites
    provides: "cargo clippy and fmt baseline (11-01)"
provides:
  - "cargo audit exits 0 — RUSTSEC-2025-0012 (backoff) and RUSTSEC-2024-0384 (instant) eliminated"
  - "pickup.rs retry logic using backon BlockingRetryable with 30s wall-clock total delay cap"
  - "All three Phase 11 gates pass: clippy, fmt, audit"
affects:
  - 12-ci

# Tech tracking
tech-stack:
  added:
    - "backon 1.6.0 — blocking retry with exponential backoff; recommended replacement for backoff in RUSTSEC-2025-0012"
  patterns:
    - "BlockingRetryable fluent API: closure.retry(builder).sleep(std::thread::sleep).when(predicate).call()"
    - ".when() predicate returns false to STOP retrying (inverse of backoff's BackoffError::permanent semantics)"
    - ".sleep(std::thread::sleep) required for blocking retries — omitting it makes retries instant (no delay)"
    - "with_total_delay(Some(Duration::from_secs(30))) preferred over with_max_times(N) for wall-clock parity"

key-files:
  created: []
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/commands/pickup.rs

key-decisions:
  - "Replace backoff with backon (not audit.toml ignore) — eliminates both RUSTSEC-2025-0012 and transitive RUSTSEC-2024-0384 (instant) at once"
  - "Use with_total_delay(Some(Duration::from_secs(30))) — verified that ExponentialBuilder has this method in backon 1.6.0, provides exact parity with original max_elapsed_time: Some(30s)"
  - "Move use backon:: to file-level imports — idiomatic Rust vs old function-scoped use backoff:: inside run_pickup()"

patterns-established:
  - "backon blocking retry: always include .sleep(std::thread::sleep) or retries have no delay"
  - "backon .when() stop semantics: return !is_permanent_error(e) (true = continue, false = stop)"

requirements-completed: [DEP-02]

# Metrics
duration: 2min
completed: 2026-02-23
---

# Phase 11 Plan 02: Prerequisites Summary

**Replaced unmaintained backoff crate with backon 1.6.0 in pickup.rs, eliminating RUSTSEC-2025-0012 and transitive RUSTSEC-2024-0384 (instant); all three Phase 11 gates (clippy, fmt, audit) now pass**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-02-23T23:52:41Z
- **Completed:** 2026-02-23T23:53:55Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Replaced `backoff = "0.4"` with `backon = "1.6"` in Cargo.toml, removing both RUSTSEC-2025-0012 (backoff unmaintained) and RUSTSEC-2024-0384 (instant unmaintained, transitive) from cargo audit output
- Rewrote retry block in `src/commands/pickup.rs` using `backon::BlockingRetryable` fluent API with `with_total_delay(30s)` for exact parity with the original `max_elapsed_time: Some(30s)` behavior
- Verified all 90 tests pass (37 lib, 39 main, 8 integration round-trip, 6 plaintext leak) with no regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace backoff with backon in Cargo.toml and rewrite retry logic in pickup.rs** - `bf770bc` (feat)
2. **Task 2: Verify all three Phase 11 gates pass** - no file changes (verification only)

**Plan metadata:** (docs commit — see final commit)

## Files Created/Modified

- `Cargo.toml` - Replaced `backoff = "0.4"` with `backon = "1.6"`
- `Cargo.lock` - Updated automatically; backoff and instant entries removed
- `src/commands/pickup.rs` - Added `use backon::{BlockingRetryable, ExponentialBuilder}` at file level; rewrote retry block using fluent `.retry().sleep().when().call()` pattern

## Decisions Made

- Used `with_total_delay(Some(Duration::from_secs(30)))` rather than `with_max_times(6)`. The plan asked to verify whether `with_total_delay` exists in backon 1.6.0 and prefer it if so. Verified via `~/.cargo/registry/src/` source that it is a `const fn` on `ExponentialBuilder` in backon 1.6.0. This gives exact parity with the original `max_elapsed_time: Some(Duration::from_secs(30))`.
- Moved the import from inside the function body (`use backoff::...` was on line 59, inside `run_pickup`) to file-level `use` statements. This is idiomatic Rust and avoids function-scoped imports that obscure dependencies.

## Deviations from Plan

None - plan executed exactly as written. The `with_total_delay` verification path was already anticipated by the plan ("If `with_total_delay` exists, prefer it over `with_max_times(6)`").

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- All three Phase 11 gates now pass: `cargo clippy --all-targets -- -D warnings` exits 0, `cargo fmt --check` exits 0, `cargo audit` exits 0.
- Phase 12 (CI gates) can add all three gates as CI enforcement without day-one failures.
- DEP-02 requirement fully resolved: both RUSTSEC advisories eliminated, no audit.toml ignores needed.

## Self-Check: PASSED

- FOUND: Cargo.toml (backoff removed, backon = "1.6" present)
- FOUND: src/commands/pickup.rs (BlockingRetryable, .sleep(std::thread::sleep), .when() predicate)
- FOUND: 11-02-SUMMARY.md
- FOUND commit: bf770bc (feat(11-02): replace backoff with backon for RUSTSEC-2025-0012)
- cargo audit exits 0 — no RUSTSEC advisories
- cargo clippy --all-targets -- -D warnings exits 0
- cargo fmt --check exits 0
- cargo test: 90 tests pass, 0 failures

---
*Phase: 11-prerequisites*
*Completed: 2026-02-23*
