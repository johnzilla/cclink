# Phase 3: Core Commands - Research

**Researched:** 2026-02-22
**Domain:** Claude Code session discovery, CLI UX (clap optional subcommands, colored output, QR codes, confirmation prompts), exponential backoff, process exec
**Confidence:** HIGH (session discovery, CLI patterns, clap, owo-colors); MEDIUM (backoff crate sync API, dialoguer integration); LOW (homeserver discovery for cross-user pickup)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Session discovery:**
- Auto-detect the most recent Claude Code session — no explicit session ID argument
- If multiple active sessions exist, list them and prompt the user to pick one
- After discovering a session, show the session ID and project/directory before publishing

**Publish output:**
- Default output: print a copyable `cclink pickup <token>` command and show TTL expiry ("Expires in 24h")
- QR code is opt-in via `--qr` flag — renders as Unicode block characters in the terminal when requested
- Default TTL is 24 hours (86400 seconds) — NOTE: this overrides REQUIREMENTS.md which says "8 hours"; user decision wins

**Pickup behavior:**
- Default action: decrypt the handoff and run `claude --resume <session-id>` automatically
- Before launching, show a confirmation prompt with session ID, project name, and how long ago it was published — user confirms with Y/n
- `--yes` / `-y` flag skips the confirmation and launches immediately

**Error & edge cases:**
- Expired handoff: clear message — "This handoff expired 3h ago. Publish a new one with cclink."
- No session found: helpful error — "No Claude Code session found. Start a session with 'claude' first."
- Network failures: retry 3 times with exponential backoff, then fail with a clear message

**Colored output:**
- Auto-detect TTY: colors when outputting to a terminal, plain text when piped
- Green for success, red for errors, yellow for warnings

### Claude's Discretion

- Exact retry backoff intervals
- QR code library/implementation
- Session discovery file paths and detection logic
- Exact layout/formatting of the publish success output
- Confirmation prompt styling

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope

</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SESS-01 | CLI can discover the most recent session ID from `~/.claude/sessions/` | Session files are at `~/.claude/projects/<encoded-cwd>/<uuid>.jsonl`; sort by mtime; read first progress record for cwd+sessionId |
| SESS-02 | User can provide an explicit session ID as a CLI argument | Top-level optional positional arg on the default (no subcommand) path |
| PUB-01 | User can publish an encrypted handoff record via `cclink` or `cclink <session-id>` | Uses existing `HomeserverClient::publish()` from Phase 2 |
| PUB-04 | User can set a custom TTL via `--ttl` (default 24h per CONTEXT) | Top-level `--ttl <secs>` flag; default 86400 |
| PUB-06 | Terminal QR code is rendered after successful publish | `qr2term` 0.3.3 crate; opt-in `--qr` flag |
| RET-01 | User can retrieve and decrypt their own latest handoff via `cclink pickup` | `HomeserverClient::get_latest(None)` + `get_record()` + `crypto::age_decrypt()` |
| RET-02 | User can retrieve another user's latest handoff via `cclink pickup <pubkey>` | `HomeserverClient::get_latest(Some(pubkey_z32))` + `get_record_by_pubkey()`; decrypt only works if pubkey matches own key (Phase 4 adds --share) |
| RET-03 | Expired records (past TTL) are refused on retrieval | Check `record.created_at + record.ttl < SystemTime::now() as secs`; return error with human-readable age |
| RET-04 | User can auto-execute `claude --resume <id>` via `cclink pickup --exec` | Default pickup behavior per CONTEXT; `--yes/-y` skips confirm; exec via `std::process::Command::exec()` (Unix) or `spawn()` (cross-platform) |
| RET-05 | User can display a scannable QR code via `cclink pickup --qr` | Same `qr2term` crate; render QR of session ID |
| RET-06 | Retrieval retries with backoff to handle DHT propagation delay | `backoff` 0.4 crate; 3 retries with exponential backoff |
| UX-01 | Colored terminal output with clear success/error states | `owo-colors` 4.x with `if_supports_color(Stdout, ...)` for TTY auto-detection |

</phase_requirements>

---

## Summary

Phase 3 wires together the Phase 2 building blocks (crypto module, HandoffRecord, HomeserverClient) into two user-facing commands. The core work breaks into five technical domains: session discovery (reading Claude Code's `~/.claude/projects/` directory), CLI restructuring (making the default `cclink` command publish via optional subcommand), UX polish (colors, QR codes, confirmation prompts), TTL enforcement, and retry/backoff.

The biggest discovery is the exact shape of Claude Code's session storage. Sessions live as JSONL files at `~/.claude/projects/<encoded-cwd>/<uuid>.jsonl`. The UUID in the filename IS the session ID. The `cwd` and `sessionId` are both available in the second line of each JSONL file (a `type=progress` record), making discovery reliable without parsing the full file. The encoded-cwd directory name uses a lossy formula (`-` + path with all `/` and `.` replaced by `-`), but reading the JSONL directly gives the exact path.

All new dependencies are minimal and well-established. The transport, record, and crypto modules from Phase 2 are complete and require no changes — Phase 3 only adds a `session` discovery module and two command handlers (`commands/publish.rs`, `commands/pickup.rs`) plus CLI restructuring.

**Primary recommendation:** Implement in three plans: (1) add dependencies + session discovery module, (2) implement the publish command end-to-end, (3) implement the pickup command end-to-end.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `owo-colors` | 4.2.3 | Colored terminal output with TTY auto-detection | Zero-allocation, no_std compatible; `if_supports_color(Stdout, ...)` handles TTY+NO_COLOR+CI detection in one call |
| `dialoguer` | 0.12.0 | Interactive `Select` (multi-session picker) and `Confirm` (pickup confirmation) prompts | Standard CLI prompt crate, pairs with `console` for terminal handling |
| `qr2term` | 0.3.3 | Print QR code as Unicode block chars to terminal | Simple one-call API (`print_qr(text)` or `generate_qr_string(text)`); built on `qrcode` + `crossterm` |
| `backoff` | 0.4.0 | Exponential backoff for network retries | Supports synchronous (non-async) retry via `retry()` closure; permanent vs transient error distinction |

### Supporting (Already in Cargo.toml)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `clap` | 4.5 | CLI parsing | Add `Option<Commands>` for optional subcommand; publish args become top-level |
| `serde_json` | 1.0 | Parse `~/.claude/history.jsonl` and session JSONL files | Already used for HandoffRecord serialization |
| `gethostname` | 0.5 | Get machine hostname for HandoffRecord | Already in Cargo.toml |
| `dirs` | 5 | Locate `~/.claude/` directory | Already in Cargo.toml |
| `std::process::Command` | stdlib | Exec `claude --resume <id>` | Use `exec()` on Unix (replaces process, no fork) |
| `std::io::IsTerminal` | stdlib (1.70+) | TTY detection fallback | `std::io::stdout().is_terminal()` — also handled by owo-colors |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `owo-colors` | `colored` | `colored` uses global state for color control; `owo-colors` is stream-aware and zero-allocation |
| `owo-colors` | `termcolor` (BurntSushi) | `termcolor` requires writing through a `ColorSpec` writer; owo-colors has simpler ergonomics for inline coloring |
| `backoff` | hand-rolled retry loop | `backoff` handles jitter, max elapsed time, and permanent vs transient error classification — don't hand-roll |
| `dialoguer` | `inquire` | `inquire` is heavier; `dialoguer` is the standard for simple Select/Confirm patterns |
| `qr2term` | `qrcode` crate directly | `qrcode` requires manual Unicode rendering; `qr2term` wraps this into one call |
| `std::process::Command::exec()` | `exec` crate | stdlib is sufficient; `exec()` on Unix replaces the process with no fork overhead |

**Installation:**
```bash
cargo add owo-colors dialoguer qr2term backoff
# Add features: owo-colors needs "supports-colors" feature
```

Cargo.toml additions:
```toml
owo-colors = { version = "4", features = ["supports-colors"] }
dialoguer = "0.12"
qr2term = "0.3"
backoff = "0.4"
```

---

## Architecture Patterns

### Recommended Project Structure

The Phase 2 source tree needs two additions:

```
src/
├── cli.rs              # CHANGED: command: Option<Commands>; add top-level publish args; add Pickup subcommand
├── commands/
│   ├── mod.rs          # CHANGED: add publish and pickup modules
│   ├── init.rs         # unchanged
│   ├── whoami.rs       # unchanged
│   ├── publish.rs      # NEW: run_publish(args)
│   └── pickup.rs       # NEW: run_pickup(args)
├── session/
│   └── mod.rs          # NEW: discover_sessions(), SessionInfo struct
├── crypto/mod.rs       # unchanged
├── error.rs            # CHANGED: add TTL-expired, session-not-found error variants
├── keys/               # unchanged
├── record/mod.rs       # unchanged
├── transport/mod.rs    # unchanged
└── main.rs             # CHANGED: match Option<Commands>; None => run_publish
```

### Pattern 1: Optional Subcommand (Default Publish)

**What:** Wrap the `Commands` enum in `Option<Commands>` in the Cli struct. When `None`, run the publish flow. When `Some(Commands::Pickup(...))`, run pickup.

**When to use:** When one subcommand is the default action (running the binary with no arguments).

**Example:**
```rust
// src/cli.rs
#[derive(Parser)]
#[command(name = "cclink", version, about = "Secure session handoff via Pubky")]
pub struct Cli {
    /// Optional session ID to publish (auto-discovers most recent if omitted)
    #[arg(value_name = "SESSION_ID")]
    pub session_id: Option<String>,

    /// Time-to-live in seconds (default: 86400 = 24 hours)
    #[arg(long, default_value = "86400")]
    pub ttl: u64,

    /// Render a QR code in the terminal after publish
    #[arg(long)]
    pub qr: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize or import a PKARR keypair
    Init(InitArgs),
    /// Show identity (public key, homeserver, fingerprint)
    Whoami,
    /// Pick up a session handoff from the homeserver
    Pickup(PickupArgs),
}

#[derive(Parser)]
pub struct PickupArgs {
    /// z32-encoded public key of the handoff publisher (defaults to own key)
    #[arg(value_name = "PUBKEY")]
    pub pubkey: Option<String>,

    /// Skip confirmation prompt and launch immediately
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Render a QR code showing the session ID
    #[arg(long)]
    pub qr: bool,
}
```

```rust
// src/main.rs
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init(args)) => commands::init::run_init(args)?,
        Some(Commands::Whoami) => commands::whoami::run_whoami()?,
        Some(Commands::Pickup(args)) => commands::pickup::run_pickup(args)?,
        None => commands::publish::run_publish(&cli)?,
    }

    Ok(())
}
```

### Pattern 2: Session Discovery

**What:** Scan `~/.claude/projects/` for all `*.jsonl` files, sort by mtime (most recent first), read the first `type=progress` record from each for the exact `cwd` and `sessionId`.

**Discovery algorithm:**
```
1. home_dir().join(".claude/projects/")
2. For each subdirectory in projects/:
     glob "*.jsonl" files (NOT subdirectories)
3. Sort all JSONL files by mtime descending
4. For each file (up to scan limit = 10):
     - session_id = filename stem (the UUID)
     - Open file, read lines until type=="progress" && cwd is non-empty
     - Extract cwd and sessionId fields
     - Filter: mtime within last 24 hours (defines "active")
5. Result: Vec<SessionInfo { session_id, cwd, mtime }>
```

**Important:** The UUID in the filename matches the `sessionId` in the JSONL records. No decoding of the directory name is needed — just read the JSONL's second line (line index 1) which is always a `type=progress` record containing `cwd` and `sessionId`.

**Example:**
```rust
// src/session/mod.rs
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub struct SessionInfo {
    pub session_id: String,
    pub project: String,  // cwd from JSONL progress record
    pub mtime: SystemTime,
}

pub fn discover_sessions() -> anyhow::Result<Vec<SessionInfo>> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    let projects_dir = home.join(".claude/projects");

    if !projects_dir.exists() {
        return Ok(vec![]);
    }

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(86400))  // 24h window for "active"
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut sessions: Vec<SessionInfo> = Vec::new();

    for project_dir in std::fs::read_dir(&projects_dir)? {
        let project_dir = project_dir?.path();
        if !project_dir.is_dir() { continue; }

        for entry in std::fs::read_dir(&project_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }

            let mtime = entry.metadata()?.modified()?;
            if mtime < cutoff { continue; }  // older than 24h, skip

            // Session ID = filename stem
            let session_id = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            // Read second line for cwd
            if let Ok(project) = read_session_cwd(&path) {
                sessions.push(SessionInfo { session_id, project, mtime });
            }
        }
    }

    // Sort by mtime descending (most recent first)
    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    Ok(sessions)
}

fn read_session_cwd(path: &std::path::Path) -> anyhow::Result<String> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(20) {  // check first 20 lines only
        let line = line?;
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(cwd) = obj.get("cwd").and_then(|v| v.as_str()) {
                return Ok(cwd.to_string());
            }
        }
    }
    anyhow::bail!("no cwd found in session file")
}
```

### Pattern 3: Colored Output with TTY Detection

**What:** Use `owo-colors` `if_supports_color` method which automatically checks TTY, NO_COLOR env var, FORCE_COLOR, and CI environment.

**Example:**
```rust
use owo_colors::{OwoColorize, Stream::Stdout};

// Success (green)
println!("{}", "Published successfully.".if_supports_color(Stdout, |t| t.green()));

// Error (red) - write to stderr
eprintln!("{}", "Error: record expired.".if_supports_color(Stderr, |t| t.red()));

// Warning (yellow)
println!("{}", "Warning: no session found in last 24h.".if_supports_color(Stdout, |t| t.yellow()));
```

**Requires `features = ["supports-colors"]` in Cargo.toml.**

### Pattern 4: Retry with Exponential Backoff

**What:** Use `backoff::retry()` with `ExponentialBackoff` for network calls that may fail due to DHT propagation delay.

**Example:**
```rust
use backoff::{retry, ExponentialBackoff, Error as BackoffError};
use std::time::Duration;

fn get_with_retry(client: &HomeserverClient, token: &str, pubkey: &pkarr::PublicKey)
    -> anyhow::Result<HandoffRecord>
{
    let backoff = ExponentialBackoff {
        max_elapsed_time: Some(Duration::from_secs(30)),  // 3 retries within 30s
        max_interval: Duration::from_secs(8),
        initial_interval: Duration::from_secs(2),
        ..Default::default()
    };

    retry(backoff, || {
        client.get_record(token, pubkey)
            .map_err(|e| {
                // Network errors are transient; 404 after max retries → permanent
                if e.to_string().contains("not found") {
                    BackoffError::permanent(e)
                } else {
                    BackoffError::transient(e)
                }
            })
    }).map_err(|e| anyhow::anyhow!("retrieval failed after retries: {}", e))
}
```

### Pattern 5: Process Exec for `claude --resume`

**What:** On Unix, use `std::os::unix::process::CommandExt::exec()` to replace the current process with `claude --resume <session-id>`. This gives the user a native claude session without a zombie parent process.

**Example:**
```rust
#[cfg(unix)]
use std::os::unix::process::CommandExt;

fn launch_claude_resume(session_id: &str) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("claude");
    cmd.arg("--resume").arg(session_id);

    #[cfg(unix)]
    {
        // exec() replaces the current process (no fork)
        let err = cmd.exec();
        Err(anyhow::anyhow!("failed to exec claude: {}", err))
    }
    #[cfg(not(unix))]
    {
        let status = cmd.status()?;
        if !status.success() {
            anyhow::bail!("claude exited with status {}", status);
        }
        Ok(())
    }
}
```

### Pattern 6: QR Code Rendering

**What:** Use `qr2term::print_qr(text)` to render a QR code to stdout. The text should be the full pickup command the user would run on their second machine.

**Example:**
```rust
// For publish: QR encodes the pickup command
let pickup_text = format!("cclink pickup {}", token);
qr2term::print_qr(&pickup_text)
    .map_err(|e| anyhow::anyhow!("QR code render failed: {}", e))?;

// For pickup: QR encodes the decrypted session ID
qr2term::print_qr(&session_id)
    .map_err(|e| anyhow::anyhow!("QR code render failed: {}", e))?;
```

### Pattern 7: Confirmation Prompt

**What:** Use `dialoguer::Confirm` for the pickup confirmation and `dialoguer::Select` for multi-session selection.

**Example:**
```rust
use dialoguer::{Confirm, Select, theme::ColorfulTheme};

// Pickup confirmation
let confirmed = Confirm::with_theme(&ColorfulTheme::default())
    .with_prompt(format!(
        "Resume session {} ({}) published {}?",
        session_id, project, human_age
    ))
    .default(true)
    .interact()?;

if !confirmed {
    println!("Aborted.");
    return Ok(());
}

// Multi-session selection
let items: Vec<String> = sessions.iter()
    .map(|s| format!("{} ({})", &s.session_id[..8], s.project))
    .collect();

let selection = Select::with_theme(&ColorfulTheme::default())
    .with_prompt("Multiple sessions found — pick one:")
    .items(&items)
    .default(0)
    .interact()?;

let chosen = &sessions[selection];
```

### Anti-Patterns to Avoid

- **Reading entire history.jsonl:** The file can have 1500+ entries. Scan `~/.claude/projects/` by mtime instead — O(files) not O(history entries).
- **Decoding project dir names:** The directory name encoding is lossy (`github.com` → `github-com`). Always read `cwd` from the JSONL file directly.
- **Using `is_terminal()` manually:** Let `owo-colors` `if_supports_color` handle TTY+NO_COLOR+CI detection — don't implement it yourself.
- **Using `std::process::Command::spawn()` for exec:** On Unix, `exec()` replaces the process; `spawn()` forks and leaves a zombie parent. Use `exec()` unless cross-platform is required.
- **Blocking on dialoguer with piped stdin:** `dialoguer` will panic or behave oddly if stdin is not a TTY. Guard with `std::io::stdin().is_terminal()` and skip prompts in non-interactive mode.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TTY detection + NO_COLOR env var | Manual `isatty()` + env::var() | `owo-colors` with `supports-colors` feature | Handles NO_COLOR, FORCE_COLOR, CI, Windows Terminal, Cygwin |
| Exponential backoff with jitter | Manual sleep loop with doubling | `backoff` 0.4 crate | Handles randomization factor, max elapsed time, permanent vs transient |
| QR code Unicode rendering | Own Unicode block char encoder | `qr2term` 0.3.3 | 200+ lines of rendering logic already tested |
| Interactive selection from list | Own stdin readline parser | `dialoguer::Select` | Handles arrow keys, terminal raw mode, cleanup |

**Key insight:** The retry, coloring, and QR domains each have 100-500 lines of non-obvious platform handling. Using the standard crates eliminates entire classes of bugs.

---

## Common Pitfalls

### Pitfall 1: TTL Default Discrepancy

**What goes wrong:** REQUIREMENTS.md says default TTL is "8 hours" but CONTEXT.md (user decision) says "24 hours." Using 8h produces wrong behavior per user decision.
**Why it happens:** Requirements were written before the user discussion; CONTEXT overrides them.
**How to avoid:** Use 86400 seconds (24 hours) as the clap `default_value` for `--ttl`.
**Warning signs:** Tests checking TTL default will pass with either value; only the user experience reveals the bug.

### Pitfall 2: Session Discovery Returning Stale Sessions

**What goes wrong:** Scanning all JSONL files by mtime includes sessions from weeks ago. The "multiple sessions" picker becomes noisy.
**Why it happens:** `~/.claude/projects/` accumulates hundreds of sessions across all projects.
**How to avoid:** Apply a 24-hour cutoff on mtime when building the "active sessions" list. Sessions older than 24h are excluded from auto-discovery.
**Warning signs:** The picker shows dozens of entries instead of 1-5.

### Pitfall 3: JSONL `cwd` Field Not on Line 1

**What goes wrong:** Reading only the first line of the JSONL file for `cwd` will get a `file-history-snapshot` record (no cwd), not the progress record.
**Why it happens:** Claude Code writes a snapshot record first, then a progress record.
**How to avoid:** Read lines until `type == "progress"` and `cwd` is present. Cap at 20 lines to avoid reading large files.
**Warning signs:** `project` field in HandoffRecord is empty.

### Pitfall 4: `dialoguer` on Non-TTY stdin

**What goes wrong:** `dialoguer::Confirm::interact()` panics or returns an error if stdin is not a terminal (e.g., piped, in CI).
**Why it happens:** dialoguer uses raw terminal mode for keypress handling.
**How to avoid:** Check `std::io::stdin().is_terminal()` before calling dialoguer. If false, treat as "no confirmation required" or error with a helpful message. The `--yes` flag also bypasses it.
**Warning signs:** CI tests crash at the confirmation step.

### Pitfall 5: owo-colors `supports-colors` Feature Not Enabled

**What goes wrong:** `if_supports_color()` method doesn't exist; `OwoColorize` only provides always-on color methods.
**Why it happens:** Stream-aware detection is behind the `supports-colors` feature flag.
**How to avoid:** Cargo.toml must have `owo-colors = { version = "4", features = ["supports-colors"] }`.
**Warning signs:** Compile error "no method named `if_supports_color`".

### Pitfall 6: Pickup `<pubkey>` Decryption Failure (RET-02)

**What goes wrong:** When retrieving another user's handoff (`cclink pickup <their-pubkey>`), decryption fails because the blob is encrypted to THEIR X25519 key, not yours.
**Why it happens:** Phase 3 only implements self-encrypted handoffs. Cross-user decryption requires `--share` (Phase 4, ENC-01).
**How to avoid:** For Phase 3, if the pubkey argument differs from own pubkey, inform the user: the record can be verified (signature) but not decrypted. Only print the project/hostname metadata from the (cleartext) HandoffRecord fields.
**Warning signs:** age decrypt returns "no matching identities" error.

### Pitfall 7: `backoff::retry()` Treats All Errors as Transient

**What goes wrong:** 404 Not Found errors are retried 3+ times unnecessarily; the 404 is not a transient network issue.
**Why it happens:** `retry()` wraps the error as transient unless you explicitly wrap with `BackoffError::permanent()`.
**How to avoid:** Map 404/"not found" errors to `BackoffError::permanent()` so they fail immediately.
**Warning signs:** A missing record causes 30 seconds of retry delay.

---

## Code Examples

### Session File Location (Verified on This Machine)

```
# Sessions are at:
~/.claude/projects/<encoded-cwd>/<uuid>.jsonl

# Encoding formula (verified):
encoded_cwd = '-' + cwd.lstrip('/').replace('/', '-').replace('.', '-')

# Example:
# cwd = /home/john/vault/projects/github.com/cclink
# dir = -home-john-vault-projects-github-com-cclink
```

### JSONL Line Structure (Line 0 = snapshot, Line 1 = progress with cwd)

```json
// Line 0: file-history-snapshot (no cwd)
{"type":"file-history-snapshot","messageId":"...","snapshot":{...}}

// Line 1: progress record (has cwd + sessionId)
{"parentUuid":null,"isSidechain":false,"userType":"external",
 "cwd":"/home/john/vault/projects/github.com/cclink",
 "sessionId":"0099a04d-b758-4a4c-81c6-950a414e57bd",
 "version":"2.1.50","gitBranch":"main","type":"progress",...}
```

### Publish Flow (End-to-End)

```rust
pub fn run_publish(cli: &Cli) -> anyhow::Result<()> {
    // 1. Load keypair
    let keypair = keys::store::load_keypair()?;
    let homeserver = keys::store::read_homeserver()?;

    // 2. Discover or use explicit session
    let session = if let Some(explicit_id) = &cli.session_id {
        // SESS-02: explicit session ID provided
        SessionInfo {
            session_id: explicit_id.clone(),
            project: std::env::current_dir()?.to_string_lossy().into(),
            mtime: SystemTime::now(),
        }
    } else {
        // SESS-01: auto-discover
        let sessions = session::discover_sessions()?;
        match sessions.len() {
            0 => anyhow::bail!("No Claude Code session found. Start a session with 'claude' first."),
            1 => sessions.into_iter().next().unwrap(),
            _ => {
                // Prompt user to pick one
                let items: Vec<String> = sessions.iter()
                    .map(|s| format!("{} ({})", &s.session_id[..8], s.project))
                    .collect();
                let selection = dialoguer::Select::new()
                    .with_prompt("Multiple sessions found — pick one:")
                    .items(&items)
                    .default(0)
                    .interact()?;
                sessions.into_iter().nth(selection).unwrap()
            }
        }
    };

    // 3. Show what we found
    println!("Discovered: {} in {}", session.session_id, session.project);

    // 4. Encrypt session ID -> blob
    let x25519_secret = crypto::ed25519_to_x25519_secret(&keypair);
    let x25519_pubkey = crypto::ed25519_to_x25519_public(&keypair);
    let recipient = crypto::age_recipient(&x25519_pubkey);
    let ciphertext = crypto::age_encrypt(session.session_id.as_bytes(), &recipient)?;
    let blob = base64::engine::general_purpose::STANDARD.encode(&ciphertext);

    // 5. Build and sign record
    let created_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
    let hostname = gethostname::gethostname().to_string_lossy().into_owned();

    let signable = record::HandoffRecordSignable {
        blob,
        created_at,
        hostname,
        project: session.project.clone(),
        pubkey: keypair.public_key().to_z32(),
        ttl: cli.ttl,
    };
    let signature = record::sign_record(&signable, &keypair)?;
    let handoff_record = record::HandoffRecord { ...signable fields..., signature };

    // 6. Publish
    let client = transport::HomeserverClient::new(&homeserver)?;
    let token = client.publish(&keypair, &handoff_record)?;

    // 7. Output
    let hours = cli.ttl / 3600;
    println!("{}", "Published!".if_supports_color(Stdout, |t| t.green()));
    println!("  cclink pickup {}", token);
    println!("  Expires in {}h", hours);

    // 8. Optional QR
    if cli.qr {
        qr2term::print_qr(&format!("cclink pickup {}", token))?;
    }

    Ok(())
}
```

### TTL Expiry Check (RET-03)

```rust
fn check_ttl(record: &HandoffRecord) -> anyhow::Result<()> {
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let expires_at = record.created_at.saturating_add(record.ttl);
    if now_secs >= expires_at {
        let expired_secs = now_secs.saturating_sub(expires_at);
        let expired_human = human_duration(expired_secs);
        anyhow::bail!(
            "This handoff expired {} ago. Publish a new one with cclink.",
            expired_human
        );
    }
    Ok(())
}

fn human_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h", secs / 3600)
    } else if secs >= 60 {
        format!("{}m", secs / 60)
    } else {
        format!("{}s", secs)
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `colored` crate (global state) | `owo-colors` with `if_supports_color` | 2022+ | Stream-aware; no global mutex; NO_COLOR/CI aware |
| `isatty` libc call | `std::io::IsTerminal` trait (stdlib) | Rust 1.70 (2023) | Cross-platform; handles Windows Terminal correctly |
| Custom terminal size/raw mode | `crossterm` (pulled by qr2term) | 2020+ | Cross-platform terminal control unified |
| `reqwest::blocking` poll loop | `backoff` crate for retry | Ongoing | Proper jitter and backoff strategy instead of naive sleep |

**Deprecated/outdated:**
- `colored` crate: Not stream-aware; uses global bool for enable/disable. Replaced by `owo-colors` for new code.
- Manual exponential backoff: `thread::sleep(Duration::from_secs(2_u64.pow(attempt)))` is naive. `backoff` adds jitter to prevent thundering herd.

---

## Open Questions

1. **What should `cclink pickup <pubkey>` do when the record is encrypted to THEIR key?**
   - What we know: Phase 3 only implements self-encryption (PUB-03). The blob cannot be decrypted without the creator's private key.
   - What's unclear: Should Phase 3 silently fail on decryption and show only cleartext metadata (project, hostname, created_at)? Or should it error?
   - Recommendation: Show cleartext metadata (project, hostname, age) and note "decryption requires the creator's key; use `--share` to create shared handoffs." This is better UX than an error.

2. **How should "multiple active sessions" be defined for the picker?**
   - What we know: CONTEXT says "if multiple active sessions exist, list them and prompt." Sessions within 24h mtime are "active."
   - What's unclear: If the user has 5 sessions modified within 24h across different projects, that's a lot of choice. Should we cap at 5?
   - Recommendation: Cap the picker at 5 sessions; show full project path for each entry.

3. **Homeserver discovery for cross-user pickup (RET-02)?**
   - What we know: PKARR can store DNS-like records including homeserver location, but requires DHT lookup with the `pkarr` 5.0.3 `dht` feature (not enabled — current Cargo.toml uses `features = ["keys"]` only).
   - What's unclear: Does `pubky.app` serve as the homeserver for all users by default, making homeserver discovery unnecessary for Phase 3?
   - Recommendation: For Phase 3, assume the caller's configured homeserver is the same for both parties (pubky.app default). Document the limitation. DHT-based homeserver resolution is Phase 4+ work.

---

## Sources

### Primary (HIGH confidence)

- Empirical observation of `~/.claude/projects/` on this machine — session file format, UUID=sessionId, JSONL line structure, encoding formula
- `claude --help` output (version 2.1.50) — confirmed `--resume <session-id>` flag
- `~/.claude/history.jsonl` — confirmed `{sessionId, project, timestamp}` format
- `/home/john/vault/projects/github.com/cclink/src/transport/mod.rs` — confirmed `get_record_by_pubkey`, `get_latest`, `publish` API already implemented
- `/home/john/vault/projects/github.com/cclink/src/crypto/mod.rs` — confirmed `age_encrypt`, `age_decrypt`, `age_identity`, `age_recipient` API
- `/home/john/vault/projects/github.com/cclink/Cargo.toml` — confirmed existing dependencies
- `std::os::unix::process::CommandExt::exec()` stdlib docs — confirmed process replacement behavior
- `clap` derive tutorial on docs.rs — confirmed `Option<Commands>` pattern for optional subcommand

### Secondary (MEDIUM confidence)

- [docs.rs/qr2term/0.3.3](https://docs.rs/qr2term/0.2.1/qr2term/) — confirmed `print_qr(text)` and `generate_qr_string(text)` API; version 0.3.3 on crates.io
- [docs.rs/owo-colors/4.x](https://docs.rs/owo-colors/4.2.3/owo_colors/) — confirmed `if_supports_color(Stream, closure)` API; requires `supports-colors` feature
- [docs.rs/dialoguer/0.12.0](https://docs.rs/dialoguer/latest/dialoguer/) — confirmed `Select` and `Confirm` structs; version 0.12.0
- [docs.rs/backoff](https://docs.rs/backoff) — confirmed synchronous `retry()` API, `ExponentialBackoff`, `BackoffError::permanent()` vs `transient()`

### Tertiary (LOW confidence)

- Homeserver discovery via PKARR DHT: Noted from pubky docs but not verified for pkarr 5.0.3 with `features = ["keys"]`; DHT feature is separate and not currently enabled.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries verified against docs.rs; existing Cargo.toml deps confirmed
- Architecture: HIGH — session file format empirically verified; clap pattern from docs; code patterns match existing Phase 1/2 style
- Pitfalls: HIGH — session JSONL line format empirically verified; TTL discrepancy documented from source files; owo-colors feature requirement from docs
- Cross-user decryption limitation: HIGH — follows directly from PUB-03 design; blob encrypted to self-key cannot be decrypted by others without --share
- Homeserver discovery: LOW — not empirically verified; assumption carries risk for RET-02

**Research date:** 2026-02-22
**Valid until:** 2026-03-22 (stable deps, low churn)
