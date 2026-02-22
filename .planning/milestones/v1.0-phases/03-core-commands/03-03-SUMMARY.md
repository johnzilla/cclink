---
phase: 03-core-commands
plan: 03
subsystem: commands
tags: [rust, pickup, age, backoff, dialoguer, qr2term, owo-colors, exec, pkarr]

# Dependency graph
requires:
  - phase: 03-02
    provides: HomeserverClient with get_latest/get_record/get_record_by_pubkey, publish command, age encrypt, record sign
  - phase: 02-01
    provides: crypto module with ed25519_to_x25519_secret, age_identity, age_decrypt
  - phase: 02-02
    provides: HandoffRecord, LatestPointer, verify_record

provides:
  - pickup command: retrieves latest handoff, verifies signature, checks TTL, decrypts session ID, shows confirmation, execs claude --resume
  - human_duration() helper: converts seconds to Xh/Xm/Xs human-readable format
  - launch_claude_resume(): Unix exec() replacement, non-Unix child process fallback
  - cross-user pickup: shows cleartext metadata with limitation notice

affects:
  - 04-advanced-encryption (--share flag, shared handoffs)
  - 05-polish (end-to-end testing, error messaging)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - backoff::retry with ExponentialBackoff for transient/permanent error discrimination
    - permanent errors for 404/not-found, transient for network failures
    - Unix exec() via CommandExt::exec() to replace cclink process with claude
    - TTY guard for confirmation prompts (skip when stdin not a terminal or --yes)
    - Cross-user vs self-pickup branch at runtime based on pubkey arg presence

key-files:
  created: []
  modified:
    - src/commands/pickup.rs

key-decisions:
  - "Self-pickup signs in (get_record via session cookie); cross-user uses get_record_by_pubkey (public multi-tenant path) — no session cookie needed for cross-user"
  - "Retry wraps the full sequence (get_latest + get_record); not-found errors are permanent to avoid pointless retries"
  - "owo_colors uses Stdout stream for all pickup output (TTY detection per-call via if_supports_color)"
  - "human_duration() is a module-private fn — intentionally not pub, used only within pickup command"

patterns-established:
  - "Backoff pattern: ExponentialBackoff with max_elapsed_time=30s, max_interval=8s, initial=2s; check error message for 'not found'/'404' to determine permanent vs transient"
  - "TTY guard for interactive prompts: args.yes || !std::io::stdin().is_terminal() — applies to both publish (selection) and pickup (confirm)"

requirements-completed: [RET-01, RET-02, RET-03, RET-04, RET-05, RET-06, UX-01]

# Metrics
duration: 2min
completed: 2026-02-22
---

# Phase 3 Plan 03: Pickup Command Summary

**`cclink pickup` completes the handoff loop: retrieves and age-decrypts the latest session pointer, enforces TTL, prompts for confirmation, and execs `claude --resume` replacing the process on Unix.**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-22T14:00:41Z
- **Completed:** 2026-02-22T14:02:17Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Pickup command replaces the `todo!()` stub with full implementation (256 lines): retrieval, TTL check, decrypt, confirm, QR, exec
- Exponential backoff (2s/8s/30s) wraps the full get_latest + get_record chain; 404 errors fail fast (permanent), network errors retry
- Cross-user pickup shows hostname, project, age metadata with a clear yellow warning about decryption limitation
- 4 new unit tests for `human_duration()` (26 total pass, 1 integration test ignored)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement pickup retrieval with retry, TTL check, and cross-user handling** - `3b50db4` (feat)
2. **Task 2: Final integration verification** - No separate commit (verification only, no file changes)

## Files Created/Modified
- `src/commands/pickup.rs` - Full pickup command: retry/backoff retrieval, TTL enforcement, age decrypt, dialoguer confirm, QR render, Unix exec()

## Decisions Made
- Self-pickup calls `client.signin()` first (needs session cookie for own /pub path), then `client.get_record()` — cross-user uses public multi-tenant path, no sign-in required
- Retry wraps both `get_latest` and `get_record` as a single operation to handle partial failures gracefully
- `owo_colors::Stream::Stdout` used for all pickup output (not Stderr) since pickup output is primary user-facing content
- `human_duration()` kept module-private — only used within pickup, no reason to expose

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered
None — compilation succeeded first attempt, all 26 tests passed.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 3 is complete: both `cclink` (publish) and `cclink pickup` are implemented and tested
- The full publish-to-pickup loop is code-complete and ready for end-to-end testing against a live pubky.app homeserver
- Phase 4 (Advanced Encryption) can begin: `--share` flag for cross-user decryption, Argon2id PIN mode

## Self-Check: PASSED

- FOUND: src/commands/pickup.rs
- FOUND: .planning/phases/03-core-commands/03-03-SUMMARY.md
- FOUND commit: 3b50db4 (feat(03-03): implement pickup command...)

---
*Phase: 03-core-commands*
*Completed: 2026-02-22*
