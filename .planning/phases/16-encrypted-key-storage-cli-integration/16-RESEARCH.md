# Phase 16: Encrypted Key Storage and CLI Integration - Research

**Researched:** 2026-02-24
**Domain:** Rust CLI integration, binary file format detection, atomic file writes, passphrase prompts, backward-compatible key loading
**Confidence:** HIGH

## Summary

Phase 16 wires the `encrypt_key_envelope` / `decrypt_key_envelope` functions from Phase 15 into the full user-facing CLI flow. The work has two halves: (1) `cclink init` gains a passphrase prompt (and a `--no-passphrase` flag), and (2) `load_keypair` gains format detection so it transparently handles both the new `CCLINKEK` binary envelope files and the legacy plaintext hex files.

The crypto layer (`encrypt_key_envelope`, `decrypt_key_envelope`) is completely done — both functions exist in `src/crypto/mod.rs`, are tested with 8 unit tests, and are annotated `#[allow(dead_code)]` awaiting Phase 16 integration. No new crate dependencies are needed. `dialoguer::Password` (v0.12.0, already in `Cargo.toml`) handles passphrase prompts; it already performs its own `is_term()` check and returns an IO error on non-interactive stdin, so callers get the same non-interactive guard behavior that PIN prompts already use throughout the codebase.

The backward-compatibility requirement (success criterion 4) is the key design decision for `load_keypair`. A file starting with `CCLINKEK` (8 bytes) is a v1.3 encrypted envelope; any other content is treated as a v1.2 plaintext hex file and loaded with the existing path. Wrong-passphrase must produce the exact message "Wrong passphrase" and exit code 1 using `eprintln!` + `std::process::exit(1)` — the same pattern already used for PIN validation failure in `run_publish`.

The atomic write for the encrypted envelope cannot use `pkarr::Keypair::write_secret_key_file` (which writes hex). Instead, write raw bytes from `encrypt_key_envelope` to a temp file then rename — the same atomic rename pattern already in `write_keypair_atomic`. For the `--no-passphrase` path, `write_keypair_atomic` continues to work unchanged via `keypair.write_secret_key_file`.

**Primary recommendation:** Add `write_encrypted_keypair_atomic` to `store.rs` for the encrypted path, update `load_keypair` to detect CCLINKEK magic and branch, add `--no-passphrase` to `InitArgs`, update `run_init` to prompt for passphrase and call the right write function. All five success criteria are addressable with changes to three files: `src/cli.rs`, `src/commands/init.rs`, `src/keys/store.rs`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| KEYS-01 | User can create a passphrase-protected keypair with `cclink init` (passphrase prompt with confirmation, min 8 chars) | `dialoguer::Password::with_confirmation` already used for PIN prompts in `run_publish`; min 8 char validation is a new guard in `run_init`. |
| KEYS-02 | User can create an unprotected keypair with `cclink init --no-passphrase` | Add `--no-passphrase: bool` flag to `InitArgs` in `src/cli.rs`; `run_init` skips passphrase prompt and calls `write_keypair_atomic` (unchanged plaintext path). |
| KEYS-03 | User is prompted for passphrase when any command loads an encrypted keypair | `load_keypair` in `src/keys/store.rs` detects CCLINKEK magic, prompts via `dialoguer::Password::new()`, calls `decrypt_key_envelope`. All commands go through `load_keypair` — no per-command changes needed. |
| KEYS-04 | User sees clear "Wrong passphrase" error on incorrect passphrase (exit 1, no retry) | Map `decrypt_key_envelope` error to `eprintln!("Wrong passphrase")` + `std::process::exit(1)` inside `load_keypair`. This matches the PIN validation exit pattern in `run_publish`. |
| KEYS-06 | Encrypted key file preserves 0600 permissions | Write raw envelope bytes to temp file, then `set_permissions(0o600)` + `rename` — the same pattern as `write_keypair_atomic`. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| dialoguer | 0.12.0 | `Password::new().with_prompt(...).with_confirmation(...).interact()` for passphrase input | Already used for PIN prompt in `run_publish`; `interact()` returns `Err` when stdin is not a terminal (non-interactive guard built in) |
| zeroize | 1.8.2 | Wrap passphrase `String` in `Zeroizing<String>` to zero memory on drop | Project standard from Phase 14; used for all PIN strings |
| pkarr | 5.0.3 | `Keypair::from_secret_key(&[u8;32])` and `keypair.secret_key()` for the encrypted path | Same API confirmed in Phase 15 research; `secret_key() -> [u8;32]`, `from_secret_key(&[u8;32]) -> Keypair` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::process | stdlib | `std::process::exit(1)` for hard exit on wrong passphrase | Matches existing pattern in `run_publish` for PIN validation failure; avoids double-printing via anyhow formatter |
| std::io::IsTerminal | stdlib | `stdin().is_terminal()` guard before prompts in non-interactive contexts | Already used in `run_publish`, `run_pickup`, `run_revoke` |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `std::process::exit(1)` for wrong passphrase | `anyhow::bail!` and let main() print | `anyhow::bail!` causes the "Error: ..." prefix to be printed twice (once by anyhow, once by our message). `process::exit` prints exactly "Wrong passphrase" with no extra wrapping, consistent with PIN validation |
| Format-detection by magic bytes | File extension (`.enc`) or metadata file | Magic bytes are self-describing; no side files; instant detection without stat overhead |
| `write_keypair_atomic` for encrypted path | New separate function | `write_keypair_atomic` calls `pkarr::Keypair::write_secret_key_file` which writes hex. Encrypted envelopes are raw binary bytes — need a separate `write_encrypted_keypair_atomic(envelope: &[u8], dest: &Path)` |

**Installation:**
```bash
# No new dependencies required. All crates already in Cargo.toml.
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── cli.rs             # Add --no-passphrase flag to InitArgs
├── commands/init.rs   # Add passphrase prompt flow, call write_encrypted_keypair_atomic or write_keypair_atomic
└── keys/store.rs      # Add write_encrypted_keypair_atomic, update load_keypair for format detection
```

No new files. All changes in three existing files.

### Pattern 1: Format Detection in `load_keypair`

**What:** Read the first 8 bytes of the key file. If they match `b"CCLINKEK"`, load as encrypted envelope. Otherwise, load as v1.2 hex (existing code path).

**When to use:** Always — this is the backward-compatibility gate for KEYS-03 and success criterion 4.

**Why reading first 8 bytes works:** The hex path reads a 64-character ASCII hex string. The first 8 bytes of a hex string are hex digits (ASCII 0x30-0x39, 0x61-0x66). `CCLINKEK` is `0x43 0x43 0x4C 0x49 0x4E 0x4B 0x45 0x4B` — uppercase letters, which are NOT valid lowercase hex digits. There is no false-positive risk.

**Example:**
```rust
// Source: codebase pattern — src/keys/store.rs
pub fn load_keypair() -> anyhow::Result<pkarr::Keypair> {
    let path = secret_key_path()?;
    if !path.exists() {
        return Err(CclinkError::NoKeypairFound.into());
    }
    check_key_permissions(&path)?;

    // Read raw bytes to detect format
    let raw = std::fs::read(&path)
        .with_context(|| format!("Failed to read key file: {}", path.display()))?;

    if raw.starts_with(b"CCLINKEK") {
        // Encrypted envelope path (v1.3)
        load_encrypted_keypair(&raw)
    } else {
        // Plaintext hex path (v1.2 and earlier) — existing code unchanged
        load_plaintext_keypair(&raw)
    }
}
```

### Pattern 2: Passphrase Prompt and Decryption in `load_encrypted_keypair`

**What:** A private helper called by `load_keypair` for the encrypted branch. Prompts for passphrase, calls `decrypt_key_envelope`, maps wrong-passphrase error to exit(1).

**When to use:** When the file starts with `CCLINKEK` magic.

**Example:**
```rust
// Source: src/commands/publish.rs — PIN validation exit pattern
fn load_encrypted_keypair(envelope: &[u8]) -> anyhow::Result<pkarr::Keypair> {
    // Non-interactive guard: passphrase prompt requires a terminal
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Encrypted keypair requires interactive terminal for passphrase entry");
    }

    let passphrase = Zeroizing::new(
        dialoguer::Password::new()
            .with_prompt("Enter key passphrase")
            .interact()
            .map_err(|e| anyhow::anyhow!("Passphrase prompt failed: {}", e))?,
    );

    match crate::crypto::decrypt_key_envelope(envelope, &passphrase) {
        Ok(seed) => Ok(pkarr::Keypair::from_secret_key(&seed)),
        Err(_) => {
            eprintln!("Wrong passphrase");
            std::process::exit(1);
        }
    }
}
```

### Pattern 3: Atomic Write for Encrypted Envelope

**What:** Write the binary envelope bytes atomically: write to `.secret_key.tmp`, set 0600, rename to final path. If rename fails, clean up the temp file.

**When to use:** `cclink init` with passphrase (v1.3 encrypted path).

**Key difference from `write_keypair_atomic`:** That function calls `keypair.write_secret_key_file()` which writes hex. For the encrypted path, we write raw `Vec<u8>` bytes directly.

**Example:**
```rust
// Source: write_keypair_atomic pattern — src/keys/store.rs
pub fn write_encrypted_keypair_atomic(envelope: &[u8], dest: &Path) -> anyhow::Result<()> {
    let parent = dest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Key destination path has no parent directory"))?;

    let tmp = parent.join(".secret_key.tmp");

    std::fs::write(&tmp, envelope)
        .map_err(|e| CclinkError::AtomicWriteFailed(e))?;

    // Set 0600 before rename (temp file is also secret material)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set 0600 permissions on temp file"))?;
    }

    if let Err(e) = std::fs::rename(&tmp, dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(CclinkError::AtomicWriteFailed(e).into());
    }

    // Permissions already set on temp file; they survive the rename on POSIX.
    // Set again on dest for defense in depth (umask, cross-device rename edge cases).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set 0600 permissions on {}", dest.display()))?;
    }

    Ok(())
}
```

### Pattern 4: `cclink init` Passphrase Prompt Flow

**What:** After the overwrite guard, before writing the keypair, either prompt for passphrase (default) or skip it (`--no-passphrase`). The passphrase-protected path calls `encrypt_key_envelope` then `write_encrypted_keypair_atomic`. The `--no-passphrase` path calls the existing `write_keypair_atomic`.

**Key detail:** The passphrase must be at least 8 characters (KEYS-01). Use `dialoguer::Password::with_confirmation` to require the user to type it twice.

**Example:**
```rust
// Source: run_publish PIN prompt — src/commands/publish.rs
if args.no_passphrase {
    // Plaintext path (v1.2-compatible, --no-passphrase flag)
    store::write_keypair_atomic(&keypair, &secret_key_path)?;
} else {
    // Encrypted path (v1.3 default)
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Use --no-passphrase for non-interactive init");
    }
    let passphrase = Zeroizing::new(
        dialoguer::Password::new()
            .with_prompt("Enter key passphrase (min 8 chars)")
            .with_confirmation("Confirm passphrase", "Passphrases don't match")
            .interact()
            .map_err(|e| anyhow::anyhow!("Passphrase prompt failed: {}", e))?,
    );
    if passphrase.len() < 8 {
        eprintln!("Error: Passphrase must be at least 8 characters");
        std::process::exit(1);
    }
    let seed: [u8; 32] = keypair.secret_key();
    let envelope = crate::crypto::encrypt_key_envelope(&seed, &passphrase)?;
    store::write_encrypted_keypair_atomic(&envelope, &secret_key_path)?;
}
```

### Pattern 5: `--no-passphrase` CLI Flag

**What:** Add `no_passphrase: bool` to `InitArgs` in `src/cli.rs`.

**Example:**
```rust
// Source: src/cli.rs — InitArgs struct (existing)
#[derive(Parser)]
pub struct InitArgs {
    #[arg(long, value_name = "PATH")]
    pub import: Option<String>,

    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Write a plaintext (unencrypted) key file — skips passphrase prompt
    #[arg(long)]
    pub no_passphrase: bool,
}
```

### Anti-Patterns to Avoid

- **Prompting for passphrase inside `run_init` when `--import` is used:** The import path reads a v1.2 hex key. Whether to encrypt it on write is orthogonal to where the key comes from. The `--no-passphrase` flag determines whether to encrypt; `--import` just provides the source keypair. Both paths should go through the same encrypt/write logic.
- **Using `anyhow::bail!` for wrong passphrase:** This causes double-printing (`anyhow` adds "Error: " prefix, then the caller in `main()` also prints). Use `eprintln!("Wrong passphrase") + process::exit(1)`.
- **Not zeroizing the passphrase string:** Wrap the `dialoguer::Password::interact()` return value in `Zeroizing::new(...)` immediately. The `Zeroizing<String>` wraps the heap buffer and zeros it on drop.
- **Calling `write_keypair_atomic` for the encrypted path:** That function calls `pkarr::Keypair::write_secret_key_file` which writes hex — it will write the seed in plaintext, defeating the purpose of encryption.
- **Setting permissions only after rename:** A file can be stat'd between write and rename. Set permissions on the temp file BEFORE the rename to minimize the window.
- **Not guarding passphrase prompt for non-interactive terminal:** `dialoguer::Password::interact()` already returns `Err` if stdin is not a terminal (`term.is_term()` check inside dialoguer). But `load_keypair` should also check `stdin().is_terminal()` first and provide a clear error message (e.g., "Encrypted keypair requires interactive terminal") rather than letting the dialoguer error propagate as an obscure IO error.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Passphrase prompt with confirmation | Custom stdin read loop | `dialoguer::Password::with_confirmation` | Already in `Cargo.toml`, handles masking, mismatch retry, non-terminal detection, and `Zeroizing<String>` internally |
| Format detection | File extension, metadata file, separate version file | 8-byte magic prefix (`b"CCLINKEK"`) | Self-describing, instant, no extra I/O, no risk of file getting out of sync |
| Atomic file write | Non-atomic write with error | temp-file + rename pattern (already in `write_keypair_atomic`) | POSIX rename is atomic within same filesystem; in-progress writes leave no partial file |
| Passphrase strength validation | Custom regex rules | Simple `len >= 8` check | REQUIREMENTS.md KEYS-01 says "min 8 chars" only — no complexity requirement for the key passphrase (unlike PIN which has strength rules). Simpler is correct here. |
| Zeroizing passphrase | Manual `memset` | `Zeroizing<String>` | Project standard since Phase 14; auto-zeros heap buffer on drop; zero-cost deref |

**Key insight:** The only genuinely new code in Phase 16 is: (a) the `write_encrypted_keypair_atomic` function in `store.rs`, (b) the format-detection branch in `load_keypair`, and (c) the passphrase prompt in `run_init`. Everything else is wiring existing, already-tested components together.

## Common Pitfalls

### Pitfall 1: Wrong Exit Pattern for Wrong Passphrase
**What goes wrong:** Using `anyhow::bail!("Wrong passphrase")` causes the error to propagate through `main()`, which prints `"Error: Wrong passphrase\n"` — the prefix "Error: " is inconsistent with the required message "Wrong passphrase".
**Why it happens:** `anyhow::bail!` is the default Rust error propagation pattern; it's natural to use it.
**How to avoid:** Use `eprintln!("Wrong passphrase"); std::process::exit(1);` in `load_encrypted_keypair` — exactly the same pattern as PIN validation failure in `run_publish`.
**Warning signs:** A test or manual run showing `"Error: Wrong passphrase"` instead of `"Wrong passphrase"`.

### Pitfall 2: Reading Entire File Before Format Detection
**What goes wrong:** Reading the file into a `String` with `fs::read_to_string` (as the current v1.2 path does) will fail on binary CCLINKEK envelope data because the raw bytes are not valid UTF-8.
**Why it happens:** The current `load_keypair` uses `read_to_string` for the hex path. If format detection is done after reading as string, binary files will error before format detection happens.
**How to avoid:** Use `fs::read` (returns `Vec<u8>`) first, then detect format via `raw.starts_with(b"CCLINKEK")`, then either parse as encrypted envelope or convert to `String` for the hex path via `String::from_utf8`.

### Pitfall 3: import path ignores encryption
**What goes wrong:** `cclink init --import /path/to/key` reads a hex key file via `pkarr::Keypair::from_secret_key_file` and writes back via `write_keypair_atomic` (plaintext hex). After Phase 16, this silently writes an unencrypted key even when passphrase protection is the default.
**Why it happens:** The import path in `run_init` currently bypasses the passphrase prompt entirely.
**How to avoid:** After importing the keypair, apply the same encrypt/no-passphrase logic as the generated-keypair path. `--import` determines the *source* of the keypair; `--no-passphrase` determines whether to encrypt the *output*.

### Pitfall 4: Passphrase Prompt in non-interactive Piped Contexts
**What goes wrong:** `cclink publish < /dev/null` calls `load_keypair` which reaches the passphrase prompt, but stdin is not a terminal. `dialoguer::Password::interact()` will return `Err(IO error: not a terminal)`.
**Why it happens:** `load_keypair` is called unconditionally for every command. If stdin is piped, the dialoguer error is opaque to the user.
**How to avoid:** In `load_encrypted_keypair`, check `std::io::stdin().is_terminal()` before calling the dialoguer prompt. Return a clear error: `"Encrypted keypair requires interactive terminal for passphrase entry"`. STATE.md explicitly flags this: "Validate non-interactive terminal guard behavior with piped invocations (e.g., `cclink publish < /dev/null`) during integration testing."
**Warning signs:** Running `echo "" | cclink whoami` on an encrypted key file produces a confusing IO error rather than a clear "requires interactive terminal" message.

### Pitfall 5: Plaintextification via `write_keypair_atomic` in the Encrypted Init Path
**What goes wrong:** `run_init` mistakenly calls `store::write_keypair_atomic(&keypair, &dest)` for the encrypted path. `write_keypair_atomic` calls `keypair.write_secret_key_file()` which writes plaintext hex — the seed is never encrypted.
**Why it happens:** `write_keypair_atomic` is the existing function; it's easy to call it by mistake.
**How to avoid:** The encrypted path must use `write_encrypted_keypair_atomic(&envelope, &dest)` where `envelope` comes from `encrypt_key_envelope(&keypair.secret_key(), &passphrase)`. Never call `write_keypair_atomic` for the encrypted path.

### Pitfall 6: Missing Permission Test for Encrypted File
**What goes wrong:** The existing `test_write_keypair_atomic_sets_0600` test in `store.rs` tests the plaintext path only. After Phase 16 there is no test verifying that `write_encrypted_keypair_atomic` sets 0600.
**Why it happens:** Oversight during implementation.
**How to avoid:** Add `test_write_encrypted_keypair_atomic_sets_0600` following the same pattern as the existing permissions test in `store.rs`.

## Code Examples

Verified patterns from official sources:

### `dialoguer::Password` with confirmation (verified from dialoguer 0.12.0 source)
```rust
// Source: dialoguer-0.12.0/src/prompts/password.rs — interact() calls interact_on(&Term::stderr())
// interact_on returns Err if term.is_term() is false (non-interactive guard built in)
let passphrase = Zeroizing::new(
    dialoguer::Password::new()
        .with_prompt("Enter key passphrase (min 8 chars)")
        .with_confirmation("Confirm passphrase", "Passphrases don't match")
        .interact()
        .map_err(|e| anyhow::anyhow!("Passphrase prompt failed: {}", e))?,
);
```

### Wrong-passphrase exit pattern (verified from publish.rs existing pattern)
```rust
// Source: src/commands/publish.rs — PIN validation failure pattern
// Used identically for wrong passphrase to match success criterion 3
match crate::crypto::decrypt_key_envelope(envelope, &passphrase) {
    Ok(seed) => Ok(pkarr::Keypair::from_secret_key(&seed)),
    Err(_) => {
        eprintln!("Wrong passphrase");
        std::process::exit(1);
    }
}
```

### `encrypt_key_envelope` call site (verified from src/crypto/mod.rs Phase 15)
```rust
// Source: src/crypto/mod.rs — encrypt_key_envelope signature
// pub fn encrypt_key_envelope(seed: &[u8; 32], passphrase: &str) -> anyhow::Result<Vec<u8>>
// keypair.secret_key() -> [u8; 32]  (verified from pkarr-5.0.3/src/keys.rs)
let seed: [u8; 32] = keypair.secret_key();
let envelope = crate::crypto::encrypt_key_envelope(&seed, &passphrase)?;
```

### `decrypt_key_envelope` call site (verified from src/crypto/mod.rs Phase 15)
```rust
// Source: src/crypto/mod.rs — decrypt_key_envelope signature
// pub fn decrypt_key_envelope(envelope: &[u8], passphrase: &str) -> anyhow::Result<Zeroizing<[u8;32]>>
// pkarr::Keypair::from_secret_key takes &[u8;32] via auto-deref on Zeroizing<[u8;32]>
let seed = crate::crypto::decrypt_key_envelope(raw_bytes, &passphrase)?;
let keypair = pkarr::Keypair::from_secret_key(&seed); // Zeroizing<[u8;32]> deref-coerces to &[u8;32]
```

### Format detection (verified from crypto/mod.rs ENVELOPE_MAGIC = b"CCLINKEK")
```rust
// Source: src/crypto/mod.rs — ENVELOPE_MAGIC constant
// Raw file bytes read with fs::read; starts_with checks first 8 bytes
let raw = std::fs::read(&path)?;
if raw.starts_with(b"CCLINKEK") {
    // Encrypted v1.3 envelope
    load_encrypted_keypair(&raw)
} else {
    // Plaintext v1.2 hex
    load_plaintext_keypair(&raw)
}
```

### `--no-passphrase` flag in `InitArgs` (verified from cli.rs existing structure)
```rust
// Source: src/cli.rs — existing InitArgs (add one field)
#[derive(Parser)]
pub struct InitArgs {
    #[arg(long, value_name = "PATH")]
    pub import: Option<String>,

    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Write a plaintext (unencrypted) key file, skipping the passphrase prompt
    #[arg(long)]
    pub no_passphrase: bool,
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `pkarr::Keypair::write_secret_key_file` writes plaintext hex | `write_encrypted_keypair_atomic` writes `CCLINKEK` binary envelope | Phase 16 | Secret key encrypted at rest; passphrase required |
| `load_keypair` reads a hex string unconditionally | `load_keypair` detects magic bytes, branches to encrypted or plaintext path | Phase 16 | Backward-compatible; v1.2 plaintext files load without prompt |
| `cclink init` always writes plaintext key | `cclink init` prompts for passphrase by default; `--no-passphrase` skips | Phase 16 | Key protection is the default; opt-out is explicit |
| All `#[allow(dead_code)]` on `encrypt_key_envelope` / `decrypt_key_envelope` | `#[allow(dead_code)]` removed; functions are actively called | Phase 16 | No more suppression warnings |

**Deprecated/outdated after Phase 16:**
- `pkarr::Keypair::write_secret_key_file` usage in `write_keypair_atomic`: Only the `--no-passphrase` path still calls this via `write_keypair_atomic`. The encrypted path entirely bypasses it.
- `pkarr::Keypair::from_secret_key_file` usage in `init.rs` (the `prompt_overwrite` fingerprint extraction): Still valid — that function reads the existing key to get a fingerprint for the overwrite confirmation prompt. If the existing key is encrypted, it will fail silently (falls back to `"(unreadable)"`). This is acceptable behavior since it's only for display purposes.

## Open Questions

1. **Should `run_init` encrypt imported keys by default?**
   - What we know: `--import` reads a v1.2 hex file via `pkarr::Keypair::from_secret_key_file`. Currently writes back as plaintext. After Phase 16, the default init flow prompts for passphrase. Imported keys could either (a) always be encrypted on write (consistent with the default), or (b) honor `--no-passphrase` same as generated keys.
   - What's unclear: Whether a user importing a key already knows they want it encrypted.
   - Recommendation: Honor `--no-passphrase` for imported keys the same as generated keys. The encryption decision is about the *output file*, not the *input source*. Default: prompt for passphrase and write encrypted. With `--no-passphrase`: skip prompt and write plaintext. This is consistent and predictable.

2. **Should the fingerprint-display in `prompt_overwrite` handle encrypted existing keys?**
   - What we know: `prompt_overwrite` calls `pkarr::Keypair::from_secret_key_file(existing_key_path)` to get a fingerprint. If the existing file is an encrypted envelope, this fails (binary != hex), and falls back to `"(unreadable)"`.
   - What's unclear: Whether the fallback is acceptable UX.
   - Recommendation: The fallback to `"(unreadable)"` is acceptable. Prompt still shows the full path. Phase 16 does not need to fix this — it's cosmetic and deferred improvement at best. Alternatively, detect the CCLINKEK magic and show `"(encrypted)"` as the identifier. This is a low-effort improvement that makes the overwrite prompt clearer.

3. **What is the exact "Wrong passphrase" message format?**
   - What we know: Success criterion 3 says "prints 'Wrong passphrase' and exits with code 1".
   - Recommendation: `eprintln!("Wrong passphrase");` — exactly that string, no "Error: " prefix. This means `load_encrypted_keypair` uses the `eprintln!` + `process::exit(1)` pattern, NOT `anyhow::bail!`.

4. **Should `write_encrypted_keypair_atomic` be public or `pub(crate)`?**
   - What we know: Only `run_init` in `src/commands/init.rs` calls it. All other store functions are `pub`.
   - Recommendation: Make it `pub` for consistency with the rest of `store.rs`. `pub(crate)` is an acceptable alternative.

## Validation Architecture

> Skipped — `workflow.nyquist_validation` is not set in `.planning/config.json`.

## Sources

### Primary (HIGH confidence)
- `/home/john/vault/projects/github.com/cclink/src/crypto/mod.rs` — Full source read; `encrypt_key_envelope`, `decrypt_key_envelope` confirmed present with `#[allow(dead_code)]`; all 8 envelope tests passing
- `/home/john/vault/projects/github.com/cclink/src/keys/store.rs` — Full source read; `load_keypair`, `write_keypair_atomic`, `check_key_permissions` all verified; `read_to_string` confirmed as current hex reading method (must change to `fs::read` for binary detection)
- `/home/john/vault/projects/github.com/cclink/src/commands/init.rs` — Full source read; `InitArgs`, `run_init`, `prompt_overwrite`, `import_from_file` verified
- `/home/john/vault/projects/github.com/cclink/src/cli.rs` — Full source read; `InitArgs` struct confirmed; `--no-passphrase` not yet present
- `/home/john/vault/projects/github.com/cclink/src/commands/publish.rs` — Full source read; `eprintln!` + `process::exit(1)` pattern for PIN validation failure confirmed; `dialoguer::Password::with_confirmation` pattern confirmed
- `/home/john/vault/projects/github.com/cclink/src/commands/pickup.rs` — Full source read; PIN passphrase prompt + `stdin().is_terminal()` guard pattern confirmed
- `/home/john/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/dialoguer-0.12.0/src/prompts/password.rs` — `interact()` source read; confirms `term.is_term()` check returns `Err(IO error: not a terminal)` on non-interactive stdin
- `~/.cargo/registry/src/.../pkarr-5.0.3/src/keys.rs` — `write_secret_key_file` confirmed writes hex; `from_secret_key_file` confirmed reads hex; `secret_key() -> [u8;32]` confirmed
- `.planning/REQUIREMENTS.md` — KEYS-01 through KEYS-06 requirements verified; KEYS-05 already complete (Phase 15)
- `.planning/STATE.md` — Key decisions from 15-01 verified: "decrypt_key_envelope returns Zeroizing<[u8;32]> not Vec<u8> — Phase 16 passes directly to pkarr::Keypair::from_secret_key with auto-deref"; blocker note about non-interactive terminal guard validation confirmed

### Secondary (MEDIUM confidence)
- `.planning/phases/15-encrypted-key-crypto-layer/15-RESEARCH.md` — Format details, API confirmations, and don't-hand-roll guidance verified; all patterns carried forward
- `.planning/phases/15-encrypted-key-crypto-layer/15-01-PLAN.md` — Phase 15 plan; confirms `#[allow(dead_code)]` annotations are present on all three crypto functions awaiting Phase 16

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates and versions read directly from Cargo.toml; dialoguer API verified from source
- Architecture: HIGH — all API signatures verified from source; patterns derived from existing code in the codebase; no speculation
- Pitfalls: HIGH — most pitfalls derived from direct inspection of existing code patterns (wrong exit pattern from publish.rs, non-interactive guard from pickup.rs, binary vs. string reading from store.rs)

**Research date:** 2026-02-24
**Valid until:** 2026-06-01 (pkarr 5.0.3 pinned; dialoguer 0.12.0 pinned; all other deps stable)
