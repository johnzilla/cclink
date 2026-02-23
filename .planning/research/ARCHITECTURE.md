# Architecture Research

**Domain:** Rust CLI — dependency audit, PIN enforcement, CI hardening, tech debt (v1.2)
**Researched:** 2026-02-23
**Confidence:** HIGH — codebase read directly; CI patterns verified against official Clippy docs and RustSec; ed25519-dalek version confirmed via cargo tree and lib.rs

---

## Context: This Is a Subsequent Milestone

The existing architecture is stable. This document focuses on **integration points for v1.2 changes only** — where new code lands, what existing files are modified, and in what order work should proceed. For the full system architecture see the v1.1 research (2026-02-21).

---

## Existing System Overview

```
src/
├── main.rs           # Entry: parse CLI, dispatch to command handlers
├── cli.rs            # Clap derive structs (Cli, Commands, PickupArgs, etc.)
├── commands/
│   ├── publish.rs    # Publish flow: discover → encrypt → sign → DHT put
│   ├── pickup.rs     # Pickup flow: DHT get → verify → decrypt → exec claude
│   ├── init.rs
│   ├── list.rs
│   ├── revoke.rs
│   └── whoami.rs
├── crypto/mod.rs     # All crypto: age enc/dec, pin_derive_key, pin_encrypt, pin_decrypt
├── record/mod.rs     # HandoffRecord, HandoffRecordSignable, sign/verify, Payload
│                     # Also: LatestPointer (dead code — #[allow(dead_code)])
├── transport/mod.rs  # DhtClient wrapping pkarr::ClientBlocking
├── keys/
│   ├── store.rs      # load_keypair / write keypair, 0600 permissions
│   └── fingerprint.rs
├── session/mod.rs    # discover_sessions(), SessionInfo
├── error.rs          # CclinkError variants
└── util.rs           # human_duration()

.github/workflows/
├── ci.yml            # cargo test --locked + OpenSSL check  ← ADD clippy + audit here
└── release.yml       # 4-platform release + crates.io publish  ← no changes

Cargo.toml            # ed25519-dalek = "=3.0.0-pre.5"  ← upgrade pin, fix URLs
install.sh            # REPO="user/cclink"  ← fix placeholder
```

---

## Integration Point 1: PIN Length Validation

**Question:** Where does minimum PIN length belong — cli.rs, publish.rs, or crypto/mod.rs?

**Answer: `src/commands/publish.rs`, immediately after `dialoguer::Password::interact()`, before calling `crypto::pin_encrypt`.**

Rationale:
- `cli.rs` only sees `--pin: bool`. The PIN value is entered interactively, not passed as a CLI argument. There is no place in `cli.rs` to validate the value.
- `crypto/mod.rs` is a pure crypto layer. Minimum length is application policy, not a cryptographic constraint. Argon2id accepts any non-empty byte string. Injecting a length check into `pin_derive_key` or `pin_encrypt` couples UX policy to the crypto primitive and makes crypto tests require policy-conforming test inputs.
- `publish.rs` owns the interactive prompt at lines 99-103. It is the earliest point where the PIN string is available and the natural place for application-level policy.

**Exact insertion point** in `publish.rs` (the `if cli.pin {` block, currently lines 97-108):

```rust
let pin = dialoguer::Password::new()
    .with_prompt("Enter PIN for this handoff")
    .with_confirmation("Confirm PIN", "PINs don't match")
    .interact()
    .map_err(|e| anyhow::anyhow!("PIN prompt failed: {}", e))?;

// ADD: enforce minimum PIN length policy
const MIN_PIN_LEN: usize = 4;   // decide the exact number
if pin.len() < MIN_PIN_LEN {
    anyhow::bail!("PIN must be at least {} characters", MIN_PIN_LEN);
}

let (ciphertext, salt) = crate::crypto::pin_encrypt(&payload_bytes, &pin)?;
```

**Files modified:** `src/commands/publish.rs` only. No changes to cli.rs, crypto/mod.rs, or any other module.

---

## Integration Point 2: ed25519-dalek Version

**Question:** Is the upgrade a Cargo.toml-only change, or does code reference dalek types directly?

**Answer: The upgrade is not Cargo.toml-only. Two source locations reference dalek types directly. But the API surface used has been stable within the 3.x pre-release series.**

**Dependency tree confirmed via `cargo tree`:**

```
ed25519-dalek v3.0.0-pre.5
├── cclink v0.1.0                  ← direct dep in Cargo.toml
├── mainline v6.1.1
│   └── pkarr v5.0.3               ← pkarr also depends on ed25519-dalek 3.x
│       └── cclink v0.1.0
└── pkarr v5.0.3 (*)
```

pkarr 5.0.3 itself depends on `ed25519-dalek 3.0.0-pre.1` (confirmed from pubky/pkarr main branch Cargo.toml). The Cargo resolver unifies both to `3.0.0-pre.5` because cclink's exact pin is newer within the pre-release series.

**The two direct dalek type references in cclink source:**

| File | Line | Usage | Present in 2.x? |
|------|------|-------|-----------------|
| `src/crypto/mod.rs:22` | `ed25519_dalek::SigningKey::from_bytes(&keypair.secret_key())` then `.to_scalar_bytes()` | Ed25519 seed → X25519 scalar | `to_scalar_bytes()` is 3.x only; 2.x uses `expand()` instead |
| `src/record/mod.rs:186` | `ed25519_dalek::Signature::from_bytes(&sig_array)` | Byte array → Signature type | API exists in 2.x but different types |

**Version landscape (confirmed 2026-02-23):**

- Latest stable: `2.2.0` (released 2025-07-09)
- Latest pre-release: `3.0.0-pre.6` (released 2026-02-04)
- No stable 3.x release exists

**Upgrade recommendation:**

Stay in the `3.0.0-pre.x` series. Upgrading to `3.0.0-pre.6` from `pre.5`:
- Change `Cargo.toml`: `ed25519-dalek = "=3.0.0-pre.6"`
- Verify the two dalek type usages still compile unchanged (the API used has been stable across pre.1 through pre.5)
- Run `cargo test --locked` to confirm no regressions

**Do not downgrade to 2.x:** pkarr 5.0.3 requires ed25519-dalek 3.x. A 2.x pin would create an unresolvable dependency conflict. Additionally, `to_scalar_bytes()` in `crypto/mod.rs:23` does not exist in 2.x, so the code would not compile without a rewrite of the X25519 derivation path.

**Files modified:** `Cargo.toml` only (if pre.5 → pre.6 has no API breaks). `src/crypto/mod.rs` and/or `src/record/mod.rs` only if the pre-release API changed.

---

## Integration Point 3: CI — Adding Clippy and Cargo Audit

**Question:** How do CI changes integrate with the existing workflow files?

**Answer: Add a parallel `lint` job to `ci.yml`. Do not touch `release.yml`.**

**Current `ci.yml` structure:**
```yaml
jobs:
  test:    # cargo test --locked + OpenSSL check
```

**Proposed addition:**
```yaml
jobs:
  test:
    # existing job — unchanged

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - name: Run Clippy
        run: cargo clippy --all-targets -- -D warnings
      - name: Security audit
        run: |
          cargo install --locked cargo-audit
          cargo audit
```

**Design decisions:**

**Separate job (not appended to `test`):** Clippy and test failures are distinct. Parallel execution. Clippy job uses the same Swatinem/rust-cache which is keyed per-job but shares the build artifact cache.

**`-- -D warnings` (not `RUSTFLAGS: "-Dwarnings"`):** Setting `RUSTFLAGS` at the job level would affect `cargo install cargo-audit` and every `cargo` invocation in the job, causing unexpected failures. Passing `-D warnings` as a Clippy argument affects only Clippy.

**Prerequisites before adding `-D warnings`:** The current codebase has 2 Clippy warnings that would fail CI immediately. Both are in test files and are the same issue (`empty_line_after_doc_comments`):

| File | Issue |
|------|-------|
| `tests/plaintext_leak.rs:1-9` | `///` doc comments on lines 1-8, blank line 9, then `use` on line 10 |
| `tests/integration_round_trip.rs:1-12` | Same pattern |

Fix: change outer doc comments (`///`) at the top of each test file to inner doc comments (`//!`). This is a 2-line change per file.

**`cargo audit` without `--deny warnings`:** The current audit output shows two non-critical advisories:

| Advisory | Crate | Type | Root cause |
|----------|-------|------|------------|
| RUSTSEC-2025-0012 | `backoff 0.4.0` | Unmaintained | cclink uses `backoff::retry` in `pickup.rs` |
| RUSTSEC-2024-0384 | `instant 0.1.13` | Unmaintained | Transitive dependency of `backoff` |

Starting with `cargo audit` (no `--deny`) establishes a baseline. Add `--deny warnings` only after resolving or explicitly ignoring known advisories. Alternative: `cargo audit --ignore RUSTSEC-2025-0012 --ignore RUSTSEC-2024-0384` to acknowledge known non-critical issues while catching new ones.

**`cargo install --locked cargo-audit` vs a marketplace action:** Using `cargo install` directly avoids a third-party action dependency (e.g., `actions-rs/audit-check@v1`). It is slower (~30-60s for the install) but simpler. The `actions-rs` org is minimally maintained. Use `cargo install` unless build speed becomes a concern.

**Files modified:** `.github/workflows/ci.yml` only.

---

## Integration Point 4: Dead Code — LatestPointer

**Location:** `src/record/mod.rs`

`LatestPointer` was the homeserver-era "latest.json" pointer. The struct definition is at lines 91-106 and is annotated `#[allow(dead_code)]`. It has zero usages outside its own module — no other module imports or constructs it. The only reference is a serialization test at lines 376-392.

```rust
// Lines 91-106 — struct definition (delete)
#[allow(dead_code)]
pub struct LatestPointer { ... }

// Lines 376-392 — serialization test (delete with the struct)
fn test_latest_pointer_serialization() { ... }
```

**Files modified:** `src/record/mod.rs` only. Deletion is safe — confirmed no other file references `LatestPointer`.

---

## Integration Point 5: Placeholder Repo Paths

**Files modified:**

| File | Line(s) | Current value | Fix |
|------|---------|---------------|-----|
| `Cargo.toml` | 7 | `repository = "https://github.com/user/cclink"` | Replace `user` with actual GitHub org/username |
| `Cargo.toml` | 8 | `homepage = "https://github.com/user/cclink"` | Same |
| `install.sh` | 2 | `# curl -fsSL https://raw.githubusercontent.com/user/cclink/...` | Same |
| `install.sh` | 7 | `REPO="user/cclink"` | Same |

Pure string substitution. No logic changes.

---

## Integration Point 6: QR + Share Bug

**Location:** `src/commands/publish.rs` lines 193-196

Current code when `--qr` is set:
```rust
qr2term::print_qr(format!("cclink pickup {}", pubkey_z32))
```

`pubkey_z32` is always the publisher's own public key. When `--share <recipient_pubkey>` is combined with `--qr`, the QR encodes `cclink pickup <publisher_pubkey>`. This is what the recipient needs to look up the record on the DHT (records are keyed by publisher pubkey). So the content is correct.

The possible bug interpretation: the QR message text before the code reads "Run on another machine: cclink pickup" but when `--share` is used the publisher's text prints "Recipient pickup command: cclink pickup <pubkey>". The QR code does not differentiate — it always renders the same `cclink pickup <pubkey>` string regardless. This is correct behavior.

**Recommendation:** Before implementing a fix, verify the exact expected behavior with a test. The current implementation is likely correct as written. If the bug is that `--qr` should not render when `--share` is used (because the QR would be shown to the publisher, who knows their own pubkey), then the fix is:

```rust
// Only render QR for self-publish; --share's QR is less useful to the publisher
if cli.qr && cli.share.is_none() {
    qr2term::print_qr(format!("cclink pickup {}", pubkey_z32))
        .map_err(|e| anyhow::anyhow!("QR code render failed: {}", e))?;
}
```

Or if the intent is for `--share + --qr` to still render (for the publisher to show the recipient), the current behavior is already correct.

**Files that would be modified:** `src/commands/publish.rs` only.

---

## Recommended Build Order

Dependencies between tasks drive this order. Independent tasks can be done in any sequence.

```
Step 1 — Fix Clippy warnings (prerequisite for CI job with -D warnings)
   tests/plaintext_leak.rs          change /// to //! at top of file
   tests/integration_round_trip.rs  change /// to //! at top of file

Step 2 — Remove LatestPointer dead code (isolated, no dependencies)
   src/record/mod.rs

Step 3 — Fix placeholder repo paths (isolated string changes)
   Cargo.toml
   install.sh

Step 4 — Investigate ed25519-dalek 3.0.0-pre.6 for breaking API changes
   If no API changes: update Cargo.toml pin only
   If API changed:    update src/crypto/mod.rs and/or src/record/mod.rs

Step 5 — Enforce minimum PIN length (isolated to publish.rs)
   src/commands/publish.rs
   Decide on minimum length (4, 6, or 8 chars) before implementing

Step 6 — Add clippy + audit to CI (after Step 1 so -D warnings passes cleanly)
   .github/workflows/ci.yml

Step 7 — Resolve backoff advisory (after Step 6 establishes audit baseline)
   Cargo.toml  (remove backoff dependency)
   src/commands/pickup.rs  (replace backoff::retry with inline loop or alternative)

Step 8 — Investigate and fix QR + share behavior (lowest priority, verify expected behavior first)
   src/commands/publish.rs
```

---

## New vs Modified Files Summary

| File | Status | Change |
|------|--------|--------|
| `src/commands/publish.rs` | MODIFIED | Add PIN length validation (3 lines) |
| `src/record/mod.rs` | MODIFIED | Remove LatestPointer struct + test |
| `Cargo.toml` | MODIFIED | Update ed25519-dalek pin; fix repo/homepage URLs |
| `install.sh` | MODIFIED | Fix REPO placeholder |
| `.github/workflows/ci.yml` | MODIFIED | Add lint job (clippy + cargo audit) |
| `tests/plaintext_leak.rs` | MODIFIED | Fix doc comment style (/// → //!) |
| `tests/integration_round_trip.rs` | MODIFIED | Fix doc comment style (/// → //!) |
| `src/commands/pickup.rs` | MODIFIED (Step 7) | Replace backoff crate usage if backoff removed |
| `src/crypto/mod.rs` | MODIFIED (conditional) | Only if ed25519-dalek pre.6 has API breaks |

**No new files** are required for any v1.2 task.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: PIN Validation in crypto/mod.rs

**What:** Adding `if pin.len() < 4 { return Err(...) }` inside `pin_derive_key` or `pin_encrypt`.

**Why it's wrong:** Crypto functions should have no awareness of application policy. Minimum length is a UX/security policy decision that could change independently of the KDF algorithm. It makes the crypto module harder to test (every test must use a policy-conforming PIN).

**Do this instead:** Validate in `publish.rs` after the interactive prompt, before calling crypto.

---

### Anti-Pattern 2: Downgrading ed25519-dalek to 2.x

**What:** Changing `ed25519-dalek = "=3.0.0-pre.5"` to `ed25519-dalek = "2.2.0"`.

**Why it's wrong:** pkarr 5.0.3 requires ed25519-dalek 3.x. Cargo will refuse to compile with two incompatible major pre-release versions. Additionally `to_scalar_bytes()` in `crypto/mod.rs:23` does not exist in 2.x — the code will not compile.

**Do this instead:** Stay in the `3.0.0-pre.x` series. Upgrade to pre.6 within the same pre-release family.

---

### Anti-Pattern 3: Adding Clippy to the Existing `test` Job

**What:** Appending `cargo clippy` steps to the current `test` job.

**Why it's wrong:** Clippy and test failures become entangled in the same job output. Cache behavior for clippy (which needs the `clippy` component) vs plain `cargo test` differs.

**Do this instead:** Add a parallel `lint` job. Both jobs share the Swatinem/rust-cache.

---

### Anti-Pattern 4: RUSTFLAGS at Job Level for Clippy Failures

**What:** Setting `env: RUSTFLAGS: "-Dwarnings"` at the `lint` job level to make Clippy fail on warnings.

**Why it's wrong:** `RUSTFLAGS` applies to every `cargo` invocation in the job, including `cargo install cargo-audit`, which can produce unexpected failures from unrelated warnings in the audit tool's build.

**Do this instead:** `cargo clippy --all-targets -- -D warnings` passes the deny flag only to Clippy.

---

## Sources

- Codebase read directly (all src/ files and workflow files): HIGH confidence
- `cargo tree` confirming ed25519-dalek 3.0.0-pre.5 shared between cclink and pkarr 5.0.3: HIGH confidence
- `cargo audit` output confirming backoff/instant advisories: HIGH confidence
- `cargo clippy --all-targets` output confirming 2 existing warnings: HIGH confidence
- [lib.rs/ed25519-dalek](https://lib.rs/crates/ed25519-dalek): latest pre-release is 3.0.0-pre.6 (2026-02-04), no stable 3.x: HIGH confidence
- pubky/pkarr main branch Cargo.toml: pkarr depends on ed25519-dalek 3.0.0-pre.1: MEDIUM confidence (HEAD of main, not pinned to 5.0.3 tag)
- [Official Clippy CI docs](https://doc.rust-lang.org/nightly/clippy/continuous_integration/github_actions.html): `cargo clippy --all-targets` with `-- -D warnings`: HIGH confidence
- [rust-audit-check marketplace action](https://github.com/marketplace/actions/rust-audit-check): cargo-audit integration pattern: MEDIUM confidence

---

*Architecture research for: cclink v1.2 — dependency audit, CI hardening, PIN enforcement, tech debt*
*Researched: 2026-02-23*
