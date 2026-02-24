# Feature Research

**Domain:** Encrypted key storage at rest and secure memory zeroization for a Rust CLI (cclink v1.3)
**Researched:** 2026-02-24
**Confidence:** HIGH — zeroize crate verified via docs.rs (1.8.2); pkarr Keypair file format verified via docs.rs (5.0.3); keyring-rs verified via docs.rs (3.6.3) and GitHub; age passphrase-encryption UX verified via official age spec (C2SP); NIST 800-63B sourced directly; SSH agent pattern from multiple official sources

---

## Context: This Is a Subsequent Milestone

v1.3 is not a greenfield build. The entire core product works: publish, pickup, list, revoke, share mode, burn-after-read, PIN-protected handoffs, DHT transport. This milestone addresses a single security gap: the Ed25519 secret key at `~/.pubky/secret_key` is currently stored as a plaintext hex string (0600 permissions but no encryption). If that file is exfiltrated, the attacker has the private key with no further work needed. The plan is:

1. **Encrypted key storage at rest** — wrap the existing hex key file in passphrase-derived encryption (Argon2id+HKDF already implemented for PIN handoffs; reuse the same pattern)
2. **Secure memory zeroization** — ensure the raw 32-byte secret key is zeroed from memory after use with the `zeroize` crate

The existing FEATURES.md files (v1.1, v1.2) cover the full product feature landscape and CI hardening. This document focuses exclusively on v1.3 additions.

---

## Current State (Confirmed by Codebase Inspection)

| Item | Current State |
|------|--------------|
| Secret key file format | Plaintext hex string at `~/.pubky/secret_key`, 0600 permissions, written by `pkarr::Keypair::write_secret_key_file()` |
| Key loading | `pkarr::Keypair::from_secret_key_file(&path)` — reads hex, no passphrase |
| Memory handling after load | `keypair` struct used across publish/pickup/list/revoke; not explicitly zeroed after use; Rust drops it when it goes out of scope but without volatile write |
| Argon2id+HKDF | Already implemented in `src/crypto/mod.rs` (`pin_derive_key`, `pin_encrypt`, `pin_decrypt`) for PIN-protected handoffs |
| System keystore | Not used; key lives entirely in `~/.pubky/secret_key` |
| `zeroize` crate | Not in `Cargo.toml` |
| Passphrase prompt tooling | `dialoguer::Password` already used for PIN flow |

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features a security CLI must have for the encrypted-at-rest promise to be credible. Missing any of these makes the "encrypted key" claim hollow.

| Feature | Why Expected | Complexity | Existing Code Dependency |
|---------|--------------|------------|--------------------------|
| Passphrase prompt on first init | Any key tool that encrypts at rest must prompt for passphrase during key generation (`cclink init`). Users expect "generate key -> choose passphrase -> key is protected." No prompt = key silently unencrypted. | LOW | `dialoguer::Password` already imported. Add validate_with() call matching PIN flow. Argon2id+HKDF (`pin_derive_key`) already in `src/crypto/mod.rs`. |
| Passphrase prompt on every key load | Every command that loads the keypair must prompt for the passphrase. The key is only decrypted in memory for the duration of the operation. Users of SSH, GPG, age all expect this pattern. | LOW | `store::load_keypair()` in `src/keys/store.rs` is the single call site. The prompt goes here. All five commands (publish, pickup, list, revoke, whoami) route through this function. |
| Clear "wrong passphrase" error | `age` or custom decryption errors on wrong passphrase must surface as a user-readable message ("Wrong passphrase. Try again."), not a raw crypto error. Re-prompt on failure (up to N attempts) is acceptable; immediately exit is also acceptable for a CLI. | LOW | age decryption returns `Err` on wrong key; map the error to a clear message. Configuring retry vs immediate exit is a UX decision (see below). |
| Encrypted key file format with salt | The stored key file must contain both the encrypted secret bytes and the KDF salt so decryption is self-contained. Format must be detectable (e.g., a header byte or magic prefix) to distinguish encrypted files from legacy plaintext hex files. | MEDIUM | New file format. pkarr's existing hex format is incompatible. New write function needed in `src/keys/store.rs`. Can reuse `pin_encrypt()` output format (age ciphertext + base64 salt) that already works for handoffs. |
| Salt stored alongside ciphertext | Same Argon2id+salt pattern used for PIN-protected handoffs must be applied here. Salt is random per key creation; must be stored with the encrypted key so the passphrase can re-derive the decryption key. | LOW | Directly reuses the `(ciphertext, salt)` return from `pin_encrypt()`. Store both in a structured format (JSON, or two-field custom binary). |
| Migration path for existing unencrypted keys | Users who installed v1.0–v1.2 have a plaintext hex key. On first run after v1.3 upgrade, the tool must detect the old format, prompt for a new passphrase, re-encrypt and write the new format. Silently breaking existing installs is unacceptable. | MEDIUM | Detection: check file header for magic bytes or attempt JSON parse to distinguish old (hex) from new (encrypted) format. The migration flow: load old key -> prompt for passphrase -> write encrypted version. One-time operation. |
| Zeroize raw secret key bytes after use | After the keypair is used to sign or derive an X25519 key, the raw 32-byte secret seed must be zeroed from memory with `zeroize::Zeroize`. Without this, the secret persists in process memory until the allocator reuses that page — visible to memory dumpers and crash reporters. | LOW | `zeroize` crate (v1.8.2) implements `Zeroize` on `[u8; 32]` (scalar arrays). The `Zeroizing<[u8; 32]>` wrapper zeroes on drop automatically. Wire into `ed25519_to_x25519_secret()` and any site that holds raw secret bytes. |
| 0600 permissions preserved on new encrypted file | The encrypted key file must still be written with 0600 permissions, same as the current plaintext file. The 0600 check in `load_keypair()` must continue to pass. | LOW | Existing `write_keypair_atomic()` already enforces 0600 after rename. Reuse the same write path. |

### Differentiators (Beyond Baseline)

Features that go beyond the minimum encrypted-at-rest implementation, adding meaningfully to security or UX.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| System keystore integration (macOS Keychain, Freedesktop Secret Service) | Store the passphrase (or the derived key) in the OS keychain so the user is not prompted every command. This is the SSH agent / GPG agent pattern: unlock once per session. The `keyring` crate (v3.6.3) provides a cross-platform API covering macOS Keychain, Windows Credential Store, and Linux DBus Secret Service. | HIGH | Significant complexity: Linux DBus is unreliable on headless systems (keyring-rs issue #133 confirms no automatic fallback). Would need: try keychain -> fall back to prompt. Gating factor: v1.3 scope says "key-at-rest first." System keystore is explicitly listed as an Active item in PROJECT.md. |
| Passphrase-change command (`cclink rekey`) | Users need a way to change their key passphrase without regenerating the key. Analogous to `ssh-keygen -p`. Without this, a compromised passphrase requires `cclink init --force` which discards the key entirely. | MEDIUM | New subcommand: load with old passphrase, prompt for new passphrase, re-encrypt with new passphrase, write. Reuses existing building blocks. Not in scope for v1.3 per PROJECT.md, but natural v1.3.x addition. |
| `--no-passphrase` flag for `cclink init` | Power users in automated environments (CI, scripts) may want to explicitly opt out of passphrase encryption. This must be an explicit opt-in choice that prints a warning, not a silent default. | LOW | Flag on `InitArgs`. Matches SSH keygen's "Enter passphrase (empty for no passphrase):" model — users deliberately leave it empty. |
| Passphrase caching in environment variable for scripting | Headless/CI environments that must run cclink without a terminal can provide the passphrase via `CCLINK_PASSPHRASE` env var, bypassing the interactive prompt. Must print a warning that env var passphrase is less secure. | LOW | Read `std::env::var("CCLINK_PASSPHRASE")` before prompting. Clear the string from env after read. Document security implications. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Encrypt the key file with the user's existing SSH key | "I already have SSH keys, why another passphrase?" | Cross-dependency on user's SSH key management. SSH keys can be rotated. Passphrase encryption is self-contained. | Use a dedicated cclink passphrase. Argon2id+HKDF already implemented for this purpose. |
| Store decrypted key in memory for entire session | "Prompt once per shell session, not every command" | Requires in-process credential caching (background daemon or env var). Significantly increases attack surface: memory dump reveals plaintext key throughout shell lifetime. | System keystore (keyring crate) handles this correctly with OS security guarantees. Do not implement in-process caching without keyring. |
| Passphrase confirmation on every pickup / list / revoke | "Confirm you really mean to do this" | Passphrase already serves as authentication. Double-prompting is friction without security benefit beyond the first decrypt. Annoying for `cclink list`. | One passphrase prompt per command is the correct baseline (same as SSH, GPG). System keychain obviates repeated prompts. |
| Stretch to 256 MB memory for Argon2id on key storage | "More memory = more secure" | 64 MB (existing PIN parameters: t=3, m=65536, p=1) is already overkill for offline brute force at 100 ms per attempt. Increasing to 256 MB stalls the CLI for 3–4 seconds on modest hardware with no material security improvement at the margins. | Keep existing Argon2id parameters (t=3, m=64MB, p=1) that are already in `pin_derive_key()`. Reuse unchanged. |
| Derive key-encryption key from the Ed25519 key itself | "No extra passphrase, use the key to protect itself" | Circular — if you already have the Ed25519 private key, you don't need to decrypt it. The encrypted file's protection comes entirely from the passphrase, not from the key. | Passphrase-based KDF is the correct model. |
| Binary-only encrypted file format (no human-readable fields) | "Pure binary is more secure" | Binary format is harder to inspect, migrate, and debug. The existing approach (base64 ciphertext + base64 salt in JSON) is already opaque from a crypto perspective. | Structured text format (JSON or TOML header) is correct: the ciphertext is opaque regardless of the container format. |
| Zeroize the entire `pkarr::Keypair` struct | "Zero everything" | `pkarr::Keypair` is not yours to implement `Zeroize` on. It uses `ed25519-dalek::SigningKey` which already implements `ZeroizeOnDrop` internally. The risk surface is the raw `[u8; 32]` bytes you derive into before passing to age. | Target `Zeroizing<[u8; 32]>` for the derived X25519 scalar (`ed25519_to_x25519_secret()` return value). The pkarr `Keypair` itself is handled by dalek's own zeroize. |

---

## Feature Dependencies

```
[Encrypted key file at rest]
    └──requires──> [Argon2id+HKDF key derivation]  (ALREADY EXISTS in src/crypto/mod.rs)
    └──requires──> [New file format with magic header + ciphertext + salt]
    └──requires──> [Migration: detect plaintext hex -> prompt -> re-encrypt]
    └──requires──> [Passphrase prompt on load in store::load_keypair()]
    └──requires──> [Passphrase prompt on init in store::write_keypair_atomic() or init.rs]

[Migration path (old hex -> new encrypted)]
    └──requires first──> [Format detection: magic header check or JSON parse attempt]
    └──requires──> [load old key via pkarr::Keypair::from_secret_key_file()]
    └──requires──> [prompt new passphrase via dialoguer::Password]
    └──requires──> [write new encrypted format via updated write_keypair_atomic()]
    └──one-time──> [delete or overwrite old plaintext file]

[Zeroize secret key bytes]
    └──requires──> [zeroize crate added to Cargo.toml]
    └──enhances──> [ed25519_to_x25519_secret() in src/crypto/mod.rs]
    └──no conflict with──> [pkarr::Keypair (dalek's SigningKey already ZeroizeOnDrop)]
    └──applies to──> [Zeroizing<[u8; 32]> wrapping derived X25519 scalar]
    └──applies to──> [Zeroizing<Vec<u8>> wrapping decrypted payload bytes where feasible]

[System keystore integration]
    └──requires first──> [Encrypted key at rest (the keychain stores the passphrase or derived key)]
    └──requires──> [keyring crate v3.6.3]
    └──requires──> [fallback to passphrase prompt when keychain unavailable]
    └──complex dependency──> [Linux DBus unreliable in headless/CI environments]

[--no-passphrase flag on cclink init]
    └──requires──> [InitArgs update in src/cli.rs]
    └──conflicts with──> [system keystore (nothing to store in keychain if no passphrase)]
    └──no conflict with──> [zeroize (still needed even without passphrase)]

[Passphrase prompt on load]
    └──must precede──> [any key use: publish, pickup, list, revoke, whoami]
    └──single call site──> [store::load_keypair() in src/keys/store.rs]
```

### Dependency Notes

- **Encrypted key at rest requires format change:** `pkarr::write_secret_key_file()` writes raw hex. The new format must be a custom file (JSON envelope or custom binary) with: magic header, base64 ciphertext, base64 salt. The pkarr API cannot be reused as-is for writing.
- **Migration is a hard requirement:** cclink already has ~30+ installed users who have plaintext key files. Silently breaking `cclink pickup` after upgrade is not acceptable.
- **Zeroize is independent of key encryption:** Even with encrypted storage, the decrypted key will be in memory during command execution. Zeroize addresses the in-memory window; encryption addresses the at-rest window. Both are needed.
- **System keystore is the highest complexity item:** Linux Secret Service via DBus has no fallback in keyring-rs for headless environments. Any implementation must gracefully degrade to passphrase prompt on any keychain failure. This is why it is sequenced after basic encrypted key storage, not before.
- **`pin_derive_key()` reuse:** The existing Argon2id+HKDF implementation (`src/crypto/mod.rs`) was built for PIN-protected handoffs but is fully applicable to key-at-rest encryption. The same function with a different HKDF info string (e.g., `"cclink-key-v1"` vs. `"cclink-pin-v1"`) domain-separates the two use cases.

---

## MVP Definition

### This Milestone: v1.3 (Key Security Hardening)

- [ ] **Encrypted key storage at rest** — passphrase-protected via Argon2id+HKDF (reuse `pin_derive_key()`), new JSON file format, 0600 permissions preserved. Prompt on `cclink init`. Prompt on every `load_keypair()`. Required: essential security feature; plaintext key file is the primary attack surface for device compromise scenarios.
- [ ] **Migration: detect and re-encrypt old plaintext key files** — detect hex format (no magic header), prompt for passphrase, write encrypted format in-place. Required: installed base has plaintext keys; upgrade must not break existing installations.
- [ ] **Secure zeroization of derived X25519 secret scalar** — wrap `ed25519_to_x25519_secret()` return in `Zeroizing<[u8; 32]>`. Add `zeroize = "1"` to Cargo.toml. Required: without zeroize, the decrypted secret persists in heap memory until the allocator reuses the page.
- [ ] **Fix QR code content when `--share` + `--qr` combined** — verify current behavior (QR encodes `cclink pickup <publisher-pubkey>`, which is actually correct for the recipient). If a genuine bug exists, fix it. Listed as Active in PROJECT.md.

### Add After Validation (not this milestone)

- [ ] **System keystore integration** — `keyring` crate, macOS Keychain + Freedesktop Secret Service + Windows Credential Store. Try keychain -> fall back to passphrase prompt. Blocked by: keyring-rs headless Linux behavior requires careful fallback. Listed as Active in PROJECT.md but highest complexity item in v1.3.
- [ ] **Passphrase-change command (`cclink rekey`)** — re-encrypt existing key with new passphrase. Natural follow-on once the encrypted format is stable.

### Future Consideration (v2+)

- [ ] **`CCLINK_PASSPHRASE` env var for CI/scripting** — document security implications, add warning on use. Defer until there is evidence of scripting use cases.
- [ ] **`--no-passphrase` flag on `cclink init`** — defer until encrypted storage is proven stable and user demand for scripting scenarios is confirmed.

---

## Feature Prioritization Matrix

| Feature | Security / User Value | Implementation Cost | Priority |
|---------|----------------------|---------------------|----------|
| Encrypted key file at rest | HIGH — closes the "exfiltrate the file" attack | MEDIUM — new file format, reuse existing crypto | P1 |
| Migration: detect + re-encrypt old keys | HIGH — without this, existing users lose access after upgrade | MEDIUM — format detection + one-time migration flow | P1 |
| Zeroize derived X25519 scalar | HIGH — closes in-memory secret exposure window | LOW — add crate, wrap one return value | P1 |
| Fix QR + share bug | LOW — edge case combination | LOW — verify first, fix if genuine | P1 (investigate scope) |
| System keystore integration | HIGH for UX (no-prompt-per-command) | HIGH — cross-platform, headless fallback required | P2 |
| `cclink rekey` command | MEDIUM — users need passphrase rotation without key change | MEDIUM — new subcommand, reuses crypto | P2 |
| `CCLINK_PASSPHRASE` env var | LOW for UX, MEDIUM for CI use cases | LOW — env read + warning | P3 |
| `--no-passphrase` on init | LOW — explicitly opting out of the security feature | LOW — flag on InitArgs | P3 |

**Priority key:**
- P1: Must have for v1.3 milestone
- P2: Should have, add when possible
- P3: Nice to have, future consideration

---

## UX Pattern Analysis: Passphrase-Protected Key Files

### Established Patterns from Reference Tools

| Tool | When Prompted | Caching | Behavior on Wrong Passphrase |
|------|--------------|---------|------------------------------|
| SSH (OpenSSH) | On every `ssh` invocation if no agent | ssh-agent caches until logout | "bad passphrase" + exit 1 |
| GPG | On every sign/decrypt if no gpg-agent | gpg-agent caches with timeout | "bad passphrase" + re-prompt |
| age identity files | On every decrypt of passphrase-encrypted identity | No built-in cache; relies on OS keychain | Error + exit |
| git credential stores | On first use, then cached | git-credential-store (file) or osxkeychain | Prompted again on cache miss |

**Recommended pattern for cclink:** Prompt on every command (no in-process cache). This is the SSH-without-agent model — simple, predictable, no daemon required. System keystore (Phase 2) handles caching with proper OS security guarantees. The prompt-every-time baseline is correct for v1.3.

### Expected Behaviors

**`cclink init` (new key generation):**
```
Generating new keypair...
Choose a passphrase to protect your secret key (min 8 chars): ****
Confirm passphrase: ****
Keypair generated successfully.
Public Key:  pk:...
Key file:    ~/.pubky/secret_key  [encrypted]
```

**`cclink` / any command after v1.3 on an encrypted key:**
```
Enter passphrase for ~/.pubky/secret_key: ****
Session: abc123 in /home/user/myproject
Published!
```

**`cclink` after v1.3 upgrade with a legacy plaintext key (migration):**
```
Key file ~/.pubky/secret_key is unencrypted. Choose a passphrase to protect it:
New passphrase (min 8 chars): ****
Confirm passphrase: ****
Key file re-encrypted. Future commands will require this passphrase.

Enter passphrase for ~/.pubky/secret_key: ****
Session: abc123 in /home/user/myproject
Published!
```

**Wrong passphrase:**
```
Enter passphrase for ~/.pubky/secret_key: ****
Error: Wrong passphrase.
```
Exit 1 immediately — no retry loop. (Same as `ssh -i key user@host` behavior: one attempt, fail cleanly.)

### Encrypted Key File Format

The new file format must be self-describing and distinct from the existing hex format.

**Proposed format** (JSON envelope, human-inspectable, matches existing pin_encrypt() output):
```json
{
  "v": 1,
  "alg": "argon2id-hkdf-age",
  "salt": "<base64-32-byte-random-salt>",
  "ct": "<base64-age-ciphertext-of-hex-encoded-secret-key>"
}
```

**Detection logic in `load_keypair()`:**
1. Read file as string.
2. Try JSON parse:
   - If JSON with `"v"` field → new encrypted format → decrypt with passphrase.
   - If JSON parse fails or no `"v"` field → old hex format → migration flow.
3. This avoids adding a separate magic byte — the JSON structure itself is the discriminator.

**Why age ciphertext for the key file (not raw ChaCha20-Poly1305):**
The project already has `pin_encrypt()` / `pin_decrypt()` working end-to-end with Argon2id+HKDF+age. Reusing the exact same crypto avoids introducing new cipher modes. The `"cclink-key-v1"` HKDF info string domain-separates key-at-rest from handoff PIN usage.

---

## Implementation Notes: Zeroization

### What to Zeroize

| Site | Type | Action |
|------|------|--------|
| `ed25519_to_x25519_secret()` return | `[u8; 32]` | Wrap in `Zeroizing<[u8; 32]>`; callers receive a `Zeroizing` that drops correctly |
| Decrypted key file bytes (passphrase flow) | `Vec<u8>` | Wrap in `Zeroizing<Vec<u8>>` after decryption, before parsing hex |
| Passphrase string from dialoguer | `String` | `Zeroizing<String>` wrapping |

### What NOT to Zeroize

| Site | Why Not |
|------|---------|
| `pkarr::Keypair` struct | `ed25519_dalek::SigningKey` already implements `ZeroizeOnDrop` internally. Rust drop handles it. |
| `age::x25519::Identity` | Not your struct to implement `Zeroize` on; it is a library type. Minimize the lifetime of this value instead. |
| DHT publish result bytes | Ciphertext, not secret material — no need to zeroize. |

### Zeroize Crate Integration

```toml
# Cargo.toml
zeroize = { version = "1", features = ["derive"] }
```

The `derive` feature enables `#[derive(Zeroize, ZeroizeOnDrop)]` for custom structs if needed. The base crate provides `Zeroizing<T>` wrapper and `.zeroize()` method for all scalar types including `[u8; 32]`.

```rust
// Example: zero X25519 secret after use
use zeroize::Zeroizing;

pub fn ed25519_to_x25519_secret(keypair: &pkarr::Keypair) -> Zeroizing<[u8; 32]> {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&keypair.secret_key());
    Zeroizing::new(signing_key.to_scalar_bytes())
}
```

Callers that currently do `let x25519_secret = ed25519_to_x25519_secret(&keypair)` continue to work; the `Zeroizing` wrapper derefs transparently. When `x25519_secret` drops, the 32 bytes are overwritten with zeros using `write_volatile`.

---

## Competitor Feature Analysis

Context: cclink is not competing with key management tools, but the encrypted-at-rest feature places it in the same UX space as SSH, GPG, and age for this specific feature.

| Feature | SSH (OpenSSH) | GPG | age | cclink v1.3 plan |
|---------|---------------|-----|-----|-----------------|
| Key encrypted at rest | Yes (PKCS#8 or OpenSSH format) | Yes (symmetric encryption, passphrase) | Optional (age passphrase recipient on identity file) | Yes (Argon2id+HKDF+age, new JSON format) |
| KDF for passphrase | bcrypt (OpenSSH), PBKDF2 (PKCS#8) | S2K (iterated-and-salted SHA-1 or SHA-256), Argon2 (GnuPG 2.3+) | scrypt (age passphrase stanza) | Argon2id (already implemented and tested) |
| Passphrase caching | ssh-agent | gpg-agent | No built-in (OS keychain via plugin) | System keystore (v1.3 optional / v1.3.x) |
| Migration from unencrypted | Manual `ssh-keygen -p` | Manual `gpg --passwd` | N/A | Automatic detection + one-time re-encrypt prompt |
| Memory zeroization | Yes (via OS primitives) | Yes | Yes (age zeroes key material on drop) | `zeroize` crate v1.8.2 |

---

## Sources

- [zeroize docs.rs (v1.8.2)](https://docs.rs/zeroize/latest/zeroize/) — Zeroize trait, ZeroizeOnDrop, Zeroizing wrapper, `[u8; 32]` support confirmed (HIGH confidence, official docs)
- [pkarr docs.rs (v5.0.3)](https://docs.rs/pkarr/5.0.3/pkarr/struct.Keypair.html) — `write_secret_key_file()` writes hex string; `from_secret_key_file()` reads hex; format confirmed (HIGH confidence, official docs)
- [keyring docs.rs (v3.6.3)](https://docs.rs/keyring/latest/keyring/index.html) — macOS Keychain, Windows Credential Store, Linux DBus Secret Service; cross-platform API (HIGH confidence, official docs)
- [keyring-rs GitHub issue #133](https://github.com/hwchen/keyring-rs/issues/133) — confirmed no automatic file-based fallback for headless Linux; maintainer explicit (MEDIUM confidence, GitHub issue)
- [age format spec (C2SP)](https://github.com/C2SP/C2SP/blob/main/age.md) — scrypt passphrase stanza is the only stanza type; format constraints confirmed (HIGH confidence, authoritative spec)
- [NIST SP 800-63B](https://pages.nist.gov/800-63-4/sp800-63b.html) — passphrase minimum length standards referenced (HIGH confidence, official NIST publication)
- [age GitHub discussion #256](https://github.com/FiloSottile/age/discussions/256) — passphrase UX intentionally disincentivized for scripting; env-var passphrase approach noted (MEDIUM confidence, official maintainer discussion)
- [GitHub Docs: SSH key passphrases](https://docs.github.com/en/authentication/connecting-to-github/working-with-ssh-key-passphrases) — SSH agent "unlock once per session" UX pattern (HIGH confidence, official docs)
- Live codebase inspection (`src/crypto/mod.rs`, `src/keys/store.rs`, `Cargo.toml`) — confirmed current plaintext hex format, existing Argon2id+HKDF implementation, `dialoguer::Password` already imported (HIGH confidence)

---

*Feature research for: cclink v1.3 — encrypted key storage at rest and secure memory zeroization*
*Researched: 2026-02-24*
