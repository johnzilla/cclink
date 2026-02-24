# Stack Research

**Domain:** Rust CLI — Encrypted key storage at rest and secure memory zeroization (cclink v1.3)
**Researched:** 2026-02-24
**Confidence:** HIGH — all versions verified live via crates.io API and Cargo.lock inspection; pkarr 5.0.3 source read directly from registry cache

---

## Context: What This Research Covers

This is a subsequent milestone research pass. The existing stack (pkarr, age, clap, argon2, hkdf, sha2, rand, backon, etc.) is validated and unchanged. This file covers only what is new or changed for v1.3:

1. `zeroize` — secure memory erasure of key material after use
2. `secrecy` — typed wrapper for secret values with automatic zeroization (optional but worth assessing)
3. `keyring` — system keystore integration (macOS Keychain / Freedesktop Secret Service)
4. Encrypted key-at-rest format — whether new crates are needed or existing deps suffice

---

## Key Finding: Two of Three New Capabilities Require Zero New Crates

The project's existing Cargo.lock (as of v1.2) already contains:

- `zeroize 1.8.2` — pulled in transitively by `ed25519-dalek` (optional feature) and `keyring` on Windows
- `zeroize_derive 1.4.3` — pulled in by `zeroize 1.8.2`
- `secrecy 0.10.3` — pulled in transitively by `age-core`

Adding `zeroize` and `secrecy` as direct dependencies promotes already-present transitive crates to explicit ones. No new compilation units, no new network fetches, no new security surface.

For encrypted key storage at rest: `argon2`, `hkdf`, `sha2`, `age`, and `rand` are all already direct dependencies. The encryption layer (Argon2id+HKDF+age) is identical to the existing PIN-protected handoff feature. No new crates are needed.

Only `keyring` (system keystore integration) would introduce genuinely new crates.

---

## Recommended Stack

### New Direct Dependencies (v1.3 additions to Cargo.toml)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `zeroize` | `1.8.2` | Erase secret key bytes from memory after use | Already in Cargo.lock (transitive). The `derive` feature provides `#[derive(Zeroize, ZeroizeOnDrop)]` for structs holding key material. `ZeroizeOnDrop` ensures erasure even on panic or early return — critical for `[u8; 32]` seed bytes. The RustSec project recommends this crate specifically for crypto key material. |
| `keyring` | `3.6.3` | System keystore integration (macOS Keychain, Freedesktop Secret Service) | Only stable version with cross-platform support. v3.6.3 released 2025-07-27. v4.x is still pre-release (rc.3 as of 2026-02-01). The `sync-secret-service + crypto-rust` feature set works without async runtime (cclink is fully sync). |

### Capabilities Requiring No New Crates

| Capability | Implemented Using | Notes |
|------------|-------------------|-------|
| Encrypted key at rest (passphrase-based) | `argon2 0.5` + `hkdf 0.12` + `sha2 0.10` + `age 0.11` + `rand 0.8` | Identical to existing `pin_derive_key` + `pin_encrypt`/`pin_decrypt` in `src/crypto/mod.rs`. Reuse directly. |
| Passphrase prompting | `dialoguer 0.12` | Already used for `--pin` interactive prompt. |
| Zeroization of `Vec<u8>` / `[u8; N]` | `zeroize 1.8.2` (see above) | `Zeroize` trait's `.zeroize()` method. Also use `zeroize::Zeroizing<Vec<u8>>` as a drop-wrapper without the derive. |

---

## Cargo.toml Changes

```toml
# Add to [dependencies]
zeroize = { version = "1.8", features = ["derive"] }
keyring = { version = "3.6", features = ["apple-native", "sync-secret-service", "crypto-rust"] }
```

No version conflicts. `zeroize 1.8.2` is already resolved in Cargo.lock. Adding it as a direct dependency with `features = ["derive"]` activates `zeroize_derive` (also already in the lock). Cargo will not re-download or recompile anything already present.

**`secrecy` assessment — NOT recommended as direct dependency:**
`secrecy 0.10.3` is already in the lock (via age-core) but adds no capability that `zeroize::Zeroizing<T>` doesn't provide. `Zeroizing<[u8; 32]>` wraps a value and zeroizes on drop. `SecretBox<T>` is heavier and designed for multi-owner scenarios (Arc-based). For cclink's single-owner key material use `Zeroizing<T>` from zeroize directly — simpler and sufficient.

---

## zeroize Integration Pattern

The three values that must be zeroized after use:

| Value | Type | Where Created | Zeroize How |
|-------|------|---------------|-------------|
| Ed25519 seed bytes | `[u8; 32]` from `keypair.secret_key()` | `crypto/mod.rs: ed25519_to_x25519_secret()` | Wrap call site in `Zeroizing<[u8; 32]>` |
| Argon2id output | `[u8; 32]` in `pin_derive_key()` | `crypto/mod.rs` | `Zeroizing::new([0u8; 32])` as output buffer |
| HKDF output (`okm`) | `[u8; 32]` in `pin_derive_key()` | `crypto/mod.rs` | Same; `Zeroizing<[u8; 32]>` |
| Passphrase string | `String` from `dialoguer::Password::interact()` | `commands/publish.rs`, new `commands/init.rs` prompt | `zeroize::Zeroizing<String>` or `.zeroize()` before drop |

Pattern (no derive needed for `[u8; 32]`):
```rust
use zeroize::Zeroizing;

let secret = Zeroizing::new(keypair.secret_key());  // zeroized when `secret` drops
```

Pattern with derive (for a custom struct):
```rust
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Zeroize, ZeroizeOnDrop)]
struct KeyMaterial {
    seed: [u8; 32],
    derived: [u8; 32],
}
```

The derive approach is only needed if a struct holds multiple sensitive fields. For cclink's current code, `Zeroizing<T>` at the call site is sufficient and requires no struct changes.

---

## Encrypted Key Storage Format

No new crates needed. The file format uses existing primitives:

```
~/.pubky/secret_key  (current plaintext format)
  64-char hex string (raw Ed25519 seed)

~/.pubky/secret_key  (encrypted format — replaces plaintext)
  Line 1: "cclink-encrypted-v1"          (magic header; non-hex, so pkarr rejects it cleanly)
  Line 2: <base64-encoded 32-byte salt>  (use base64 = 0.22, already in Cargo.toml)
  Line 3+: <base64-encoded age ciphertext of 32-byte seed>
```

Detection in `load_keypair()`: if the file starts with `"cclink-encrypted-v1"`, prompt for passphrase and decrypt via `pin_derive_key` + `age_decrypt`. Otherwise treat as legacy plaintext hex (backward compatible).

This mirrors the PIN-protected handoff flow exactly. `pin_derive_key` and `pin_decrypt` from `src/crypto/mod.rs` are reused without modification.

---

## keyring Integration Pattern

```toml
# Feature flags needed per platform:
#   apple-native    → macOS Keychain (uses security-framework)
#   sync-secret-service + crypto-rust → Linux GNOME Keyring / KWallet via D-Bus (sync, no tokio)
#   windows-native  → Windows Credential Manager
# All three features are mutually compatible in 3.6.3 and Cargo handles platform-conditional deps.
keyring = { version = "3.6", features = ["apple-native", "sync-secret-service", "crypto-rust"] }
```

Usage:
```rust
let entry = keyring::Entry::new("cclink", &username)?;
entry.set_password(&hex_secret_key)?;     // store
let hex = entry.get_password()?;           // retrieve
entry.delete_credential()?;               // remove
```

The keyring crate stores a string value. Store the 64-char hex seed string (same format pkarr's `write_secret_key_file` uses). On retrieval, validate the hex before constructing the keypair.

**Platform reality check:** On Linux, `sync-secret-service` requires D-Bus and a running secret service daemon (GNOME Keyring or KWallet). Headless Linux (CI, servers) will have no secret service. `keyring` returns `NoStorageAccess` in this case. The cclink code must fall back to the encrypted-file format when keyring fails. This is the correct behavior — not an error.

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `zeroize 1.8` direct dep | Rely on ed25519-dalek's optional zeroize (not enabled by cclink) | ed25519-dalek's `zeroize` feature only zeroizes inside dalek's types (SigningKey). It does not cover cclink's local `[u8; 32]` copies of seed bytes, argon2 output, or passphrase strings. Must zeroize these explicitly. |
| `zeroize::Zeroizing<T>` wrapper | `secrecy::SecretBox<T>` | `SecretBox` is Arc-based and designed for shared ownership of secrets. cclink's key material is owned by a single call frame. `Zeroizing<T>` is simpler and sufficient. `secrecy` adds no capability here. |
| `keyring 3.6.3` (stable) | `keyring 4.0.0-rc.3` | rc.3 released 2026-02-01; still pre-release. The 4.x API restructures backend selection. Use stable 3.6.3 now; upgrade to 4.x when it stabilizes. |
| `sync-secret-service + crypto-rust` | `async-secret-service + tokio` | cclink is entirely synchronous. Adding tokio for keyring alone would bloat the binary and runtime. Sync D-Bus access is correct. |
| Argon2id+HKDF+age (existing deps) for key-at-rest | `age::scrypt` passphrase recipient | age has built-in scrypt passphrase encryption. However, cclink already has Argon2id+HKDF implemented and tested for PIN handoffs. Reusing that path keeps the codebase consistent and avoids a new crypto primitive. |
| Custom `"cclink-encrypted-v1"` file format | JSON envelope | JSON adds serde complexity for a two-field structure (salt + ciphertext). A line-based format with a magic header is simpler and easier to inspect in a hex editor. |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `memzero` / `clear_on_drop` | Unmaintained; zeroize supersedes both | `zeroize 1.8` |
| `ring` crate for key derivation | Pulls in large C build; argon2+hkdf already present and tested | Existing `argon2 0.5` + `hkdf 0.12` |
| `age` scrypt passphrase encryption for key-at-rest | Inconsistent with existing PIN flow; would add second KDF strategy | Reuse `pin_derive_key` + `pin_encrypt`/`pin_decrypt` from `crypto/mod.rs` |
| `keyring 4.0.0-rc.3` | Pre-release; API still in flux | `keyring 3.6.3` |
| `tokio` | Project is sync; adding async runtime for keyring alone is disproportionate | `sync-secret-service` feature of `keyring 3.6.3` |
| `memsec` / `secrets` | Niche crates with minimal adoption; zeroize is the ecosystem standard | `zeroize 1.8` |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `zeroize@1.8.2` | All existing deps | Already in Cargo.lock. No conflicts. `zeroize_derive@1.4.3` also already resolved. |
| `keyring@3.6.3` | Rust 1.75+ MSRV | Confirmed no async deps with `sync-secret-service` feature. On Linux, `dbus-secret-service@4.0.0-rc.2` is pulled in; not yanked (verified 2026-02-24). |
| `keyring@3.6.3` | `zeroize@1.8.1+` (zeroize is a direct dep of keyring on Windows) | Exact version `^1.8.1` required by keyring; `1.8.2` already in lock — satisfied. |
| Encrypted file format | `pkarr::Keypair::from_secret_key_file` | The `"cclink-encrypted-v1"` magic header is not valid hex, so pkarr returns `InvalidData`. `load_keypair()` can catch this error and branch to the decryption path before calling pkarr. No pkarr changes needed. |

---

## Sources

- `crates.io/api/v1/crates/zeroize` — version 1.8.2 confirmed current, published 2025-09-29 (HIGH confidence, verified live)
- `~/.cargo/registry/src/.../zeroize-1.8.2/Cargo.toml` — features verified: `derive = ["zeroize_derive"]`, `default = ["alloc"]` (HIGH confidence, read directly)
- `cclink/Cargo.lock` — `zeroize 1.8.2` and `zeroize_derive 1.4.3` already present (transitive); `secrecy 0.10.3` already present via age-core (HIGH confidence, read directly)
- `crates.io/api/v1/crates/keyring` — version 3.6.3 latest stable (2025-07-27); 4.0.0-rc.3 latest pre-release (2026-02-01) (HIGH confidence, verified live)
- `github.com/open-source-cooperative/keyring-rs v3.6.3/Cargo.toml` — feature flags `sync-secret-service`, `crypto-rust`, `apple-native`, `windows-native` verified (HIGH confidence, fetched directly)
- `crates.io/api/v1/crates/dbus-secret-service` — 4.1.0, published 2025-08-26, not yanked (HIGH confidence, verified live)
- `crates.io/api/v1/crates/secrecy` — 0.10.3, depends on `zeroize ^1.6` (HIGH confidence, verified live)
- `~/.cargo/registry/src/.../pkarr-5.0.3/src/keys.rs` — `from_secret_key_file` reads hex string, returns `InvalidData` on non-hex input; `write_secret_key_file` writes 64-char hex (HIGH confidence, source read directly)
- `crates.io/api/v1/crates/secret-service` — 5.1.0 confirmed available for keyring Linux async backend; 4.x used by keyring 3.6.3 (HIGH confidence, verified live)

---

*Stack research for: cclink v1.3 — encrypted key storage at rest and secure memory zeroization*
*Researched: 2026-02-24*
