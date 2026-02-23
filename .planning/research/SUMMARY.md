# Project Research Summary

**Project:** cclink v1.2 — Dependency Audit, CI Hardening, and Code Quality
**Domain:** Rust CLI — security tool hardening, CI pipeline enforcement, tech debt elimination
**Researched:** 2026-02-23
**Confidence:** HIGH

## Executive Summary

cclink v1.2 is a hardening milestone, not a feature sprint. The core product (Ed25519-keyed DHT publish/pickup with age encryption and Argon2id PIN-derived key wrapping) ships and works. What v1.2 addresses is a cluster of quality and security posture gaps: the CI pipeline only runs `cargo test` and an OpenSSL absence check; `--pin` accepts any string including "1"; two RUSTSEC unmaintained-crate advisories exist and are invisible to CI; clippy and rustfmt are not enforced; and a handful of dead code and placeholder artifacts remain from the homeserver-to-DHT migration.

The recommended approach is to execute eight targeted, low-complexity changes in a dependency-respecting order. All changes touch existing files only — no new source files are required. The most structurally important constraint is the `ed25519-dalek` pre-release pin: pkarr 5.0.3 requires `ed25519-dalek ^3.0.0-pre.1`, which makes stable `2.x` incompatible. The correct action is to bump the exact pin from `=3.0.0-pre.5` to `=3.0.0-pre.6` (the latest pre-release, released 2026-02-04) and add a comment explaining why the pin exists. The `=` exact pin is correct practice and must be preserved.

The key risks are all sequencing risks, not complexity risks. Adding `cargo clippy --all-targets -- -D warnings` to CI before fixing two `empty_line_after_doc_comments` warnings in test files causes an immediate CI failure. Adding `cargo audit` with deny-on-warnings before resolving or explicitly ignoring two unmaintained-crate advisories (`backoff`, `instant`) causes the same. Adding PIN validation to `src/commands/pickup.rs` as well as `publish.rs` would silently break backward compatibility for records published with older binaries. Every task in this milestone follows a fix-then-gate pattern: resolve the local issue first, then wire the enforcement into CI.

---

## Key Findings

### Recommended Stack

The core stack is stable and unchanged from v1.1. This milestone's stack work is narrowly scoped to two items: bumping `ed25519-dalek` from `=3.0.0-pre.5` to `=3.0.0-pre.6`, and adding two CI actions (`actions-rust-lang/audit@v1` for cargo-audit and `dtolnay/rust-toolchain@stable` with the `clippy` component for lint enforcement). No new runtime dependencies are introduced by the milestone itself.

The `backoff` crate (RUSTSEC-2025-0012, unmaintained) is a direct dependency used only for a retry loop in `src/commands/pickup.rs`. The preferred fix is replacing it with `backon` 1.6.0 (the actively maintained successor, released 2025-10-18) or an inline retry loop, which eliminates both `backoff` and its transitive `instant` advisory (RUSTSEC-2024-0384) in one step. If replacement is deferred, explicit per-advisory ignores in `audit.toml` with documented reasons must be added before CI gates on audit clean.

**Core technologies (unchanged from v1.1):**
- `pkarr 5.0.3`: Mainline DHT transport — locked, do not change in v1.2
- `ed25519-dalek =3.0.0-pre.6`: Ed25519 signing, pulled in by both cclink and pkarr — bump from pre.5, keep exact pin
- `age 0.11.x`: X25519 encryption — no advisories, no changes
- `argon2 0.5`: Argon2id PIN hashing — no advisories, no changes

**New CI tooling for v1.2:**
- `actions-rust-lang/audit@v1` (v1.2.7, Jan 2026): cargo-audit in CI — current standard, supersedes `rustsec/audit-check`
- `cargo clippy --all-targets -- -D warnings`: lint enforcement — pass `-D warnings` to clippy directly, not via `RUSTFLAGS`
- `backon 1.6.0` (P2): replacement for unmaintained `backoff` crate

### Expected Features

v1.2 is a quality and hardening milestone. All items are either P1 (must-have) or P2 (should-have). The only user-facing behavioral change is PIN enforcement, which is a security correctness fix: NIST SP 800-63B sets the floor at 8 characters for user-chosen memorized secrets.

**Must have (P1 — table stakes for a security tool):**
- Minimum PIN length enforcement (8 chars, NIST floor) — `publish.rs` only, `validate_with()` closure using `chars().count()` not `len()`
- `cargo fmt --check` in CI — fix 2 divergent files in `src/crypto/mod.rs` first, then add the CI step
- `cargo clippy --all-targets -- -D warnings` in CI — fix 2 `empty_line_after_doc_comments` warnings in `tests/` first, then add
- `cargo audit` in CI — allow `unmaintained`, deny `vulnerability` and `unsound`; resolve or ignore `backoff`/`instant` advisories before enabling
- Fix `user/cclink` placeholder paths in `Cargo.toml` and `install.sh` — breaks curl installer for real users

**Should have (P2):**
- ed25519-dalek bump to `=3.0.0-pre.6` — routine within same pre-release API surface; verify compile, run full tests
- Remove dead `LatestPointer` struct and its test from `src/record/mod.rs` — dead code from homeserver migration
- Scheduled weekly `cargo audit` job (cron trigger) — catches post-release advisories

**Defer (post-v1.2):**
- Stable `ed25519-dalek 3.x` — no stable 3.x exists as of 2026-02-23; monitor `dalek-cryptography`
- `cargo-deny` for license checking — overkill for single-binary personal tool at this stage
- QR + share behavior clarification — investigate before any fix; may not be a real bug

**Anti-features to reject:**
- PIN complexity rules (uppercase, symbols) — NIST 800-63B-4 explicitly removes mandatory complexity; length beats complexity
- `#[deny(warnings)]` baked into source code — deny only in CI env var; baked-in `deny(warnings)` breaks builds on new Rust lint additions every 6 weeks
- Numeric-only PIN mode — reduces character search space below what Argon2id parameters can compensate

### Architecture Approach

All v1.2 changes are targeted modifications to existing files with no new source files required. The integration points are well-defined and isolated: `src/commands/publish.rs` (PIN enforcement), `src/record/mod.rs` (LatestPointer removal), `Cargo.toml` (ed25519-dalek pin, placeholder URLs), `install.sh` (placeholder URLs), `.github/workflows/ci.yml` (new parallel lint and audit jobs), and two test files for doc comment style fixes.

**Integration points and responsible files:**
1. `src/commands/publish.rs` — PIN length validation (one `validate_with()` closure, `chars().count() >= 8`)
2. `.github/workflows/ci.yml` — new parallel `lint` job (clippy + fmt) and `audit` job alongside existing `test` job
3. `src/record/mod.rs` — delete `LatestPointer` struct (lines 91-106) and its test (lines 376-392) together in one edit
4. `Cargo.toml` — bump `ed25519-dalek` exact pin, fix `repository`/`homepage` fields
5. `install.sh` — fix `REPO` variable and usage comment
6. `tests/plaintext_leak.rs`, `tests/integration_round_trip.rs` — change leading `///` to `//!` (2-line change per file)

PIN validation belongs exclusively in `publish.rs`. It must not be added to `crypto/mod.rs` (a policy-free crypto layer) or `pickup.rs` (backward compatibility: records published with older binaries must remain decryptable regardless of PIN length).

### Critical Pitfalls

1. **ed25519-dalek upgrade breaks pkarr type resolution** — pkarr 5.0.3 requires `^3.0.0-pre.1`; stable `2.x` is incompatible. Do not remove the `=` exact pin. Verify with `cargo tree | grep ed25519-dalek` (must show a single node) after any Cargo.toml change. Run the full test suite.

2. **Clippy CI step missing `--all-targets`** — `cargo clippy -- -D warnings` (no `--all-targets`) passes today but silently ignores 2 real errors in `tests/`. Always use `--all-targets`. Fix the 2 `empty_line_after_doc_comments` warnings (`///` to `//!`) in both test files before adding the CI step.

3. **`cargo audit` with deny-on-warnings before resolving existing advisories** — two unmaintained-crate warnings exist now (`backoff`, `instant`). Adding `--deny warnings` or equivalent before resolving or explicitly ignoring them causes every CI run to fail. Either replace `backoff` with `backon` or add `audit.toml` per-advisory ignores with documented reasons first.

4. **Removing `LatestPointer` struct without its test** — `#[allow(dead_code)]` hides that the struct's only consumer is its own test. Delete struct and test together in a single edit. Verify with `cargo test --lib` (not just `cargo check`) immediately after.

5. **PIN validation at pickup breaks backward compatibility** — adding enforcement to `pickup.rs` means records published with a short PIN using an older binary become undecryptable with the new binary. Validate only at publish time; at pickup, let decryption failure signal a wrong PIN.

---

## Implications for Roadmap

The fix-then-gate sequencing constraint drives a clear 3-phase structure. Each phase unblocks the next.

### Phase 1: Prerequisites — Fix What Would Break CI

**Rationale:** CI hardening is the highest-value output of this milestone, but two issues in the codebase today would cause immediate failures if enforcement gates were added now. Fix the issues first so Phase 2 produces a green build from day one.
**Delivers:** A codebase that passes `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and `cargo audit` (with advisory ignores if backoff replacement is deferred to P2).
**Addresses:** Fix 2 doc comment warnings in `tests/plaintext_leak.rs` and `tests/integration_round_trip.rs`; decide on backoff strategy (replace with `backon` now or add `audit.toml` ignores); fix any `cargo fmt` divergences.
**Avoids:** Pitfalls 2 and 3 — CI failing immediately upon enabling new gates.

### Phase 2: CI Hardening

**Rationale:** With prerequisites resolved, add the enforcement gates. Parallel jobs keep lint and test failures separate and report faster. This phase is the highest-leverage deliverable — CI gates prevent regression permanently.
**Delivers:** A `lint` job (clippy + fmt) and an `audit` job running on every PR, plus optional scheduled weekly audit cron trigger.
**Uses:** `actions-rust-lang/audit@v1`, `dtolnay/rust-toolchain@stable` with `clippy` component, `Swatinem/rust-cache@v2`.
**Implements:** `.github/workflows/ci.yml` modification only — new parallel jobs alongside the existing `test` job.
**Avoids:** Anti-pattern of appending clippy to the existing `test` job (entangles failure attribution); `RUSTFLAGS: "-Dwarnings"` at job level (affects `cargo install` in the same job).

### Phase 3: Code Quality and Security Hardening

**Rationale:** These changes are independent of each other and of CI but are all P1/P2 for the milestone. They close the security gap (PIN enforcement), eliminate dead code and false maintenance signals, and fix metadata that would prevent real users from using the tool.
**Delivers:** PIN minimum length enforcement (8 chars), dead `LatestPointer` code removed, correct `repository`/`homepage` and `install.sh` paths, ed25519-dalek bumped to `=3.0.0-pre.6`.
**Addresses:** PIN enforcement (P1), placeholder path fix (P1), `LatestPointer` removal (P2), ed25519-dalek pin bump (P2).
**Avoids:** Pitfalls 1, 4, 5 — ed25519-dalek incompatibility, LatestPointer test compile error, PIN validation breaking backward compatibility at pickup.

### Phase Ordering Rationale

- Phase 1 before Phase 2: the 2 clippy warnings and unresolved `cargo audit` advisories would cause the new CI gates to fail on the first commit if added without the prerequisites resolved first.
- Phase 2 before Phase 3 (loosely): CI gates should be in place to catch any regressions introduced during Phase 3 changes.
- Phase 3 tasks are independent of each other. The recommended internal sub-order from ARCHITECTURE.md: fix clippy (done in Phase 1) → remove LatestPointer → fix placeholders → bump ed25519-dalek (verify API, run tests) → enforce PIN → investigate QR/share bug.

### Research Flags

Phases with standard, well-documented patterns — no additional research needed:
- **Phase 1:** Doc comment style change (`///` to `//!`) and `audit.toml` per-advisory ignore format are both well-documented. `backon` migration is a simple builder-pattern swap from `backoff::retry`.
- **Phase 2:** CI YAML structure for parallel jobs is a standard GitHub Actions pattern. `actions-rust-lang/audit@v1` has official documented YAML.
- **Phase 3 (most tasks):** dialoguer `validate_with()`, Cargo.toml string edits, dead code deletion — all straightforward.

Phases that benefit from a verification step during implementation:
- **Phase 3 (ed25519-dalek bump):** The two direct dalek type usages in `src/crypto/mod.rs` (`to_scalar_bytes()`) and `src/record/mod.rs` (`Signature::from_bytes()`) must be confirmed to compile unchanged against pre.6. Pre.5 to pre.6 only updated downstream crate versions (rand_core, digest, sha2), so API breakage is unlikely but must be confirmed with `cargo build` after the Cargo.toml edit — not a research gap, a verification step.
- **Phase 3 (QR + share):** PITFALLS.md identifies an inconsistency between printed text and QR content for `--share + --qr`. Requires a product decision on intended UX before any code change. Do not implement without defining the target behavior first.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified live via crates.io API and `cargo tree`; `cargo audit` run live against project |
| Features | HIGH | NIST 800-63B cited directly (official publication); dialoguer API confirmed on docs.rs; all clippy/fmt findings from live tool runs |
| Architecture | HIGH | All integration points derived from direct codebase inspection; file locations and line numbers confirmed |
| Pitfalls | HIGH | All pitfalls verified by live tool runs (`cargo clippy --all-targets`, `cargo audit`, `cargo tree`); not theoretical |

**Overall confidence: HIGH**

### Gaps to Address

- **QR + share intended behavior:** Research identified an inconsistency between printed text and QR content when `--share + --qr` are combined but could not determine the intended behavior from code alone. Requires a product decision before implementation: should `--share + --qr` encode the full command string or just the pubkey? Should `--qr` suppress when `--share` is active?
- **backoff replacement scope decision:** Whether to replace `backoff` with `backon` now or add `audit.toml` ignores and defer is a scope decision, not a technical uncertainty. Both approaches are well-understood. Must be decided before Phase 1 work begins.
- **Actual GitHub org/username:** The `user/cclink` placeholder fix requires knowing the real repository owner. Verify with `git remote -v` before editing `Cargo.toml` and `install.sh`.
- **ed25519-dalek pre.6 API confirmation:** The two direct dalek type usages (`to_scalar_bytes()`, `Signature::from_bytes()`) should be confirmed against pre.6 during Phase 3 implementation with a `cargo build` check. The changelog suggests no API break, but the exact-pin change should be followed immediately by a build verification.

---

## Sources

### Primary (HIGH confidence)
- `crates.io/api/v1/crates/pkarr/5.0.3/dependencies` — pkarr 5.0.3 requires `ed25519-dalek ^3.0.0-pre.1` (verified live)
- `crates.io/api/v1/crates/ed25519-dalek` — latest stable 2.2.0; latest pre-release 3.0.0-pre.6 (Feb 2026) (verified live)
- `cargo audit` run live (2026-02-23) — 2 unmaintained warnings (`backoff`, `instant`), 0 vulnerabilities
- `cargo clippy --all-targets -- -D warnings` run live (2026-02-23) — 2 `empty_line_after_doc_comments` errors in `tests/`
- `cargo tree | grep ed25519-dalek` run live (2026-02-23) — single node `v3.0.0-pre.5` shared with pkarr
- NIST SP 800-63B — 8-character minimum for user-chosen memorized secrets (official NIST publication)
- NIST SP 800-63B-4 Second Public Draft (Sep 2024) — removal of mandatory complexity requirements
- [Official Clippy CI docs](https://doc.rust-lang.org/nightly/clippy/continuous_integration/github_actions.html) — `cargo clippy --all-targets -- -D warnings` pattern; `-D warnings` via arg not `RUSTFLAGS`
- [actions-rust-lang/audit@v1](https://github.com/actions-rust-lang/audit) — v1.2.7, January 2026; recommended YAML verified
- [dialoguer::Password docs](https://docs.rs/dialoguer/latest/dialoguer/struct.Password.html) — `validate_with()` confirmed in 0.12
- [Rust Design Patterns — deny(warnings) anti-pattern](https://rust-unofficial.github.io/patterns/anti_patterns/deny-warnings.html)
- [backon on crates.io](https://crates.io/crates/backon) — v1.6.0, released 2025-10-18; no `instant` dependency
- Direct codebase inspection — all `src/` files and `.github/workflows/ci.yml`
- RustSec advisories RUSTSEC-2025-0012 and RUSTSEC-2024-0384 — `backoff` and `instant` unmaintained (not CVEs)

### Secondary (MEDIUM confidence)
- pubky/pkarr main branch `Cargo.toml` — pkarr depends on `ed25519-dalek 3.0.0-pre.1` (HEAD of main, not the 5.0.3 tag)
- `rustsec/audit-check@v2` — still maintained (v2.0.0, Sep 2024) but older than `actions-rust-lang/audit`

---
*Research completed: 2026-02-23*
*Ready for roadmap: yes*
