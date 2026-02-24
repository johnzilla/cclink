# Phase 15: Encrypted Key Crypto Layer - Research

**Researched:** 2026-02-24
**Domain:** Rust binary envelope format design, Argon2id key derivation, age encryption, HKDF domain separation
**Confidence:** HIGH

## Summary

Phase 15 adds two new functions to `src/crypto/mod.rs`: `encrypt_key_envelope` and `decrypt_key_envelope`. These functions implement a self-describing binary format (`CCLINKEK`) that wraps an Ed25519 seed (32 bytes) in a passphrase-encrypted envelope using Argon2id + HKDF-SHA256 + age encryption — the same crypto stack already in use for PIN-protected handoffs in `pin_encrypt` / `pin_decrypt`.

The binary envelope format stores the magic header, version byte, Argon2 parameters (m_cost, t_cost, p_cost as 4-byte big-endian u32s), a 32-byte salt, and the age ciphertext in a single flat binary blob. The Argon2 parameters are embedded in the envelope header so that future parameter upgrades do not break decryption of existing files — decryption reads them from the file, not from constants. The HKDF info string `"cclink-key-v1"` is distinct from the existing PIN-derivation info string `"cclink-pin-v1"`, providing domain separation between the two uses.

No new crate dependencies are required. All required crates — `argon2 0.5.3`, `hkdf 0.12.4`, `sha2 0.10.9`, `age 0.11.2`, `rand 0.8.5`, and `zeroize 1.8.2` — are already direct or transitive dependencies in `Cargo.toml`. The `pkarr::Keypair::from_secret_key` and `keypair.secret_key()` APIs are confirmed to take and return `[u8; 32]` (`ed25519_dalek::SecretKey = [u8; 32]`), so the boundary between Phase 15's crypto layer and the Phase 16 storage integration is clean.

**Primary recommendation:** Implement `encrypt_key_envelope` and `decrypt_key_envelope` in `src/crypto/mod.rs`, following the exact same Argon2id + HKDF + age pattern as `pin_encrypt` / `pin_decrypt`, with a flat binary envelope format that encodes the magic header, version, Argon2 params, salt, and age ciphertext.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| KEYS-05 | Encrypted key file uses self-describing format (JSON envelope with version, salt, ciphertext) | Note: REQUIREMENTS.md says "JSON envelope" but the phase success criteria and architecture decision in STATE.md call for a binary format with the `CCLINKEK` magic header. Research supports binary as more correct for this domain (no JSON base64 overhead, magic header enables instant format detection). Planner should adopt the binary format as specified by the phase success criteria. The functions `encrypt_key_envelope` / `decrypt_key_envelope` implement this requirement. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| argon2 | 0.5.3 (direct dep) | Argon2id password-to-key derivation | Already used for PIN derivation; same parameters (m=65536, t=3, p=1) established in Phase 11 |
| hkdf | 0.12.4 (direct dep) | HKDF-SHA256 key expansion with domain-separation info string | Already used in `pin_derive_key`; `"cclink-key-v1"` info string provides domain separation from `"cclink-pin-v1"` |
| sha2 | 0.10.9 (direct dep) | SHA256 hash function for HKDF | Already in use |
| age | 0.11.2 (direct dep) | X25519 encryption of the 32-byte seed | Already used for session payload encryption; `age_encrypt` / `age_decrypt` / `age_identity` / `age_recipient` functions already exist in `src/crypto/mod.rs` |
| zeroize | 1.8.2 (direct dep) | `Zeroizing<T>` for all intermediate key material | Already used throughout Phase 14 changes; wrap all intermediate buffers |
| rand | 0.8.5 (transitive) | Generate random 32-byte salt | Already used in `pin_encrypt` via `rand::thread_rng().gen()` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| pkarr | 5.0.3 (direct dep) | `Keypair::from_secret_key(&[u8;32])` and `keypair.secret_key() -> [u8;32]` | Phase 16 integration — Phase 15 only needs the raw `[u8; 32]` boundary |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Binary flat envelope | JSON envelope (as REQUIREMENTS.md says) | Binary is simpler (no base64 encoding overhead, no JSON parser), magic header enables `file(1)` detection, no ambiguity in parsing. Phase success criteria explicitly requires binary with `CCLINKEK` magic. |
| age for seed encryption | AES-256-GCM directly | age is already in use, handles key material securely with ephemeral key wrapping. No reason to add a raw AEAD implementation. |
| HKDF info `"cclink-key-v1"` | Any other string | REQUIREMENTS.md success criteria 5 explicitly requires this exact string. |

**Installation:**
```bash
# No new dependencies required. All crates already in Cargo.toml.
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── crypto/mod.rs        # Add encrypt_key_envelope, decrypt_key_envelope here
├── keys/store.rs        # Phase 16 will call these functions; Phase 15 is crypto only
└── (no new files)
```

Phase 15 is crypto-only: two new functions in `src/crypto/mod.rs` with unit tests in the same file. No storage integration, no command changes. That is Phase 16.

### Pattern 1: Binary Envelope Layout

**What:** The `CCLINKEK` binary format is a fixed-layout header followed by variable-length age ciphertext.

**Envelope layout:**
```
Offset  Size  Field
0       8     Magic bytes: b"CCLINKEK"
8       1     Version byte: 0x01
9       4     m_cost (Argon2 memory, u32 big-endian)
13      4     t_cost (Argon2 iterations, u32 big-endian)
17      4     p_cost (Argon2 parallelism, u32 big-endian)
21      32    Salt (random bytes)
53      N     Age ciphertext (variable length, rest of blob)
```

Total header: 53 bytes. Total envelope: 53 + len(age ciphertext).

**Why big-endian for u32 params?** Big-endian is the standard network byte order for binary protocol headers; consistent with the age format itself.

**Why age ciphertext as remainder?** Age ciphertext is variable-length (depends on ephemeral key + overhead). Reading "everything after offset 53" is simpler and more robust than storing a length field.

**Example:**
```rust
// Source: codebase pattern (pin_encrypt / pin_decrypt in src/crypto/mod.rs)
const MAGIC: &[u8; 8] = b"CCLINKEK";
const VERSION: u8 = 0x01;
const HEADER_LEN: usize = 8 + 1 + 4 + 4 + 4 + 32; // = 53 bytes

pub fn encrypt_key_envelope(seed: &[u8; 32], passphrase: &str) -> anyhow::Result<Vec<u8>> {
    let salt: [u8; 32] = rand::thread_rng().gen();
    let m_cost: u32 = 65536;
    let t_cost: u32 = 3;
    let p_cost: u32 = 1;

    // Derive the X25519 key-encryption key from passphrase + salt
    let kek = key_derive_key(passphrase, &salt, m_cost, t_cost, p_cost)?;

    // Build age Identity from the derived key and get the Recipient
    let identity = age_identity(&kek);
    let recipient = identity.to_public();

    // Encrypt the 32-byte seed with age
    let ciphertext = age_encrypt(seed, &recipient)?;

    // Serialize the envelope
    let mut envelope = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    envelope.extend_from_slice(MAGIC);
    envelope.push(VERSION);
    envelope.extend_from_slice(&m_cost.to_be_bytes());
    envelope.extend_from_slice(&t_cost.to_be_bytes());
    envelope.extend_from_slice(&p_cost.to_be_bytes());
    envelope.extend_from_slice(&salt);
    envelope.extend_from_slice(&ciphertext);
    Ok(envelope)
}
```

### Pattern 2: Key Derivation Function — `key_derive_key`

**What:** Extract the Argon2id + HKDF-SHA256 derivation into a private helper function `key_derive_key` that accepts the passphrase, salt, and the three Argon2 parameters read from the envelope header. This enables `decrypt_key_envelope` to pass the parameters decoded from the header rather than hardcoded constants.

**When to use:** Always — this is the pattern that satisfies success criterion 4 ("Argon2 parameters are read from the file header on decryption, not from hardcoded constants").

**Example:**
```rust
// Private helper — mirrors pin_derive_key but uses "cclink-key-v1" info string
// and accepts Argon2 params as arguments (for decryption path)
fn key_derive_key(
    passphrase: &str,
    salt: &[u8; 32],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 params error: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut argon2_output = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, argon2_output.as_mut())
        .map_err(|e| anyhow::anyhow!("argon2 hash error: {}", e))?;

    let hkdf = Hkdf::<Sha256>::new(None, &*argon2_output);
    let mut okm = Zeroizing::new([0u8; 32]);
    // CRITICAL: "cclink-key-v1" — distinct from "cclink-pin-v1" (domain separation)
    hkdf.expand(b"cclink-key-v1", okm.as_mut())
        .map_err(|e| anyhow::anyhow!("hkdf expand error: {}", e))?;

    Ok(okm)
}
```

### Pattern 3: Decryption with Header Parsing

**What:** Parse the fixed header, extract Argon2 parameters from the header bytes (not from constants), re-derive the key, decrypt.

**Example:**
```rust
pub fn decrypt_key_envelope(envelope: &[u8], passphrase: &str) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    // Validate minimum length
    if envelope.len() < HEADER_LEN {
        anyhow::bail!("Invalid key envelope: too short ({} bytes)", envelope.len());
    }

    // Validate magic
    if &envelope[..8] != MAGIC {
        anyhow::bail!("Invalid key envelope: wrong magic bytes");
    }

    // Validate version
    if envelope[8] != VERSION {
        anyhow::bail!("Unsupported key envelope version: {}", envelope[8]);
    }

    // Decode Argon2 params from header (not from constants)
    let m_cost = u32::from_be_bytes(envelope[9..13].try_into().unwrap());
    let t_cost = u32::from_be_bytes(envelope[13..17].try_into().unwrap());
    let p_cost = u32::from_be_bytes(envelope[17..21].try_into().unwrap());

    // Extract 32-byte salt
    let salt: [u8; 32] = envelope[21..53].try_into().unwrap();

    // age ciphertext is the remainder
    let ciphertext = &envelope[53..];

    // Re-derive key-encryption key from passphrase + header params
    let kek = key_derive_key(passphrase, &salt, m_cost, t_cost, p_cost)?;
    let identity = age_identity(&kek);

    // Decrypt the seed
    let plaintext = age_decrypt(ciphertext, &identity)
        .map_err(|_| anyhow::anyhow!("Wrong passphrase or corrupted key envelope"))?;

    // Validate recovered seed is exactly 32 bytes
    if plaintext.len() != 32 {
        anyhow::bail!("Decrypted key envelope has wrong size: {} bytes", plaintext.len());
    }

    let mut seed = Zeroizing::new([0u8; 32]);
    seed.copy_from_slice(&plaintext);
    Ok(seed)
}
```

### Pattern 4: Wrong-Passphrase Error Handling

**What:** age decryption failure must return a clear error string (not a panic, not a corrupt-data error). Map the age error explicitly before returning.

**Key insight:** `age_decrypt` in `src/crypto/mod.rs` propagates the raw age error string. For the key envelope, the caller should never see a raw `age decrypt error: ...` message — it should see `"Wrong passphrase or corrupted key envelope"`. The `.map_err` in Pattern 3 above achieves this.

**Why:** Success criterion 3 says "wrong passphrase returns a clear error (not a panic or corrupt-data error)". The age library returns an error on AEAD authentication failure — it does not panic. But the error message is implementation-specific and not user-friendly.

### Anti-Patterns to Avoid

- **Reading Argon2 params from constants in decrypt:** The parameters must come from the envelope header. Hardcoding them in decrypt would break forward compatibility when params are upgraded.
- **Using `"cclink-pin-v1"` as the HKDF info string:** Domain separation is required. Key derivation and PIN derivation must use different info strings. Success criterion 5 explicitly names `"cclink-key-v1"`.
- **Leaking raw age error message on wrong passphrase:** Wrap the age decrypt error with a user-friendly message before returning to the caller.
- **Not wrapping `plaintext` Vec in a Zeroizing buffer:** The `age_decrypt` return is a plain `Vec<u8>`. Copy the seed bytes into a `Zeroizing<[u8;32]>` before dropping the Vec.
- **Storing Argon2 params as little-endian:** Use big-endian for all multi-byte integers in binary protocols. Be consistent.
- **Adding new crate dependencies:** Everything needed is already in `Cargo.toml`. Do not add new deps.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Password-to-key derivation | Custom KDF | `argon2 0.5.3` with same params as `pin_derive_key` | Already in use; battle-tested; parameters already validated for the project |
| Symmetric encryption of the seed | AES-GCM manually | `age_encrypt` / `age_decrypt` from `src/crypto/mod.rs` | Already in codebase; handles ephemeral key material, AEAD authentication, and wrong-key detection |
| Binary serialization | `serde` + `bincode` | Manual `Vec::extend_from_slice` | The envelope is 53 bytes of fixed header + variable ciphertext; no serde needed for a flat binary format this simple |
| Hex encoding of seed in envelope | Storing seed as hex | Store raw 32 bytes | The seed is binary — no reason to hex-encode it inside an encrypted envelope |
| Zeroization of intermediate keys | Manual `ptr::write_volatile` | `Zeroizing<[u8;32]>` | Phase 14 established this as the project standard |

**Key insight:** Phase 15 is a thin composition layer over primitives that already exist in the codebase. The crypto primitives (`pin_derive_key`, `age_encrypt`, `age_decrypt`, `age_identity`, `age_recipient`) are all already present in `src/crypto/mod.rs`. Phase 15 only needs to define the envelope binary format, write two public functions, and add comprehensive tests.

## Common Pitfalls

### Pitfall 1: HKDF Info String Collision
**What goes wrong:** Using `"cclink-pin-v1"` instead of `"cclink-key-v1"` in `key_derive_key`. This means a passphrase-encrypted key file and a PIN-derived session encryption key derived from the same passphrase produce the same key.
**Why it happens:** Copy-paste from `pin_derive_key` without changing the info string.
**How to avoid:** The info string `b"cclink-key-v1"` must be a named constant to make it visible and auditable. Success criterion 5 explicitly requires it.
**Warning signs:** A test that verifies `key_derive_key("pass", &salt, ...)` and `pin_derive_key("pass", &salt)` produce *different* keys will catch this immediately.

### Pitfall 2: Argon2 Params Hardcoded in Decrypt
**What goes wrong:** `decrypt_key_envelope` calls `Params::new(65536, 3, 1, Some(32))` instead of reading `m_cost`, `t_cost`, `p_cost` from the envelope header. Old encrypted files with different params cannot be decrypted after a parameter upgrade.
**Why it happens:** It's the natural way to write it; the params "feel" constant.
**How to avoid:** `key_derive_key` signature accepts `m_cost`, `t_cost`, `p_cost` as arguments. `decrypt_key_envelope` must decode them from `envelope[9..21]` and pass them in.
**Warning signs:** Success criterion 4 calls this out explicitly. A test that encrypts with non-default params and decrypts successfully (but fails with the default params) validates this.

### Pitfall 3: Wrong-Passphrase Error Leaks age Internals
**What goes wrong:** The raw age error propagates: `"age decrypt error: Failed to decrypt the file's data encryption key. Did you use the right key?"`. While informative, it is implementation-specific and could confuse users who don't know what "file's data encryption key" means.
**Why it happens:** `age_decrypt` returns an `anyhow::Error`; callers that use `?` propagate it unchanged.
**How to avoid:** `.map_err(|_| anyhow::anyhow!("Wrong passphrase or corrupted key envelope"))` in `decrypt_key_envelope` before the `?`.
**Warning signs:** A test that checks the error string on wrong passphrase will catch raw age error leakage.

### Pitfall 4: Returning `Vec<u8>` Instead of `Zeroizing<[u8;32]>`
**What goes wrong:** `decrypt_key_envelope` returns `anyhow::Result<Vec<u8>>`. The caller in Phase 16 must then copy into a `Zeroizing<[u8;32]>` manually — and might forget.
**Why it happens:** `age_decrypt` returns `Vec<u8>`; easiest to propagate the same type.
**How to avoid:** `decrypt_key_envelope` returns `anyhow::Result<Zeroizing<[u8;32]>>`. This is the canonical return type for 32-byte secret seeds throughout the codebase (established in Phase 14 for `pin_derive_key`).
**Warning signs:** Phase 16 code calling `pkarr::Keypair::from_secret_key(&seed)` where `seed` is a plain `Vec<u8>` is a sign the return type was wrong.

### Pitfall 5: Truncated Parsing with `try_into().unwrap()`
**What goes wrong:** `envelope[9..13].try_into().unwrap()` panics if the envelope is too short, bypassing the length check at the top of `decrypt_key_envelope`.
**Why it happens:** The length check at the top guards against this, but it's easy to get the minimum length constant wrong (off-by-one, wrong arithmetic).
**How to avoid:** Define `HEADER_LEN` as a named constant: `8 + 1 + 4 + 4 + 4 + 32 = 53`. Verify `envelope.len() >= HEADER_LEN` at the top. Use `unwrap()` only after the length check — document why it's safe.
**Warning signs:** A test that passes a 52-byte (or shorter) buffer should confirm the error path, not a panic.

### Pitfall 6: age Ciphertext Size Not Validated
**What goes wrong:** `envelope` is 53+ bytes but the age ciphertext portion is zero bytes. `age_decrypt` on empty input returns an error that looks like a wrong-passphrase error rather than a "corrupted envelope" error.
**Why it happens:** The length check only validates minimum header length, not minimum ciphertext length.
**How to avoid:** After the header length check, additionally assert `ciphertext.len() > 0` (or use a more specific minimum for age overhead). Alternatively, let the age error stand — both cases produce a clear error, not a panic. This is a minor robustness concern, not a security concern.

## Code Examples

Verified patterns from official sources:

### `key_derive_key` helper — mirrors `pin_derive_key` with different info string and parametric Argon2
```rust
// Source: src/crypto/mod.rs — pin_derive_key (Phase 14), adapted for key envelope
use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

/// Info string for HKDF domain separation (distinct from "cclink-pin-v1")
const KEY_HKDF_INFO: &[u8] = b"cclink-key-v1";

fn key_derive_key(
    passphrase: &str,
    salt: &[u8; 32],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 params error: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut argon2_output = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, argon2_output.as_mut())
        .map_err(|e| anyhow::anyhow!("argon2 hash error: {}", e))?;

    let hkdf = Hkdf::<Sha256>::new(None, &*argon2_output);
    let mut okm = Zeroizing::new([0u8; 32]);
    hkdf.expand(KEY_HKDF_INFO, okm.as_mut())
        .map_err(|e| anyhow::anyhow!("hkdf expand error: {}", e))?;

    Ok(okm)
}
```

### Binary envelope constants and magic
```rust
// Source: codebase pattern + phase success criteria
const ENVELOPE_MAGIC: &[u8; 8] = b"CCLINKEK";
const ENVELOPE_VERSION: u8 = 0x01;
// Fixed header: 8 magic + 1 version + 4 m_cost + 4 t_cost + 4 p_cost + 32 salt = 53 bytes
const ENVELOPE_HEADER_LEN: usize = 53;

// Default Argon2 parameters — same as pin_derive_key (m=64MB, t=3, p=1)
const KDF_M_COST: u32 = 65536;
const KDF_T_COST: u32 = 3;
const KDF_P_COST: u32 = 1;
```

### Argon2 `Params::new` signature (verified from argon2-0.5.3 source)
```rust
// Source: ~/.cargo/registry/src/.../argon2-0.5.3/src/params.rs
// pub const fn new(m_cost: u32, t_cost: u32, p_cost: u32, output_len: Option<usize>) -> Result<Self>
// m_cost: memory in KiB (65536 = 64 MB)
// t_cost: iterations (3)
// p_cost: parallelism (1)
// output_len: Some(32) for 32-byte output
let params = Params::new(65536, 3, 1, Some(32)).unwrap();
// params.m_cost() -> 65536
// params.t_cost() -> 3
// params.p_cost() -> 1
```

### `pkarr::Keypair::from_secret_key` signature (verified from pkarr-5.0.3 source)
```rust
// Source: ~/.cargo/registry/src/.../pkarr-5.0.3/src/keys.rs
// pub fn from_secret_key(secret_key: &SecretKey) -> Keypair
// where SecretKey = ed25519_dalek::SecretKey = [u8; 32]
// keypair.secret_key() -> [u8; 32]
let seed: Zeroizing<[u8; 32]> = decrypt_key_envelope(&blob, "passphrase")?;
let keypair = pkarr::Keypair::from_secret_key(&seed);
```

### Round-trip test skeleton
```rust
#[test]
fn test_key_envelope_round_trip() {
    let seed = [42u8; 32];
    let passphrase = "correct-horse-battery-staple";
    let blob = encrypt_key_envelope(&seed, passphrase)
        .expect("encrypt_key_envelope should succeed");

    // Validate magic header
    assert_eq!(&blob[..8], b"CCLINKEK");
    assert_eq!(blob[8], 0x01);
    assert!(blob.len() > 53, "envelope must be longer than header");

    let recovered = decrypt_key_envelope(&blob, passphrase)
        .expect("decrypt_key_envelope should round-trip");
    assert_eq!(*recovered, seed);
}

#[test]
fn test_key_envelope_wrong_passphrase() {
    let seed = [42u8; 32];
    let blob = encrypt_key_envelope(&seed, "correct-passphrase")
        .expect("encrypt should succeed");
    let result = decrypt_key_envelope(&blob, "wrong-passphrase");
    assert!(result.is_err(), "wrong passphrase must return Err");
    // Must not panic, and error must mention passphrase or envelope
    let msg = result.unwrap_err().to_string().to_lowercase();
    assert!(
        msg.contains("passphrase") || msg.contains("envelope"),
        "error should mention passphrase or envelope, got: {}",
        msg
    );
}

#[test]
fn test_key_envelope_params_stored_in_header() {
    // Verify that Argon2 params are recoverable from the binary header
    let blob = encrypt_key_envelope(&[1u8; 32], "test")
        .expect("encrypt should succeed");
    let m_cost = u32::from_be_bytes(blob[9..13].try_into().unwrap());
    let t_cost = u32::from_be_bytes(blob[13..17].try_into().unwrap());
    let p_cost = u32::from_be_bytes(blob[17..21].try_into().unwrap());
    assert_eq!(m_cost, 65536);
    assert_eq!(t_cost, 3);
    assert_eq!(p_cost, 1);
}

#[test]
fn test_key_hkdf_info_distinct_from_pin() {
    // "cclink-key-v1" and "cclink-pin-v1" must produce different outputs
    // for the same passphrase and salt
    let salt = [7u8; 32];
    let key_kek = key_derive_key("same-passphrase", &salt, 65536, 3, 1)
        .expect("key derivation should succeed");
    // pin_derive_key uses the same Argon2 params but "cclink-pin-v1"
    let pin_key = pin_derive_key("same-passphrase", &salt)
        .expect("pin derivation should succeed");
    assert_ne!(*key_kek, *pin_key, "key and PIN derivation must produce different keys");
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Plaintext hex seed at `~/.pubky/secret_key` | Binary `CCLINKEK` envelope (Phase 15+16) | v1.3 | Secret key encrypted at rest; passphrase required to load |
| Fixed Argon2 params hardcoded in constants | Params embedded in envelope header | Phase 15 design | Future param upgrades do not break existing encrypted files |
| `pin_derive_key` with `"cclink-pin-v1"` | `key_derive_key` with `"cclink-key-v1"` | Phase 15 | Domain separation between PIN and key encryption |

**Deprecated/outdated:**
- `pkarr::Keypair::write_secret_key_file` / `from_secret_key_file`: These write/read a plain hex seed. Phase 16 will bypass them entirely for the encrypted format (established in Phase 14 via `load_keypair` reimplementation). Phase 15 does not need to touch these.

## Open Questions

1. **Should `encrypt_key_envelope` and `decrypt_key_envelope` be public or pub(crate)?**
   - What we know: Phase 16 (`store.rs`) calls them from within the same crate. No external consumer needs them. `src/crypto/mod.rs` functions used by commands are public; functions used only within the crate can be `pub(crate)`.
   - What's unclear: Whether the test harness in the same file requires `pub` vs `pub(crate)` — tests in the same module can call private functions.
   - Recommendation: Make them `pub` for consistency with the rest of `src/crypto/mod.rs` (all functions there are `pub`). `pub(crate)` is an acceptable alternative that the planner may choose.

2. **Should `key_derive_key` be exposed as a public function?**
   - What we know: It is a private helper used only by `encrypt_key_envelope` and `decrypt_key_envelope`. Exposing it would allow callers to derive the key without building an envelope, which has no current use case.
   - Recommendation: Make it a private `fn key_derive_key(...)`. Phase 15 tests can call it directly since they are in the same module (`mod tests { use super::*; }`).

3. **Should `encrypt_key_envelope` accept `seed: &[u8; 32]` or `seed: &Zeroizing<[u8;32]>`?**
   - What we know: `pkarr::Keypair::secret_key()` returns `[u8; 32]` (a plain array, not `Zeroizing`). Phase 16 will call `keypair.secret_key()` and pass the result. Accepting `&[u8; 32]` is the natural interface.
   - Recommendation: Accept `&[u8; 32]`. The caller (Phase 16) is responsible for wrapping `secret_key()` output in `Zeroizing` before passing if desired; Phase 15's job is just the envelope format.

4. **What is the expected age ciphertext size for a 32-byte input?**
   - What we know: age format overhead is the age header (ASCII lines including the stanza) + payload MAC. For X25519 recipients with 32-byte plaintext, the total is typically 200-250 bytes (empirical; age's binary format includes version line, header, MAC, body). The total envelope will be approximately 250-310 bytes.
   - Recommendation: No hardcoded size check needed. The round-trip test verifies the correct size implicitly.

## Sources

### Primary (HIGH confidence)
- `~/.cargo/registry/src/.../argon2-0.5.3/src/params.rs` — `Params::new(m_cost, t_cost, p_cost, output_len)`, `m_cost()`, `t_cost()`, `p_cost()` accessors verified directly from source
- `~/.cargo/registry/src/.../pkarr-5.0.3/src/keys.rs` — `from_secret_key(&SecretKey)` and `secret_key() -> SecretKey` verified; `SecretKey = ed25519_dalek::SecretKey = [u8; 32]` verified from `ed25519-dalek-3.0.0-pre.5/src/signing.rs`
- `~/.cargo/registry/src/.../age-0.11.2/src/x25519.rs` — `age_encrypt` / `age_decrypt` pattern verified; confirms AEAD authentication failure returns `Err` not panic
- `~/.cargo/registry/src/.../hkdf-0.12.4/Cargo.toml` — version 0.12.4 confirmed in lock file
- `/home/john/vault/projects/github.com/cclink/src/crypto/mod.rs` — Full source read; `pin_derive_key`, `pin_encrypt`, `pin_decrypt`, `age_encrypt`, `age_decrypt`, `age_identity`, `age_recipient` all verified; HKDF info `b"cclink-pin-v1"` confirmed
- `/home/john/vault/projects/github.com/cclink/Cargo.toml` — All direct deps verified: argon2 0.5, hkdf 0.12, sha2 0.10, age 0.11, zeroize 1, rand 0.8
- `cargo tree` output — argon2 v0.5.3, hkdf v0.12.4, sha2 v0.10.9, age v0.11.2, zeroize v1.8.2, rand v0.8.5 all confirmed in lock
- `.planning/STATE.md` — Decision "Bypass pkarr I/O for encrypted format" and "Crypto layer before storage layer" confirmed; blocker note about `from_secret_key` API resolved (confirmed `&[u8;32]`)

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` — KEYS-05 text says "JSON envelope" but phase success criteria specifies binary `CCLINKEK` format; binary is technically correct for this use case and is what the phase description specifies
- `.planning/phases/14-memory-zeroization/14-RESEARCH.md` — Confirmed `Zeroizing<[u8;32]>` return type convention and pattern established for Phase 14

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crate versions read directly from cargo registry and cargo tree; no version ambiguity
- Architecture: HIGH — envelope format derived directly from phase success criteria; all referenced APIs verified from source
- Pitfalls: HIGH — most pitfalls derived from direct code inspection of existing patterns and phase success criteria requirements; not speculation

**Research date:** 2026-02-24
**Valid until:** 2026-06-01 (argon2 0.5.x, hkdf 0.12.x, age 0.11.x, pkarr 5.0.3 all pinned; unlikely to change)
