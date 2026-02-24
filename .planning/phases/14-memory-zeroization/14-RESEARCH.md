# Phase 14: Memory Zeroization - Research

**Researched:** 2026-02-24
**Domain:** Rust memory zeroization, zeroize crate, sensitive key material lifecycle
**Confidence:** HIGH

## Summary

Phase 14 applies the `zeroize` crate's `Zeroizing<T>` wrapper to three categories of sensitive material: the X25519 secret scalar derived in `ed25519_to_x25519_secret`, the raw decrypted key file bytes in `load_keypair`, and the PIN/passphrase strings returned by `dialoguer::Password::interact`. In all three cases the fix is a targeted type change at the production site — no new modules, no architecture changes.

The `zeroize` crate (v1.8.2) is already a transitive dependency of the workspace pulled in by `pkarr`, `ed25519-dalek`, and others. There is no version conflict to resolve and no new `Cargo.toml` entry required beyond adding `zeroize` as a direct dependency to make the import explicit and stable. `Zeroizing<Z>` impls `Deref` and `DerefMut` so all downstream call sites that take `&[u8; 32]` or `&str` continue to work with zero or minimal change via auto-deref.

The key insight for ZERO-02 is that `pkarr::Keypair::from_secret_key_file` internally builds a non-zeroized `Vec<u8>` of raw secret bytes before constructing the `Keypair`. Since cclink owns `load_keypair` in `src/keys/store.rs`, the fix is to replace the call to pkarr's file reader with an inline reimplementation that uses `Zeroizing<Vec<u8>>` for the raw bytes and `Zeroizing<[u8; 32]>` for the decoded seed, then calls `pkarr::Keypair::from_secret_key(&seed)` and drops the zeroized buffers immediately.

**Primary recommendation:** Add `zeroize = "1"` as a direct dependency, change `ed25519_to_x25519_secret` return type to `Zeroizing<[u8; 32]>`, replace pkarr's file reader in `load_keypair` with an inline zeroizing reimplementation, and wrap all `dialoguer::Password::interact` results in `Zeroizing::new(...)` at the call site.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ZERO-01 | Derived X25519 secret scalar is zeroized from memory after use | `ed25519_to_x25519_secret` return type changed to `Zeroizing<[u8;32]>`; all 4 call sites auto-deref to `&[u8;32]` for downstream functions |
| ZERO-02 | Decrypted key file bytes are zeroized from memory after parsing | `load_keypair` reimplemented to read+hex-decode with `Zeroizing` buffers instead of calling pkarr's `from_secret_key_file` |
| ZERO-03 | Passphrase and PIN strings from user prompts are zeroized after use | `dialoguer::Password::interact()` returns plain `String`; callers must wrap result in `Zeroizing::new(pin)` immediately |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| zeroize | 1.8.2 (already transitive) | `Zeroizing<T>` wrapper, ensures memory is wiped on drop using `write_volatile` + atomic fences | RustCrypto standard; used by ed25519-dalek, curve25519-dalek, age, and every serious crypto crate in the ecosystem |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none new) | — | — | All required types (`Zeroizing<String>`, `Zeroizing<[u8;32]>`, `Zeroizing<Vec<u8>>`) come from the single `zeroize` crate |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `Zeroizing<T>` wrapper | Manual `ptr::write_volatile` loops | No: `Zeroizing` is the right abstraction; hand-rolling misses the compiler fence, is harder to audit |
| Wrapping return type | Adding `.zeroize()` call at each use site | No: manual call is error-prone (easy to forget on early-return paths); RAII wrapper is the correct pattern |

**Installation:**
```toml
# Cargo.toml [dependencies] — make zeroize a direct dep for explicit import
zeroize = "1"
```

## Architecture Patterns

### Recommended Project Structure

No structural changes. All changes are within existing files:
```
src/
├── crypto/mod.rs        # Change ed25519_to_x25519_secret return type; add Zeroizing to pin_derive_key intermediates
├── keys/store.rs        # Replace load_keypair body with zeroizing reimplementation
└── commands/
    ├── publish.rs       # Wrap PIN string in Zeroizing at prompt site
    └── pickup.rs        # Wrap PIN string in Zeroizing at prompt site
```

### Pattern 1: Zeroizing Return Value (ZERO-01)

**What:** Change function return type from `[u8; 32]` to `Zeroizing<[u8; 32]>`. All callers that take `&[u8; 32]` continue to compile via `Deref<Target=[u8;32]>`.

**When to use:** When the function is the single production site for a secret value and all callers consume it by reference.

**Example:**
```rust
// Source: zeroize 1.8.2 docs + codebase pattern
use zeroize::Zeroizing;

pub fn ed25519_to_x25519_secret(keypair: &pkarr::Keypair) -> Zeroizing<[u8; 32]> {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&keypair.secret_key());
    Zeroizing::new(signing_key.to_scalar_bytes())
}

// Callers need zero changes — auto-deref provides &[u8; 32]:
let x25519_secret = crate::crypto::ed25519_to_x25519_secret(&keypair);
let identity = crate::crypto::age_identity(&x25519_secret);  // takes &[u8;32], auto-derefs
```

### Pattern 2: Zeroizing Intermediate Buffers (ZERO-02)

**What:** In `load_keypair`, bypass pkarr's `from_secret_key_file` (which does not zeroize its intermediate `Vec<u8>`) and re-implement the read+decode inline using `Zeroizing` wrappers for both the hex string and the decoded seed bytes.

**When to use:** When a third-party function internally allocates sensitive bytes without zeroizing them, and you own the call site.

**Example:**
```rust
// Source: pkarr 5.0.3 src/keys.rs (read, then reimplemented with zeroize)
use zeroize::Zeroizing;

pub fn load_keypair() -> anyhow::Result<pkarr::Keypair> {
    let path = secret_key_path()?;
    if !path.exists() {
        return Err(CclinkError::NoKeypairFound.into());
    }
    check_key_permissions(&path)?;

    // Read hex file; wrap in Zeroizing so the hex string is wiped on drop
    let hex_string = Zeroizing::new(
        std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read key file: {}", e))?
    );
    let hex_string = hex_string.trim();

    // Decode hex into raw seed bytes, wrapped in Zeroizing
    let mut seed = Zeroizing::new([0u8; 32]);
    hex::decode_to_slice(hex_string, seed.as_mut())
        .map_err(|_| anyhow::anyhow!("Invalid secret key file: not valid 64-char hex"))?;

    Ok(pkarr::Keypair::from_secret_key(&seed))
    // `seed` (Zeroizing<[u8;32]>) and `hex_string` (Zeroizing<String>) drop here and are zeroed
}
```

**Note on `hex` crate:** The codebase doesn't currently use the `hex` crate for hex decoding (pkarr does it manually). You can either add `hex = "0.4"` or reimplement the manual byte-by-byte decode into the `Zeroizing<[u8;32]>` buffer directly, avoiding the intermediate `Vec<u8>`. The manual decode into a fixed array is actually preferable here because it avoids a heap allocation entirely.

### Pattern 3: Zeroizing User-Supplied Strings (ZERO-03)

**What:** `dialoguer::Password::interact()` returns `Result<String>`. The internal `Zeroizing` it uses during confirmation matching is an implementation detail — the value returned to the caller is a plain `String`. Wrap the result immediately at the call site.

**When to use:** Immediately at every `dialoguer::Password` call site.

**Example:**
```rust
// Source: dialoguer 0.12.0 src/prompts/password.rs confirms internal Zeroizing is not propagated
use zeroize::Zeroizing;

let pin = Zeroizing::new(
    dialoguer::Password::new()
        .with_prompt("Enter PIN for this handoff")
        .with_confirmation("Confirm PIN", "PINs don't match")
        .interact()
        .map_err(|e| anyhow::anyhow!("PIN prompt failed: {}", e))?
);

// Use pin as &str via Deref:
if let Err(reason) = validate_pin(&pin) { ... }
let (ciphertext, salt) = crate::crypto::pin_encrypt(&payload_bytes, &pin)?;
// pin drops at end of block and is zeroed
```

### Pattern 4: Zeroizing Internal Crypto Intermediates (ZERO-01 extension)

**What:** Inside `pin_derive_key`, the `argon2_output` and `okm` arrays hold derived key material and should also be zeroized. Change them to `Zeroizing<[u8; 32]>`.

**Example:**
```rust
pub fn pin_derive_key(pin: &str, salt: &[u8; 32]) -> anyhow::Result<Zeroizing<[u8; 32]>> {
    let params = Params::new(65536, 3, 1, Some(32))
        .map_err(|e| anyhow::anyhow!("argon2 params error: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut argon2_output = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(pin.as_bytes(), salt, argon2_output.as_mut())
        .map_err(|e| anyhow::anyhow!("argon2 hash error: {}", e))?;

    let hkdf = Hkdf::<Sha256>::new(None, argon2_output.as_ref());
    let mut okm = Zeroizing::new([0u8; 32]);
    hkdf.expand(b"cclink-pin-v1", okm.as_mut())
        .map_err(|e| anyhow::anyhow!("hkdf expand error: {}", e))?;

    Ok(okm)
}
```

### Anti-Patterns to Avoid

- **Calling `.zeroize()` manually instead of using `Zeroizing<T>`:** Manual calls are easy to miss on early-return paths or when a refactor adds a new return. Use RAII via `Zeroizing<T>` instead.
- **Shadowing the pin variable without re-wrapping:** `let pin = pin.trim().to_string()` after unwrapping the `Zeroizing` drops the protection. Always keep sensitive material in the `Zeroizing` wrapper.
- **Storing in a field longer than necessary:** Do not store `Zeroizing<String>` in a struct that lives beyond the scope of use. The goal is to drop as soon as the value is consumed.
- **Assuming dialoguer zeroizes for you:** The `dialoguer::Password` source confirms the internal `Zeroizing` wrapping is local to confirmation matching; `interact()` returns a plain `String`. This is a known and confirmed gotcha.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Secure memory zeroing | Custom `memset` / `ptr::write_bytes` loops | `zeroize::Zeroizing<T>` | Compiler can optimize away writes to memory it proves is unused; `write_volatile` + fences prevent this |
| Hex decode with zeroization | Custom hex decoder that zeroizes | Decode directly into `Zeroizing<[u8;32]>` using pkarr's existing manual decode pattern, or add `hex` crate | A small manual decode into a fixed buffer is acceptable here; no large custom utility needed |

**Key insight:** The `zeroize` crate solves the optimizer problem that makes naive zeroing insecure. Custom solutions almost always miss the `core::sync::atomic::compiler_fence` that prevents the optimizer from treating dead-store elimination as valid.

## Common Pitfalls

### Pitfall 1: dialoguer Returns Plain String
**What goes wrong:** Developer assumes `dialoguer::Password::interact()` returns `Zeroizing<String>` because the dialoguer source uses `Zeroizing` internally. The function actually returns `Result<String>`.
**Why it happens:** The internal `Zeroizing` wrapper in dialoguer's confirmation loop is dropped before the return value is cloned out — `return Ok((*password).clone())` at line ~50 in dialoguer's `interact_on`.
**How to avoid:** Always wrap `interact()` results in `Zeroizing::new(...)` immediately at the call site, before any use.
**Warning signs:** A review of `src/commands/publish.rs` and `src/commands/pickup.rs` shows bare `let pin = dialoguer::Password::new()...interact()?` — both need wrapping.

### Pitfall 2: Hex-Decode Intermediate Vec
**What goes wrong:** Reimplementing `load_keypair` with `let mut bytes = vec![]; ...bytes.push(byte); ...bytes.try_into()` — this Vec is not zeroized before drop.
**Why it happens:** Pattern copied from pkarr's own `from_secret_key_file` which does the same thing.
**How to avoid:** Decode directly into a `Zeroizing<[u8;32]>` array using index writes, or use `hex::decode_to_slice` with the `hex` crate.
**Warning signs:** Any `Vec<u8>` allocation in the new `load_keypair` body that holds secret bytes.

### Pitfall 3: Function Signature Change Breaks Callers
**What goes wrong:** Changing `ed25519_to_x25519_secret` to return `Zeroizing<[u8;32]>` causes compile errors at call sites that try to pass the value where `[u8;32]` is expected by value (not by reference).
**Why it happens:** `Zeroizing<T>` derefs to `T` (giving `&T`) but does not automatically coerce to `T` by value.
**How to avoid:** All four call sites in `pickup.rs`, `revoke.rs`, `list.rs`, and `publish.rs` pass the result to `age_identity(&x25519_secret)` — the function already takes `&[u8;32]`, so `&*x25519_secret` (or just `&x25519_secret` via auto-deref) will work. Check all call sites at compile time.
**Warning signs:** Compiler errors mentioning "expected `[u8; 32]`, found `Zeroizing<[u8; 32]>`".

### Pitfall 4: Tests Fail Due to Type Change
**What goes wrong:** The existing unit tests for `ed25519_to_x25519_secret` do `assert_eq!(scalar1, scalar2)` — if return type changes to `Zeroizing<[u8;32]>`, the Deref means `*scalar1 == *scalar2` is still valid, but bare `scalar1 == scalar2` may require `Zeroizing` to impl `PartialEq` (it does).
**Why it happens:** `Zeroizing<Z: Zeroize + Clone>` impls `Clone` and `Zeroizing<Z: Zeroize + PartialEq>` — actually check: `Zeroizing` impls `PartialEq` delegating to the inner value.
**How to avoid:** Tests should compile unchanged. Verify with `cargo test` after each change.
**Warning signs:** Compile errors in test code involving `==` on `Zeroizing` values.

### Pitfall 5: pin_derive_key / pin_encrypt / pin_decrypt Chain
**What goes wrong:** If `pin_derive_key` return type changes to `Zeroizing<[u8;32]>`, callers `pin_encrypt` and `pin_decrypt` that call `age_identity(&derived_key)` must use `&*derived_key` or just `&derived_key` (auto-deref). This is safe but must be verified at compile time.
**Why it happens:** The function chain is three levels deep; the type change propagates.
**How to avoid:** Change `pin_derive_key` return type and let the compiler find the affected sites.

## Code Examples

Verified patterns from official sources:

### Adding zeroize as a direct dependency
```toml
# Cargo.toml
[dependencies]
zeroize = "1"
```
This resolves to 1.8.2 (already in the lock file as a transitive dep). No conflict.

### Zeroizing<[u8;32]> as a return type
```rust
// Source: zeroize 1.8.2 docs — Zeroizing::new wraps any Zeroize-implementing type
use zeroize::Zeroizing;

pub fn ed25519_to_x25519_secret(keypair: &pkarr::Keypair) -> Zeroizing<[u8; 32]> {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&keypair.secret_key());
    Zeroizing::new(signing_key.to_scalar_bytes())
}
```

### Zeroizing<String> at dialoguer call site
```rust
// Source: dialoguer 0.12.0 — interact() returns Result<String>, not Zeroizing<String>
use zeroize::Zeroizing;

let pin = Zeroizing::new(
    dialoguer::Password::new()
        .with_prompt("Enter PIN for this handoff")
        .with_confirmation("Confirm PIN", "PINs don't match")
        .interact()
        .map_err(|e| anyhow::anyhow!("PIN prompt failed: {}", e))?
);
// &*pin coerces to &str for validate_pin and pin_encrypt
```

### load_keypair with zeroizing hex decode (no heap allocation for secret bytes)
```rust
// Source: pkarr 5.0.3 from_secret_key_file pattern, reimplemented with Zeroizing
use zeroize::Zeroizing;

pub fn load_keypair() -> anyhow::Result<pkarr::Keypair> {
    let path = secret_key_path()?;
    if !path.exists() {
        return Err(CclinkError::NoKeypairFound.into());
    }
    check_key_permissions(&path)?;

    let hex_string = Zeroizing::new(
        std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read key file: {}", path.display()))?
    );
    let hex_trimmed = hex_string.trim();

    if hex_trimmed.len() != 64 {
        anyhow::bail!("Invalid secret key file: expected 64 hex chars, got {}", hex_trimmed.len());
    }

    let mut seed = Zeroizing::new([0u8; 32]);
    for i in 0..32 {
        let byte_str = &hex_trimmed[i * 2..i * 2 + 2];
        seed[i] = u8::from_str_radix(byte_str, 16)
            .map_err(|_| anyhow::anyhow!("Invalid hex in secret key file at byte {}", i))?;
    }

    Ok(pkarr::Keypair::from_secret_key(&seed))
    // seed (Zeroizing<[u8;32]>) and hex_string (Zeroizing<String>) zeroed here on drop
}
```

### Verify zeroize implements PartialEq (for tests)
```rust
// zeroize 1.8.2 — Zeroizing<Z: Zeroize + PartialEq> implements PartialEq
// Existing tests: assert_eq!(scalar1, scalar2) will compile unchanged
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Return `[u8; 32]` from crypto functions | Return `Zeroizing<[u8; 32]>` | Phase 14 | Scalar bytes automatically zeroed when variable goes out of scope |
| Call pkarr's `from_secret_key_file` | Inline zeroizing reimplementation | Phase 14 | Removes the single unzeroized `Vec<u8>` allocation in the key load path |
| Plain `String` from dialoguer | `Zeroizing::new(dialoguer...interact()?)` | Phase 14 | PIN/passphrase zeroed from heap on drop |

**Deprecated/outdated:**
- Calling `pkarr::Keypair::from_secret_key_file` in `load_keypair`: replaced by inline zeroizing version. The pkarr version remains in the library but should not be called by cclink for the key load path.

## Open Questions

1. **Should `age_identity` accept `&Zeroizing<[u8;32]>` or `&[u8;32]`?**
   - What we know: `age_identity` takes `&[u8;32]`. `&Zeroizing<[u8;32]>` auto-derefs to `&[u8;32]`.
   - What's unclear: Whether changing the parameter type to `&Zeroizing<[u8;32]>` provides any additional safety guarantee (it does not — Deref gives the same safety).
   - Recommendation: Keep `age_identity` signature as `&[u8;32]`. Callers use auto-deref. No change needed to `age_identity` itself.

2. **Should `pin_derive_key` return `Zeroizing<[u8;32]>` or should the caller wrap?**
   - What we know: `pin_derive_key` is called by `pin_encrypt` and `pin_decrypt`, both of which pass the result to `age_identity`. The intermediates `argon2_output` and `okm` inside `pin_derive_key` are the critical secret bytes.
   - Recommendation: Change `pin_derive_key` to return `Zeroizing<[u8;32]>` and also wrap `argon2_output` internally. This zeroizes the intermediate hash output even in the error path.

3. **Does the `hex` crate need to be added?**
   - What we know: The project does not currently depend on `hex`. The manual byte-by-byte decode in the new `load_keypair` is ~8 lines and avoids adding a new dep.
   - Recommendation: Use the manual decode into `Zeroizing<[u8;32]>`. No new dependency needed. Matches pkarr's existing decode pattern except it targets a fixed array directly.

## Affected Call Sites Inventory

This is a complete map of every site that needs a change, to aid task planning.

### ZERO-01: X25519 scalar (ed25519_to_x25519_secret)

Change return type in:
- `src/crypto/mod.rs` — `ed25519_to_x25519_secret` function signature + body

Callers that auto-deref (no change needed):
- `src/commands/pickup.rs` — two sites (`x25519_secret` passed to `age_identity(&x25519_secret)`)
- `src/commands/revoke.rs` — one site
- `src/commands/list.rs` — one site
- `src/commands/publish.rs` — one site (in the non-PIN branch via `ed25519_to_x25519_public`, not `ed25519_to_x25519_secret`; but the scalar is NOT used in publish for non-PIN path — only `ed25519_to_x25519_public` is. Scalar IS used in the `test_age_encrypt_decrypt_round_trip` test via direct call.)

Tests in `src/crypto/mod.rs` that call `ed25519_to_x25519_secret`:
- `test_ed25519_to_x25519_secret_deterministic` — `assert_eq!(scalar1, scalar2)` still works (PartialEq delegated)
- `test_age_encrypt_decrypt_round_trip` — passes to `age_identity(&secret)` — auto-deref works
- `test_age_decrypt_wrong_key_fails` — same
- `test_recipient_from_z32_round_trip` — same

### ZERO-02: Key file bytes (load_keypair)

Change body in:
- `src/keys/store.rs` — `load_keypair` function

Tests that exercise `load_keypair` path:
- `src/keys/store.rs` — `test_write_keypair_atomic_sets_0600`, `test_enforce_permissions_*` — these call `write_keypair_atomic` and `check_key_permissions`, not `load_keypair` directly. The new load path should be covered by an integration-style unit test using `tempfile`.

### ZERO-03: PIN and passphrase strings

Change at prompt sites:
- `src/commands/publish.rs` — one `dialoguer::Password` site (PIN prompt for `--pin` flag)
- `src/commands/pickup.rs` — one `dialoguer::Password` site (PIN prompt for PIN-protected records)

Note: Passphrase prompts for keypair decryption (Phase 16) do not yet exist. Phase 14 only needs to cover the PIN sites that currently exist.

### ZERO-01 extension: Crypto intermediates in pin_derive_key

Change in:
- `src/crypto/mod.rs` — `pin_derive_key` body: `argon2_output` and `okm` become `Zeroizing<[u8;32]>`; return type becomes `Zeroizing<[u8;32]>`

Callers of `pin_derive_key`:
- `pin_encrypt` — `let derived_key = pin_derive_key(pin, &salt)?;` — passes to `age_identity(&derived_key)` — auto-deref works
- `pin_decrypt` — same

## Sources

### Primary (HIGH confidence)
- `~/.cargo/registry/src/.../zeroize-1.8.2/src/lib.rs` — Full source read; `Zeroizing<Z>` struct, `Deref`, `DerefMut`, `Drop` impls verified directly
- `~/.cargo/registry/src/.../zeroize-1.8.2/Cargo.toml` — Features confirmed: `alloc` (default), `derive`; version 1.8.2
- `~/.cargo/registry/src/.../dialoguer-0.12.0/src/prompts/password.rs` — Source read confirms `interact()` returns `Result<String>`; internal `Zeroizing` does not propagate to return value
- `~/.cargo/registry/src/.../pkarr-5.0.3/src/keys.rs` — `from_secret_key_file` source read; `Vec<u8>` allocation confirmed as non-zeroized
- `~/.cargo/registry/src/.../ed25519-dalek-3.0.0-pre.5/Cargo.toml.orig` — pkarr enables ed25519-dalek with `alloc` only (not `zeroize`); `SigningKey: ZeroizeOnDrop` is NOT active
- `cargo tree` output — `zeroize` v1.8.2 confirmed as transitive dependency, no version conflict
- docs.rs/zeroize/1.8.1 — `Zeroizing<String>` and `Zeroizing<[u8;32]>` confirmed supported
- `src/crypto/mod.rs`, `src/keys/store.rs`, `src/commands/publish.rs`, `src/commands/pickup.rs`, `src/commands/revoke.rs`, `src/commands/list.rs` — All call sites inventoried directly

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` — Out-of-scope note: "Zeroize `pkarr::Keypair` struct — ed25519_dalek::SigningKey already implements ZeroizeOnDrop internally." This note is technically correct only when the `zeroize` feature of ed25519-dalek is enabled. Since pkarr does not enable it, the `Keypair` struct is NOT auto-zeroized in this codebase. The out-of-scope note remains correct in spirit (we should not try to add ZeroizeOnDrop to pkarr::Keypair) but Phase 14 should NOT rely on it for correctness.

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zeroize 1.8.2 source read directly; no version ambiguity
- Architecture: HIGH — all source files read; exact change sites identified with line-level precision
- Pitfalls: HIGH — dialoguer source confirmed; pkarr source confirmed; no speculation

**Research date:** 2026-02-24
**Valid until:** 2026-06-01 (zeroize 1.x is very stable; pkarr 5.0.3 pin unlikely to change in the near term)
