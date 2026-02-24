---
phase: 14-memory-zeroization
plan: 02
subsystem: commands
tags: [zeroize, memory-safety, PIN, dialoguer, publish, pickup]

# Dependency graph
requires:
  - phase: 14-01
    provides: "Zeroizing<[u8;32]> patterns for crypto layer and key store"
provides:
  - "Zeroizing<String> wrapping of dialoguer::Password::interact() results in publish.rs"
  - "Zeroizing<String> wrapping of dialoguer::Password::interact() results in pickup.rs"
  - "PIN string memory zeroized on drop at both prompt sites (ZERO-03)"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Zeroizing::new(dialoguer::Password::new()...interact()?) — wrap prompt result directly"
    - "Zeroizing<String> auto-derefs to &String then &str for all downstream crypto calls"

key-files:
  created: []
  modified:
    - src/commands/publish.rs
    - src/commands/pickup.rs

key-decisions:
  - "Wrap at the interact() call site with Zeroizing::new() so no bare String copy escapes — Zeroizing<String> drops the heap buffer on scope exit"
  - "No downstream changes needed — Zeroizing<String> Deref<Target=String> then String Deref<Target=str> means &pin passes where &str expected"

requirements-completed: [ZERO-03]

# Metrics
duration: 3min
completed: 2026-02-24
---

# Phase 14 Plan 02: Memory Zeroization — PIN Prompt Wrapping Summary

**Zeroizing<String> applied at both dialoguer::Password::interact() call sites in publish.rs and pickup.rs, ensuring user-entered PIN strings are automatically wiped from heap memory when they go out of scope**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-02-24T14:34:42Z
- **Completed:** 2026-02-24T14:37:00Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Added `use zeroize::Zeroizing;` import to `src/commands/publish.rs`
- Added `use zeroize::Zeroizing;` import to `src/commands/pickup.rs`
- Wrapped `dialoguer::Password::interact()` result in `Zeroizing::new(...)` in publish.rs (PIN prompt for `--pin` protected handoffs)
- Wrapped `dialoguer::Password::interact()` result in `Zeroizing::new(...)` in pickup.rs (PIN prompt for decrypting PIN-protected handoffs)
- All downstream usage (`validate_pin(&pin)`, `pin_encrypt(&payload_bytes, &pin)`, `pin_decrypt(&ciphertext, &pin, &salt)`) unchanged — `Zeroizing<String>` auto-derefs to `&str` via the `Deref` chain
- All 103 tests pass, clippy clean

## Task Commits

1. **Task 1: Wrap PIN prompts in Zeroizing in publish.rs and pickup.rs** - `fcd6d9d` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `src/commands/publish.rs` — Added `use zeroize::Zeroizing`; wrapped dialoguer PIN prompt in `Zeroizing::new(...)`
- `src/commands/pickup.rs` — Added `use zeroize::Zeroizing`; wrapped dialoguer PIN prompt in `Zeroizing::new(...)`

## Decisions Made

- Wrapped at the `interact()` call site directly with `Zeroizing::new(...)` — the prompt result `String` is immediately placed inside the `Zeroizing` wrapper before any other use, so no bare unprotected `String` allocation exists at any point after the prompt returns
- No downstream changes to `validate_pin`, `pin_encrypt`, or `pin_decrypt` were needed — `Zeroizing<String>` implements `Deref<Target=String>` and `String` implements `Deref<Target=str>`, so `&pin` satisfies `&str` parameters through the two-step deref chain

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None.

## Phase 14 Completion

Phase 14 (Memory Zeroization) is now complete:
- **14-01:** `Zeroizing<[u8;32]>` applied to `ed25519_to_x25519_secret`, `pin_derive_key` (with internal intermediates), and `load_keypair` hex decode buffer (ZERO-01, ZERO-02)
- **14-02:** `Zeroizing<String>` applied at both `dialoguer::Password::interact()` call sites in publish.rs and pickup.rs (ZERO-03)

All PIN and key material entering the process is now wrapped in `Zeroizing` from the point of collection or derivation, ensuring automatic heap zeroing on drop.

## Next Phase Readiness

- Phase 15 (encrypted key storage) can build on established `Zeroizing<[u8;32]>` patterns
- Phase 16 (PIN on load) can use `Zeroizing<String>` as the canonical type for PIN prompts (pattern now established in both command files)

---
*Phase: 14-memory-zeroization*
*Completed: 2026-02-24*

## Self-Check: PASSED

- FOUND: src/commands/publish.rs
- FOUND: src/commands/pickup.rs
- FOUND: 14-02-SUMMARY.md
- FOUND commit: fcd6d9d (Task 1)
- use zeroize::Zeroizing in publish.rs: PASS
- use zeroize::Zeroizing in pickup.rs: PASS
- Zeroizing::new wrapping in publish.rs: PASS
- Zeroizing::new wrapping in pickup.rs: PASS
