# Stack Research

**Domain:** Rust CLI — Ed25519/PKARR identity, Pubky homeserver, age encryption, QR codes, terminal UX
**Researched:** 2026-02-21
**Confidence:** HIGH (core stack verified via official docs and docs.rs; Pubky SDK verified at v0.6.0 released 2026-01-15)

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `pubky` | 0.6.0 | Pubky homeserver client, PKARR identity, session auth | The official Rust SDK from the pubky-core project. Provides `Pubky` facade, `PubkySigner` for homeserver signup/signin, `SessionStorage` for authenticated PUT/GET/DELETE, and `Pkdns` for publishing/resolving `_pubky` DNS records. This is the only correct choice — there is no alternative for the Pubky protocol. |
| `pkarr` | 5.0.3 | Ed25519 keypair generation and management | `pkarr::Keypair` is the canonical key type for the Pubky ecosystem: `Keypair::random()`, `Keypair::from_secret_key()`, `write_secret_key_file()`, `to_z32()`. The pubky crate depends on it. Provides both blocking and async APIs. |
| `age` | 0.11.2 | File/payload encryption | The Rust implementation of the age-encryption.org/v1 spec. Native x25519 recipients via `x25519::Recipient` and `x25519::Identity`. Still pre-1.0 (beta) but widely used and matches the PROJECT.md constraint. Rage CLI and reference Go implementation use the same wire format. |
| `clap` | 4.5.60 | CLI argument parsing and subcommand dispatch | The de facto standard. Version 4 derive macros make subcommand definitions clean. Handles help generation, error suggestions, shell completions. Used by pubky-cli itself. |
| `tokio` | 1.49.0 | Async runtime | Required by the pubky SDK, which is async throughout. Multi-threaded scheduler handles homeserver HTTP calls without blocking. Standard choice for async Rust. |

### Cryptographic Stack

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `ssh-to-age` | 0.2.0 | Ed25519-to-X25519 key conversion | Converting a pkarr Ed25519 keypair to an age-compatible X25519 identity for self-encryption. This is the only Rust-native library solving this conversion; released June 2025. Use when building the `age::x25519::Identity` from the PKARR secret key. |
| `hkdf` | 0.12.4 | HKDF key derivation (PIN mode) | Deriving a symmetric encryption key from a 4-digit PIN plus the public key as salt. Required for `--pin` flag functionality. RustCrypto crate, pairs with `sha2`. |
| `sha2` | 0.10.9 | SHA-256 for HKDF | Feed into `hkdf` as the PRF. Also useful for generating the session token hash for the `latest.json` pointer. |
| `zeroize` | 1.8.2 | Zeroing secret key bytes from memory | Prevents compiler-optimized-away memory clears on sensitive data. Critical for Ed25519 secret key handling. Use `Zeroize` derive on any struct holding key material. |
| `curve25519-dalek` | 4.1.3 | Low-level curve operations | Only if needing direct Montgomery-form conversion not covered by ssh-to-age. Likely not needed directly — ssh-to-age wraps this. |

### Terminal UX

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `qr2term` | 0.3.3 | Terminal QR code rendering | One-line API: `qr2term::print_qr(url)?`. Depends on crossterm and qrcode internally. Use after successful publish and on pickup. PURPOSE-BUILT for terminal QR — do not use the lower-level `qrcode` crate directly. |
| `indicatif` | 0.18.4 | Progress bars and spinners | Use for homeserver HTTP operations (signup, PUT, GET). Shows activity during network calls. Thread-safe, Tokio-compatible. |
| `console` | 0.16.2 | Colored output and terminal detection | Base layer for styled output (`style("OK").green().bold()`). Automatically disables color when output is piped. Required by indicatif and dialoguer — add it directly for status line formatting. |
| `dialoguer` | 0.12.0 | Interactive prompts | Use for PIN entry (`Password` prompt), and `--exec` confirmation. Pairs with console. |

### Serialization and Storage

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serde` | 1.0.228 | Serialization framework | Required for handoff payload JSON. Enable `derive` feature on all data structs. |
| `serde_json` | 1.0.149 | JSON encoding/decoding | Handoff records are JSON published to `/pub/cclink/sessions/<token>.json` and `latest.json`. Standard choice. |
| `dirs` | 6.0.0 | Platform-aware home/config paths | Finding `~/.cclink/keys` and `~/.claude/sessions/` correctly across Linux/macOS/Windows. Follows XDG on Linux. Use `dirs::home_dir()`. |

### Error Handling

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `anyhow` | 1.0.102 | Application-level error propagation | Use in `main()` and command handlers: `anyhow::Result<()>`. Context attachment with `.context("...")` produces useful CLI error messages. Standard for binary crates. |
| `thiserror` | 2.0.18 | Typed domain errors | Define `CclinkError` enum for categorized errors (keypair not found, homeserver unreachable, decryption failed). Use in internal modules. Combine with anyhow at the boundary: `thiserror` inside, `anyhow` at command level. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo-dist` | Binary release automation | v0.30.0 (Sept 2025). Produces cross-platform GitHub release artifacts. Handles macOS/Linux/Windows targets. Add to `Cargo.toml` workspace metadata. |
| `cross` | Cross-compilation | `cross build --target=x86_64-unknown-linux-musl --release` for portable Linux binaries. Docker-based. Use instead of manual musl toolchain setup. |
| `cargo-release` | Crate version bumping | Bump version, tag, push. Integrates with cargo-dist release flow. |

---

## Installation

```toml
[package]
name = "cclink"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "cclink"
path = "src/main.rs"

[dependencies]
# Pubky protocol
pubky = "0.6.0"
pkarr = "5.0.3"

# Encryption
age = { version = "0.11.2", features = ["x25519"] }
ssh-to-age = "0.2.0"
hkdf = "0.12.4"
sha2 = "0.10.9"
zeroize = { version = "1.8.2", features = ["derive"] }

# CLI
clap = { version = "4.5.60", features = ["derive"] }
tokio = { version = "1.49.0", features = ["full"] }

# Terminal UX
qr2term = "0.3.3"
indicatif = "0.18.4"
console = "0.16.2"
dialoguer = "0.12.0"

# Serialization
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"

# Filesystem / paths
dirs = "6.0.0"

# Error handling
anyhow = "1.0.102"
thiserror = "2.0.18"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `pubky` 0.6.0 | Direct HTTP via `reqwest` | Never — the pubky SDK handles auth header signing, session management, PKDNS resolution, and retry logic. Rolling this by hand is a significant implementation risk for a peripheral concern. |
| `pkarr::Keypair` | `ed25519-dalek` directly | Never for this project — pkarr::Keypair IS the identity type the pubky SDK expects. Using ed25519-dalek directly would require conversion shims that might not be correct. |
| `age` crate | `sodiumoxide` / `libsodium` | Only if needing NaCl box semantics specifically. age has a simpler API, is well-audited, and the wire format is stable and interoperable. |
| `ssh-to-age` | Manual Ed25519→X25519 conversion via curve25519-dalek | Only if you need to avoid the dependency. ssh-to-age is small and correct; manual conversion is where bugs hide. |
| `qr2term` | `qrcode` (lower-level) | `qrcode` if you need image output. For terminal-only, `qr2term` is one line with no rendering plumbing required. |
| `anyhow` + `thiserror` | `eyre` | `eyre` is a `anyhow` fork with better error reporting hooks (color-eyre). Use it if you want prettier backtraces; otherwise anyhow is simpler. |
| `tokio` | `async-std` | `async-std` has lower adoption and the pubky SDK likely assumes tokio. Do not mix runtimes. |
| `cargo-dist` | Manual GitHub Actions matrix | cargo-dist is significantly less boilerplate for multi-platform binary releases. The only reason to skip it is if you need tight control over the CI pipeline for other reasons. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `reqwest` directly for homeserver calls | The pubky SDK wraps HTTP with auth signing and PKDNS resolution. Bypassing it means re-implementing the auth protocol, which changes with SDK updates. | `pubky::Pubky` / `SessionStorage` |
| `sodiumoxide` / `libsodium` bindings | C FFI dependency breaks musl static builds; age is pure Rust and covers this use case. | `age` + `ssh-to-age` |
| `openssl` (system) | Breaks static binary builds; TLS needs work on musl. | `rustls` (pulled in transitively by pubky/reqwest via `rustls-tls` feature) |
| `structopt` | Deprecated; merged into clap v3+ derive. Old tutorials still reference it. | `clap` 4.x with `derive` feature |
| `ratatui` | Full TUI framework; major overkill for status output in a CLI that doesn't need interactive rendering. | `indicatif` + `console` for progress/color |
| `keyring` (system keychain) | Extra dependency and system integration complexity for v1. The PROJECT.md spec is `~/.cclink/keys` with 0600 permissions. | File-based key storage with `0o600` permission set via `std::fs` |
| Async-only `age` streaming API | Adds complexity for payloads that fit in memory (session IDs are tiny). | Synchronous `age` encrypt/decrypt for in-memory buffers |

---

## Stack Patterns by Variant

**For self-encryption (default handoff):**
- Use `ssh-to-age` to derive `age::x25519::Identity` from the pkarr secret key
- Encrypt the session ID payload to the corresponding `x25519::Recipient`
- The same keypair encrypts and decrypts — no key sharing needed

**For share-mode (`--share <pubkey>`):**
- Derive recipient's `age::x25519::Recipient` from their PKARR public key (same conversion via ssh-to-age public key path)
- Encrypt only to recipient; sender cannot decrypt

**For PIN-protected (`--pin`):**
- Use `hkdf` with SHA-256: IKM = PIN bytes, salt = public key bytes, info = `b"cclink-pin-v1"`
- Derive 32-byte key → use as `age::x25519::Identity` seed (or as `age::scrypt` passphrase input)
- Document: LOW security, convenience only — 4 digits = 10,000 possibilities

**For static Linux binary (CI):**
```bash
rustup target add x86_64-unknown-linux-musl
cargo install cross
cross build --target x86_64-unknown-linux-musl --release
```
Ensure all deps support musl — pubky pulls in `rustls` which is pure Rust. Avoid any `openssl` feature flags.

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `pubky@0.6.0` | `pkarr@5.0.3` | pubky-core repo released together 2026-01-15; use matching versions |
| `age@0.11.2` | `ssh-to-age@0.2.0` | ssh-to-age targets age 0.11.x; verify before bumping age |
| `clap@4.5.x` | `indicatif@0.18.x` | No conflict; independent |
| `tokio@1.49.0` | `pubky@0.6.0` | pubky SDK requires tokio; use `features = ["full"]` or at minimum `["rt-multi-thread", "macros"]` |
| `zeroize@1.8.x` | `pkarr@5.0.3`, `age@0.11.x` | RustCrypto crates all use zeroize 1.x; single version in lockfile |

---

## Sources

- `docs.rs/pubky/0.6.0/pubky/` — SDK APIs, signup/signin/storage methods (HIGH confidence)
- `github.com/pubky/pubky-core` — pubky-core v0.6.0 released 2026-01-15, confirmed (HIGH confidence)
- `docs.pubky.org/Explore/PubkyCore/API` — PUT/GET/DELETE paths, auth header format (HIGH confidence)
- `docs.rs/pkarr/latest/pkarr/struct.Keypair.html` — Keypair API (HIGH confidence)
- `docs.rs/age/latest/age/` — age 0.11.2, x25519 encryption (HIGH confidence)
- `lib.rs/crates/ssh-to-age` — ssh-to-age 0.2.0 released June 2025, Ed25519→X25519 (HIGH confidence)
- `docs.rs/clap/latest/clap/` — clap 4.5.60 (HIGH confidence)
- `docs.rs/tokio/latest/tokio/` — tokio 1.49.0 (HIGH confidence)
- `docs.rs/qr2term/latest/qr2term/` — qr2term 0.3.3 (HIGH confidence)
- `docs.rs/indicatif/latest/indicatif/` — indicatif 0.18.4 (HIGH confidence)
- `docs.rs/console/latest/console/` — console 0.16.2 (HIGH confidence)
- `docs.rs/dialoguer/latest/dialoguer/` — dialoguer 0.12.0 (HIGH confidence)
- `docs.rs/hkdf/latest/hkdf/` — hkdf 0.12.4 (HIGH confidence)
- `docs.rs/sha2/latest/sha2/` — sha2 0.10.9 (HIGH confidence)
- `docs.rs/zeroize/latest/zeroize/` — zeroize 1.8.2 (HIGH confidence)
- `docs.rs/anyhow/latest/anyhow/` — anyhow 1.0.102 (HIGH confidence)
- `docs.rs/thiserror/latest/thiserror/` — thiserror 2.0.18 (HIGH confidence)
- `docs.rs/serde/latest/serde/` — serde 1.0.228 (HIGH confidence)
- `docs.rs/serde_json/latest/serde_json/` — serde_json 1.0.149 (HIGH confidence)
- `docs.rs/dirs/latest/dirs/` — dirs 6.0.0 (HIGH confidence)
- `github.com/axodotdev/cargo-dist` — cargo-dist v0.30.0 released Sept 2025 (MEDIUM confidence; from WebSearch)
- WebSearch: musl static binary best practices 2025 (MEDIUM confidence)

---
*Stack research for: Rust CLI — cclink (PKARR identity, Pubky homeserver, age encryption)*
*Researched: 2026-02-21*
