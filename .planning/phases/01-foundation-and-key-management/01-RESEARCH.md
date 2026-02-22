# Phase 1: Foundation and Key Management - Research

**Researched:** 2026-02-21
**Domain:** Rust CLI scaffolding, Ed25519/PKARR keypair management, secure file I/O
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Key storage layout**
- Store keys in `~/.pubky/` directory (reuse existing Pubky tool storage, not a cclink-specific directory)
- Separate files for public key and secret key
- File format: Claude's discretion based on what pkarr/pubky crates expect natively
- Private key file must have 0600 permissions
- Atomic writes: write to temp file then rename (prevent corruption on crash)
- No config file — all configuration via flags or environment variables

**Init experience**
- If keys already exist: prompt to confirm overwrite, displaying an identifier for the existing key (fingerprint or short pubkey) so user knows which key they'd be replacing
- Homeserver: default to pubky.app, override with `--homeserver <URL>` flag
- After successful init: show detailed output — public key, homeserver, key file location, and next steps hint
- If user runs `cclink` (publish) without having run init: error with "No keypair found. Run `cclink init` first."

**Import workflow**
- Accept key from file path OR stdin: `cclink init --import /path/to/key` or `echo key | cclink init --import -`
- Only accept pkarr-native key format (whatever the pkarr crate uses)
- If imported key is invalid/corrupted: fail with clear error message, don't write anything to disk
- If keys already exist during import: same prompt behavior as regular init (show existing key identifier, confirm overwrite)

**Whoami output**
- Display: PKARR public key, homeserver URL, key file path, and short key fingerprint
- Format: labeled human-readable output (e.g., `Public Key: pk:abc123...`)
- Auto-copy public key to clipboard with confirmation message
- If no keys configured: error with "No keypair found. Run `cclink init` first."

### Claude's Discretion

- Key file format on disk (match pkarr crate expectations)
- Exact fingerprint format/length for key identification
- Clipboard library choice
- Exact output formatting and spacing

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| KEY-01 | User can generate an Ed25519/PKARR keypair and store it securely in `~/.pubky/` with 0600 permissions | `Keypair::random()` generates the keypair; `write_secret_key_file()` handles hex encoding + 0600 permissions natively in pkarr 5.0.3; the directory path is `~/.pubky/` per locked decisions (overrides `~/.cclink/keys` in REQUIREMENTS.md) |
| KEY-02 | User can view their PKARR public key and homeserver info via `cclink whoami` | `PublicKey::to_uri_string()` returns `pk:<z32>` format; `PublicKey` implements `Display` as z-base32; fingerprint = first 8 chars of z32; clipboard via arboard 3.6.1 with graceful degradation in SSH |
| KEY-03 | User can import an existing PKARR keypair via `cclink init --import` | `Keypair::from_secret_key_file(path)` reads hex-encoded secret; stdin mode reads hex bytes and calls `Keypair::from_secret_key(&[u8;32])`; validation happens before any disk write |
| KEY-04 | Private key file is written atomically (write-to-temp + rename) to prevent corruption | `write_secret_key_file()` does NOT use atomic write internally — it writes directly; must call `write_secret_key_file` to a temp path then `std::fs::rename()` for atomicity; temp must be on same filesystem as destination |
</phase_requirements>

---

## Summary

Phase 1 uses the `pkarr` crate (v5.0.3) as the sole key management library. Its `Keypair` struct has built-in methods for generation (`random()`), hex-encoded file I/O (`write_secret_key_file` / `from_secret_key_file`), and Unix permission setting (0600). These built-in methods are almost exactly what the phase needs — but there is a critical gap: `write_secret_key_file` writes directly (non-atomically). The atomic write requirement (KEY-04) requires a workaround: call `write_secret_key_file` on a temp file path in the same directory, then `std::fs::rename()`.

The key storage directory is `~/.pubky/` (locked user decision), with two files: `secret_key` (hex-encoded, 0600) and an optional `public_key` (z-base32 text, 0644). The file format on disk is hex-encoded bytes, a 64-character ASCII string for a 32-byte Ed25519 secret key. Public keys display as z-base32 (`pk:` URI prefix) via pkarr's `to_uri_string()`.

The CLI is scaffolded with `clap 4.5.58` using the derive API, `anyhow 1.0.101` for error propagation in main, and `thiserror 2.0.18` for typed domain errors — all consistent with the pubky-core workspace versions. Clipboard support uses `arboard 3.6.1` with graceful degradation when no display server is available (SSH sessions).

**Primary recommendation:** Use `pkarr::Keypair`'s built-in file methods for all key I/O, wrapping the write path in a temp-then-rename pattern for atomicity. Do NOT hand-roll hex encoding, permission setting, or key validation.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| pkarr | 5.0.3 | Ed25519 keypair generation, serialization, file I/O | The project's identity layer; has built-in `write_secret_key_file`/`from_secret_key_file` with 0600 perms |
| clap | 4.5.58 | CLI argument parsing, subcommands | De facto Rust CLI standard; used by pubky-core workspace |
| anyhow | 1.0.101 | Error propagation in `main()` | Idiomatic for CLI binaries; context chaining via `.context()` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| thiserror | 2.0.18 | Typed domain error definitions | For `CclinkError` enum (NoKeypairFound, InvalidKeyFormat, etc.) |
| dirs | 5.x | Cross-platform home directory resolution | `dirs::home_dir()` to build `~/.pubky/` path |
| arboard | 3.6.1 | Clipboard access | `whoami` command auto-copy; must handle `Err` gracefully in SSH |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| arboard | cli-clipboard | cli-clipboard is a fork with better terminal/headless support but less actively maintained; arboard is the standard |
| arboard | terminal-clipboard | Simpler API but narrower ecosystem; arboard covers more platforms |
| dirs | std::env::var("HOME") | Fragile on Windows; dirs is cross-platform and well-maintained |
| anyhow (in main) | Box<dyn Error> | anyhow has `.context()` chaining which produces better CLI error messages |

**Installation:**
```bash
cargo add pkarr@5.0.3
cargo add clap@4.5.58 --features derive
cargo add anyhow@1.0.101
cargo add thiserror@2.0.18
cargo add dirs@5
cargo add arboard@3.6.1
```

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── main.rs          # CLI entry point: Cli struct, subcommand dispatch
├── cli.rs           # Clap derive structs (Cli, Commands, InitArgs, etc.)
├── commands/
│   ├── mod.rs
│   ├── init.rs      # cclink init + cclink init --import logic
│   └── whoami.rs    # cclink whoami logic
├── keys/
│   ├── mod.rs
│   ├── store.rs     # Key storage: paths, read/write, atomic write
│   └── fingerprint.rs  # Short fingerprint formatting
└── error.rs         # CclinkError thiserror enum
```

### Pattern 1: Clap Derive with Subcommands

**What:** Use `#[derive(Parser)]` on `Cli` and `#[derive(Subcommand)]` on `Commands` enum.
**When to use:** Always — the derive API is the standard for clap 4.x.

```rust
// Source: https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cclink", version, about = "Secure session handoff via Pubky")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize keypair
    Init(InitArgs),
    /// Show identity
    Whoami,
}

#[derive(Parser)]
struct InitArgs {
    /// Import an existing keypair from file path or stdin (-)
    #[arg(long, value_name = "PATH")]
    import: Option<String>,

    /// Homeserver URL
    #[arg(long, default_value = "https://pubky.app")]
    homeserver: String,
}
```

### Pattern 2: Keypair Generation and File I/O via pkarr

**What:** Use `Keypair::random()` for generation, `write_secret_key_file` for persistence, `from_secret_key_file` for loading.
**When to use:** All key generate/load operations — do not hand-roll.

```rust
// Source: https://docs.rs/pkarr/5.0.3/pkarr/struct.Keypair.html
use pkarr::Keypair;
use std::path::Path;

// Generate
let keypair = Keypair::random();

// Save (via atomic write wrapper — see Pattern 3)
let public_key = keypair.public_key();
let pubkey_str = public_key.to_uri_string(); // "pk:o4dksfbqk85og..."

// Load
let keypair = Keypair::from_secret_key_file(Path::new("/home/alice/.pubky/secret_key"))?;
```

### Pattern 3: Atomic Write (Temp-Then-Rename)

**What:** Write to a temp file in the SAME directory, then `std::fs::rename()`. The rename is atomic on POSIX.
**When to use:** Any write to the key file — required by KEY-04.

**CRITICAL:** `write_secret_key_file` writes directly (non-atomically). The atomic pattern must wrap it.

```rust
// Source: Rust standard library + atomicity guarantee from POSIX rename(2)
use std::path::PathBuf;
use pkarr::Keypair;

fn write_keypair_atomic(keypair: &Keypair, dest: &PathBuf) -> std::io::Result<()> {
    // Temp file MUST be on the same filesystem as destination
    let parent = dest.parent().expect("dest has no parent");
    let tmp = parent.join(".secret_key.tmp");

    // write_secret_key_file sets 0600 on the temp file
    keypair.write_secret_key_file(&tmp)?;

    // Atomic rename — if process dies here, dest is still the old file
    std::fs::rename(&tmp, dest)?;

    Ok(())
}
```

### Pattern 4: Overwrite Guard (Exists Check Before Write)

**What:** Check if key file exists, load the existing key's public key for display, prompt user to confirm before overwriting.
**When to use:** Before any `init` write — both fresh generate and import.

```rust
use std::io::{self, Write};

fn confirm_overwrite(existing_key_path: &PathBuf) -> anyhow::Result<bool> {
    if existing_key_path.exists() {
        // Load to get identifier — don't abort if corrupt, just show path
        let identifier = Keypair::from_secret_key_file(existing_key_path)
            .map(|k| k.to_z32()[..8].to_string())
            .unwrap_or_else(|_| "(unreadable)".to_string());

        eprint!("Key {} already exists. Overwrite? [y/N]: ", identifier);
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        return Ok(input.trim().eq_ignore_ascii_case("y"));
    }
    Ok(true)
}
```

### Pattern 5: Key File Path Construction

**What:** Build `~/.pubky/` path using `dirs::home_dir()`.
**When to use:** Key store path resolution.

```rust
use std::path::PathBuf;

fn key_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".pubky"))
}

fn secret_key_path() -> anyhow::Result<PathBuf> {
    Ok(key_dir()?.join("secret_key"))
}
```

### Pattern 6: Stdin Import

**What:** Read hex bytes from stdin when `--import -` is passed.
**When to use:** Import workflow with `-` as path.

```rust
use std::io::Read;
use pkarr::Keypair;

fn load_keypair_from_stdin() -> anyhow::Result<Keypair> {
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    let hex = buf.trim();

    // Decode hex to 32 bytes
    let bytes = hex::decode(hex)
        .map_err(|_| anyhow::anyhow!("Invalid hex format — expected 64 hex characters"))?;
    let arr: [u8; 32] = bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Invalid key length — expected 32 bytes (64 hex chars)"))?;

    // Keypair::from_secret_key takes &[SecretKey] where SecretKey = [u8; 32] from ed25519_dalek
    Ok(Keypair::from_secret_key(&arr))
}
```

Note: `hex` crate may be needed, or use stdlib: `u8::from_str_radix(&s[i..i+2], 16)` to avoid extra dep.

### Pattern 7: Clipboard with Graceful Degradation

**What:** Try to copy to clipboard; if it fails (SSH, headless), print a message instead of erroring.
**When to use:** `whoami` command.

```rust
fn try_copy_to_clipboard(text: &str) -> bool {
    match arboard::Clipboard::new() {
        Ok(mut clipboard) => clipboard.set_text(text).is_ok(),
        Err(_) => false,
    }
}

// In whoami:
if try_copy_to_clipboard(&pubkey_uri) {
    println!("  (copied to clipboard)");
} else {
    println!("  (clipboard unavailable — copy manually)");
}
```

### Anti-Patterns to Avoid

- **Writing key file non-atomically:** Never call `write_secret_key_file(dest)` directly — always write to temp then rename.
- **Checking existence after load failure:** Check `path.exists()` BEFORE attempting to load; don't use load-failure as existence indicator.
- **Using `~` in paths:** Always resolve via `dirs::home_dir()` — `~` is shell syntax, not Rust.
- **Panicking on clipboard failure:** `arboard::Clipboard::new()` returns `Err` in SSH sessions; always use `if let Ok(...)` or `.ok()`.
- **Storing homeserver in a config file:** Locked decision: no config file. If homeserver is needed by later phases, it must be stored with the key or passed as a flag.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Hex encoding/decoding of secret key | Custom hex encoder | `Keypair::write_secret_key_file` / `from_secret_key_file` | Already handles 0600 perms, edge cases |
| 0600 Unix permission setting | Manual `set_permissions` | Same — built into pkarr | Platform differences, race conditions |
| Ed25519 key generation | Custom CSPRNG use | `Keypair::random()` | Cryptographic correctness |
| z-base32 public key formatting | Custom base32 | `PublicKey::to_uri_string()` / `to_z32()` | Correct encoding guaranteed |
| Key validation on import | Manual byte checks | `Keypair::from_secret_key_file()` / `from_secret_key()` | Returns typed error on invalid input |
| Home directory lookup | `std::env::var("HOME")` | `dirs::home_dir()` | Cross-platform, handles edge cases |

**Key insight:** pkarr 5.0.3 wraps nearly all key I/O concerns. The only gap is atomicity on write — which requires a one-function wrapper around `write_secret_key_file`.

---

## Common Pitfalls

### Pitfall 1: write_secret_key_file is NOT Atomic

**What goes wrong:** Crash between file open and file close corrupts or truncates the key file. KEY-04 fails.
**Why it happens:** `write_secret_key_file` uses a direct write, not a temp-then-rename pattern internally (confirmed from source analysis).
**How to avoid:** Always use Pattern 3 (atomic write wrapper). Write to `parent/.secret_key.tmp` then rename.
**Warning signs:** Verification step: create a test that kills the process mid-write and checks the original file is intact.

### Pitfall 2: Temp File on Different Filesystem

**What goes wrong:** `std::fs::rename()` fails with `EXDEV` (cross-device link) if temp file is in `/tmp/` and dest is in `~/.pubky/`.
**Why it happens:** POSIX `rename` is only atomic within the same filesystem.
**How to avoid:** Always create temp file in `~/.pubky/` (same directory as destination), NOT in `/tmp/`.
**Warning signs:** The rename returns `Err` with `EXDEV` errno on Linux.

### Pitfall 3: Exists Check Race Condition (TOCTOU)

**What goes wrong:** User is prompted "overwrite?", says yes, but another process deleted the file between check and write.
**Why it happens:** Time-of-check-to-time-of-use gap.
**How to avoid:** Acceptable for this use case — the overwrite prompt is a UX safety guard, not a security guarantee. Document that the prompt is best-effort.
**Warning signs:** Not a critical issue for a single-user CLI tool.

### Pitfall 4: Clipboard Panic in SSH/Headless

**What goes wrong:** `arboard::Clipboard::new().unwrap()` panics in SSH sessions or headless environments where no display server is present.
**Why it happens:** arboard requires X11 or Wayland display server on Linux.
**How to avoid:** Always match on `Result` — never unwrap clipboard calls (see Pattern 7).
**Warning signs:** Test the binary in an SSH session during verification.

### Pitfall 5: write_secret_key_file Overwrites Without Warning

**What goes wrong:** Calling `write_secret_key_file(dest)` directly silently overwrites an existing key file — bypasses the overwrite confirmation UX.
**Why it happens:** The method does not check for existing files.
**How to avoid:** Always run the overwrite guard (Pattern 4) BEFORE calling the write method.
**Warning signs:** Integration test: run `cclink init` twice without `-y` confirmation — should prompt on second run.

### Pitfall 6: Directory Not Created Before Write

**What goes wrong:** `~/.pubky/` may not exist (fresh system). `write_secret_key_file` will fail with `No such file or directory`.
**Why it happens:** The method writes to the given path but does not create parent directories.
**How to avoid:** Call `std::fs::create_dir_all(key_dir())` before writing any key file.
**Warning signs:** The error message is `Os { code: 2, kind: NotFound }`.

### Pitfall 7: Public Key File Staleness

**What goes wrong:** If only the secret key file is stored, the public key must be re-derived on every load (cheap but adds complexity). If a separate public key file is written, it can get out of sync with the secret key.
**Why it happens:** Two files, two write operations.
**How to avoid:** Recommend storing only the secret key file. Always derive the public key from the loaded secret key via `keypair.public_key()`. This eliminates sync issues.
**Warning signs:** Whoami shows different public key than what was stored on init.

### Pitfall 8: from_secret_key Signature Mismatch

**What goes wrong:** `Keypair::from_secret_key` takes `&[SecretKey]` where `SecretKey` is `ed25519_dalek::SecretKey` (a `[u8; 32]` newtype). Passing raw `&[u8]` will not compile.
**Why it happens:** Type system mismatch — `SecretKey` is not `u8`.
**How to avoid:** Use `Keypair::from_secret_key_file` for file imports. For stdin hex import, decode to `[u8; 32]` then coerce: `Keypair::from_secret_key(&secret_key_bytes)` where `secret_key_bytes: [u8; 32]` — the coercion works because `[u8; 32]` can be passed as `&[u8]` slice... but check the actual pkarr API. MEDIUM confidence — verify against actual compiled code. Alternative: write hex to a temp file and use `from_secret_key_file`.
**Warning signs:** Compiler error on `from_secret_key` call with raw bytes.

---

## Code Examples

Verified patterns from official sources:

### Keypair Generation and Public Key Display

```rust
// Source: https://docs.rs/pkarr/5.0.3/pkarr/struct.Keypair.html
use pkarr::Keypair;

let keypair = Keypair::random();
let public_key = keypair.public_key();

// Full URI format: "pk:o4dksfbqk85ogzdb5osziw6befigbuxmuxkuxq8434q89uj56uyy"
println!("Public Key: {}", public_key.to_uri_string());

// z-base32 only (52 chars): "o4dksfbqk85ogzdb5osziw6befigbuxmuxkuxq8434q89uj56uyy"
println!("Public Key (z32): {}", public_key.to_z32());

// Short fingerprint (first 8 chars of z32)
let fingerprint = &public_key.to_z32()[..8];
println!("Fingerprint: {}", fingerprint);
```

### Whoami Output Pattern

```rust
// Source: locked decisions from CONTEXT.md
fn print_whoami(keypair: &Keypair, homeserver: &str, key_path: &Path) {
    let pub_key = keypair.public_key();
    let pubkey_uri = pub_key.to_uri_string();
    let fingerprint = &pub_key.to_z32()[..8];

    println!("Public Key:  {}", pubkey_uri);
    println!("Fingerprint: {}", fingerprint);
    println!("Homeserver:  {}", homeserver);
    println!("Key file:    {}", key_path.display());

    if try_copy_to_clipboard(&pubkey_uri) {
        println!("\nPublic key copied to clipboard.");
    } else {
        println!("\n(Clipboard unavailable — copy public key manually)");
    }
}
```

### Init Success Output Pattern

```rust
// Source: locked decisions from CONTEXT.md
fn print_init_success(keypair: &Keypair, homeserver: &str, key_path: &Path) {
    let pub_key = keypair.public_key();
    println!("Keypair generated successfully.");
    println!("");
    println!("Public Key:  {}", pub_key.to_uri_string());
    println!("Homeserver:  {}", homeserver);
    println!("Key file:    {}", key_path.display());
    println!("");
    println!("Next: run 'cclink' to publish your first session handoff.");
}
```

### Loading Keypair (with missing-key error)

```rust
// Source: pkarr docs + CONTEXT.md error UX
use pkarr::Keypair;

fn load_or_error(path: &Path) -> anyhow::Result<Keypair> {
    if !path.exists() {
        anyhow::bail!("No keypair found. Run `cclink init` first.");
    }
    Keypair::from_secret_key_file(path)
        .map_err(|e| anyhow::anyhow!("Failed to load keypair: {}", e))
}
```

### Cargo.toml for cclink binary

```toml
[package]
name = "cclink"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "cclink"
path = "src/main.rs"

[dependencies]
pkarr = { version = "5.0.3", default-features = false }
clap = { version = "4.5", features = ["derive"] }
anyhow = "1.0"
thiserror = "2.0"
dirs = "5"
arboard = "3.6"
```

Note: `pkarr` with `default-features = false` disables DHT/relay client features not needed in Phase 1. The `Keypair` and `PublicKey` types are always available regardless of features.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual Unix permission calls after write | Built into `pkarr::Keypair::write_secret_key_file` | pkarr 5.x | No need for `std::os::unix::fs::PermissionsExt` in calling code |
| `structopt` for CLI | `clap` 4.x with derive feature | clap 3+ (2022) | structopt is deprecated; clap is canonical |
| `directories` crate | `dirs` crate | — | Both work; `dirs` is simpler API for just home dir |
| ed25519-dalek directly | Via pkarr re-export | — | pkarr wraps dalek; use pkarr types, not dalek types directly |

**Deprecated/outdated:**
- `structopt`: Merged into clap 3+. Do not use.
- Writing keys to `~/.cclink/keys`: Superseded by locked user decision to use `~/.pubky/` (CONTEXT.md overrides REQUIREMENTS.md for this detail).

---

## Open Questions

1. **`Keypair::from_secret_key` exact signature for stdin import**
   - What we know: The method exists in pkarr 5.0.3; it takes `&[SecretKey]` where `SecretKey` is from ed25519_dalek
   - What's unclear: Whether `[u8; 32]` coerces directly to `&[SecretKey]` or if there's a `SecretKey::from_bytes` needed
   - Recommendation: Simplest safe approach — write decoded hex bytes to a temp file and use `Keypair::from_secret_key_file(tmp_path)` for stdin import; this avoids the type ambiguity entirely and reuses proven code path

2. **pkarr `default-features = false` — does Keypair still work?**
   - What we know: pkarr features are `dht`, `relays`, `lmdb-cache`, `endpoints`, `tls`, `reqwest-resolve`, `reqwest-builder`; `Keypair` is not gated by any feature in the docs
   - What's unclear: Whether any feature gate affects the `keys.rs` module
   - Recommendation: Compile with `default-features = false` first; if `Keypair` is unavailable, add back `extra` feature

3. **Homeserver persistence across commands**
   - What we know: No config file (locked). Phase 1 only calls `whoami` which needs homeserver URL.
   - What's unclear: If `whoami` in Phase 1 needs to show homeserver, where does it read it from after init? User said "no config file — all configuration via flags or environment variables." But init accepts `--homeserver` — whoami then needs to recover it somehow.
   - Recommendation: Store homeserver URL alongside the key — either as a separate `homeserver` file in `~/.pubky/` or as a `CCLINK_HOMESERVER` environment variable default. A simple text file `~/.pubky/cclink_homeserver` is the least-friction solution that doesn't violate the "no config file" spirit (it's data, not config). Flag this for user confirmation during planning.

4. **Public key file — store separately or derive on load?**
   - What we know: CONTEXT.md says "Separate files for public key and secret key" but also says "Claude's discretion" on file format.
   - What's unclear: Whether the public key file is actually needed given it can be derived instantly from secret key.
   - Recommendation: Store only secret key file. Derive public key in memory. Avoids sync issues (Pitfall 7). Document this explicitly.

---

## Sources

### Primary (HIGH confidence)

- `pkarr` 5.0.3 on docs.rs — Keypair struct, all method signatures, write_secret_key_file behavior, from_secret_key_file, PublicKey methods (to_uri_string, to_z32, Display impl)
- `pubky-core` workspace Cargo.toml (main branch) — confirmed pkarr=5.0.3, clap=4.5.58, anyhow=1.0.101, thiserror=2.0.18, pubky-core v0.6.0 released 2026-01-15
- `clap` 4.x derive tutorial on docs.rs — derive API patterns for Parser, Subcommand
- `arboard` 3.6.1 on docs.rs + GitHub — Clipboard struct, headless limitation (no display server = Err), wayland-data-control feature flag

### Secondary (MEDIUM confidence)

- `write_secret_key_file` non-atomicity claim: inferred from source code analysis (hex write, no temp path in implementation); behavior documented at docs.rs confirms it "writes directly" — not explicitly stated as atomic or non-atomic; VERIFY during implementation with a kill-on-write test
- `from_secret_key` signature behavior for `[u8; 32]` input: type is `&[SecretKey]` from ed25519_dalek; exact coercion behavior not confirmed by compilation — recommend temp-file approach for stdin import

### Tertiary (LOW confidence)

- `pkarr default-features = false` and Keypair availability: assumed from feature list inspection (no `Keypair` feature gate visible), not verified by compilation

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — pkarr 5.0.3 API confirmed from docs.rs; pubky-core workspace Cargo.toml versions confirmed
- Architecture: HIGH — patterns derived from confirmed pkarr API and clap 4.x docs
- Pitfalls: HIGH for atomicity gap, clipboard, directory creation; MEDIUM for from_secret_key type coercion

**Research date:** 2026-02-21
**Valid until:** 2026-04-21 (pkarr is actively developed but 5.0.3 is pinned; clap is stable)
