---
phase: 15-encrypted-key-crypto-layer
plan: 01
subsystem: crypto
tags: [argon2, hkdf, age, zeroize, binary-format, key-derivation]

# Dependency graph
requires:
  - phase: 14-memory-zeroization
    provides: Zeroizing<[u8;32]> return type convention and pin_derive_key pattern
  - phase: 11-pin-kdf
    provides: Argon2id+HKDF-SHA256 crypto stack and pin_derive_key baseline
provides:
  - encrypt_key_envelope(seed: &[u8;32], passphrase: &str) -> anyhow::Result<Vec<u8>>
  - decrypt_key_envelope(envelope: &[u8], passphrase: &str) -> anyhow::Result<Zeroizing<[u8;32]>>
  - key_derive_key private helper with cclink-key-v1 HKDF domain separation
  - CCLINKEK binary envelope format (8-byte magic + version + Argon2 params + salt + age ciphertext)
affects: [16-encrypted-key-storage]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CCLINKEK binary envelope: fixed 53-byte header (magic+version+m_cost+t_cost+p_cost+salt) + variable age ciphertext"
    - "key_derive_key accepts Argon2 params as arguments (not constants) for forward-compatible decrypt"
    - "age decrypt error mapped to user-friendly message via .map_err(|_| anyhow!(...))"
    - "#[allow(dead_code)] on new public functions until wired in by Phase 16"

key-files:
  created: []
  modified:
    - src/crypto/mod.rs

key-decisions:
  - "Argon2 params stored in envelope header (not hardcoded in decrypt) for forward compatibility with future param upgrades"
  - "HKDF info string b\"cclink-key-v1\" distinct from b\"cclink-pin-v1\" — domain separation is a named constant (KEY_HKDF_INFO)"
  - "decrypt_key_envelope returns Zeroizing<[u8;32]> not Vec<u8> — caller (Phase 16) gets automatic zeroing with no extra work"
  - "age decrypt error replaced with \"Wrong passphrase or corrupted key envelope\" — no raw age internals leak to user"
  - "#[allow(dead_code)] used on new items (matching existing store.rs pattern) until Phase 16 wires them into init/load"

patterns-established:
  - "Pattern: binary envelope format with self-describing Argon2 params enables forward-compatible decryption"
  - "Pattern: private key_derive_key with parametric Argon2 args mirrors pin_derive_key but uses different HKDF info"

requirements-completed: [KEYS-05]

# Metrics
duration: 4min
completed: 2026-02-24
---

# Phase 15 Plan 01: CCLINKEK Binary Envelope Encrypt/Decrypt Summary

**CCLINKEK binary envelope crypto layer: Argon2id+HKDF+age encrypt/decrypt for Ed25519 seeds with self-describing header for forward-compatible Argon2 param upgrades**

## Performance

- **Duration:** ~4 min
- **Started:** 2026-02-24T15:24:44Z
- **Completed:** 2026-02-24T15:28:02Z
- **Tasks:** 2 (RED + GREEN)
- **Files modified:** 1

## Accomplishments
- Implemented `encrypt_key_envelope` and `decrypt_key_envelope` in `src/crypto/mod.rs` — the crypto layer Phase 16 will wire into `cclink init` and key loading
- CCLINKEK binary envelope stores Argon2 params in the header (not as constants) enabling forward-compatible decryption when params are upgraded
- `key_derive_key` uses HKDF info `b"cclink-key-v1"` (distinct from `b"cclink-pin-v1"`) — domain separation verified by a dedicated test
- 8 new comprehensive unit tests: round-trip, magic+version header, params-in-header, wrong passphrase, too-short, wrong-magic, HKDF domain separation, determinism — all pass

## Task Commits

Each task was committed atomically:

1. **RED: Add failing envelope tests** - `f77e4c0` (test)
2. **GREEN: Implement key envelope functions** - `8a3db92` (feat)

## Files Created/Modified
- `src/crypto/mod.rs` - Added CCLINKEK constants, `key_derive_key` private helper, `encrypt_key_envelope`, `decrypt_key_envelope`, and 8 unit tests

## Decisions Made
- Used `#[allow(dead_code)]` on new public functions/constants (matching `store.rs` pattern) — Phase 16 will remove these when wired in
- Accepted binary format over JSON (plan spec): no base64 overhead, `file(1)` detection, magic-byte validation
- `decrypt_key_envelope` returns `Zeroizing<[u8;32]>` not `Vec<u8>` — consistent with Phase 14 convention, Phase 16 can pass directly to `pkarr::Keypair::from_secret_key`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `#[allow(dead_code)]` to new items to satisfy `cargo clippy --all-targets -- -D warnings`**
- **Found during:** GREEN task (verification step)
- **Issue:** New public functions and constants trigger clippy "never used" errors since Phase 16 hasn't wired them into the binary yet
- **Fix:** Added `#[allow(dead_code)]` to `ENVELOPE_MAGIC`, `ENVELOPE_VERSION`, `ENVELOPE_HEADER_LEN`, `KEY_HKDF_INFO`, `KDF_M_COST`, `KDF_T_COST`, `KDF_P_COST`, `key_derive_key`, `encrypt_key_envelope`, `decrypt_key_envelope` — matching the existing `store.rs` pattern
- **Files modified:** `src/crypto/mod.rs`
- **Verification:** `cargo clippy --all-targets -- -D warnings` exits 0
- **Committed in:** `8a3db92` (GREEN task commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking clippy failure)
**Impact on plan:** Required to satisfy verification criteria. `#[allow(dead_code)]` will be removed naturally when Phase 16 wires these functions in.

## Issues Encountered
None beyond the clippy dead_code issue documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `encrypt_key_envelope` and `decrypt_key_envelope` are ready for Phase 16 integration into `cclink init` (write encrypted key file) and key loading (decrypt on startup)
- `pkarr::Keypair::from_secret_key(&seed)` accepts `&[u8;32]` — `Zeroizing<[u8;32]>` deref coerces cleanly, no adapter needed
- Phase 16 should remove `#[allow(dead_code)]` attributes as each function gets called from commands

---
*Phase: 15-encrypted-key-crypto-layer*
*Completed: 2026-02-24*
