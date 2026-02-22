---
phase: 09-pin-protected-handoffs
plan: 01
subsystem: crypto
tags: [argon2id, hkdf, sha2, age, x25519, pin, key-derivation, encryption]

# Dependency graph
requires:
  - phase: 02-crypto-and-transport
    provides: age_encrypt, age_decrypt, age_identity, age_recipient functions
  - phase: 06-signed-record-format
    provides: HandoffRecord and HandoffRecordSignable struct definitions
provides:
  - PIN key derivation via Argon2id (t=3, m=64MB, p=1) + HKDF-SHA256 with cclink-pin-v1 info
  - pin_encrypt: generates random salt, derives X25519 key, encrypts with age
  - pin_decrypt: re-derives X25519 key from PIN+salt, decrypts with age; wrong PIN returns Err
  - pin_salt field in HandoffRecord and HandoffRecordSignable (signed into envelope)
affects: 09-02-pin-protected-handoffs (will wire pin_encrypt/pin_decrypt into publish/pickup commands)

# Tech tracking
tech-stack:
  added:
    - "argon2 = 0.5 (Argon2id password hashing)"
    - "hkdf = 0.12 (HKDF key derivation)"
    - "sha2 = 0.10 (SHA-256 for HKDF)"
    - "rand = 0.8 (random salt generation)"
  patterns:
    - "PIN -> Argon2id(64MB, t=3) -> raw bytes -> HKDF-SHA256(info=cclink-pin-v1) -> X25519 scalar"
    - "PIN-derived key used as raw X25519 scalar input to age_identity() for age encryption"
    - "Random 32-byte salt stored as base64 in pin_salt field of HandoffRecord"
    - "dead_code suppressed with #[allow(dead_code)] for pub fns not yet wired to binary"

key-files:
  created: []
  modified:
    - "Cargo.toml - added argon2, hkdf, sha2, rand dependencies"
    - "src/crypto/mod.rs - added pin_derive_key, pin_encrypt, pin_decrypt functions"
    - "src/record/mod.rs - added pin_salt field to HandoffRecord and HandoffRecordSignable"
    - "src/commands/publish.rs - updated HandoffRecordSignable/HandoffRecord constructions with pin_salt: None"
    - "src/transport/mod.rs - updated all test HandoffRecord/HandoffRecordSignable constructions with pin_salt: None"
    - "tests/integration_round_trip.rs - updated HandoffRecord/HandoffRecordSignable constructions with pin_salt: None"

key-decisions:
  - "Argon2id parameters: t_cost=3, m_cost=65536 (64MB), p_cost=1 — balances security with usability on CLI"
  - "HKDF info string cclink-pin-v1 for domain separation — version in info string allows future algorithm migration"
  - "pin_salt placed alphabetically between project and pubkey in both HandoffRecord and HandoffRecordSignable"
  - "#[allow(dead_code)] on pin_derive_key/pin_encrypt/pin_decrypt — functions are pub but not yet called by bin; annotation removed in 09-02 when wired up"
  - "PIN-derived X25519 scalar fed directly into age_identity() — HKDF expansion ensures correct domain even if Argon2id has bias"

patterns-established:
  - "PIN crypto pattern: pin_derive_key -> age_identity(derived) -> age_encrypt for encryption; same derivation -> age_decrypt for decryption"
  - "All struct constructions updated with pin_salt: None for backwards compatibility (serde default = None)"

requirements-completed: [SEC-03]

# Metrics
duration: 4min
completed: 2026-02-22
---

# Phase 9 Plan 01: PIN Key Derivation and Crypto Foundation Summary

**Argon2id+HKDF-SHA256 PIN key derivation with age X25519 encrypt/decrypt; pin_salt field added to HandoffRecord signed envelope**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-22T22:57:48Z
- **Completed:** 2026-02-22T23:01:48Z
- **Tasks:** 2 (TDD: RED + GREEN)
- **Files modified:** 6

## Accomplishments

- Implemented `pin_derive_key(pin, salt)` using Argon2id (t=3, m=64MB, p=1) followed by HKDF-SHA256 expansion with domain-separation info `cclink-pin-v1` — deterministic 32-byte output
- Implemented `pin_encrypt(plaintext, pin)` generating random salt, deriving X25519 key, and encrypting with age — returns `(ciphertext, salt)`
- Implemented `pin_decrypt(ciphertext, pin, salt)` that re-derives the key and decrypts — wrong PIN returns `Err`, never panics
- Added `pin_salt: Option<String>` to `HandoffRecord` and `HandoffRecordSignable` in alphabetical position (between `project` and `pubkey`), signed into the envelope
- Updated 6 files (publish.rs, transport/mod.rs, integration tests) with `pin_salt: None` for backwards compatibility

## Task Commits

TDD plan executed with RED/GREEN commits:

1. **RED: Failing tests for PIN functions** - `a609202` (test)
2. **GREEN: Implement pin_derive_key, pin_encrypt, pin_decrypt + record fields** - `88828ff` (feat)

**Plan metadata:** (created in final commit)

_TDD tasks have two commits: test (RED) then feat (GREEN)._

## Files Created/Modified

- `/home/john/vault/projects/github.com/cclink/Cargo.toml` - Added argon2, hkdf, sha2, rand dependencies
- `/home/john/vault/projects/github.com/cclink/src/crypto/mod.rs` - Added pin_derive_key, pin_encrypt, pin_decrypt with imports and tests
- `/home/john/vault/projects/github.com/cclink/src/record/mod.rs` - Added pin_salt field to both structs, updated From impl and all test helpers
- `/home/john/vault/projects/github.com/cclink/src/commands/publish.rs` - Added pin_salt: None to HandoffRecordSignable and HandoffRecord construction
- `/home/john/vault/projects/github.com/cclink/src/transport/mod.rs` - Added pin_salt: None to all 4 test HandoffRecordSignable/HandoffRecord instances
- `/home/john/vault/projects/github.com/cclink/tests/integration_round_trip.rs` - Added pin_salt: None to both tamper detection test constructions

## Decisions Made

- **Argon2id parameters:** t_cost=3, m_cost=65536 (64MB), p_cost=1. Conservative for a CLI tool — strong enough for PIN protection without excessive memory use.
- **HKDF info string `cclink-pin-v1`:** Provides domain separation and a version indicator. If the algorithm changes in future, `cclink-pin-v2` can be introduced without key collision.
- **Alphabetical field placement:** `pin_salt` placed between `project` and `pubkey` in both structs, consistent with the existing alphabetical field ordering convention for deterministic JSON serialization.
- **`#[allow(dead_code)]` annotations:** Applied to the three new `pub` functions to suppress bin-target warnings. These functions are used in tests but not yet called from the binary — 09-02 will wire them up and the annotations will be removed.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added `#[allow(dead_code)]` to suppress bin target warnings**

- **Found during:** GREEN phase (cargo build verification)
- **Issue:** `pin_derive_key`, `pin_encrypt`, and `pin_decrypt` are `pub` but not yet called by the binary, causing 3 dead_code warnings. Plan required "zero compiler warnings."
- **Fix:** Added `#[allow(dead_code)]` attribute with comment explaining the functions will be wired in 09-02. Lib build (used by tests) had zero warnings already.
- **Files modified:** `src/crypto/mod.rs`
- **Verification:** `cargo build` produces no warnings
- **Committed in:** `88828ff` (part of GREEN phase commit)

---

**Total deviations:** 1 auto-fixed (Rule 2 - missing critical: warning suppression)
**Impact on plan:** Minimal scope adjustment. The `#[allow(dead_code)]` annotations are temporary and will be removed in 09-02 when the functions are wired into publish/pickup commands.

## Issues Encountered

None — plan executed cleanly with TDD methodology.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `pin_derive_key`, `pin_encrypt`, `pin_decrypt` are fully tested and ready for use
- `pin_salt: Option<String>` field is in both `HandoffRecord` and `HandoffRecordSignable`, signed into the envelope
- 09-02 should wire up: `--pin <PIN>` flag in `publish` CLI, call `pin_encrypt` instead of `age_encrypt`, store salt as base64 in `pin_salt`, prompt for PIN in `pickup` when `pin_salt` is present, call `pin_decrypt`

## Self-Check: PASSED

- FOUND: src/crypto/mod.rs (pin_derive_key, pin_encrypt, pin_decrypt implemented)
- FOUND: src/record/mod.rs (pin_salt field in HandoffRecord and HandoffRecordSignable)
- FOUND: Cargo.toml (argon2, hkdf, sha2, rand dependencies)
- FOUND: 09-01-SUMMARY.md
- FOUND: a609202 (RED phase commit)
- FOUND: 88828ff (GREEN phase commit)
- All 6 test suites pass (40+43 unit tests, 6 integration, 3 plaintext-leak)
- Zero compiler warnings in cargo build

---
*Phase: 09-pin-protected-handoffs*
*Completed: 2026-02-22*
