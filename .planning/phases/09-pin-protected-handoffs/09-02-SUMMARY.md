---
phase: 09-pin-protected-handoffs
plan: 02
subsystem: cli, commands, crypto
tags: [pin, publish, pickup, dialoguer, password-prompt, integration-tests, argon2id, age]

# Dependency graph
requires:
  - phase: 09-01
    provides: pin_encrypt, pin_decrypt, pin_derive_key, pin_salt field in HandoffRecord/HandoffRecordSignable
  - phase: 08-cli-fixes-and-documentation
    provides: --burn/--share conflicts_with pattern; CLI structure
provides:
  - "--pin flag on Cli struct (conflicts_with share)"
  - "PIN prompt in publish flow (dialoguer::Password with confirmation)"
  - "pin_salt populated in HandoffRecordSignable and HandoffRecord when --pin is set"
  - "PIN detection in pickup flow (pin_salt.is_some() triggers PIN prompt)"
  - "pin_decrypt call in pickup with clear error on wrong PIN"
  - "Integration tests: test_pin_encrypt_round_trip, test_pin_record_owner_cannot_decrypt"
affects: none — phase 9 feature complete

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "PIN publish: cli.pin branch -> dialoguer::Password with confirmation -> pin_encrypt -> base64(salt) in pin_salt field"
    - "PIN pickup: pin_salt.is_some() guard (before is_cross_user check) -> dialoguer::Password single entry -> pin_decrypt -> Err prints 'Incorrect PIN'"
    - "Non-interactive guard: bail if stdin.is_terminal() is false when pin_salt present"
    - "PIN path independent of keypair ownership (works for both self and cross-user pickup)"

key-files:
  created: []
  modified:
    - "src/cli.rs - added --pin flag with conflicts_with = share"
    - "src/commands/publish.rs - branched section 4 for --pin path; pin_salt_value populated; PIN notice in output"
    - "src/commands/pickup.rs - added PIN detection block before is_cross_user; dialoguer::Password; pin_decrypt; wrong PIN error"
    - "src/crypto/mod.rs - removed #[allow(dead_code)] from pin_derive_key, pin_encrypt, pin_decrypt"
    - "tests/integration_round_trip.rs - added test_pin_encrypt_round_trip and test_pin_record_owner_cannot_decrypt"

key-decisions:
  - "--pin conflicts_with share (not burn): --pin + --burn is valid (burn-after-read PIN-protected record); --pin + --share is nonsensical since PIN replaces key-based encryption"
  - "PIN pickup path runs BEFORE is_cross_user check: PIN-derived key is independent of keypair identity, so cross-user vs self-pickup distinction is irrelevant for PIN records"
  - "Single-entry PIN prompt on pickup (no confirmation): pickup is read-only; confirmation prompt would be redundant and user-hostile"
  - "Non-interactive guard on pickup: bail with clear message when pin_salt present but stdin is not a terminal"
  - "#[allow(dead_code)] annotations removed from pin_derive_key/pin_encrypt/pin_decrypt — functions now wired to binary, warnings gone"

# Metrics
duration: 2min
completed: 2026-02-22
---

# Phase 9 Plan 02: PIN-Protected Handoff CLI Integration Summary

**End-to-end --pin flag wiring: publish prompts for PIN and calls pin_encrypt; pickup detects pin_salt and calls pin_decrypt with single-entry PIN prompt**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-02-22T23:05:23Z
- **Completed:** 2026-02-22T23:07:55Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `--pin` flag to `Cli` struct in `src/cli.rs` with `conflicts_with = "share"`, so `--pin --share` is rejected at parse time (same pattern as `--burn`/`--share`)
- Modified section 4 of `publish.rs` to branch on `cli.pin`: when set, prompts for PIN via `dialoguer::Password` with confirmation, calls `crate::crypto::pin_encrypt`, stores salt as base64 in `pin_salt_value`; the existing age-encrypt path is now the else branch
- Updated `HandoffRecordSignable` and `HandoffRecord` construction to use `pin_salt: pin_salt_value.clone()` / `pin_salt: pin_salt_value` instead of hardcoded `None`
- Added PIN-protected notice to output section 7 (yellow, after --burn warning)
- Added PIN detection block in `pickup.rs` before the `is_cross_user` branch: detects `record.pin_salt.is_some()`, guards against non-interactive stdin, decodes salt, prompts via `dialoguer::Password` (single entry), calls `pin_decrypt` — wrong PIN prints "Incorrect PIN" and bails
- Removed all three `#[allow(dead_code)]` annotations from `src/crypto/mod.rs` since functions are now called by the binary
- Added `test_pin_encrypt_round_trip` integration test: correct PIN decrypts, wrong PIN returns Err
- Added `test_pin_record_owner_cannot_decrypt` integration test: owner's keypair identity cannot decrypt PIN data, correct PIN succeeds

## Task Commits

1. **Task 1:** `d10a433` — feat(09-02): add --pin CLI flag and wire publish flow with PIN encryption
2. **Task 2:** `0dc30cc` — feat(09-02): wire pickup flow for PIN-protected records and add integration tests

## Files Created/Modified

- `/home/john/vault/projects/github.com/cclink/src/cli.rs` — Added `--pin` flag with `conflicts_with = "share"`
- `/home/john/vault/projects/github.com/cclink/src/commands/publish.rs` — PIN branch in section 4, pin_salt wired through, PIN notice in output
- `/home/john/vault/projects/github.com/cclink/src/commands/pickup.rs` — PIN detection and decryption block
- `/home/john/vault/projects/github.com/cclink/src/crypto/mod.rs` — Removed `#[allow(dead_code)]` from 3 functions
- `/home/john/vault/projects/github.com/cclink/tests/integration_round_trip.rs` — 2 new PIN integration tests

## Decisions Made

- **--pin conflicts_with share but not burn:** `--pin --burn` is a valid combination (burn-after-read PIN-protected record). `--pin --share` is nonsensical since PIN-derived key replaces key-based encryption entirely.
- **PIN pickup path before is_cross_user:** PIN decryption uses a derived key independent of keypair identity. Whether the pickup is cross-user or self doesn't matter for PIN records — the PIN path runs first.
- **Single-entry PIN on pickup:** Confirmation is needed at publish time (you set the PIN). At pickup, you're entering a known PIN — confirmation would be redundant.
- **Non-interactive guard on pickup:** If `pin_salt` is present but stdin is not a terminal, bail with a clear error rather than hanging or failing obscurely.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. All tests passed on first run after implementation.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

Phase 9 is complete. Both plans (09-01 crypto foundation, 09-02 CLI wiring) are done. The PIN-protected handoff feature is fully implemented end-to-end:
- `cclink --pin` publishes a PIN-protected record with non-null `pin_salt`
- `cclink pickup` detects `pin_salt` and prompts for PIN; wrong PIN shows clear error
- All 54 unit tests and 8 integration tests pass with zero compiler warnings

## Self-Check: PASSED

- FOUND: src/cli.rs (--pin flag with conflicts_with = "share")
- FOUND: src/commands/publish.rs (pin_encrypt call, pin_salt_value in both struct constructions)
- FOUND: src/commands/pickup.rs (PIN detection block with pin_decrypt call)
- FOUND: src/crypto/mod.rs (#[allow(dead_code)] removed from all 3 PIN functions)
- FOUND: tests/integration_round_trip.rs (test_pin_encrypt_round_trip, test_pin_record_owner_cannot_decrypt)
- FOUND: d10a433 (Task 1 commit)
- FOUND: 0dc30cc (Task 2 commit)
- All tests pass: 40 lib tests + 42 bin tests + 8 integration tests + 3 plaintext_leak tests = zero failures
- Zero compiler warnings in cargo build

---
*Phase: 09-pin-protected-handoffs*
*Completed: 2026-02-22*
