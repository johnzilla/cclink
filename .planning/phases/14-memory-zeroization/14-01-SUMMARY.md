---
phase: 14-memory-zeroization
plan: 01
subsystem: crypto
tags: [zeroize, memory-safety, ed25519, x25519, argon2, hkdf, keypair]

# Dependency graph
requires:
  - phase: 13-pin-strength
    provides: "PIN validation for publish command — crypto layer already in place"
provides:
  - "zeroize = 1 as direct Cargo dependency"
  - "ed25519_to_x25519_secret returns Zeroizing<[u8;32]> — X25519 scalar zeroed on drop"
  - "pin_derive_key returns Zeroizing<[u8;32]> with argon2_output and okm also Zeroizing"
  - "load_keypair reads hex file into Zeroizing<String> and decodes into Zeroizing<[u8;32]>"
affects: [15-encrypted-key-storage, 16-pin-on-load]

# Tech tracking
tech-stack:
  added: [zeroize = "1"]
  patterns:
    - "Zeroizing<[u8;32]> as return type for all crypto secret derivation functions"
    - "Manual hex decode loop into Zeroizing<[u8;32]> — avoids Vec<u8> allocation for secret bytes"
    - "Deref-based assert comparisons: assert_ne!(*zeroizing_val, [0u8;32]) for test assertions"

key-files:
  created: []
  modified:
    - Cargo.toml
    - src/crypto/mod.rs
    - src/keys/store.rs

key-decisions:
  - "Use Zeroizing<[u8;32]> as return type (not a newtype) so callers auto-deref with no changes"
  - "Wrap argon2_output and okm internally in pin_derive_key so intermediate secrets are also zeroed"
  - "Manual byte-by-byte hex decode in load_keypair avoids any intermediate Vec<u8> holding secret bytes"
  - "from_secret_key_file calls in init.rs (import path) are out of scope for ZERO-01/ZERO-02 — deferred"

patterns-established:
  - "Zeroizing wrapper pattern: Zeroizing::new([0u8;32]) for stack buffers holding secret material"
  - "Auto-deref compatibility: Zeroizing<[u8;32]> implements Deref<Target=[u8;32]>, so &secret passes where &[u8;32] expected"
  - "Test assertion pattern for Zeroizing: use *val to deref before assert_ne! with bare array literal"

requirements-completed: [ZERO-01, ZERO-02]

# Metrics
duration: 3min
completed: 2026-02-24
---

# Phase 14 Plan 01: Memory Zeroization — Crypto and KeyStore Summary

**Zeroizing<[u8;32]> applied to all secret material: X25519 scalar, PIN-derived key, and hex-decoded keypair seed are automatically zeroed on drop via the zeroize crate**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-02-24T14:29:30Z
- **Completed:** 2026-02-24T14:31:58Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Added `zeroize = "1"` as a direct dependency in Cargo.toml
- `ed25519_to_x25519_secret` now returns `Zeroizing<[u8;32]>` — scalar wiped when caller drops it
- `pin_derive_key` now returns `Zeroizing<[u8;32]>` with internal `argon2_output` and `okm` also wrapped — all intermediate secret bytes zeroed on drop
- `load_keypair` replaced `pkarr::from_secret_key_file` with a manual implementation reading into `Zeroizing<String>` and decoding hex byte-by-byte into `Zeroizing<[u8;32]>` — no unprotected heap allocation holds secret bytes
- All 103 tests pass, clippy clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Add zeroize dependency and wrap crypto return types** - `3f46dd2` (feat)
2. **Task 2: Reimplement load_keypair with Zeroizing buffers** - `e2ab726` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `Cargo.toml` - Added `zeroize = "1"` to dependencies
- `src/crypto/mod.rs` - Added `use zeroize::Zeroizing`; changed `ed25519_to_x25519_secret` and `pin_derive_key` return types; wrapped internal `argon2_output` and `okm`
- `src/keys/store.rs` - Added `use zeroize::Zeroizing`; replaced `from_secret_key_file` call with manual hex-decode using `Zeroizing` buffers

## Decisions Made
- Used `Zeroizing<[u8;32]>` as return type directly (not a newtype) so all callers auto-deref without changes — `age_identity(&x25519_secret)` passes `&Zeroizing<[u8;32]>` which auto-derefs to `&[u8;32]` as required
- Wrapped intermediate `argon2_output` and `okm` inside `pin_derive_key` — both are secret material and must be zeroed, not just the final output
- Manual byte-by-byte hex decode chosen over using the `hex` crate to avoid any `Vec<u8>` intermediate allocation that would hold secret bytes on the heap unprotected

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed assert_ne! type mismatch in tests after Zeroizing return type change**
- **Found during:** Task 1 (verifying cargo test after return type changes)
- **Issue:** `assert_ne!(scalar1, [0u8; 32])` fails to compile because `Zeroizing<[u8;32]>` implements `PartialEq<Zeroizing<[u8;32]>>` but not `PartialEq<[u8;32]>` directly — `assert_ne!` macro requires identical types
- **Fix:** Changed `assert_ne!(scalar1, [0u8; 32])` to `assert_ne!(*scalar1, [0u8; 32])` in two test functions: `test_ed25519_to_x25519_secret_deterministic` and `test_pin_derive_key_deterministic`
- **Files modified:** `src/crypto/mod.rs`
- **Verification:** `cargo test` passes with all 103 tests after fix
- **Committed in:** `3f46dd2` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - type mismatch bug)
**Impact on plan:** Auto-fix necessary for correctness. Deref pattern (`*val`) is the idiomatic solution. No scope creep.

## Deferred Items

- `pkarr::Keypair::from_secret_key_file` is still called in `src/commands/init.rs` (3 sites: `prompt_overwrite`, `import_from_file`, `import_from_stdin`). These are in the `cclink init --import` path and are pre-existing uses outside the scope of ZERO-01/ZERO-02, which target the operational key-load path (`load_keypair`). Deferred to a future plan if the import path also needs zeroization.

## Issues Encountered
None beyond the auto-fixed type mismatch above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 14 Plan 01 complete: Zeroizing patterns established for crypto layer and key store
- Phase 14 Plan 02 (if any) can build on these patterns
- Phase 15 (encrypted key storage) can use `Zeroizing<[u8;32]>` as the canonical type for secret material throughout the new encrypted load path

---
*Phase: 14-memory-zeroization*
*Completed: 2026-02-24*

## Self-Check: PASSED

- FOUND: Cargo.toml
- FOUND: src/crypto/mod.rs
- FOUND: src/keys/store.rs
- FOUND: 14-01-SUMMARY.md
- FOUND commit: 3f46dd2 (Task 1)
- FOUND commit: e2ab726 (Task 2)
- zeroize = "1" in Cargo.toml: PASS
- Zeroizing<[u8; 32]> return types in crypto/mod.rs: PASS
- Zeroizing::new in keys/store.rs: PASS
