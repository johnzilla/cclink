# Architecture Research

**Domain:** Rust CLI with Ed25519 identity, age encryption, and decentralized publishing
**Researched:** 2026-02-21
**Confidence:** MEDIUM — core Rust CLI patterns are HIGH; pubky SDK API details are MEDIUM (active development, rc versions); Ed25519-to-X25519 conversion is HIGH (well-documented cryptographic primitive)

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                        CLI Layer (clap)                           │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ ┌─────────┐  │
│  │  init    │ │  whoami  │ │  publish │ │ pickup │ │  list/  │  │
│  │          │ │          │ │          │ │        │ │  revoke │  │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └───┬────┘ └────┬────┘  │
└───────┼────────────┼────────────┼────────────┼────────────┼──────┘
        │            │            │            │            │
        ▼            ▼            ▼            ▼            ▼
┌──────────────────────────────────────────────────────────────────┐
│                      Context / Config Layer                       │
│               (AppConfig, KeyManager, resolved Ctx)               │
└──────────────────────────────────────────────────────────────────┘
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐  ┌──────────────────┐  ┌───────────────────────┐
│  Key Store    │  │  Crypto Engine   │  │  Session Discovery    │
│               │  │                  │  │                       │
│ ~/.cclink/    │  │ Ed25519 → X25519 │  │ ~/.claude/sessions/   │
│   keys/       │  │ age encrypt/     │  │   UUID dir scanning   │
│ (0600 perms)  │  │   decrypt        │  │                       │
│               │  │ HKDF PIN derive  │  │                       │
└───────────────┘  └──────────────────┘  └───────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                     Pubky Transport Layer                         │
│                                                                   │
│  ┌──────────────────────────┐  ┌────────────────────────────┐    │
│  │  pubky SDK (Client)      │  │  pkarr Client (DHT)        │    │
│  │  PUT /pub/cclink/...     │  │  Identity resolution       │    │
│  │  GET /pub/cclink/...     │  │  (--share recipient lookup)│    │
│  │  DELETE /pub/cclink/...  │  └────────────────────────────┘    │
│  └──────────────────────────┘                                     │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                    Homeserver (remote)                            │
│   /pub/cclink/sessions/<token>.json                               │
│   /pub/cclink/sessions/latest.json                                │
└──────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| CLI Layer | Argument parsing, subcommand dispatch, help/error text | `clap` derive macros; one file per command in `src/commands/` |
| Context / Config | Normalize CLI flags + config file into a single `Ctx` struct | `confy` or manual TOML; `dirs` crate for XDG paths |
| Key Store | Persist Ed25519 keypair at `~/.cclink/keys`; enforce 0600 permissions; load/save | Raw file I/O with `std::fs`; `pkarr::Keypair` or `ed25519-dalek` |
| Crypto Engine | Ed25519→X25519 conversion, age encrypt/decrypt, HKDF for PIN derivation | `age` crate (x25519 module); `hkdf` crate; `curve25519-dalek` for conversion |
| Session Discovery | Enumerate `~/.claude/sessions/` UUIDs; identify most recent | `std::fs::read_dir`; sort by mtime |
| Pubky Transport | Authenticate to homeserver; PUT/GET/DELETE records at PKARR paths | `pubky` SDK crate; `reqwest`-compatible async HTTP |
| pkarr Resolver | Resolve a recipient's public key from DHT for `--share` mode | `pkarr` crate `Client` |
| Output / Display | Colored terminal output, QR code rendering, status indicators | `colored` or `owo-colors`; `qrcode` crate |

## Recommended Project Structure

```
src/
├── main.rs                  # Entry point: parse args, route to commands
├── cli.rs                   # Top-level clap App struct + Commands enum
├── ctx.rs                   # AppCtx: normalized context struct
├── config.rs                # Config file (TOML): homeserver URL, defaults
├── commands/
│   ├── mod.rs               # Re-exports + shared command trait/dispatch
│   ├── init.rs              # cclink init — keygen, write key store
│   ├── whoami.rs            # cclink whoami — display public key
│   ├── publish.rs           # cclink publish — discover session, encrypt, PUT
│   ├── pickup.rs            # cclink pickup — GET, decrypt, optional --exec
│   ├── list.rs              # cclink list — GET index, display records
│   └── revoke.rs            # cclink revoke — DELETE record(s)
├── keys/
│   ├── mod.rs               # Re-exports
│   ├── store.rs             # KeyStore: load/save keypair at ~/.cclink/keys/
│   └── convert.rs           # Ed25519 → X25519 birational conversion
├── crypto/
│   ├── mod.rs               # Re-exports
│   ├── encrypt.rs           # age encrypt (self, --share, --pin modes)
│   ├── decrypt.rs           # age decrypt (self, --share, --pin modes)
│   └── hkdf.rs              # PIN → HKDF-derived key derivation
├── session/
│   ├── mod.rs               # Re-exports
│   └── discover.rs          # ~/.claude/sessions/ enumeration + selection
├── transport/
│   ├── mod.rs               # Re-exports
│   ├── pubky_client.rs      # Thin wrapper around pubky SDK (put/get/delete)
│   ├── pkarr_client.rs      # Thin wrapper for PKARR DHT resolution
│   └── record.rs            # HandoffRecord: JSON payload type + serde
└── output/
    ├── mod.rs               # Re-exports
    ├── display.rs           # Colored status messages, tables
    └── qr.rs                # QR code rendering to terminal
```

### Structure Rationale

- **`commands/`:** One file per top-level subcommand. Each file owns its argument struct and `run()` function. This keeps command logic co-located and avoids a monolithic `main.rs`. Mirrors the pattern used by `pubky-cli` and recommended by the Rust CLI community (Kevin K's blog, Rain's CLI recommendations).
- **`keys/`:** Isolated from crypto so key persistence (I/O, permissions) can be tested without touching encryption logic. The conversion module is its own file because the Ed25519→X25519 map is a distinct cryptographic operation, not a key management concern.
- **`crypto/`:** Pure functions — no I/O, no network. Encrypt/decrypt take `&[u8]` in, return `Vec<u8>` out. Testable in isolation.
- **`session/`:** Thin module; may be a single file long-term. Separate because session discovery touches the filesystem and is fully independent of crypto and network.
- **`transport/`:** Wraps the external pubky SDK and pkarr crate. Single responsibility: speak to the network. `record.rs` defines the wire format so the rest of the app is not coupled to JSON structure.
- **`output/`:** Strictly presentation. Commands call `output::display::success(...)` — they do not format strings themselves. Makes testing command logic straightforward without capturing stdout.

## Architectural Patterns

### Pattern 1: Context Struct (AppCtx)

**What:** A `Ctx` or `AppCtx` struct that holds normalized, resolved values from all config sources (config file, environment variables, CLI flags) before any command logic runs.

**When to use:** Always — the single source of truth pattern eliminates `--no-confirm` vs `--confirm` type conflicts and makes functions testable without re-parsing CLI args.

**Trade-offs:** Small upfront boilerplate; large ongoing maintainability gain. Recommended by multiple authoritative Rust CLI sources.

**Example:**
```rust
// ctx.rs
pub struct AppCtx {
    pub homeserver_url: String,
    pub key_path: PathBuf,
    pub verbose: bool,
    pub ttl_seconds: u64,
}

impl AppCtx {
    pub fn from_config_and_args(cfg: &Config, args: &GlobalArgs) -> Self {
        AppCtx {
            homeserver_url: args.homeserver.clone()
                .unwrap_or_else(|| cfg.homeserver_url.clone()),
            key_path: args.key_path.clone()
                .unwrap_or_else(default_key_path),
            verbose: args.verbose,
            ttl_seconds: args.ttl.unwrap_or(cfg.default_ttl_seconds),
        }
    }
}
```

### Pattern 2: Command-per-Module with run() Function

**What:** Each subcommand lives in its own module. The module defines an `Args` struct (clap derive) and a `run(ctx: &AppCtx, args: &Args) -> anyhow::Result<()>` function. Dispatch in `commands/mod.rs` matches on the `Commands` enum.

**When to use:** Any CLI with 3+ subcommands.

**Trade-offs:** Slightly more files; each command is independently readable and testable.

**Example:**
```rust
// commands/mod.rs
pub fn dispatch(ctx: &AppCtx, cmd: Commands) -> anyhow::Result<()> {
    match cmd {
        Commands::Init(args)    => init::run(ctx, &args),
        Commands::Whoami(args)  => whoami::run(ctx, &args),
        Commands::Publish(args) => publish::run(ctx, &args),
        Commands::Pickup(args)  => pickup::run(ctx, &args),
        Commands::List(args)    => list::run(ctx, &args),
        Commands::Revoke(args)  => revoke::run(ctx, &args),
    }
}
```

### Pattern 3: Thin Transport Wrapper

**What:** Wrap the pubky SDK behind a local `PubkyClient` struct that exposes only the operations `cclink` needs (`put_record`, `get_record`, `delete_record`, `list_records`). The rest of the codebase never imports the pubky SDK directly.

**When to use:** When depending on an SDK that is in active development (pubky is at `0.6.0-rc.6`). Isolates API churn to one file.

**Trade-offs:** One layer of indirection; pays off significantly when the upstream SDK changes.

**Example:**
```rust
// transport/pubky_client.rs
pub struct PubkyClient { inner: pubky::Client }

impl PubkyClient {
    pub async fn put_record(&self, path: &str, data: &[u8]) -> Result<()> { ... }
    pub async fn get_record(&self, path: &str) -> Result<Option<Vec<u8>>> { ... }
    pub async fn delete_record(&self, path: &str) -> Result<()> { ... }
}
```

### Pattern 4: Error Layering (thiserror + anyhow)

**What:** Define domain error enums per module using `thiserror` (e.g., `CryptoError`, `KeyStoreError`). Use `anyhow` in command handlers where error context matters for user output.

**When to use:** Standard Rust CLI pattern. `thiserror` in libraries/modules, `anyhow` at the application boundary.

**Trade-offs:** Minimal. The alternative (single giant error enum or all-anyhow) is worse for both testability and error message quality.

## Data Flow

### Publish Flow

```
User runs: cclink publish [--pin CODE] [--share PUBKEY] [--ttl 4h] [--burn]
     │
     ▼
cli.rs: parse args → Commands::Publish(PublishArgs)
     │
     ▼
commands/publish.rs:
  1. load AppCtx (homeserver URL, key path, TTL)
  2. session/discover.rs → enumerate ~/.claude/sessions/ → select UUID
  3. keys/store.rs → load Ed25519 keypair from ~/.cclink/keys/
  4. Build HandoffRecord { session_id, hostname, project, timestamp, ttl }
  5. Serialize HandoffRecord to JSON bytes
  6. crypto/encrypt.rs:
       - default mode:  Ed25519 pubkey → X25519 (keys/convert.rs) → age encrypt
       - --pin mode:    PIN + salt → HKDF → passphrase → age encrypt
       - --share mode:  pkarr_client.rs resolve recipient pubkey → age encrypt
  7. Sign envelope (Ed25519 signature over ciphertext)
  8. transport/pubky_client.rs:
       PUT /pub/cclink/sessions/<token>.json  (encrypted envelope)
       PUT /pub/cclink/sessions/latest.json   (pointer)
  9. output/qr.rs → render QR of pubky URL to terminal
 10. output/display.rs → print success with pubky:// URL
```

### Pickup Flow

```
User runs: cclink pickup [--pin CODE] [--burn] [--exec]
     │
     ▼
cli.rs: parse args → Commands::Pickup(PickupArgs)
     │
     ▼
commands/pickup.rs:
  1. load AppCtx
  2. keys/store.rs → load Ed25519 keypair
  3. transport/pubky_client.rs:
       GET /pub/cclink/sessions/latest.json → token
       GET /pub/cclink/sessions/<token>.json → encrypted envelope
  4. Verify Ed25519 signature on envelope
  5. crypto/decrypt.rs:
       - default mode:  Ed25519 seckey → X25519 → age decrypt
       - --pin mode:    PIN + salt (from envelope) → HKDF → age decrypt
  6. Deserialize HandoffRecord → extract session_id
  7. If --burn: transport/pubky_client.rs DELETE record(s)
  8. output/qr.rs → render QR of session resume command
  9. output/display.rs → print session ID, hostname, project, age
 10. If --exec: exec("claude", ["--resume", &session_id])
```

### Key Management Flow

```
cclink init
     │
     ▼
commands/init.rs:
  1. Check if ~/.cclink/keys/ already exists → warn if so
  2. pkarr::Keypair::random() → 64-byte Ed25519 seed+pubkey
  3. keys/store.rs → write to ~/.cclink/keys/identity
       std::fs::set_permissions(0o600) — critical
  4. output/display.rs → print public key (z-base32 encoded)
```

### Key Data Flows Summary

1. **Session ID** flows: disk → HandoffRecord → JSON → age ciphertext → Pubky homeserver → ciphertext → age plaintext → JSON → HandoffRecord → session_id → shell exec
2. **Ed25519 keypair** flows: disk (0600) → memory only; secret key bytes are never printed, logged, or transmitted
3. **X25519 keys** are derived ephemerally at encrypt/decrypt time, never persisted
4. **PIN** flows: CLI arg (or prompt) → HKDF → ephemeral key → age; PIN is never stored or transmitted
5. **Handoff record path** on homeserver: `/pub/cclink/sessions/<token>.json` where `token` is a random 16-byte hex string

## Scaling Considerations

This is a single-user CLI tool; traditional user-count scaling does not apply. The relevant scaling axis is feature complexity.

| Concern | MVP (single-user, self) | Share Mode (2-party) | Team Mode (v2, out of scope) |
|---------|------------------------|----------------------|------------------------------|
| Key management | One keypair, one file | Same keypair; recipient resolved via pkarr DHT | Namespace/ACL management needed |
| Encryption | Self-encrypt (Ed25519→X25519) | Recipient pubkey via pkarr resolve | Group key distribution |
| Record paths | `/pub/cclink/sessions/*` on own homeserver | Same paths, encrypted for recipient | Shared namespace coordination |
| Transport | pubky SDK, single authenticated client | Same + one pkarr DHT resolution | Multiple homeserver sessions |

The biggest architectural risk is the pubky SDK API changing between rc versions. The thin-wrapper pattern in `transport/pubky_client.rs` directly mitigates this.

## Anti-Patterns

### Anti-Pattern 1: Importing pubky SDK throughout command modules

**What people do:** Call `pubky::Client::put(...)` directly inside `commands/publish.rs`, `commands/pickup.rs`, etc.

**Why it's wrong:** The pubky crate is at `0.6.0-rc.6`. When the API changes, every command file requires edits. Async initialization of the client also needs a single place.

**Do this instead:** `PubkyClient` wrapper in `transport/pubky_client.rs`. Commands call `ctx.transport.put_record(path, data)`.

### Anti-Pattern 2: Doing key conversion inline in crypto functions

**What people do:** Embed the Ed25519→X25519 montgomery map inside the encrypt/decrypt function with a comment.

**Why it's wrong:** The birational map is a distinct cryptographic operation. Burying it inside encrypt logic makes it untestable, and bugs here are catastrophic (encrypting to the wrong key).

**Do this instead:** `keys/convert.rs` with a dedicated `ed25519_pubkey_to_x25519(pubkey: &[u8; 32]) -> [u8; 32]` function that is unit-tested against known vectors.

### Anti-Pattern 3: Storing the X25519 key derived from Ed25519

**What people do:** Derive the X25519 identity once at init time, store it alongside the Ed25519 key.

**Why it's wrong:** It doubles secret key material on disk, increases attack surface, and is unnecessary — the conversion is deterministic and cheap.

**Do this instead:** Derive ephemerally at encrypt/decrypt time. Only the Ed25519 seed bytes live on disk.

### Anti-Pattern 4: Skipping the signature over the ciphertext envelope

**What people do:** Publish the age ciphertext directly with no outer authentication layer, relying on age's internal recipient check as the only integrity protection.

**Why it's wrong:** age verifies the recipient can decrypt, but doesn't authenticate the publisher. A malicious record on the homeserver could be substituted. A detached Ed25519 signature over the ciphertext proves the record was published by the keypair owner.

**Do this instead:** Sign `sha256(ciphertext)` with the Ed25519 secret key and include `{ciphertext_b64, signature_b64}` in the outer envelope JSON.

### Anti-Pattern 5: Monolithic main.rs

**What people do:** Put all subcommand logic in `main.rs` with a large `match`.

**Why it's wrong:** The file grows unbounded, the crypto + I/O + network + display concerns all interleave, and tests require mocking everything.

**Do this instead:** `main.rs` is 10-20 lines: parse args, build `AppCtx`, call `commands::dispatch(ctx, cmd)?`.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Pubky homeserver | pubky SDK `Client` (async HTTP, authenticated via Ed25519) | Homeserver URL from config; SDK handles auth token. SDK is rc — wrap it. |
| Mainline DHT (PKARR) | `pkarr::Client` (async) | Only needed for `--share` mode to resolve recipient's pubkey from their PKARR identity |
| `~/.claude/sessions/` | `std::fs::read_dir` | Read-only; select most recent modified UUID directory |
| `claude --resume` | `std::process::Command::exec` (Unix) | Only for `--exec` flag; replaces the cclink process entirely |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `commands/` ↔ `keys/` | Direct function call with `&AppCtx` | Commands never touch raw key bytes; KeyStore returns opaque typed keys |
| `commands/` ↔ `crypto/` | `encrypt(plaintext, recipient_pubkey) → Vec<u8>` | Pure functions; no I/O, no network |
| `commands/` ↔ `transport/` | Async `PubkyClient` methods via `AppCtx` | Transport is async; commands must be async or use block_on |
| `crypto/` ↔ `keys/` | `keys/convert.rs` exports conversion fn; `crypto/` imports it | One-way dependency: crypto uses key conversion, not vice versa |
| `commands/` ↔ `output/` | Output functions take typed values, not raw strings | Prevents formatting logic leaking into command logic |

### Suggested Build Order (Phase Dependencies)

The components have a clear dependency DAG that should drive phase ordering:

```
Phase 1 — Foundation
  keys/store.rs + keys/convert.rs
    (Ed25519 keypair generation, persistence, Ed25519→X25519 conversion)
    → enables: init, whoami, and all crypto/transport

Phase 2 — Core Crypto
  crypto/encrypt.rs + crypto/decrypt.rs (self-encrypt mode only)
    (age x25519 encrypt/decrypt using derived X25519 from Phase 1)
    → depends on: Phase 1
    → enables: publish/pickup without network

Phase 3 — Transport
  transport/record.rs + transport/pubky_client.rs
    (HandoffRecord JSON type, pubky SDK PUT/GET/DELETE)
    → depends on: Phase 1 (for authentication)
    → enables: network publish/pickup

Phase 4 — End-to-End Commands
  commands/publish.rs + commands/pickup.rs + session/discover.rs
    → depends on: Phases 1, 2, 3
    → first working user flow

Phase 5 — Secondary Encryption Modes
  crypto/hkdf.rs (PIN mode) + pkarr_client.rs (--share mode)
    → depends on: Phase 2 already established
    → enables: --pin, --share, --burn

Phase 6 — Polish Commands + Output
  commands/list.rs + commands/revoke.rs + output/qr.rs
    → depends on: Phase 3 transport
    → enables: full feature set
```

## Sources

- Kevin K's Blog "CLI Structure in Rust": https://kbknapp.dev/cli-structure-01/ — Context Struct pattern (HIGH confidence, Clap author)
- Rain's Rust CLI Recommendations: https://rust-cli-recommendations.sunshowers.io/handling-arguments.html — App/Command/Args hierarchy (HIGH confidence)
- pubky-cli source structure: https://github.com/pubky/pubky-cli — per-command module pattern with pubky SDK (MEDIUM confidence, active repo)
- pubky-core repository: https://github.com/pubky/pubky-core — homeserver PUT/GET/DELETE semantics (MEDIUM confidence, rc SDK)
- age crate docs: https://docs.rs/age — Encryptor/Decryptor x25519 API (HIGH confidence, stable crate)
- Filippo Valsorda on Ed25519→X25519: https://words.filippo.io/using-ed25519-keys-for-encryption/ — birational map cryptographic basis (HIGH confidence, written by age co-author)
- pkarr crate: https://lib.rs/crates/pkarr — Client, SignedPacket, Keypair (MEDIUM confidence, pubky ecosystem)
- Rust error handling: https://www.shakacode.com/blog/thiserror-anyhow-or-how-i-handle-errors-in-rust-apps/ — thiserror+anyhow pattern (HIGH confidence, widely adopted)

---
*Architecture research for: cclink — Rust CLI with Ed25519 identity, age encryption, Pubky publishing*
*Researched: 2026-02-21*
