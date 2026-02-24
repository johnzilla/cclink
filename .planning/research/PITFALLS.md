# Pitfalls Research

**Domain:** Rust CLI — encrypted key storage at rest (passphrase + Argon2id+HKDF) and secure memory zeroization (zeroize crate)
**Researched:** 2026-02-24
**Confidence:** HIGH (verified against live codebase, cargo tree output, pkarr 5.0.3 source, and zeroize 1.8.2 documentation)

---

## Critical Pitfalls

### Pitfall 1: pkarr's Secret Key File Format Is Plaintext Hex — You Cannot Wrap It

**What goes wrong:**

The pkarr `Keypair::write_secret_key_file` writes a 64-character lowercase hex string (no newline, no header, no structure). `Keypair::from_secret_key_file` reads that same hex string back. This format is entirely determined by pkarr — cclink calls these functions directly today.

When adding encrypted key storage, the naive approach is to "encrypt the file pkarr writes." This is impossible because pkarr controls the write path (`write_secret_key_file`) and the read path (`from_secret_key_file`). Passing an encrypted blob through `from_secret_key_file` will cause a hex parse failure immediately.

The correct design requires cclink to own the storage layer entirely: read the raw seed bytes (`keypair.secret_key()` → `[u8; 32]`), encrypt those bytes, write the result in a new format cclink controls, and load by decrypting first and then constructing the keypair via `Keypair::from_secret_key(&seed)`. The pkarr file I/O functions must be bypassed for the encrypted path.

**Why it happens:**

Developers look at `write_keypair_atomic` in `src/keys/store.rs` and see it already calls `keypair.write_secret_key_file`. The instinct is to "add encryption around the file write." But pkarr owns both the serialization format and the I/O — there is no hook for "write encrypted bytes instead." The solution requires understanding that `keypair.secret_key()` returns the raw 32-byte seed that can be used as the actual secret to encrypt, and `Keypair::from_secret_key(&[u8; 32])` reconstructs it without touching the file.

**How to avoid:**

The encrypted key storage implementation must:
1. Extract the seed: `let seed: [u8; 32] = keypair.secret_key();`
2. Encrypt seed bytes with passphrase via Argon2id+HKDF (same pattern as existing `pin_derive_key` + `age_encrypt`).
3. Write a cclink-owned file format (not pkarr's hex format) containing: `[magic_bytes][version][salt][ciphertext]`.
4. On load: detect encrypted vs. plaintext by magic bytes, decrypt, call `Keypair::from_secret_key(&seed)`.

Never call `pkarr::Keypair::from_secret_key_file` on an encrypted file. Never call `keypair.write_secret_key_file` to produce a file that will be later re-read as encrypted.

**Warning signs:**

- Code that calls `keypair.write_secret_key_file` followed by encryption of the resulting file.
- Load code that calls `pkarr::Keypair::from_secret_key_file` without first checking the file header.
- Hex parse error at startup on an encrypted key file: `"Invalid hex string"` from pkarr internals.

**Phase to address:** Encrypted key storage phase — first implementation task before any other changes.

---

### Pitfall 2: No Magic Bytes / Version Header Means Encrypted and Plaintext Files Are Indistinguishable

**What goes wrong:**

The current `~/.pubky/secret_key` file contains a 64-char hex string. If an encrypted file is written in a format that happens to be the same length or similar byte pattern, the load code cannot tell which format it is looking at. Worse: existing users already have a plaintext key file at that path. Without a format discriminator, the load code cannot offer "try passphrase prompt" (encrypted) vs. "load directly" (plaintext) without guessing — and guessing wrong means either silently loading garbage or prompting for a passphrase that was never set.

This becomes a migration crisis: cclink 1.3 ships, users upgrade, the key file is still plaintext v1.2 format, and the new binary does not know what to do with it.

**Why it happens:**

Developers implementing "add passphrase encryption to key file" write a new encrypted blob to the same path without embedding a discriminating header. The reasoning is "we'll always require a passphrase from now on" — but users who run `cclink init` on v1.3 get encrypted format, while users upgrading from v1.2 have plaintext format. The binary must handle both.

**How to avoid:**

Define a compact magic bytes header for the encrypted format. Example:
- Plaintext format (existing): starts with a lowercase hex character (`0-9`, `a-f`) — the first byte is always ASCII `0x30`-`0x39` or `0x61`-`0x66`.
- Encrypted format (new): starts with a fixed 4-byte magic `CCLK` (`0x43 0x43 0x4C 0x4B`) followed by a version byte (`0x01`), then salt (32 bytes), then age ciphertext.

Load logic:
```rust
fn load_keypair_from_file(path: &Path) -> anyhow::Result<pkarr::Keypair> {
    let data = std::fs::read(path)?;
    if data.starts_with(b"CCLK") {
        // encrypted — prompt for passphrase, decrypt, reconstruct
    } else {
        // legacy plaintext hex — load via pkarr
        pkarr::Keypair::from_secret_key_file(path)
            .map_err(|e| anyhow::anyhow!("Failed to load keypair: {}", e))
    }
}
```

Version the format from day one. Version `0x01` is all you need for v1.3. When the KDF parameters change in a future version, increment the version byte rather than breaking old files.

**Warning signs:**

- Load code that does not branch on file content before deciding whether to decrypt.
- Users who upgraded from v1.2 report `"Failed to load keypair"` or silent passphrase prompts on a key that never had one.
- No test that reads a plaintext key file with the v1.3 binary and confirms it still works.

**Phase to address:** Encrypted key storage phase — format definition is prerequisite to any write/read implementation.

---

### Pitfall 3: Stack-Allocated `[u8; 32]` Seed Gets Moved (Copied) Before Zeroization

**What goes wrong:**

The Ed25519 seed — `keypair.secret_key()` → `[u8; 32]` — is `Copy`. Every time you pass this array to a function or return it from one, Rust performs a bitwise memory copy. The original stack location still contains the secret bytes. When you call `zeroize()` on the variable you hold, you zero one copy. The previous copies on the stack are not zeroed.

Example of the problem:
```rust
let seed: [u8; 32] = keypair.secret_key();   // copy #1: seed on stack frame A
let encrypted = encrypt_key_to_disk(&seed)?;  // copy #2: seed copied into frame B
seed.zeroize();  // zeros frame A only; frame B still dirty
```

This is documented explicitly by the zeroize crate: "moving a value compiles into a memory copy in the general case." The zeroize documentation cites this as a known limitation that "cannot be avoided without OS-level memory management primitives."

**Why it happens:**

The fix looks obvious — wrap the seed in `Zeroizing<[u8; 32]>` and it will auto-zeroize on drop. But `Zeroizing<T>` implements `Drop`, which zeroes the memory at the `Zeroizing` wrapper's current address. If the wrapper was moved (copied) from one stack frame to another, only the final frame's address is zeroed. Developers trust the `Zeroizing` wrapper to "make secrets safe" without realizing moves still produce copies.

**How to avoid:**

1. Use `Zeroizing<[u8; 32]>` as the type from the point of extraction through all use sites — this at minimum zeroes the last-known location reliably on drop, which is better than nothing.
2. Minimize moves: extract the seed, use it immediately for encryption in the same function scope, do not pass it through multiple function boundaries.
3. Accept the documented limitation: stack copies cannot be fully prevented in safe Rust without OS-level `mlock`. The zeroize crate's own documentation states register clearing and full stack hygiene are out of scope. The realistic goal is zeroing the primary allocation, not all possible copies.
4. For the passphrase (a `String` from dialoguer), wrap with `zeroize::Zeroizing<String>` or call `.zeroize()` immediately after use before the function returns.

The key is not to let the secret live any longer than needed: derive the encrypted output, then zeroize, do not store intermediate values in named variables that outlive their immediate use.

**Warning signs:**

- Seed bytes in a named `let seed = keypair.secret_key()` variable that is passed to multiple functions.
- A `String` passphrase from `dialoguer::Password::interact()` that is used but never explicitly zeroized.
- `Zeroizing<[u8; 32]>` returned from a function (causes a move/copy before the drop fires at the call site).

**Phase to address:** Zeroization phase — address after encrypted storage works, as part of a focused zeroization sweep.

---

### Pitfall 4: `dialoguer::Password` Returns a Plain `String` That Is Never Zeroized

**What goes wrong:**

`dialoguer::Password::interact()` returns `Result<String>`. A `String` in Rust holds its content in a heap-allocated buffer. When the `String` is dropped normally, Rust frees the allocator block but does not zero the bytes. The passphrase bytes remain readable in the process's memory (and in any core dump or memory snapshot) until the allocator reuses that page.

In the existing code for PIN mode (`src/commands/publish.rs` lines 154-158 and `src/commands/pickup.rs` lines 137-140), the PIN `String` is used and then dropped without explicit zeroization. Adding a similar pattern for a storage passphrase (which is more sensitive than a PIN, since it protects the long-term identity key) without fixing this leaves the passphrase in heap memory for the process lifetime.

**Why it happens:**

Rust's memory safety guarantees cover use-after-free and dangling pointers, not content confidentiality after drop. Developers see "it's dropped" and assume it is gone. The heap allocator may reuse the block quickly in practice, but there is no guarantee, and a memory dump taken at any point after prompt but before reuse will expose the passphrase.

**How to avoid:**

Wrap the passphrase immediately after collection:
```rust
use zeroize::Zeroizing;

let passphrase: Zeroizing<String> = Zeroizing::new(
    dialoguer::Password::new()
        .with_prompt("Enter passphrase for key decryption")
        .interact()
        .map_err(|e| anyhow::anyhow!("Passphrase prompt failed: {}", e))?
);
// passphrase.as_str() is accessible via Deref<Target = String>
// Dropped at end of scope: zeroize crate zeros the heap buffer.
```

`zeroize v1.8.2` is already a transitive dependency (verified by `cargo tree`). Adding it as a direct dependency costs nothing — it is already compiled. The `Zeroizing<T>` wrapper is the zero-cost abstraction for auto-zeroize on drop.

Apply the same fix to the existing PIN `String` in `publish.rs` and `pickup.rs` as part of the zeroization phase. The PIN has the same problem.

**Warning signs:**

- `dialoguer::Password::interact()?` assigned to a `String` variable without `Zeroizing::new(...)` wrapper.
- `use zeroize` not imported in `publish.rs` or `pickup.rs`.
- PIN prompt code in `pickup.rs` line 137-140 where the `pin` variable is `String`, not `Zeroizing<String>`.

**Phase to address:** Zeroization phase — fix existing PIN prompts at the same time as the new passphrase prompt.

---

### Pitfall 5: Passphrase-Encrypted Key File Written Non-Atomically Creates a Corrupt Key Window

**What goes wrong:**

`std::fs::write` is not atomic — it is documented as such in a Rust tracking issue. If the process is killed or the disk is full mid-write, the file at `~/.pubky/secret_key` can contain a partial ciphertext. On next startup, the load code reads a truncated blob, fails to decrypt, and reports a corrupted key file. The user's identity is inaccessible; their key is effectively lost.

This is a regression from the current behavior: `write_keypair_atomic` already does write-to-temp-then-rename to prevent this. Adding encrypted key writing must preserve this guarantee.

**Why it happens:**

The atomic write in `src/keys/store.rs` is implemented specifically for pkarr's hex format. When implementing a new encrypted write path, developers write a helper like `std::fs::write(path, encrypted_bytes)?` for simplicity, not realizing this abandons the atomic guarantee the codebase already has for the plaintext path.

**How to avoid:**

The encrypted write path must mirror `write_keypair_atomic` exactly:
1. Write ciphertext to `.secret_key.tmp` in the same directory.
2. `std::fs::rename(&tmp, &dest)` — atomic on POSIX (same filesystem).
3. `std::fs::set_permissions(&dest, Permissions::from_mode(0o600))` — maintain the 0600 guarantee.
4. On failure: attempt cleanup of `.secret_key.tmp`, return the error.

Also: the temp file itself must never have loose permissions during the write window. Set 0600 on the temp file before writing the ciphertext if the umask is not already restrictive. A simpler approach: set permissions on the temp path immediately after creation, before writing.

**Warning signs:**

- Any encrypted key write that uses `std::fs::write(path, data)` directly.
- No temp file in the encrypted write path.
- Loss of 0600 permission enforcement on the new format (test: `stat ~/.pubky/secret_key` after `cclink init --passphrase`).

**Phase to address:** Encrypted key storage phase — implement alongside the file format change, not after.

---

### Pitfall 6: `Vec<u8>` Heap Buffers for Ciphertext/Plaintext Are Not Zeroized After Use

**What goes wrong:**

The `age_encrypt`, `age_decrypt`, `pin_encrypt`, and `pin_decrypt` functions in `src/crypto/mod.rs` all return `Vec<u8>`. The decrypted seed bytes (plaintext from decryption) and the intermediate derived key bytes live in `Vec<u8>` allocations. When these `Vec`s are dropped normally, the heap pages are freed but not zeroed.

The `zeroize` crate documents this explicitly: "The Zeroize impls for Vec, String and CString zeroize the entire capacity of their backing buffer, but cannot guarantee copies of the data were not previously made by buffer reallocation." A `Vec<u8>` that grew via `push` or `extend` will have had its buffer reallocated at least once, leaving an unzeroed copy of the early bytes at the old allocation address.

**Why it happens:**

Decrypted plaintext feels like "output, not a secret" — developers focus on zeroing the key material that went in, not the decrypted result that came out. But the decrypted output of the key file is the raw seed bytes, which is the secret itself.

**How to avoid:**

Two layers of defense:

1. Wrap the decrypted seed `Vec<u8>` immediately in `Zeroizing<Vec<u8>>` so the buffer is zeroed on drop, even if reallocation copies cannot be recovered.
2. Pre-allocate `Vec<u8>` with the expected capacity to minimize reallocations: `Vec::with_capacity(32)` for the seed, `Vec::with_capacity(age_ciphertext_overhead + 32)` for ciphertext. Fewer reallocations means fewer unzeroed prior buffers.

For the passphrase derivation, the intermediate `argon2_output: [u8; 32]` and `okm: [u8; 32]` in `pin_derive_key` are stack arrays — wrap them in `Zeroizing` for auto-zero on drop.

**Warning signs:**

- Decrypted seed stored as a bare `Vec<u8>` that is passed through multiple function boundaries.
- `let plaintext = age_decrypt(...)?;` without `Zeroizing::new(...)`.
- Intermediate KDF arrays like `argon2_output` and `okm` that are named `let` bindings without `Zeroizing` wrapping.

**Phase to address:** Zeroization phase — comprehensive sweep across `crypto/mod.rs` and `keys/store.rs`.

---

### Pitfall 7: Argon2id Parameters Stored in Code, Not in the File Header — Future Parameter Changes Break Existing Keys

**What goes wrong:**

The current `pin_derive_key` hardcodes `t_cost=3, m_cost=65536, p_cost=1`. If the passphrase-based key derivation for the key file uses the same approach (parameters hardcoded in source), then any future change to the parameters (e.g., increasing m_cost for better security) silently changes the derived key. An existing encrypted key file encrypted with the old parameters will produce a different derived key with the new parameters, making the file undecryptable — even with the correct passphrase.

The user enters the correct passphrase, decryption fails, and they believe their key is corrupted or they mistyped. There is no way to recover without knowing which parameters were used.

**Why it happens:**

Parameters are hardcoded as constants because "we'll always use these parameters." The assumption is parameters never change. But security recommendations evolve: what is adequate in 2025 may be insufficient in 2027. Argon2id memory requirements are expected to increase as hardware improves.

**How to avoid:**

Store the Argon2id parameters in the encrypted key file header alongside the salt. The file format should include:
- `t_cost` (iteration count) as `u32` or `u8`
- `m_cost` (memory in KB) as `u32`
- `p_cost` (parallelism) as `u8`

On load, read the parameters from the header and pass them to the Argon2id constructor — never use hardcoded constants for decryption. On write, write the parameters used at the time of encryption. This allows future parameter upgrades: users with old files still decrypt successfully because the old parameters are in the header.

This is the same approach used by the Argon2 PHC string format and by age's passphrase mode.

**Warning signs:**

- `Params::new(65536, 3, 1, Some(32))` appears in both the encrypt and decrypt path with hardcoded values.
- No Argon2 parameters in the file format specification or written to the file.
- No migration test: "old parameters in file, new binary with different default parameters, should still decrypt."

**Phase to address:** Encrypted key storage phase — define the file format to include parameters before any code is written.

---

### Pitfall 8: `check_key_permissions` Rejects the Encrypted Key File at Load Time

**What goes wrong:**

`src/keys/store.rs:load_keypair()` calls `check_key_permissions(&path)` which verifies the file has exactly `0600` permissions. The encrypted key file will also live at `~/.pubky/secret_key`. The permission check runs before reading file content. This is correct for both formats — the check is format-agnostic and should pass.

However, the issue arises in `write_keypair_atomic`: it calls `keypair.write_secret_key_file(&tmp)` which uses pkarr's write, then renames. The new encrypted write path bypasses pkarr's write and uses raw `std::fs::write`. If the encrypted write path fails to call `set_permissions(dest, Permissions::from_mode(0o600))` after the rename, the file will have the process's umask permissions (often `0644`), and `check_key_permissions` will reject it on next startup — the user's key becomes immediately inaccessible.

**Why it happens:**

Pkarr's `write_secret_key_file` sets 0600 internally (verified in the pkarr 5.0.3 source). When cclink bypasses pkarr's write for the encrypted path, it also loses that implicit permission setting. The 0600 enforcement in `write_keypair_atomic` happens after the rename, but only if the code path reaches that line. An encrypted write helper that exits early on error after rename but before permission setting would leave a 0644 file.

**How to avoid:**

The encrypted write path must explicitly call `std::fs::set_permissions(dest, Permissions::from_mode(0o600))` as a required step, not an optional one. Set permissions in the `write_keypair_atomic` equivalent for encrypted keys, not inside the function that writes ciphertext. Test this: write an encrypted key file and `stat` it.

Update `write_keypair_atomic` to handle both plaintext (existing) and encrypted (new) write paths, or introduce a parallel `write_encrypted_keypair_atomic` that follows the same temp-rename-chmod sequence.

**Warning signs:**

- `stat ~/.pubky/secret_key` shows `0644` after `cclink init --passphrase`.
- `check_key_permissions` error at startup: "Key file has insecure permissions 0644."
- No test asserting 0600 permissions on the encrypted key file after write.

**Phase to address:** Encrypted key storage phase — permissions test is part of the write-path verification.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode Argon2 parameters (no header storage) | Simpler code | Future parameter change breaks all existing key files | Never — store params in file header from day one |
| Reuse `pin_derive_key` for key file derivation with same HKDF info tag | Reuses existing code | Domain separation break: same info string means same derivation for same PIN; semantically different contexts | Never — use a distinct info string like `"cclink-key-v1"` not `"cclink-pin-v1"` |
| Skip `Zeroizing` wrapper on passphrase string | Shorter code | Passphrase lives in heap after use; visible in core dumps | Never — zeroize is already a transitive dep, zero cost to add |
| No magic bytes header in encrypted file | Simpler format | Cannot distinguish encrypted from plaintext; breaks upgrade path for existing users | Never — header is 5 bytes, entirely worth it |
| Use `std::fs::write` for encrypted key file | Simpler implementation | Non-atomic write creates corrupt-key window; loss of 0600 guarantee | Never — always use temp-rename-chmod pattern |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| pkarr `Keypair` I/O functions | Calling `write_secret_key_file` / `from_secret_key_file` for encrypted format | Bypass pkarr I/O; use `keypair.secret_key()` and `Keypair::from_secret_key()` directly |
| `dialoguer::Password` | Using the returned `String` without `Zeroizing` wrapper | Wrap immediately: `Zeroizing::new(password_prompt.interact()?)` |
| Argon2id for key derivation vs. PIN derivation | Using the same `pin_derive_key` function (same HKDF info tag) | Write a separate `passphrase_derive_key` with info tag `"cclink-key-v1"` |
| `zeroize` as a dependency | Adding it as a new dependency | Already present as transitive dep v1.8.2; add as direct dep with same version constraint |
| `age_decrypt` output | Treating decrypted `Vec<u8>` as non-secret | Wrap in `Zeroizing<Vec<u8>>`; it contains the raw seed |
| Atomic write for encrypted format | Writing directly to the destination path | Write to `.secret_key.tmp`, rename, then set 0600 — identical to existing plaintext path |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Prompting for passphrase on every command (not just init/load) | User types passphrase more often → shoulder surfing, logs, history risk | Prompt only at key load time; cache the loaded `Keypair` in process memory for the command duration |
| Deriving the key file encryption key with the same HKDF info as PIN mode | Domain confusion: same passphrase used as PIN and as key passphrase produces same derived key | Use distinct info strings: `"cclink-key-v1"` vs. `"cclink-pin-v1"` |
| Logging or displaying passphrase in error messages | Passphrase visible in terminal history, log files | Never include passphrase in error strings; errors should say "passphrase incorrect" not repeat the passphrase |
| Not confirming passphrase on `cclink init` | Typo in passphrase locks user out of their key permanently | Require confirmation prompt on first set (write path); single prompt only on load (read path) |
| Skipping the permission check for the encrypted format | Encrypted key with 0644 is readable by other users on multi-user systems | Run `check_key_permissions` on the encrypted file path before decrypting |
| Accepting empty passphrase | Empty passphrase provides no protection; user may not realize it does nothing | Reject empty passphrase at the write path with a clear error |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| "Incorrect passphrase" vs. "Key file corrupt" — same error message | User cannot distinguish a typo from a corrupted file | Error for age decryption failure: "Incorrect passphrase" (expected fail); error for format parse failure: "Key file is corrupt or unrecognized format" |
| No escape hatch for users who forget passphrase | Key is permanently inaccessible; they must re-run `cclink init` and lose their identity | Document at `cclink init` time: "If you forget this passphrase, your key cannot be recovered. Store it securely." |
| Passphrase prompt with no context ("Enter passphrase:") | User unsure which passphrase is being asked for if they have many | Prompt: "Enter passphrase for cclink key (Argon2id, ~/.pubky/secret_key):" |
| Always prompting for passphrase even when key is unencrypted | v1.2 users who upgrade get unexpected passphrase prompts | Detect format by magic bytes; only prompt for passphrase if the file is encrypted format |
| Argon2id takes 200-500ms — no indication to user | User thinks the CLI is hung | Print "Deriving key (this takes a moment)..." before the Argon2 call, clear/overwrite with status after |

---

## "Looks Done But Isn't" Checklist

- [ ] **Encrypted format detected by magic bytes:** `cat ~/.pubky/secret_key | xxd | head -1` shows `43 43 4C 4B` (or chosen magic). If it starts with a hex char, the plaintext path is being written.
- [ ] **Plaintext v1.2 files still load in v1.3:** Run `cclink whoami` with a v1.2 key file — must work without passphrase prompt.
- [ ] **0600 permissions on encrypted key file:** `stat ~/.pubky/secret_key` after `cclink init --passphrase` shows `-rw-------`.
- [ ] **Atomic write for encrypted path:** Kill the process during key write (or simulate with a test); confirm the key file is either fully written or absent (not truncated).
- [ ] **Passphrase zeroized after use:** `Zeroizing<String>` wraps the result of `dialoguer::Password::interact()` in all three call sites (init, publish passphrase path, load).
- [ ] **Argon2 parameters in file header:** Decryption reads `t_cost`, `m_cost`, `p_cost` from the file, not from hardcoded constants.
- [ ] **Empty passphrase rejected at write:** `cclink init --passphrase` with empty input exits with error, not a silently unprotected key.
- [ ] **Passphrase confirmation at init:** `cclink init --passphrase` prompts twice and rejects mismatch.
- [ ] **Seed bytes zeroized after encryption:** The `[u8; 32]` seed extracted from the keypair is wrapped in `Zeroizing` and dropped before the function returns.
- [ ] **HKDF info strings differ:** The passphrase KDF uses `"cclink-key-v1"`, not `"cclink-pin-v1"` — grep confirms they are distinct.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Encrypted file written without magic bytes — old binary can't load | HIGH | Requires a one-time migration tool or manual re-init; no automatic recovery |
| Argon2 params not stored — parameter change breaks old key files | HIGH | Re-encrypt all key files with old params re-derived; requires knowing what the old params were |
| Non-atomic write causes partial key file | MEDIUM | If temp file remains: decrypt `.secret_key.tmp` with same passphrase; if gone: user must restore from backup |
| Wrong permissions (0644) after encrypted write | LOW | `chmod 600 ~/.pubky/secret_key`; update write path to set permissions correctly |
| Passphrase not zeroized — discoverable in memory dump | LOW (code fix) / HIGH (if already exploited) | Fix code to add `Zeroizing` wrapper; no recovery possible after a memory dump has occurred |
| pkarr `from_secret_key_file` called on encrypted blob | LOW | Fix load path to branch on magic bytes before calling pkarr I/O; re-test with plaintext and encrypted files |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| pkarr I/O functions bypass required | Encrypted key storage — file format design | `cargo test` with a test that writes encrypted key and reads it back via cclink (not pkarr functions) |
| No magic bytes — format detection broken | Encrypted key storage — file format design | Test: v1.2 plaintext file loads without passphrase prompt in v1.3 binary |
| Argon2 params not in file header | Encrypted key storage — file format design | Test: change default params in code, existing encrypted file still decrypts |
| Non-atomic write | Encrypted key storage — write path | Test: `stat` shows 0600; interrupt test confirms no partial file at destination |
| 0600 permissions not set on encrypted file | Encrypted key storage — write path | `stat ~/.pubky/secret_key` after init shows `-rw-------` |
| `check_key_permissions` rejects encrypted file | Encrypted key storage — load path | Test: write encrypted file, load it; permission check passes |
| Stack copies of seed | Zeroization — sweep phase | Code review: no `let seed = keypair.secret_key()` variable passed to multiple functions |
| `dialoguer::Password` returns unzeroized `String` | Zeroization — sweep phase | `grep -n "Password.*interact" src/` — all results show `Zeroizing::new(...)` wrapping |
| `Vec<u8>` decrypted seed not zeroized | Zeroization — sweep phase | `grep -n "age_decrypt\|pin_decrypt" src/` — all results assign to `Zeroizing<Vec<u8>>` |
| PIN prompts in existing code also not zeroized | Zeroization — sweep phase | Fix publish.rs and pickup.rs PIN prompts alongside new passphrase prompt |
| Wrong HKDF info string for key derivation | Encrypted key storage — crypto implementation | `grep "cclink-" src/crypto/mod.rs` shows two distinct info strings |
| No passphrase confirmation at init | Encrypted key storage — UX | Manual test: `cclink init --passphrase` prompts twice; mismatch shows error |

---

## Sources

- pkarr 5.0.3 source `keys.rs` — `write_secret_key_file` writes a 64-char lowercase hex string; `from_secret_key_file` reads and parses hex — verified via `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/pkarr-5.0.3/src/keys.rs`
- zeroize 1.8.2 documentation: documented limitations on Vec/String reallocation, stack copies, and register clearing — https://docs.rs/zeroize/latest/zeroize/
- "A pitfall of Rust's move/copy/drop semantics and zeroing data" — heap allocation prevents copies that stack allocation causes — https://benma.github.io/2020/10/16/rust-zeroize-move.html
- `dialoguer::Password::interact()` returns `Result<String>` (not a zeroizing wrapper) — https://docs.rs/dialoguer/latest/dialoguer/struct.Password.html
- `cargo tree | grep zeroize` on cclink v1.2 (2026-02-24): `zeroize v1.8.2` is already a transitive dep via `ed25519-dalek v3.0.0-pre.5` and pkarr — verified live
- `std::fs::write` is not atomic: Rust tracking issue #82590 — https://github.com/rust-lang/rust/issues/82590
- cclink `src/keys/store.rs` — existing `write_keypair_atomic` uses temp-rename-chmod pattern (verified by code read)
- cclink `src/crypto/mod.rs` — `pin_derive_key` uses HKDF info `"cclink-pin-v1"` (verified by code read); key derivation must use a distinct info string
- Argon2 PHC string format — parameters encoded in the hash string for future-proof verification — https://github.com/P-H-C/phc-string-format/blob/master/phc-sf-spec.md

---

*Pitfalls research for: Rust CLI — encrypted key storage at rest and memory zeroization (cclink v1.3)*
*Researched: 2026-02-24*
