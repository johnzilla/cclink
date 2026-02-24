# Project Research Summary

**Project:** cclink v1.3 — Key Security Hardening
**Domain:** Rust CLI — encrypted key storage at rest and secure memory zeroization
**Researched:** 2026-02-24
**Confidence:** HIGH

## Executive Summary

cclink v1.3 addresses a single, well-defined security gap: the Ed25519 secret key at `~/.pubky/secret_key` is stored as a plaintext hex string. A device compromise exfiltrates the private key with no additional work needed from an attacker. This milestone closes that gap with two orthogonal features: passphrase-encrypted key storage at rest (Argon2id+HKDF+age, reusing existing crypto already implemented for PIN-protected handoffs) and secure memory zeroization of the raw 32-byte secret scalar after use (the `zeroize` crate, already a transitive dependency at v1.8.2).

The recommended approach reuses the project's existing cryptographic machinery rather than introducing new primitives. The `pin_derive_key` + `pin_encrypt` / `pin_decrypt` functions in `src/crypto/mod.rs` are fully applicable to key-at-rest encryption; only the HKDF info string changes (`"cclink-key-v1"` vs. `"cclink-pin-v1"`) to maintain domain separation. The new cclink-owned binary file format (`CCLINKEK` magic header + version + Argon2 parameters + salt + age ciphertext) replaces pkarr's plaintext hex format for users who opt in via `cclink init --encrypt`. Existing users with plaintext key files continue working without changes — the format is detected by magic bytes before any passphrase prompt is issued.

The primary risks are file format and storage correctness issues, not cryptographic ones. The crypto choices are sound and already battle-tested in the codebase. The pitfalls that could produce real user harm are: writing the encrypted file non-atomically (corrupt key window), failing to store Argon2 parameters in the file header (future parameter change breaks existing keys), and calling pkarr's I/O functions on the encrypted blob instead of owning the read/write path in cclink. All three are avoidable with explicit design decisions made before writing code.

---

## Key Findings

### Recommended Stack

Two new direct dependencies are needed; everything else reuses what is already in Cargo.lock. `zeroize 1.8.2` is already a transitive dependency via ed25519-dalek and pkarr — promoting it to a direct dep with `features = ["derive"]` adds no new compilation units. `keyring 3.6.3` is the only genuinely new crate, and it is only needed for the optional system keystore integration which is out of scope for the core v1.3 milestone.

The encrypted file format is implemented entirely from existing deps: `argon2 0.5` + `hkdf 0.12` + `sha2 0.10` + `age 0.11` + `rand 0.8`. This is the same stack already verified end-to-end for PIN-protected handoffs.

**Core technologies:**
- `zeroize 1.8.2` (direct dep, no compile cost): auto-zeroize `[u8; 32]` seed, `Vec<u8>` decrypted payloads, and `String` passphrase on drop — already in Cargo.lock at this version
- `argon2 0.5` + `hkdf 0.12` + `age 0.11` (existing direct deps): Argon2id+HKDF key derivation and age encryption for the key file — identical to existing PIN crypto, reused with distinct HKDF info string
- `keyring 3.6.3` (new, deferred to P2): macOS Keychain / Freedesktop Secret Service / Windows Credential Manager for passphrase caching — requires careful fallback for headless Linux; do not ship in v1.3 core

**Cargo.toml addition (core milestone only):**
```toml
zeroize = { version = "1.8", features = ["derive"] }
```

### Expected Features

**Must have (table stakes — v1.3):**
- Encrypted key file at rest (passphrase-protected, `CCLINKEK` binary format, Argon2id+HKDF+age, 0600 permissions preserved, Argon2 params stored in file header)
- Passphrase prompt on `cclink init --encrypt` (with confirmation, empty passphrase rejected)
- Passphrase prompt on every `load_keypair()` when file is encrypted (single call site: `src/keys/store.rs`)
- Format detection by magic bytes — plaintext v1.2 keys load without passphrase prompt in v1.3 binary (full backward compatibility)
- Secure zeroization of derived X25519 scalar (`Zeroizing<[u8;32]>` return from `ed25519_to_x25519_secret()`) and passphrase string (`Zeroizing<String>` from dialoguer)
- Argon2 parameters stored in file header (not hardcoded constants) — required for future-proof decryption when parameters evolve

**Should have (competitive differentiators — P2, separate milestone):**
- System keystore integration (`keyring 3.6.3`) — OS keychain caching to eliminate per-command prompts; requires graceful fallback when keychain unavailable (headless Linux)
- `cclink rekey` subcommand — re-encrypt existing key with new passphrase without key regeneration

**Defer (v2+):**
- `CCLINK_PASSPHRASE` env var for CI/scripting (document security implications when added)
- `--no-passphrase` flag on `cclink init` (explicit opt-out; defer until encrypted storage is proven stable)

### Architecture Approach

v1.3 modifies two load-bearing modules (`src/crypto/mod.rs` and `src/keys/store.rs`) and makes minor adjustments to `src/commands/init.rs`, `src/cli.rs`, and the two command files that use the X25519 scalar (`publish.rs`, `pickup.rs`). No new source files are required. The pkarr I/O functions (`write_secret_key_file` / `from_secret_key_file`) are bypassed for the encrypted path; cclink owns the read/write lifecycle via `keypair.secret_key()` → encrypt → write and read → decrypt → `Keypair::from_secret_key()`.

**Major components:**

1. `crypto::encrypt_key_envelope()` / `decrypt_key_envelope()` (NEW functions in `src/crypto/mod.rs`) — produce and consume the `CCLINKEK` binary envelope; return `Zeroizing<[u8;32]>` on decryption; HKDF info `"cclink-key-v1"`
2. `keys::store::load_keypair()` (MODIFIED in `src/keys/store.rs`) — reads raw bytes, branches on magic header, prompts passphrase only for encrypted format, includes non-interactive terminal guard before dialoguer call
3. `keys::store::write_keypair_atomic()` (MODIFIED in `src/keys/store.rs`) — accepts `Option<&str>` passphrase; encrypted path uses temp-rename-chmod sequence, identical to existing plaintext path
4. `commands::init::run_init()` + `cli::InitArgs` (MODIFIED) — `--encrypt` flag triggers passphrase prompt with confirmation before write
5. `crypto::ed25519_to_x25519_secret()` (MODIFIED return type) — returns `Zeroizing<[u8;32]>` instead of `[u8;32]`; callers in `publish.rs` and `pickup.rs` use via transparent Deref with minimal changes

**Encrypted file format:**
```
Offset  Length  Field
0       8       Magic: b"CCLINKEK"
8       1       Version: 0x01
9       1       KDF ID: 0x01 = Argon2id
10      1       Cipher ID: 0x01 = age
11      1       Reserved: 0x00
12      32      Salt (random, 32 bytes)
44      4+4+4   Argon2 params: t_cost(u32 LE), m_cost(u32 LE), p_cost(u32 LE) — stored for future-proof decryption
56      N       age ciphertext (~200-300 bytes)
```
HKDF info string: `b"cclink-key-v1"` — distinct from `b"cclink-pin-v1"` used for handoff PIN derivation.

### Critical Pitfalls

1. **pkarr I/O functions cannot be used for the encrypted format** — `from_secret_key_file` parses hex and returns `InvalidData` on binary input; `write_secret_key_file` produces hex only. Bypass both: extract seed via `keypair.secret_key()`, encrypt those bytes, write the cclink-owned `CCLINKEK` format; on load, check magic bytes before calling any pkarr I/O.

2. **Argon2 parameters must be stored in the file header, not hardcoded** — any future change to `t_cost` / `m_cost` / `p_cost` in source code would silently produce a different derived key, making all existing encrypted key files undecryptable with the correct passphrase. Store the three parameters in the header and read them back on decryption. Mirrors the Argon2 PHC string format and age's scrypt stanza design.

3. **Encrypted file write must use the atomic temp-rename-chmod pattern** — `std::fs::write` is not atomic (Rust tracking issue #82590). A partial write corrupts the key file irreversibly. The existing `write_keypair_atomic` already does write-to-temp then rename then chmod 0600; the encrypted path must follow the same sequence, including explicit 0600 chmod on the destination after rename (pkarr's implicit chmod is no longer called on the encrypted path).

4. **`dialoguer::Password` returns a plain `String` that is not zeroized on drop** — wrap immediately as `Zeroizing::new(password_prompt.interact()?)`. Apply to the new passphrase prompt and also fix the existing PIN prompts in `publish.rs` and `pickup.rs` as part of the zeroization sweep.

5. **HKDF domain separation is required** — using `"cclink-pin-v1"` as the info string for key-file derivation would mean the same passphrase used as both a PIN and a key passphrase produces the same derived key. Use `"cclink-key-v1"` in a separate derivation function. Never call `pin_derive_key()` directly for the key file.

---

## Implications for Roadmap

Two independent features are sequenced as separate phases within v1.3. Zeroization comes first because it is contained (one crate, a few call sites, no file format decisions) and validates the `Zeroizing<T>` wrapper patterns before they appear in the encrypted key load path.

### Phase 1: Memory Zeroization Sweep

**Rationale:** Completely independent of file format changes. Lowest-risk phase: the `zeroize` crate is already in the dependency graph, the return type change in `ed25519_to_x25519_secret()` propagates via Deref with minimal call-site churn, and existing PIN prompts in `publish.rs` / `pickup.rs` get fixed as a side effect. Doing this first means Phase 2's encrypted key path is built on a zeroization-clean foundation.

**Delivers:** X25519 derived scalar wrapped in `Zeroizing<[u8;32]>` and zeroed on drop; passphrase/PIN strings from dialoguer wrapped in `Zeroizing<String>` and zeroed on drop; intermediate KDF arrays in `pin_derive_key()` wrapped for defense-in-depth.

**Addresses:** Secure memory zeroization (P1 feature); fixes existing PIN-prompt zeroization gap in `publish.rs` and `pickup.rs` as a side effect.

**Avoids:** Pitfalls 3, 4, 6 (stack copies of seed, unzeroized passphrase String, unzeroized Vec<u8> decrypted output).

**Research flag:** No deeper research needed. zeroize 1.8.2 patterns are well-documented; call sites are enumerated from direct codebase inspection.

---

### Phase 2: Encrypted Key File — Crypto Layer

**Rationale:** The two new crypto functions (`encrypt_key_envelope`, `decrypt_key_envelope`) are the foundation that `keys::store` depends on. Building and validating the crypto layer in isolation — with unit tests — allows verifying correctness before any user-facing code changes. Format definition (magic bytes, header layout, Argon2 param storage) must be finalized in this phase before any write/read code is written.

**Delivers:** `crypto::encrypt_key_envelope()` and `crypto::decrypt_key_envelope()` with unit tests covering: correct passphrase round-trip, wrong passphrase returns error, output has correct magic bytes, Argon2 parameters are stored in and read from the header, HKDF info string is distinct from PIN derivation.

**Addresses:** Encrypted key file at rest (P1 feature — crypto half); Argon2 parameter storage in header (pitfall 2).

**Avoids:** Pitfall 1 (these functions never call pkarr I/O), Pitfall 5 (HKDF domain separation via `"cclink-key-v1"`), Pitfall 2 (Argon2 params in header from the start, not hardcoded).

**Research flag:** No deeper research needed. The Argon2id+HKDF+age pattern is already implemented in `pin_derive_key`/`pin_encrypt`/`pin_decrypt`; this phase adapts it with a distinct info string and a new binary envelope layout.

---

### Phase 3: Encrypted Key File — Storage Layer and CLI Integration

**Rationale:** Depends on Phase 2 crypto functions. Modifies `store::load_keypair()` and `write_keypair_atomic()`, adds `--encrypt` flag to `cclink init`, and wires the full flow together. The non-interactive terminal guard, atomic write sequence, and 0600 permission enforcement all belong in this phase. Integration tests for the complete lifecycle must ship here.

**Delivers:** Full encrypted key lifecycle — `cclink init --encrypt` prompts for passphrase and writes encrypted binary file; any command that calls `load_keypair()` detects format by magic bytes and either prompts passphrase (encrypted) or loads directly (plaintext); existing v1.2 key files continue to work unmodified.

**Required integration tests:**
- `init --encrypt` + `load_keypair()` with correct passphrase succeeds
- `init --encrypt` + `load_keypair()` with wrong passphrase fails with clear error (not a corrupt-key error)
- v1.2 plaintext key file loads in v1.3 binary without passphrase prompt
- Encrypted key file has 0600 permissions after write
- Interrupting the write (simulated) leaves no partial file at the destination path

**Addresses:** All P1 features — passphrase prompt on init, passphrase prompt on load, format detection and backward compatibility, 0600 permissions, atomic write.

**Avoids:** Pitfall 1 (magic-byte detection precedes any pkarr I/O call), Pitfall 2 (format detection by magic header), Pitfall 3 (atomic temp-rename-chmod), Pitfall 8 (explicit 0600 chmod in encrypted write path — not inherited from pkarr).

**Research flag:** No deeper research needed. All edge cases are enumerated in PITFALLS.md; atomic write and permission patterns already exist in the codebase.

---

### Phase 4: System Keystore Integration (P2, separate milestone after v1.3)

**Rationale:** Sequenced after basic encrypted storage is stable. The system keystore stores the passphrase (or derived key) in the OS keychain so users are not prompted on every command — the SSH agent / GPG agent model. The `keyring 3.6.3` crate covers macOS Keychain, Freedesktop Secret Service (Linux), and Windows Credential Manager. The Linux headless fallback is the blocking complexity: keyring-rs issue #133 confirms no automatic fallback when D-Bus secret service is unavailable. Any implementation must explicitly degrade to a passphrase prompt on keychain failure.

**Delivers:** Optional per-session passphrase caching via OS keychain. `try keychain → fall back to prompt` behavior. No regressions in headless or CI environments.

**Addresses:** P2 differentiator — eliminates per-command passphrase prompts for interactive users without a security tradeoff.

**Research flag:** Needs `/gsd:research-phase` during planning. Linux headless fallback behavior requires integration testing. keyring 4.x (rc.3 as of 2026-02-01) may stabilize before this phase ships — evaluate the upgrade path at planning time rather than committing to 3.6.3 now.

---

### Phase Ordering Rationale

- **Zeroization before encrypted storage:** Phase 1 is self-contained and validates `Zeroizing<T>` patterns before they appear in the encrypted load path. The decrypted key bytes and passphrase zeroization in Phase 3 rely on patterns established in Phase 1.
- **Crypto layer before storage layer:** Phase 2 must produce tested, correct `encrypt_key_envelope` / `decrypt_key_envelope` before Phase 3 integrates them. This isolates failures: crypto correctness failures appear in Phase 2 tests; storage integration failures appear in Phase 3.
- **Core encrypted storage before system keystore:** Phase 4 depends on Phase 3 being stable. The OS keychain caches the passphrase for an encrypted key file; without Phase 3, there is nothing to protect.
- **Backward compatibility is a Phase 3 hard requirement:** The magic-byte format detection ensures v1.2 users are never prompted for a passphrase they never set. This test must pass before Phase 3 ships.

### Research Flags

Phases needing deeper research during planning:
- **Phase 4 (System Keystore):** Linux DBus headless fallback behavior, keyring 4.x API stabilization timeline, CI/scripting environment compatibility matrix — all require investigation at planning time

Phases with standard patterns (skip research):
- **Phase 1 (Zeroization):** Well-documented zeroize 1.8.2 patterns; call sites enumerated from codebase inspection
- **Phase 2 (Crypto Layer):** Directly adapts existing `pin_derive_key`/`pin_encrypt` pattern; binary format fully specified in ARCHITECTURE.md
- **Phase 3 (Storage Layer):** All edge cases enumerated in PITFALLS.md checklist; atomic write pattern already exists in the codebase

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified live via crates.io API and Cargo.lock inspection; pkarr 5.0.3 source read directly from registry cache; `zeroize 1.8.2` and `secrecy 0.10.3` confirmed in Cargo.lock as transitive deps |
| Features | HIGH | Codebase inspected directly for current state; feature scope tightly bounded; passphrase UX patterns validated against SSH/GPG/age official documentation and NIST 800-63B |
| Architecture | HIGH | All modified files read directly; `cargo tree` output verified; pkarr `keys.rs` inspected in registry cache confirming format constraints; file format specified in complete detail |
| Pitfalls | HIGH | All pitfalls verified against live codebase; pkarr source confirms hex-only format; zeroize docs confirm copy semantics limitations; Rust tracking issue #82590 confirms `fs::write` non-atomicity |

**Overall confidence:** HIGH

### Gaps to Address

- **`pkarr::Keypair::from_secret_key` API signature:** Architecture.md assumes this function exists as a public API in pkarr 5.0.3 and accepts `&[u8; 32]`. Confirm the exact signature during Phase 2 implementation. If the API differs (e.g., different input type), the crypto layer's reconstruction step will need adjustment — but the overall approach (own the I/O, bypass pkarr file functions) remains correct.
- **Non-interactive terminal detection in piped contexts:** The proposed guard `std::io::stdin().is_terminal()` before the passphrase prompt correctly handles interactive vs. headless. Validate behavior during Phase 3 integration testing with `cclink publish < /dev/null` and similar piped invocations to confirm the guard fires and the error message is clear.
- **keyring 4.x readiness at Phase 4 planning time:** keyring 4.0.0-rc.3 was released 2026-02-01. By the time Phase 4 is planned, 4.x may be stable. The 4.x API restructures backend selection; evaluate the upgrade path at Phase 4 planning rather than pre-committing to 3.6.3.

---

## Sources

### Primary (HIGH confidence)

- `crates.io/api/v1/crates/zeroize` — version 1.8.2 confirmed current, published 2025-09-29
- `~/.cargo/registry/src/.../zeroize-1.8.2/Cargo.toml` — features `derive = ["zeroize_derive"]` verified directly
- `cclink/Cargo.lock` — `zeroize 1.8.2`, `zeroize_derive 1.4.3`, `secrecy 0.10.3` confirmed as transitive deps
- `crates.io/api/v1/crates/keyring` — 3.6.3 latest stable (2025-07-27); 4.0.0-rc.3 latest pre-release (2026-02-01)
- `~/.cargo/registry/src/.../pkarr-5.0.3/src/keys.rs` — `write_secret_key_file` writes 64-char hex; `from_secret_key_file` reads hex; `InvalidData` on non-hex input — read directly
- `cclink/src/crypto/mod.rs`, `src/keys/store.rs`, `src/commands/init.rs`, `src/commands/publish.rs`, `Cargo.toml` — v1.2 codebase direct inspection confirming current state
- docs.rs/zeroize/1.8.2 — `Zeroizing<T>` wrapper, `ZeroizeOnDrop`, documented stack copy limitations
- docs.rs/pkarr/5.0.3 — `write_secret_key_file`, `from_secret_key_file` API confirmed
- docs.rs/keyring/3.6.3 — macOS Keychain, DBus Secret Service, Windows Credential Manager API confirmed
- NIST SP 800-63B — passphrase minimum length standards
- C2SP age format spec — scrypt passphrase stanza format

### Secondary (MEDIUM confidence)

- keyring-rs GitHub issue #133 — confirmed no automatic file-based fallback for headless Linux; maintainer explicit
- age GitHub discussion #256 — passphrase UX for scripting; env-var approach noted by maintainer
- kerkour.com Rust file encryption with Argon2 — binary header format pattern with magic, version, salt
- benma.github.io — "A pitfall of Rust's move/copy/drop semantics and zeroing data" — stack copy limitations

### Tertiary (LOW confidence)

- None. All findings validated by direct source inspection or official documentation.

---
*Research completed: 2026-02-24*
*Ready for roadmap: yes*
