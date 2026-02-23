# Pitfalls Research

**Domain:** Rust CLI — dependency audit, CI hardening, and code cleanup on an existing security project (cclink v1.2)
**Researched:** 2026-02-23
**Confidence:** HIGH (verified against actual codebase with live `cargo audit` and `cargo clippy` runs, official docs)

---

## Critical Pitfalls

### Pitfall 1: Upgrading ed25519-dalek Without Checking pkarr's Pinned Version

**What goes wrong:**

The codebase pins `ed25519-dalek = "=3.0.0-pre.5"`. pkarr 5.0.3 requires `ed25519-dalek = "3.0.0-pre.1"` (same pre-release series). Cargo currently resolves both to a single shared `v3.0.0-pre.5` node — verified by live `cargo tree`. Upgrading the direct pin to any incompatible version (a newer pre-release or the stable `2.x` line) without first confirming pkarr can tolerate it will either force two incompatible copies of ed25519-dalek into the dependency graph, or cause type mismatch errors when passing pkarr's `Keypair`/`PublicKey`/`Signature` types to functions expecting the newer version's types.

RUSTSEC-2022-0093 (Double Public Key Signing Function Oracle Attack) affects `ed25519-dalek < 2.0`. The current `3.0.0-pre.5` is technically in that range, but `cargo audit` does NOT flag it today (the advisory's patched version is `>= 2.0`, and the RustSec database appears to treat the pre-release series separately). The clean security fix is to upgrade to stable `2.x` — but pkarr 5.0.3 does not support that API yet. Upgrading dalek without upgrading pkarr first will break the build.

**Why it happens:**

Pre-release version numbering is counterintuitive. `3.0.0-pre.5` looks "newer" than `2.2.0` but is from a separate experimental branch. The stable security-fixed line is `2.x`. Developers see the `=` pin, assume it is overly conservative, remove it, and Cargo picks a version that pkarr cannot use.

**How to avoid:**

1. Run `cargo tree | grep ed25519-dalek` first. It should show exactly one node. If it shows two, there is already a conflict.
2. Check pkarr's Cargo.toml for its ed25519-dalek constraint before touching the pin. pkarr 5.0.3 requires `"3.0.0-pre.1"` — it accepts `3.0.0-pre.x` but not `2.x`.
3. The correct action for v1.2 is: leave the pin, add a comment in Cargo.toml explaining it, and file a note to revisit when pkarr publishes a release that uses stable `2.x`.
4. If investigation finds a newer pkarr version that has migrated to stable `2.x`, upgrade pkarr and ed25519-dalek together in a single atomic Cargo.toml change.

**Warning signs:**

- `cargo tree` shows two `ed25519-dalek` entries at different versions.
- Compile errors like `expected type ed25519_dalek::Keypair found ed25519_dalek::Keypair` (same name, different crate versions).
- `cargo update` quietly changes the resolved dalek version after removing the `=` constraint.

**Phase to address:** Dependency audit phase. Investigate the constraint first, document it with a comment, and only change if pkarr supports it.

---

### Pitfall 2: Adding `cargo clippy -- -D warnings` to CI Without Running `--all-targets` First

**What goes wrong:**

`cargo clippy -- -D warnings` (lib and binary targets only) passes cleanly on this codebase. `cargo clippy --all-targets -- -D warnings` fails with two real errors — verified by live run:

- `tests/integration_round_trip.rs` line 12: `empty_line_after_doc_comments` — outer `///` doc comment at file level followed by blank line.
- `tests/plaintext_leak.rs` line 8: same lint, same cause.

A CI step using `cargo clippy -- -D warnings` (no `--all-targets`) will pass, silently hiding lint errors in test files. Users assume the codebase is clean. The next time someone adds a test file with the same pattern, there is no CI signal.

**Why it happens:**

Developers test clippy locally on the main binary (`cargo clippy`) and add it to CI with `-- -D warnings`. Without `--all-targets`, the integration test binaries in `tests/` are not compiled by clippy. The errors only appear when `--all-targets` is added.

**How to avoid:**

Always use `cargo clippy --all-targets -- -D warnings` in CI. Before adding the CI step, run it locally and fix all errors. The fix for `empty_line_after_doc_comments` in both test files is to change the leading `///` block to `//!` (inner doc comment), which is semantically correct for file-level module documentation.

Fix for `tests/integration_round_trip.rs`: change `///` to `//!` for the top-of-file comment block (lines 1-12).
Fix for `tests/plaintext_leak.rs`: change `///` to `//!` for the top-of-file comment block (lines 1-8).

**Warning signs:**

- `cargo clippy -- -D warnings` exits 0 but `--all-targets` fails.
- CI workflow shows `cargo clippy` without `--all-targets`.
- Test files in `tests/` use `///` doc comments at the file root level.

**Phase to address:** CI hardening phase. Fix the two doc comment issues in test files before adding the clippy CI step.

---

### Pitfall 3: Adding `cargo audit --deny warnings` Before Resolving Existing Advisories

**What goes wrong:**

`cargo audit` today produces two warnings (not errors) — verified by live run:

- `backoff 0.4.0` — RUSTSEC-2025-0012, unmaintained (direct cclink dependency).
- `instant 0.1.13` — RUSTSEC-2024-0384, unmaintained (transitive, pulled in by backoff).

Adding `cargo audit --deny warnings` to CI immediately causes every CI run to fail. The `backoff` crate is used in one place: the exponential retry loop in `src/commands/pickup.rs`. Replacing it is a small, contained change.

**Why it happens:**

`--deny warnings` is the canonical "make CI strict" flag. But it treats "unmaintained" (soft signal, no CVE) identically to a real security vulnerability. Many otherwise-sound crates receive an "unmaintained" advisory when the author steps back, without there being any exploitable flaw.

**How to avoid:**

Two approaches in order of preference:

1. **Replace backoff (preferred):** The entire use of `backoff` in pickup is a 10-line exponential retry loop. Replace it with an inline retry loop using `std::thread::sleep` and a counter, eliminating both advisories in one step.

2. **Staged ignore:** Add `audit.toml` at the project root with explicit per-advisory ignores and a documented reason:
   ```toml
   [advisories]
   ignore = [
       { id = "RUSTSEC-2025-0012", reason = "backoff is unmaintained but has no CVE; replacing in v1.3" },
       { id = "RUSTSEC-2024-0384", reason = "transitive via backoff; resolved when backoff is replaced" },
   ]
   ```

Do NOT add `cargo audit --deny warnings` to CI before one of these two steps. The CI will fail immediately on every run.

**Warning signs:**

- `cargo audit` output shows `warning: N allowed warnings found`.
- Any advisory with `Warning: unmaintained` status present before adding the CI step.
- CI log shows `error: 2 vulnerabilities found!` immediately after adding cargo audit.

**Phase to address:** Dependency audit phase. Resolve `backoff` or add explicit `audit.toml` ignores before wiring cargo audit into CI.

---

### Pitfall 4: Removing `LatestPointer` Without Also Removing Its Test

**What goes wrong:**

`LatestPointer` in `src/record/mod.rs` is marked `#[allow(dead_code)]` — it is dead production code left from the homeserver era. However, it has a test (`test_latest_pointer_serialization`) in `src/record/mod.rs` lines 376-392 that constructs and round-trips it. Deleting the struct without deleting the test produces a compile error. Running `cargo check` after removing the struct does not catch this — only `cargo test --lib` does.

The `#[allow(dead_code)]` attribute suppresses the compiler warning that would otherwise signal "this struct is never constructed outside of tests." The attribute's purpose was to silence the warning during migration; it now masks the fact that the struct's only consumer is its own test.

**Why it happens:**

The `#[allow(dead_code)]` attribute hides the real usage pattern. A developer doing a "find all usages of LatestPointer" search outside test modules finds nothing and concludes it is safe to delete the struct alone. The `#[allow]` acts as a false-positive suppressor for the dead_code lint that would have guided them to also remove the test.

**How to avoid:**

Remove `LatestPointer` (lines 91-106 in `src/record/mod.rs`), the `#[allow(dead_code)]` annotation above it, and the `test_latest_pointer_serialization` test (lines 376-392) in a single edit. Verify with `cargo test` (not just `cargo check`) immediately after.

Confirmed by grep: `LatestPointer` appears only in `src/record/mod.rs`. No other source file references it.

**Warning signs:**

- A struct with `#[allow(dead_code)]` that has a test exercising it.
- `cargo check` succeeds but `cargo test` fails after removing a struct.
- A test whose sole purpose is to round-trip a struct that has no other callers.

**Phase to address:** Code cleanup phase. Run `cargo test` (not `cargo check`) after the removal to verify.

---

### Pitfall 5: QR Code Content Does Not Match the Displayed Text Command

**What goes wrong:**

In `src/commands/publish.rs` lines 193-196, the QR code always renders:

```
cclink pickup {publisher_own_pubkey}
```

But the text instruction above the QR shows different content depending on context:

- Without `--share`: "Run on another machine: `cclink pickup`" (no pubkey in text, but QR has the full pubkey form).
- With `--share`: "Recipient pickup command: `cclink pickup {pubkey}`" (text and QR agree — but the QR encodes the command string rather than a scannable pubkey for a mobile QR reader).

The bug is the inconsistency between what the printed text says and what the QR encodes. For self-pickup, the text says `cclink pickup` but the QR says `cclink pickup <pubkey>`. This is confusing: the user reads the text instruction on screen, then the QR encodes something different that a mobile device would interpret as text.

For `--share + --qr`, the better QR content is the raw pubkey string (not the full command with prefix), so the recipient can scan it with a phone camera and see the pubkey to type manually, rather than a long command string.

**Why it happens:**

The QR code was added for the self-pickup use case and the pubkey form was hard-coded. When `--share` was added as a feature, the QR path was not updated to account for the different audience (recipient, not self) and different UX (scan pubkey, not copy command).

**How to avoid:**

Define a consistent QR content policy and implement it:

- **Self-pickup (`--qr`, no `--share`):** QR encodes `cclink pickup {pubkey}` so another device can scan the complete command. OR just the pubkey if the user is expected to type `cclink pickup` themselves.
- **Share mode (`--share <key> --qr`):** QR encodes just the raw pubkey string (no command prefix), since the recipient may use it as-is or the text instruction already tells them the command.

Pick one consistent strategy. The simplest safe fix: always encode the full `cclink pickup {pubkey}` command (consistent and functional for both cases), and update the self-pickup text to match: "Run on another machine: `cclink pickup {pubkey}`".

**Warning signs:**

- The string passed to `qr2term::print_qr()` differs from the string shown in the `println!` above it.
- Manual test: run `cclink --qr` and compare what the terminal text says vs. what a QR scanner reads.

**Phase to address:** Bug fix phase. Requires a decision on desired UX before implementing.

---

### Pitfall 6: Enforcing Minimum PIN Length at Both Publish and Pickup

**What goes wrong:**

If minimum PIN length is validated in both `src/commands/publish.rs` and `src/commands/pickup.rs`, a handoff published with an older binary (before enforcement) will be undecryptable with the new binary. The user enters a 4-character PIN that was valid at publish time; the new pickup binary rejects it before even attempting decryption.

**Why it happens:**

Developers add validation wherever user input is collected. Pickup collects a PIN from the user — it feels natural to add the same "minimum 6 characters" check there. But pickup is a consumer of a record that was created by a possibly older binary; its validation must allow any input and delegate to the cryptographic result.

**How to avoid:**

Add PIN length validation ONLY in `src/commands/publish.rs`, at the `dialoguer::Password` prompt. Do not add validation in `src/commands/pickup.rs`. Pickup must accept any string and pass it to `crate::crypto::pin_decrypt`; decryption failure is the only valid "wrong PIN" signal.

**Warning signs:**

- PIN validation logic appears in `pickup.rs`.
- A user who published a handoff with an older binary cannot pick it up with the new binary (backward compatibility broken).
- Tests that check pickup rejects a short PIN (the correct test is that pickup returns a decryption error for a wrong PIN, not a validation error for length).

**Phase to address:** PIN enforcement phase.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| `#[allow(dead_code)]` on `LatestPointer` | Silenced warning during homeserver→DHT migration | Dead code ships; its test provides false coverage signal; future devs don't know it is dead | Never — remove when migration is done |
| `user/cclink` placeholder in Cargo.toml and install.sh | Allowed ship without real repository URL | `cargo publish` fails; curl installer fetches from wrong URL | Never — must be fixed before first publish |
| Pre-release pin `=3.0.0-pre.5` without a comment | Works with pkarr today | Future maintainers remove `=`, causing version conflict with pkarr | Acceptable short-term — add comment explaining pkarr constraint |
| `backoff 0.4.0` (unmaintained) for retry logic | 10 lines of retry code avoided | Two RUSTSEC advisories block `cargo audit --deny warnings` in CI | Acceptable temporarily if audit.toml ignores are added; better to replace the crate |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `cargo audit` in CI | Add `--deny warnings` unconditionally | Run `cargo audit` first locally; resolve or explicitly ignore existing advisories before adding `--deny warnings` |
| `cargo clippy` in CI | Use `-- -D warnings` without `--all-targets` | Always `cargo clippy --all-targets -- -D warnings`; covers `tests/` integration test binaries |
| ed25519-dalek upgrade | Remove `=` pin without checking pkarr's constraint | Verify pkarr's Cargo.toml dependency requirement first; change both together if safe |
| `cargo publish` to crates.io | Publish with placeholder `repository` and `homepage` fields | Fix both to the real GitHub URL before first publish; crates.io indexes these fields |
| Cargo.lock in CI | Running `cargo test` without `--locked` allows dependency drift | Add `--locked` flag so CI uses the committed Cargo.lock |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Ignoring RUSTSEC advisories wholesale via `audit.toml` `ignore = ["*"]` or ignoring all | True security vulnerabilities silenced alongside unmaintained advisories | Always use per-advisory ID in ignore list with a documented reason |
| Removing `=` from the ed25519-dalek pin without testing | Could silently upgrade to an incompatible version that uses a different API for signing | Run `cargo tree | grep ed25519-dalek` after any Cargo.toml change; run full test suite |
| Adding complexity requirements to PIN (digits, symbols) | Users choose predictable patterns (P@ssw0rd1) to satisfy rules; marginal real security gain | Enforce minimum length only (6+ characters); the Argon2id KDF already in use makes short PINs expensive to brute-force |
| Allowing PIN validation at pickup | Records published with old binary become undecryptable | Validate PIN only at publish time; at pickup, let cryptographic failure speak |

---

## "Looks Done But Isn't" Checklist

- [ ] **cargo audit in CI:** `cargo audit` exits 0 before adding `--deny warnings`. Current state: 2 unmaintained warnings exist that will fail under `--deny warnings`.
- [ ] **cargo clippy in CI:** `cargo clippy --all-targets -- -D warnings` exits 0 locally before wiring into CI. Current state: 2 `empty_line_after_doc_comments` errors in `tests/` files.
- [ ] **LatestPointer removal:** `cargo test` (not just `cargo check`) passes after removal. The test `test_latest_pointer_serialization` must also be deleted.
- [ ] **Placeholder URLs fixed:** `grep "user/cclink" Cargo.toml install.sh` returns nothing.
- [ ] **PIN enforcement one-sided:** No PIN length validation in `src/commands/pickup.rs`.
- [ ] **QR content consistent:** Text instruction and QR content encode the same string (verify by running `cclink --qr` and `cclink --share <key> --qr` and comparing).
- [ ] **ed25519-dalek pin documented:** A comment in Cargo.toml explains the `=3.0.0-pre.5` pin is required for pkarr 5.0.3 compatibility.
- [ ] **CI uses --locked:** `cargo test --locked` in CI to pin to committed Cargo.lock.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| ed25519-dalek upgrade breaks pkarr types | HIGH | Revert Cargo.toml to `=3.0.0-pre.5`; run `cargo update -p ed25519-dalek`; verify `cargo tree` shows a single node; run full test suite |
| cargo audit blocks CI after adding `--deny warnings` | LOW | Add `audit.toml` with per-advisory `ignore` entries with documented reasons; plan backoff replacement |
| clippy `--all-targets` fails after CI added | LOW | Change `///` to `//!` at top of `tests/integration_round_trip.rs` and `tests/plaintext_leak.rs`; re-run to confirm |
| LatestPointer removal breaks cargo test | LOW | Also delete `test_latest_pointer_serialization` in `src/record/mod.rs`; re-run `cargo test` |
| PIN enforcement at pickup breaks old records | MEDIUM | Remove PIN length validation from `src/commands/pickup.rs`; keep only in `publish.rs`; rebuild |
| cargo publish fails on placeholder URL | LOW | Fix `repository` and `homepage` in Cargo.toml to real GitHub URL; re-run `cargo publish` |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| ed25519-dalek upgrade breaks pkarr | Dependency audit: investigate pin before changing it | `cargo tree \| grep ed25519-dalek` shows single node; `cargo test --locked` passes |
| cargo audit blocks CI on existing advisories | Dependency audit: replace backoff or add audit.toml ignores first | `cargo audit` exits 0 before CI step is added |
| clippy `--all-targets` fails in test files | CI hardening: fix doc comment style in tests/ before adding CI step | `cargo clippy --all-targets -- -D warnings` exits 0 locally |
| LatestPointer test compile error after removal | Code cleanup: remove struct and test together | `cargo test --lib` passes after removal |
| QR content does not match printed text | Bug fix phase: decide on content policy, implement consistently | Manual: `cclink --qr` and `cclink --share <k> --qr` show QR matching text |
| PIN validation at pickup breaks old records | PIN enforcement phase: validate only at publish | `cargo test` passes; old short-PIN record still decryptable at pickup |
| Placeholder URLs break crates.io publish | Code cleanup: fix Cargo.toml and install.sh | `grep "user/cclink" Cargo.toml install.sh` returns nothing |

---

## Sources

- Live `cargo audit` on cclink (2026-02-23): `backoff` RUSTSEC-2025-0012 and `instant` RUSTSEC-2024-0384 — both warnings, neither security CVEs
- Live `cargo clippy --all-targets -- -D warnings` on cclink (2026-02-23): 2 `empty_line_after_doc_comments` errors in `tests/`
- Live `cargo tree | grep ed25519-dalek` on cclink (2026-02-23): single `v3.0.0-pre.5` node shared with pkarr 5.0.3
- RUSTSEC-2022-0093: Double Public Key Signing Function Oracle Attack on ed25519-dalek, affects `< 2.0`, patched in `>= 2.0` — https://rustsec.org/advisories/RUSTSEC-2022-0093.html
- pkarr 5.0.3 Cargo.toml: `ed25519-dalek = { version = "3.0.0-pre.1", features = ["alloc"] }` — verified via https://github.com/pubky/pkarr/blob/main/pkarr/Cargo.toml
- cargo-deny advisory config, `unmaintained = "workspace"` option — https://embarkstudios.github.io/cargo-deny/checks/advisories/cfg.html
- Rust Clippy usage docs — https://doc.rust-lang.org/clippy/usage.html
- Cargo dependency resolution and pre-release semver — https://doc.rust-lang.org/cargo/reference/resolver.html
- Direct code inspection: `src/record/mod.rs`, `src/commands/publish.rs`, `src/commands/pickup.rs`, `src/cli.rs`, `Cargo.toml`, `.github/workflows/ci.yml`

---

*Pitfalls research for: Rust CLI dependency audit, CI hardening, code cleanup (cclink v1.2)*
*Researched: 2026-02-23*
