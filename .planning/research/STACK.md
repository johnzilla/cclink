# Stack Research

**Domain:** Rust CLI — Dependency audit, CI hardening, ed25519-dalek pre-release management
**Researched:** 2026-02-23
**Confidence:** HIGH (pkarr dependency tree verified live via crates.io API; cargo audit run live against the project; CI action versions verified via GitHub)

---

## Context: What This Research Covers

This is a **subsequent milestone** research pass. The core stack (pkarr, age, clap, argon2, etc.) is validated and unchanged. This file covers only what is needed for the v1.2 milestone:

1. Whether ed25519-dalek can be upgraded from `=3.0.0-pre.5` to a stable or newer release
2. What GitHub Action to use for `cargo audit` in CI
3. How to add `cargo clippy` to CI correctly
4. The `backoff` unmaintained advisory and its resolution

---

## Recommended Stack

### Core Technologies (unchanged from v1.1)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `pkarr` | 5.0.3 | Mainline DHT transport, Ed25519 SignedPackets | Locked. Do not change — v1.2 scope excludes transport changes. |
| `ed25519-dalek` | `=3.0.0-pre.5` → `=3.0.0-pre.6` | Ed25519 signing (pulled in by pkarr) | See version analysis below. Update pin from pre.5 to pre.6. |
| `age` | 0.11.x | X25519 encryption | Unchanged. |
| `argon2` | 0.5 | Argon2id for PIN hashing | Unchanged. |

### CI Tooling (new for v1.2)

| Tool | Version/Tag | Purpose | Why Recommended |
|------|-------------|---------|-----------------|
| `actions-rust-lang/audit` | `v1` (v1.2.7, Jan 2026) | Cargo audit in CI — runs cargo-audit against RustSec advisory DB | The current standard. Actively maintained, creates GitHub issues for vulnerabilities, has configurable ignore list. Supersedes the older `rustsec/audit-check` action. |
| `cargo clippy` | Bundled with `dtolnay/rust-toolchain@stable` | Lint enforcement | Clippy is pre-installed on GitHub runners with the rust-toolchain action. No separate action needed — run directly with `cargo clippy -- -D warnings`. |
| `cargo-deny` | Alternative to cargo-audit | Combined license + advisory checking | Not recommended for this project — cargo-audit is simpler and the project has no license complexity. |

---

## The ed25519-dalek Constraint: What Can Be Changed

### The Hard Constraint

**pkarr 5.0.3 requires `ed25519-dalek = "^3.0.0-pre.1"`.**

This was verified live from the crates.io dependency API. The `^` operator applied to a pre-release version in Cargo semver means: "compatible with 3.0.0-pre.1, at minimum." In practice, Cargo resolves this to the highest available `3.0.0-pre.x` that satisfies the constraint.

**Implication: stable ed25519-dalek 2.2.0 cannot be used.** pkarr 5.0.3 will not resolve against it because `2.x` is a different major version and predates the `3.0.0-pre.1` minimum.

### Why There Is No Stable 3.x

As of 2026-02-23, ed25519-dalek has no stable 3.0.0 release. The 3.x series is entirely pre-release. The latest pre-release is `3.0.0-pre.6` (released 2026-02-04). The latest stable is `2.2.0` (released 2025-07-09).

The project is actively moving toward stable 3.0 — recent pre-releases (pre.3 through pre.6) only updated `rand_core`, `digest`, and `sha2` dependencies, indicating the API is stabilizing.

### Current vs Upgradeable

| Version | Status | Pkarr Compatible | Notes |
|---------|--------|-----------------|-------|
| `2.2.0` | Stable | NO | Different major; pkarr requires `^3.0.0-pre.1` |
| `3.0.0-pre.5` | Pre-release | YES | Current pin in Cargo.toml |
| `3.0.0-pre.6` | Pre-release | YES | Latest; released 2026-02-04 |
| `3.0.0` | Does not exist yet | — | No ETA |

### Recommendation: Upgrade Pin to 3.0.0-pre.6

Change Cargo.toml from:
```toml
ed25519-dalek = "=3.0.0-pre.5"
```
to:
```toml
ed25519-dalek = "=3.0.0-pre.6"
```

This is a routine dependency bump within the same API surface. pre.4 through pre.6 only changed downstream crate versions (rand_core, digest, sha2); no API breaks were introduced between pre.5 and pre.6.

**Do not remove the `=` exact pin.** Without it, Cargo resolves pre-releases non-deterministically and `cargo update` may silently pick a new pre-release that breaks the build. The exact pin is correct practice for pre-releases.

**Note:** `cargo update --dry-run --verbose` confirms that Cargo cannot update ed25519-dalek beyond pre.5 under the current `=` pin, and confirms pre.6 is available. The update from pre.5 to pre.6 requires a manual Cargo.toml edit.

### What Changes in 3.0.0-pre vs 2.x API

The major API break happened at 2.0.0 (already adopted). The 3.0.0-pre.0 changes are:
- Edition upgraded to 2024, MSRV raised to 1.85
- `std` feature removed
- `pkcs8::spki::SignatureAlgorithmIdentifier` replaces `DynSignatureAlgorithmIdentifier`
- `ed25519` and `signature` dependency versions updated

These are not relevant to cclink, which uses `pkarr::Keypair` as its key type and never calls ed25519-dalek's API directly. The pkarr crate handles all ed25519-dalek interactions internally.

---

## Cargo Audit CI Integration

### Current Audit Status (verified live)

Running `cargo audit` against the current lockfile produces:

```
Warning: backoff 0.4.0 — RUSTSEC-2025-0012 (unmaintained)
Warning: instant 0.1.13 — RUSTSEC-2024-0384 (unmaintained)
```

No vulnerabilities (security errors). Two warnings for unmaintained crates. Both are transitive — `instant` is a transitive dependency of `backoff`.

**`backoff` is a direct dependency** and causes both advisories. The fix is replacing `backoff` with `backon` (see below).

### Recommended GitHub Action

Use `actions-rust-lang/audit@v1` (latest tag: v1.2.7, released January 2026).

```yaml
- name: Audit Rust dependencies
  uses: actions-rust-lang/audit@v1
  with:
    ignore: ""  # leave empty unless a known-safe advisory needs ignoring
```

**Why this action over `rustsec/audit-check@v2`:**
- `actions-rust-lang/audit` is newer (v1.2.7, Jan 2026 vs v2.0.0, Sep 2024)
- Creates GitHub issues for found vulnerabilities automatically (configurable)
- Does not require a separate `GITHUB_TOKEN` parameter — defaults to the workflow token
- Handles the advisory DB fetch internally

### Recommended Workflow Additions to ci.yml

Add two new jobs alongside the existing `test` job:

```yaml
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/audit@v1

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all-targets -- -D warnings
```

**Why separate jobs:** Audit and clippy failures are independent concerns. Parallel jobs report faster and give clearer failure attribution than a single long job.

**Why `RUSTFLAGS: "-Dwarnings"` is not used:** The official Clippy docs recommend passing `-D warnings` directly to the clippy invocation (`-- -D warnings`) rather than via `RUSTFLAGS`. The `RUSTFLAGS` approach also affects `rustc` itself, which can cause false failures from compiler warnings unrelated to clippy lints.

**Clippy components:** The `dtolnay/rust-toolchain@stable` action supports `components: clippy` to ensure clippy is installed even if the runner image doesn't have it. This prevents silent skips.

---

## The backoff Advisory: What to Do

### Advisory Details

- **RUSTSEC-2025-0012**: `backoff` 0.4.0 is unmaintained (issued 2025-03-04)
- **RUSTSEC-2024-0384**: `instant` 0.1.13 is unmaintained (transitive dep of backoff)

Both are warnings, not vulnerabilities. `cargo audit` exits 0 with these. However, they will generate noise in CI and may escalate if `actions-rust-lang/audit` is configured with `denyWarnings: true`.

### How backoff Is Used

```rust
// src/commands/pickup.rs
use backoff::{retry, ExponentialBackoff, Error as BackoffError};
let backoff_config = ExponentialBackoff {
    max_elapsed_time: Some(Duration::from_secs(30)),
    max_interval: Duration::from_secs(8),
    initial_interval: Duration::from_secs(2),
    ..Default::default()
};
let record = retry(backoff_config, || { ... });
```

Simple retry loop with exponential backoff. No async, no tokio integration.

### Recommended Replacement: backon 1.6.0

`backon` (v1.6.0, released 2025-10-18) is the standard maintained alternative. It is actively maintained, has no `instant` dependency, and supports both sync and async retry.

**Migration approach:** Replace the `backoff::retry()` call with `backon`'s closure-based retry builder. The API differs (backon uses a builder pattern: `.retry(ExponentialBuilder::default())`), but the logic is equivalent.

**Alternative — ignore the advisory:** If replacing `backoff` is out of scope for v1.2, add a `.cargo/audit.toml` to suppress the two warnings:

```toml
[advisories]
ignore = ["RUSTSEC-2025-0012", "RUSTSEC-2024-0384"]
```

This is appropriate if the CI gate should pass cleanly now and `backoff` replacement is deferred to v1.3. The unmaintained status poses no security risk — it means no future patches, not that it is currently broken or exploitable.

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `actions-rust-lang/audit@v1` | `rustsec/audit-check@v2` | Use `audit-check@v2` if you specifically need GitHub check annotation styling rather than issue creation. Both work; `actions-rust-lang/audit` is more recently maintained. |
| `cargo clippy -- -D warnings` | `RUSTFLAGS="-Dwarnings" cargo clippy` | Use `RUSTFLAGS` only if you also want to deny rustc compiler warnings at the same time. For clippy-only enforcement, pass `-D warnings` to clippy directly. |
| `backon` 1.6.0 | Ignore RUSTSEC-2025-0012 | Ignore the advisory if the v1.2 scope is tightly constrained. The advisory is "unmaintained," not a security CVE. |
| `=3.0.0-pre.6` exact pin | Loose `^3.0.0-pre.1` | Loose pin only when you trust all pre-release bumps in the series to be non-breaking. For crypto crates at pre-release, exact pinning is safer. |

---

## What NOT to Change

| Do Not Change | Why |
|---------------|-----|
| `pkarr = "5.0.3"` | Stable, tested transport layer. v1.2 scope excludes transport changes. |
| `age = "0.11"` | Unchanged encryption layer. No advisories. |
| `argon2 = "0.5"` | PIN hashing. No advisories. |
| `ed25519-dalek` to stable `2.2.0` | Impossible — pkarr 5.0.3 requires `^3.0.0-pre.1`. |
| Existing `test` job in ci.yml | Add new jobs; do not modify the passing test job. |
| `--locked` flag on `cargo test` | The lockfile must remain authoritative. Do not remove `--locked`. |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `pkarr@5.0.3` | `ed25519-dalek@^3.0.0-pre.1` | Verified via crates.io API. pre.5 and pre.6 both satisfy this constraint. |
| `ed25519-dalek@3.0.0-pre.6` | `curve25519-dalek@5.0.0-pre.6` | pre.6 bumps curve25519-dalek from pre.5 to pre.6. No user-visible API change. |
| `actions-rust-lang/audit@v1` | Any Rust project with Cargo.lock | No Rust toolchain installation needed; action handles it internally. |
| `backon@1.6.0` | Rust stable, no-std compatible | Does not depend on `instant`; no async runtime required for sync retry. |

---

## Sources

- `crates.io/api/v1/crates/pkarr/5.0.3/dependencies` — pkarr 5.0.3 requires `ed25519-dalek ^3.0.0-pre.1` (HIGH confidence, verified live)
- `crates.io/api/v1/crates/pkarr` — Latest pkarr is 6.0.0-rc.0; stable is 5.0.3 (HIGH confidence, verified live)
- `crates.io/api/v1/crates/ed25519-dalek` — Latest stable 2.2.0 (Jul 2025); latest pre-release 3.0.0-pre.6 (Feb 2026) (HIGH confidence, verified live)
- `cargo audit` run live in project — two unmaintained warnings (backoff, instant), zero vulnerabilities (HIGH confidence)
- `github.com/actions-rust-lang/audit` — v1.2.7, January 2026. Recommended workflow YAML verified (HIGH confidence)
- `github.com/rustsec/audit-check` — v2.0.0, September 2024. Still maintained but older (HIGH confidence)
- `doc.rust-lang.org/nightly/clippy/continuous_integration/github_actions.html` — Official Clippy CI recommendation: `cargo clippy --all-targets --all-features` with `-Dwarnings` (HIGH confidence)
- `crates.io/api/v1/crates/backon` — backon 1.6.0, released 2025-10-18 (HIGH confidence, verified live)
- `rustsec.org/advisories/RUSTSEC-2025-0012` — backoff unmaintained advisory, issued 2025-03-04 (HIGH confidence)
- `rustsec.org/advisories/RUSTSEC-2024-0384` — instant unmaintained advisory (HIGH confidence)
- `cargo update ed25519-dalek --dry-run --verbose` run live — confirms pre.6 is available, current pin blocks the update (HIGH confidence)

---
*Stack research for: cclink v1.2 — dependency audit and CI hardening*
*Researched: 2026-02-23*
