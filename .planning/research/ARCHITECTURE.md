# Architecture Research

**Domain:** Rust CLI — encrypted key storage at rest and secure memory zeroization (cclink v1.3)
**Researched:** 2026-02-24
**Confidence:** HIGH (based on direct source code inspection, live `cargo tree` output, and verified crate docs)

---

## Context: What This Research Covers

This is a **subsequent milestone** architecture pass for v1.3. The existing module structure, command flow, and crypto primitives are unchanged. This document covers only:

1. How encrypted key storage at rest integrates with `src/keys/store.rs`
2. How secure memory zeroization integrates with `src/crypto/mod.rs` and call sites
3. New file format for the encrypted key file
4. Build order and component boundaries for implementation

---

## Existing Architecture (Baseline)

### Current Key Lifecycle

```
cclink init
    |
pkarr::Keypair::random()
    |
store::write_keypair_atomic()
    | calls
keypair.write_secret_key_file(&path)     <- pkarr writes raw hex (64 ASCII chars, no newline)
    |
chmod 0600                               <- cclink enforces, pkarr also sets on Unix
~/.pubky/secret_key                      <- plaintext hex on disk

cclink [any command that needs the key]
    |
store::load_keypair()
    | calls
pkarr::Keypair::from_secret_key_file()   <- reads hex, parses [u8;32], builds Keypair
    |
keypair used in command, then dropped    <- NO zeroization today
```

### Current Secret Key Format on Disk

```
~/.pubky/secret_key (64 bytes, ASCII text, no newline, 0600 perms)
example: a3f9e2c1d4b7... (64 hex chars = 32 bytes raw secret)
```

Confirmed by direct inspection: the file is 64 bytes of ASCII text with no line terminator (file reports "ASCII text, with no line terminators"). pkarr's `write_secret_key_file()` and `from_secret_key_file()` own this format. The current `store.rs` delegates both read and write to pkarr for the plaintext path.

### Current Module Responsibilities

| Module | File | What It Owns |
|--------|------|--------------|
| `keys::store` | `src/keys/store.rs` | Key path resolution, atomic write, permission check, keypair load |
| `crypto` | `src/crypto/mod.rs` | age encrypt/decrypt, X25519 derivation, Argon2id+HKDF (PIN mode) |
| `commands::init` | `src/commands/init.rs` | Keypair generation, import from file/stdin, calls store |
| `commands::publish` | `src/commands/publish.rs` | Calls `store::load_keypair()` at command start |
| `commands::pickup` | `src/commands/pickup.rs` | Calls `store::load_keypair()` at command start |
| `commands::whoami` | `src/commands/whoami.rs` | Calls `store::load_keypair()` |
| `commands::list` | `src/commands/list.rs` | Calls `store::load_keypair()` |
| `commands::revoke` | `src/commands/revoke.rs` | Calls `store::load_keypair()` |

---

## New Architecture: v1.3 Changes

### Two Orthogonal Features

The v1.3 features are independent and can be built in either order, but zeroization is simpler and provides immediate value, making it the recommended first phase.

**Feature A: Memory Zeroization** — Call `zeroize` on raw secret key bytes after use. Zero structural changes; only adds `Zeroizing<T>` wrapper types at existing key derivation sites.

**Feature B: Encrypted Key Storage** — Replace the plaintext hex file with an encrypted envelope. Requires a new file format, passphrase prompt, new functions in `store.rs` and `crypto/mod.rs`, and a format detection mechanism for backward compatibility.

---

### System Overview: v1.3 Target State

```
+-----------------------------------------------------------------------+
|                          CLI Commands                                  |
|  init  whoami  publish  pickup  list  revoke                           |
+------------------------------------+----------------------------------+
                                     | calls load_keypair() /
                                     | write_keypair_atomic()
+------------------------------------+----------------------------------+
|                       keys::store  (MODIFIED)                          |
|                                                                         |
|  load_keypair()                                                         |
|    +-- detect format: plaintext hex vs. encrypted envelope             |
|    +-- [encrypted] prompt passphrase -> crypto::decrypt_key_envelope() |
|    +-- [plaintext] pkarr::from_secret_key_file() (unchanged path)      |
|                                                                         |
|  write_keypair_atomic()                                                 |
|    +-- [with passphrase] -> crypto::encrypt_key_envelope()             |
|    |    -> write encrypted binary file (NEW format)                    |
|    +-- [no passphrase] -> pkarr::write_secret_key_file() (unchanged)   |
+------------------------------------+----------------------------------+
                                     | returns Zeroizing<[u8;32]>
+------------------------------------+----------------------------------+
|                     crypto::mod (MODIFIED)                              |
|                                                                         |
|  encrypt_key_envelope(secret: &[u8;32], passphrase: &str)              |
|    -> Argon2id+HKDF derive key -> age encrypt -> binary envelope        |
|                                                                         |
|  decrypt_key_envelope(ciphertext: &[u8], passphrase: &str)             |
|    -> Argon2id+HKDF re-derive -> age decrypt -> Zeroizing<[u8;32]>     |
|                                                                         |
|  ed25519_to_x25519_secret() -> Zeroizing<[u8;32]>  (type change)       |
+-----------------------------------------------------------------------+
```

---

### Encrypted Key File Format

The existing file is a 64-byte ASCII hex string that pkarr reads directly. The new encrypted file cannot use the same path or format — pkarr's `from_secret_key_file()` would fail to parse it. The solution is a binary envelope with a magic header, distinct from pkarr's hex format.

**Recommended binary layout:**

```
Offset  Length  Field
------  ------  -----
0       8       Magic: b"CCLINKEK"  (CCLink Encrypted Key)
8       1       Version: 0x01
9       1       KDF ID: 0x01 = Argon2id
10      1       Cipher ID: 0x01 = age/X25519
11      1       Reserved: 0x00
12      32      Salt (random, 32 bytes)
44      N       age ciphertext (variable length, ~200-300 bytes in practice)
```

**Format rationale:**
- Magic + version lets `load_keypair()` detect format without ambiguity: plaintext hex starts with `[0-9a-f]`; binary starts with `C` (0x43). These never overlap.
- Argon2id parameters (t=3, m=65536, p=1) are identical to the values already used in `crypto::pin_derive_key()` — no new parameters to introduce
- age ciphertext wraps the raw 32-byte secret key (not the hex encoding) to keep the encrypted payload small and avoid double-encoding
- The format is a new cclink-owned format, not age's `encrypted::Identity` file format (which uses scrypt and has a different wire layout)
- `HKDF info = b"cclink-key-v1"` to domain-separate from the PIN KDF which uses `b"cclink-pin-v1"`

**Detection logic in `load_keypair()`:**

```rust
// Read file as raw bytes instead of delegating to pkarr
let raw = std::fs::read(&path)?;
if raw.starts_with(b"CCLINKEK") {
    // New encrypted format
    let passphrase = Zeroizing::new(
        dialoguer::Password::new()
            .with_prompt("Enter key passphrase")
            .interact()?
    );
    crypto::decrypt_key_envelope(&raw, &passphrase)
        .map(|secret| pkarr::Keypair::from_secret_key(&secret))
} else {
    // Legacy plaintext hex -- delegate to pkarr unchanged
    pkarr::Keypair::from_secret_key_file(&path)
        .map_err(|e| anyhow::anyhow!("Failed to load keypair: {}", e))
}
```

---

### Component Boundaries: New vs. Modified

| Component | Status | What Changes |
|-----------|--------|--------------|
| `keys::store::load_keypair()` | **MODIFIED** | Add format detection; call `crypto::decrypt_key_envelope()` for encrypted files; prompt for passphrase via dialoguer |
| `keys::store::write_keypair_atomic()` | **MODIFIED** | Accept `Option<&str>` passphrase; call `crypto::encrypt_key_envelope()` when passphrase provided; write binary file instead of delegating to pkarr |
| `commands::init::run_init()` | **MODIFIED** | Add `--encrypt` flag; prompt for passphrase at init time; pass to `write_keypair_atomic()` |
| `crypto::encrypt_key_envelope()` | **NEW** | Argon2id+HKDF key derivation + age encrypt; returns `Vec<u8>` (binary envelope with header) |
| `crypto::decrypt_key_envelope()` | **NEW** | Argon2id+HKDF key re-derivation + age decrypt; returns `Zeroizing<[u8;32]>` |
| `crypto::ed25519_to_x25519_secret()` | **MODIFIED** | Return type changes to `Zeroizing<[u8;32]>` |
| `commands::publish` | **MODIFIED** | x25519_secret call site: `Zeroizing` is transparent via `Deref` — minimal change |
| `commands::pickup` | **MODIFIED** | Same as publish — x25519_secret call site update |
| `cli::InitArgs` | **MODIFIED** | Add `--encrypt` bool flag |
| `keys::store::check_key_permissions()` | **UNCHANGED** | 0600 permission check still applies to both file formats |
| All other modules | **UNCHANGED** | No changes needed |

---

### Memory Zeroization: Call Site Map

`zeroize 1.8.2` is already in the dependency graph (confirmed via `cargo tree`: transitive via pkarr, age, and secrecy 0.10.3). Adding it as a direct dependency requires only a `Cargo.toml` entry — no new crate download.

**Where to zeroize:**

```
src/crypto/mod.rs
|
+-- ed25519_to_x25519_secret()
|     Return Zeroizing<[u8;32]> instead of [u8;32]
|     -> The wrapper zeroizes the scalar when it goes out of scope
|
+-- decrypt_key_envelope() [NEW]
|     Return Zeroizing<[u8;32]>
|     Internal: wrap derived key bytes in Zeroizing<[u8;32]> immediately
|     after derivation, before passing to age decrypt
|
+-- pin_derive_key() [OPTIONAL - low priority]
      Wrap argon2_output and okm in Zeroizing<[u8;32]>
      Current: [u8;32] stack arrays are cleared at end of function scope anyway
      Worth adding for defense-in-depth on long-lived stacks

src/keys/store.rs
+-- load_keypair() [encrypted path]
      Passphrase string: wrap in Zeroizing<String> before dialoguer call
      -> Zeroizing<String> zeros heap allocation on drop
      After pkarr::Keypair::from_secret_key(&secret):
      Zeroizing<[u8;32]> drops automatically -> bytes zeroed

src/commands/publish.rs, pickup.rs
+-- let x25519_secret = crypto::ed25519_to_x25519_secret(&keypair);
      Currently [u8;32] -> becomes Zeroizing<[u8;32]>
      Callers use via Deref: &*x25519_secret or coercion to &[u8;32]
      The X25519 scalar is zeroized when x25519_secret drops at scope end
```

**What is NOT zeroized (and why it is acceptable):**

- `pkarr::Keypair` itself: pkarr does not expose a `Zeroize` impl on `Keypair`. The `SigningKey` inside pkarr uses `ed25519_dalek::SigningKey`, which has `zeroize` as an optional feature. Whether pkarr enables that feature is not verified — do not assume it does.
- DHT `SignedPacket` payloads: contain public key material and ciphertext only, not secret key bytes.
- age ciphertext blobs in memory: not secret; ciphertext is already the encrypted form.

**Practical priority:** Zeroizing the X25519 scalar is the highest-value target. This 32-byte value, if leaked from a memory dump, allows decryption of all self-encrypted handoffs. The Ed25519 seed bytes inside `pkarr::Keypair` are also sensitive but controlled by pkarr's memory management.

---

### Data Flow: Key Load After v1.3

```
User runs: cclink [any command]
    |
store::load_keypair()
    | read file as raw bytes (fs::read instead of pkarr's fs::read_to_string)
format detection (magic check)
    |
    +-- plaintext hex (starts with [0-9a-f]):
    |     pkarr::from_secret_key_file() -> pkarr::Keypair [unchanged path]
    |
    +-- encrypted envelope (starts with b"CCLINKEK"):
          Zeroizing<String> passphrase <- dialoguer::Password
                |
          crypto::decrypt_key_envelope(&bytes, &passphrase)
                | parse header: version, KDF ID, cipher ID, extract salt
                | Argon2id+HKDF(passphrase, salt, "cclink-key-v1") -> [u8;32]
                | age decrypt(ciphertext, derived_key_as_identity) -> 32 bytes
                | return Zeroizing<[u8;32]>
                |
          pkarr::Keypair::from_secret_key(&*secret_bytes)
                |
          Zeroizing<[u8;32]> drops -> secret bytes zeroed
          Zeroizing<String> drops -> passphrase heap bytes zeroed
                |
    pkarr::Keypair [returned to caller]
    |
Command executes
    | e.g. publish: derive X25519 scalar
crypto::ed25519_to_x25519_secret(&keypair) -> Zeroizing<[u8;32]>
    | used for age encrypt
    | Zeroizing<[u8;32]> drops at end of scope -> X25519 scalar zeroed
```

### Data Flow: Key Write After v1.3

```
User runs: cclink init --encrypt
    |
commands::init::run_init()
    | generate or import keypair
    |
    +-- --encrypt flag present:
    |     Zeroizing<String> passphrase <- dialoguer::Password (with confirmation)
    |     store::write_keypair_atomic(&keypair, &path, Some(&passphrase))
    |           |
    |     crypto::encrypt_key_envelope(&keypair.secret_key(), &passphrase)
    |           | random 32-byte salt via rand::thread_rng().gen()
    |           | Argon2id+HKDF(passphrase, salt, "cclink-key-v1") -> [u8;32]
    |           | age encrypt(raw 32-byte secret, derived_key_as_recipient)
    |           | prepend b"CCLINKEK" + version(0x01) + KDF(0x01) + cipher(0x01) + 0x00 + salt
    |           | return Vec<u8> (binary envelope ~250 bytes)
    |           |
    |     write binary file atomically (temp + rename)
    |     chmod 0600
    |
    +-- no --encrypt flag:
          store::write_keypair_atomic(&keypair, &path, None)
          -> existing path: pkarr::write_secret_key_file() [unchanged]
```

---

### Recommended Project Structure After v1.3

The module structure does not change. New functions are added to existing modules:

```
src/
+-- crypto/
|   +-- mod.rs            MODIFIED: add encrypt_key_envelope(), decrypt_key_envelope()
|                          MODIFIED: ed25519_to_x25519_secret() returns Zeroizing<[u8;32]>
+-- keys/
|   +-- store.rs          MODIFIED: load_keypair() format detection + passphrase prompt
|                          MODIFIED: write_keypair_atomic() accepts Option<&str> passphrase
+-- commands/
|   +-- init.rs           MODIFIED: --encrypt flag, passphrase prompt, pass to store
|   +-- publish.rs        MODIFIED: x25519_secret call site (Zeroizing deref, minor)
|   +-- pickup.rs         MODIFIED: x25519_secret call site (Zeroizing deref, minor)
+-- cli.rs                MODIFIED: add --encrypt flag to InitArgs
+-- [all other files]     UNCHANGED
```

No new source files are required. `crypto::mod.rs` and `keys/store.rs` are the two load-bearing files.

---

### Argon2id Parameters for Key File KDF

Use the same parameters already in `crypto::pin_derive_key()`:
- `t_cost = 3` (time), `m_cost = 65536` (64 MB), `p_cost = 1` (parallelism)
- HKDF info string: `b"cclink-key-v1"` (different from `b"cclink-pin-v1"` to domain-separate)

These parameters are encoded implicitly by the KDF ID byte in the header (version 0x01 = these exact parameters). If parameters must change in a future format, bump the version byte and document new values. Do not embed raw parameter values in the v1 header — it would bloat the format and the parameters are not expected to change.

**Why NOT use age's `Encryptor::with_user_passphrase` (scrypt) for the key file:**
- cclink already has Argon2id+HKDF in `crypto::pin_derive_key()` — reusing the same KDF keeps the security model consistent and avoids two KDF libraries for the same task
- age's passphrase path uses scrypt internally; mixing KDFs (scrypt for key file, Argon2id for PIN handoffs) in the same codebase creates unnecessary audit complexity
- A small custom binary envelope for a 32-byte secret is more auditable than an age-format file designed for arbitrary content

---

### Backward Compatibility

The format detection approach provides full backward compatibility with no migration needed:

- Existing users with plaintext `~/.pubky/secret_key` continue to work without changes
- New users running `cclink init --encrypt` get an encrypted key file
- `cclink init` without `--encrypt` continues to write a plaintext hex file
- Users who want to encrypt an existing key can run `cclink init --encrypt --import <old_key>` to re-import with encryption (this flow already works; no new code needed beyond the `--encrypt` flag)

**v1.3 does not require any key migration and does not break existing installations.**

---

### Dependency Changes

| Crate | Change | Notes |
|-------|--------|-------|
| `zeroize` | Add as **direct** dependency | Already in graph at 1.8.2 (transitive via pkarr, age, and secrecy). Adding as direct with `features = ["derive"]` unlocks `#[derive(Zeroize)]`, `Zeroizing<T>`, and `ZeroizeOnDrop`. No new crate download. |
| `argon2` | **Unchanged** | Already direct dependency at 0.5. |
| `hkdf` | **Unchanged** | Already direct dependency at 0.12. |
| `sha2` | **Unchanged** | Already direct dependency at 0.10. |
| `age` | **Unchanged** | Already direct dependency at 0.11. Used for envelope encryption. |
| `rand` | **Unchanged** | Already direct at 0.8. Used for 32-byte salt generation. |
| `secrecy` | **Do NOT add as direct dep** | Already transitive at 0.10.3. Wrapping passphrase in `Secret<String>` adds type complexity; `Zeroizing<String>` from `zeroize` is sufficient and simpler. |
| `chacha20poly1305` | **Do NOT add** | age already handles AEAD internally. |

Cargo.toml addition:
```toml
zeroize = { version = "1.8", features = ["derive"] }
```

---

### Anti-Patterns to Avoid

**Anti-Pattern 1: Trying to Call `.zeroize()` on `pkarr::Keypair`**

What people do: Add `keypair.zeroize()` after use because it holds the secret key.
Why it's wrong: `pkarr::Keypair` does not implement `Zeroize`. The code will not compile.
Do this instead: Zeroize intermediate `[u8;32]` secret bytes before passing to `pkarr::Keypair::from_secret_key()`, and zeroize the derived X25519 scalar via `Zeroizing<[u8;32]>` return types.

**Anti-Pattern 2: Storing the Passphrase in a Plain `String`**

What people do: `let passphrase = dialoguer::Password::new().interact()?;` then pass `&passphrase` to the encryption function, leaving the plain `String` on the heap until end of function.
Why it's wrong: `String::drop()` does not zero heap contents. The passphrase remains readable in memory after drop.
Do this instead: `let passphrase = Zeroizing::new(dialoguer::Password::new().interact()?);` — the heap allocation is zeroed when `Zeroizing<String>` drops.

**Anti-Pattern 3: Writing the Encrypted File Without Atomic Replacement**

What people do: Call `fs::write(&path, envelope_bytes)` directly, overwriting `~/.pubky/secret_key` in a single non-atomic step.
Why it's wrong: If the write is interrupted, the file contains partial data — the user permanently loses their key and the old plaintext is destroyed.
Do this instead: Reuse the existing `write_keypair_atomic()` temp-file-then-rename pattern. Extend it rather than bypassing it.

**Anti-Pattern 4: Prompting for Passphrase Unconditionally in Every Command**

What people do: Move the passphrase prompt into `store::load_keypair()` with no format check — all commands always prompt.
Why it's wrong: Commands called in non-interactive contexts (pipes, scripts, CI) will hang indefinitely waiting for a passphrase.
Do this instead: Check format first. Only prompt when the file starts with `CCLINKEK`. Add a non-interactive guard: `if !std::io::stdin().is_terminal() { anyhow::bail!("encrypted key requires interactive terminal"); }` before the dialoguer prompt.

**Anti-Pattern 5: Using `b"cclink-pin-v1"` as HKDF Info for the Key File**

What people do: Call `pin_derive_key()` directly for the key file to reuse the existing function.
Why it's wrong: Domain separation is violated. The same passphrase with the same salt would produce the same derived key for a PIN handoff and a key file — confusing for auditors and creates unnecessary coupling.
Do this instead: Use `b"cclink-key-v1"` as the HKDF info string. A new `key_derive_key()` function (or a generalized version with an info parameter) keeps domains separate.

---

### Build Order

Feature A (zeroization) and Feature B (encrypted storage) are independent. Recommended order within each:

**Feature A: Memory Zeroization (do first)**

1. Add `zeroize = { version = "1.8", features = ["derive"] }` to `Cargo.toml`
2. Change `ed25519_to_x25519_secret()` return type to `Zeroizing<[u8;32]>`
3. Update `publish.rs` and `pickup.rs` call sites (deref coercion makes this minimal)
4. Optionally: wrap intermediate bytes in `pin_derive_key()` as `Zeroizing<[u8;32]>` for defense-in-depth
5. Verify: `cargo test` passes, `cargo clippy --all-targets -- -D warnings` passes

**Feature B: Encrypted Key Storage (do second)**

1. Add `crypto::encrypt_key_envelope()` with tests (round-trip, output is valid binary envelope)
2. Add `crypto::decrypt_key_envelope()` with tests (correct passphrase succeeds, wrong passphrase fails, returns Zeroizing)
3. Modify `store::load_keypair()`: format detection + encrypted branch + passphrase prompt + non-interactive guard
4. Modify `store::write_keypair_atomic()`: accept `Option<&str>` passphrase, encrypted branch
5. Add `--encrypt` flag to `cli::InitArgs` and update `commands::init::run_init()`
6. Integration test: `init --encrypt` then `load_keypair` with correct passphrase succeeds
7. Integration test: `init --encrypt` then `load_keypair` with wrong passphrase fails (not a corrupt key error; an incorrect passphrase error)
8. Integration test: existing plaintext key file still loads correctly

**Key dependency within Feature B:** steps 1-2 (crypto functions) must be done before steps 3-4 (store integration). All other steps are independent.

---

### Integration Points Summary

| Integration Point | Change Type | File |
|-------------------|-------------|------|
| `store::load_keypair()` | Modified — format detection + encrypted branch | `src/keys/store.rs` |
| `store::write_keypair_atomic()` | Modified signature — `Option<&str>` passphrase | `src/keys/store.rs` |
| `crypto::encrypt_key_envelope()` | New function | `src/crypto/mod.rs` |
| `crypto::decrypt_key_envelope()` | New function | `src/crypto/mod.rs` |
| `crypto::ed25519_to_x25519_secret()` | Modified return type only | `src/crypto/mod.rs` |
| `commands::init::run_init()` | Modified — new `--encrypt` flag + passphrase prompt | `src/commands/init.rs` |
| `commands::publish` x25519 call site | Minor — Zeroizing deref | `src/commands/publish.rs` |
| `commands::pickup` x25519 call site | Minor — Zeroizing deref | `src/commands/pickup.rs` |
| `cli::InitArgs` | Modified — add `--encrypt` bool | `src/cli.rs` |
| `Cargo.toml` | Modified — add zeroize direct dep | `Cargo.toml` |

---

## Sources

- Live `cargo tree` on cclink — zeroize 1.8.2 and secrecy 0.10.3 confirmed in dependency graph (HIGH confidence)
- `Cargo.lock` inspection — zeroize 1.8.2 checksum and secrecy 0.10.3 verified (HIGH confidence)
- `~/.pubky/secret_key` — confirmed 64-byte ASCII text, no newline; format is 64 hex chars (HIGH confidence, direct inspection via `ls -la` and `file`)
- [docs.rs/pkarr/5.0.3/pkarr/struct.Keypair.html](https://docs.rs/pkarr/5.0.3/pkarr/struct.Keypair.html) — `write_secret_key_file` stores hex, `from_secret_key_file` reads hex, pkarr sets 0600 on Unix (HIGH confidence)
- [docs.rs/zeroize/latest/zeroize](https://docs.rs/zeroize/latest/zeroize/) — `Zeroize` trait, `Zeroizing<T>` wrapper, `ZeroizeOnDrop` derive macro (HIGH confidence)
- [docs.rs/age/latest/age](https://docs.rs/age/latest/age/) — `Encryptor::with_user_passphrase`, `scrypt::Recipient/Identity` available; age 0.11.x API stable (HIGH confidence)
- [kerkour.com Rust file encryption with Argon2](https://kerkour.com/rust-file-encryption-chacha20poly1305-argon2) — binary header format pattern with magic, version, salt (MEDIUM confidence, community blog post)
- Direct code inspection: `src/crypto/mod.rs`, `src/keys/store.rs`, `src/commands/init.rs`, `src/commands/publish.rs`, `src/commands/pickup.rs`, `src/cli.rs`, `Cargo.toml` — v1.2 codebase state (HIGH confidence)

---

*Architecture research for: cclink v1.3 — encrypted key storage at rest and secure memory zeroization*
*Researched: 2026-02-24*
