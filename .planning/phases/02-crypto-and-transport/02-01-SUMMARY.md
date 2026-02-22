---
phase: 02-crypto-and-transport
plan: 01
subsystem: crypto
tags: [age, ed25519, x25519, bech32, key-derivation, encryption, rust]

# Dependency graph
requires:
  - phase: 01-foundation-and-key-management
    provides: pkarr::Keypair with Ed25519 keypair, secret_key() and public_key() APIs

provides:
  - Ed25519-to-X25519 key derivation (ed25519_to_x25519_secret, ed25519_to_x25519_public)
  - age X25519 Identity and Recipient construction from raw bytes (age_identity, age_recipient)
  - age encrypt/decrypt round-trip for arbitrary plaintext (age_encrypt, age_decrypt)
  - All Phase 2 dependencies in Cargo.toml (age, reqwest, postcard, bech32, base64, serde, serde_json, gethostname)

affects:
  - 02-02-record (uses age_encrypt, ed25519_to_x25519_public, age_recipient from this module)
  - 02-03-transport (uses reqwest, postcard, serde dependencies added here)

# Tech tracking
tech-stack:
  added:
    - age 0.11.2 (X25519 age encryption)
    - reqwest 0.13.2 blocking+cookies+rustls (HTTP transport for Phase 2.03)
    - postcard 1.1.3 with alloc (binary serialization for homeserver AuthToken)
    - bech32 0.9.1 (encode X25519 bytes as age Identity/Recipient strings)
    - base64 0.22.1 (Ed25519 signature encoding for record JSON)
    - serde 1.0.228 with derive (HandoffRecord serialization)
    - serde_json 1.0.149 (canonical JSON for signing)
    - gethostname 0.5.0 (hostname metadata field in HandoffRecord)
    - ed25519-dalek 3.0.0-pre.6 explicitly (same version as pkarr transitive dep; required for direct import)
  patterns:
    - Raw [u8; 32] boundary between curve25519-dalek 4 (age) and curve25519-dalek 5 (pkarr) — never pass types between them
    - Bech32-encode X25519 bytes with age HRP strings for injection via parse::<age::x25519::Identity>()
    - age ciphertext stored intact (including age header with ephemeral pubkey) — never strip header

key-files:
  created:
    - src/crypto/mod.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/main.rs

key-decisions:
  - "ed25519-dalek must be listed explicitly in Cargo.toml even though it is a pkarr transitive dep — Rust requires direct Cargo.toml declaration for direct crate imports"
  - "reqwest 0.13 feature name is 'rustls' (not 'rustls-tls' from older versions) — renamed in the 0.13 release"
  - "age ciphertext round-trips via complete blob including header — never strip age-encryption.org header from ciphertext"
  - "curve25519-dalek 4 (age) and 5-pre.6 (pkarr) coexist safely — convert at [u8; 32] boundary only"

patterns-established:
  - "Pattern: Ed25519-to-X25519 via SigningKey::to_scalar_bytes() for secret, VerifyingKey::to_montgomery().to_bytes() for public"
  - "Pattern: age key injection via bech32::encode(HRP, bytes.to_base32(), Variant::Bech32).to_ascii_uppercase().parse::<age::x25519::Identity>()"
  - "Pattern: age_encrypt wraps full ciphertext including age header; age_decrypt takes complete blob"

requirements-completed: [PUB-03]

# Metrics
duration: 3min
completed: 2026-02-22
---

# Phase 2 Plan 01: Crypto Module Summary

**Ed25519-to-X25519 key derivation with age X25519 encrypt/decrypt using bech32-encoded keys injected from pkarr Ed25519 keypair bytes**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-22T00:47:10Z
- **Completed:** 2026-02-22T00:50:24Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- All Phase 2 dependencies added to Cargo.toml in a single step (age, reqwest, postcard, bech32, base64, serde, serde_json, gethostname)
- Crypto module implemented with TDD: 5 tests covering determinism, round-trip, ephemeral ciphertext uniqueness, and wrong-key failure
- Ed25519-to-X25519 key derivation works deterministically via ed25519-dalek's `to_scalar_bytes()` and `to_montgomery()`
- age encrypt/decrypt round-trip verified for arbitrary plaintext; each encryption produces unique ciphertext (ephemeral keys)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add all Phase 2 dependencies to Cargo.toml** - `2f551ca` (chore)
2. **Task 2: Implement crypto module with TDD** - `fa8a4b3` (feat)

_Note: TDD executed within Task 2 commit (RED skeleton → GREEN implementation in one commit after all tests passed)_

## Files Created/Modified

- `src/crypto/mod.rs` - Ed25519-to-X25519 derivation + age encrypt/decrypt with 5 unit tests
- `src/main.rs` - Added `mod crypto;`
- `Cargo.toml` - All Phase 2 dependencies added; ed25519-dalek pinned to =3.0.0-pre.6
- `Cargo.lock` - 216 packages locked including new dependencies

## Decisions Made

- Added ed25519-dalek explicitly at =3.0.0-pre.6 to enable direct `use ed25519_dalek::SigningKey` — pkarr brings it in transitively but Rust requires explicit declaration for direct use. Same version as locked transitive dep, no conflict.
- reqwest 0.13 uses feature name `rustls` not `rustls-tls` (renamed from 0.12 API) — discovered during cargo check and fixed inline.
- age ciphertext stored as complete blob including the age-encryption.org header (contains ephemeral pubkey stanza) — required for decryption; stripping it would cause InvalidHeader error.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] reqwest 0.13 feature rename: rustls-tls -> rustls**
- **Found during:** Task 1 (Add Phase 2 dependencies)
- **Issue:** Plan specified `features = ["blocking", "cookies", "rustls-tls"]` but reqwest 0.13 renamed the feature to just `rustls` (it was `rustls-tls` in 0.12)
- **Fix:** Changed feature to `"rustls"` in Cargo.toml
- **Files modified:** Cargo.toml
- **Verification:** `cargo check` passes without dependency selection error
- **Committed in:** 2f551ca (Task 1 commit)

**2. [Rule 3 - Blocking] ed25519-dalek must be explicitly declared in Cargo.toml**
- **Found during:** Task 2 (Implement crypto module)
- **Issue:** Plan said "Do NOT add ed25519-dalek directly — it's already a transitive dependency" but Rust 2021 edition requires crates to be in Cargo.toml for direct `use` imports; compilation error E0433 otherwise
- **Fix:** Added `ed25519-dalek = "=3.0.0-pre.6"` to Cargo.toml, pinned to the exact version already in the lock file to guarantee no duplicate compile
- **Files modified:** Cargo.toml, Cargo.lock
- **Verification:** `cargo build` and all 5 tests pass
- **Committed in:** fa8a4b3 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 bug: feature name, 1 blocking: missing explicit dep declaration)
**Impact on plan:** Both auto-fixes were necessary for compilation. No scope creep; the ed25519-dalek addition pulls in zero new transitive deps since it was already locked.

## Issues Encountered

- reqwest 0.13 feature naming changed from 0.12 — `rustls-tls` became `rustls`; diagnosed immediately from cargo error message
- ed25519-dalek transitive dep not importable without Cargo.toml declaration — Rust 2021 edition external crate hygiene; pinned to =3.0.0-pre.6 to guarantee single copy

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `src/crypto/mod.rs` exports: `ed25519_to_x25519_secret`, `ed25519_to_x25519_public`, `age_identity`, `age_recipient`, `age_encrypt`, `age_decrypt` — all ready for 02-02 (record module)
- All Phase 2 dependencies are in Cargo.toml and locked — 02-02 and 02-03 can proceed without touching Cargo.toml
- No blockers

---
*Phase: 02-crypto-and-transport*
*Completed: 2026-02-22*

## Self-Check: PASSED

- src/crypto/mod.rs: FOUND
- Cargo.toml: FOUND
- 02-01-SUMMARY.md: FOUND
- Commit 2f551ca (Task 1): FOUND
- Commit fa8a4b3 (Task 2): FOUND
- cargo test -- crypto: 5 passed, 0 failed
