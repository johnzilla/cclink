# Feature Research

**Domain:** PIN enforcement and CI hardening for a security-focused Rust CLI (cclink v1.2)
**Researched:** 2026-02-23
**Confidence:** HIGH — industry standards from NIST (official publication); Rust tooling from official Clippy docs + live codebase inspection; dialoguer API from docs.rs; ed25519-dalek version from live cargo search

---

## Context: This Is a Subsequent Milestone

v1.2 is not a greenfield feature sprint. All core commands ship and work. The milestone addresses:

1. PIN strength enforcement — `--pin` currently accepts any string including "1"
2. CI hardening — only `cargo test` + OpenSSL check run today
3. Tech debt cleanup — dead code, placeholder paths, pre-release dependency audit

The existing FEATURES.md (v1.1, 2026-02-21) covers the product feature landscape. This document focuses exclusively on what v1.2 is adding.

---

## Current State (Confirmed by Codebase Inspection)

| Item | Current State |
|------|--------------|
| PIN entry | `dialoguer::Password` with confirmation; no length validation |
| Minimum PIN length | None — any non-empty string accepted |
| `cargo fmt` in CI | Not present; 2 files currently diverge from rustfmt output |
| `cargo clippy` in CI | Not present; 2 existing warnings (doc-comment style in test files) |
| `cargo audit` in CI | Not present; 2 unmaintained-crate advisories exist now (no vulnerabilities) |
| ed25519-dalek version | `=3.0.0-pre.5`; latest pre-release is `3.0.0-pre.6` (released 2026-02-04) |
| Dead code | `LatestPointer` struct + test in `src/record/mod.rs` — leftover from homeserver migration |
| Placeholder repo paths | `https://github.com/user/cclink` in `Cargo.toml`; `REPO="user/cclink"` in `install.sh` |
| QR + share bug | QR always encodes `cclink pickup <publisher-pubkey>` — needs verification this is correct for share mode |

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features a security tool must have. Missing = credibility gap.

| Feature | Why Expected | Complexity | Existing Code Dependency |
|---------|--------------|------------|--------------------------|
| Minimum PIN length enforcement | Security tools must reject trivially weak PINs; Argon2id alone does not save "1" from offline brute-force in a low-attempt scenario; NIST 800-63B sets the floor at 8 chars for user-chosen secrets | LOW | `dialoguer::Password::validate_with()` — already available in `dialoguer 0.12`, no new deps. Add inline closure in `src/commands/publish.rs` before `.interact()` |
| Clear rejection message on short PIN | Users must understand why they were rejected and what the requirement is; re-prompt until valid | LOW | `validate_with()` closure returns `Err("PIN must be at least 8 characters")` — dialoguer re-prompts automatically |
| `cargo fmt --check` in CI | Formatting divergence is code noise; security-focused projects especially cannot afford reviewer distraction from substantive review | LOW | Two files currently fail: `src/crypto/mod.rs` (import order, method chain formatting). Must fix files before adding CI step or first CI run fails |
| `cargo clippy` in CI | Clippy catches real bugs; a security CLI must not ignore lints | LOW | Two existing warnings in test files (`empty_line_after_doc_comments`). Must fix before enabling `-D warnings` in CI |
| `cargo audit` in CI | A security tool that doesn't audit its own dependencies is internally inconsistent | LOW | Use `actions-rust-lang/audit@v1` (maintained). Two current advisories are `informational/unmaintained`, not vulnerabilities — configure to deny only `unsound` and `vulnerability` severity |

### Differentiators (Competitive Advantage)

Features beyond baseline that strengthen developer trust or security posture.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Scheduled weekly `cargo audit` job | New advisories appear after code ships; a one-time CI check misses post-release disclosures | LOW | Add `schedule: cron` trigger (e.g., `0 9 * * 1` — Mondays 9am UTC). Separate job or added trigger to existing `ci.yml` |
| Upgrade `ed25519-dalek` to `3.0.0-pre.6` | Stays current on an active pre-release dep; `pre.5` may have had bugs fixed in `pre.6` | LOW | Change `=3.0.0-pre.5` to `=3.0.0-pre.6` in `Cargo.toml`. Verify `.to_montgomery()` API unchanged. Run full test suite |
| Remove dead `LatestPointer` code | Reduces attack surface and cognitive load; dead crypto-adjacent code is a maintenance liability | LOW | Delete struct (line 97) + test (`test_latest_pointer_serialization`) in `src/record/mod.rs`. No callers in command paths |
| Fix placeholder `user/cclink` paths | Breaks `install.sh` for any real user who tries the curl install command; reflects badly on a security tool with obviously wrong metadata | LOW | `Cargo.toml`: `repository` and `homepage` fields. `install.sh`: line 2 comment and line 6 `REPO=` variable |
| QR + share bug investigation and fix | When `--share` is used, the QR encodes `cclink pickup <publisher-pubkey>` — recipient needs publisher's pubkey, which is what gets encoded. Needs verification this is actually wrong before fixing | LOW | Audit: QR content for `--share` is `cclink pickup <publisher-pubkey>`, which IS what the recipient runs. May not be a bug — or if the issue is something else (wrong key, missing pubkey in QR), clarify and fix |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| PIN complexity rules (uppercase + symbol + digit requirements) | "More rules = more security" intuition | NIST 800-63B-4 (September 2024 draft) explicitly removes mandatory complexity requirements. Complexity rules push users toward predictable substitutions ("P@ssw0rd1!") that are weaker than simple long passphrases. OWASP and NIST now converge: length beats complexity | Enforce minimum 8-character length only. Let the user choose any string |
| PIN strength meter / entropy estimation | Good UX in web apps | Significant implementation complexity for a CLI confirm-prompt flow; false sense of security if entropy estimate is naive; not actionable (user can't see the meter while typing hidden input) | Trust Argon2id (t=3, m=64MB) to make brute force infeasible regardless of exact entropy. Minimum length prevents zero-effort PINs |
| `cargo-deny` for license + advisory checking | More comprehensive than `cargo-audit` | Requires `deny.toml` configuration overhead; more false positives; overkill for a single-binary personal tool at this stage | `cargo-audit` covers the security advisory portion. Add `cargo-deny` only if crate gains complex dependency policies or multiple published crates |
| `#[deny(warnings)]` baked into source code | Makes all builds fail on any warning | Causes downstream compilation failures when Rust stable adds new lints (this happens every 6 weeks). Rust Design Patterns explicitly lists this as an anti-pattern | Use `RUSTFLAGS: "-Dwarnings"` in CI only (env var in `ci.yml`). Source code stays clean of `#[deny]` attributes |
| Numeric-only PIN mode | Simpler UX for some users | At 8 digits, only 10^8 = 100M combinations — fast to brute-force offline even with Argon2id at low memory settings | Accept any string >= 8 characters. The Argon2id parameters provide the real hardening; character set restriction only hurts security |

---

## Feature Dependencies

```
[cargo fmt --check in CI]
    └──requires first──> [fix 2 rustfmt divergences in src/crypto/mod.rs]

[cargo clippy -D warnings in CI]
    └──requires first──> [fix 2 doc-comment warnings in test files]
                             (tests/plaintext_leak.rs, tests/integration_round_trip.rs)

[cargo audit in CI]
    └──no code dependencies, YAML-only change]
    └──configure: deny unsound + vulnerability; allow unmaintained]

[PIN minimum length enforcement]
    └──enhances──> [existing dialoguer::Password prompt in src/commands/publish.rs]
    └──requires──> [validate_with() closure added BEFORE .with_confirmation()]
    └──no conflict with──> [existing confirmation prompt — dialoguer chains correctly]
    └──no conflict with──> [--pin + --share mutual exclusion already enforced in CLI]

[ed25519-dalek pre.5 -> pre.6]
    └──requires──> [verify .to_montgomery() API unchanged between pre-release versions]
    └──requires──> [verify pkarr 5.0.3 does not re-export conflicting dalek types]
    └──validated by──> [full cargo test suite passing after bump]

[dead LatestPointer removal]
    └──no dependencies, no callers — standalone struct + test]

[placeholder path fix]
    └──no code dependencies — metadata-only change in Cargo.toml + install.sh]
```

### Dependency Notes

- **fmt and clippy CI steps require pre-fixes:** Adding either CI step before fixing the existing violations causes an immediate red build. The correct order: fix locally, verify passes, then add the CI step in the same PR.
- **`validate_with()` order matters in dialoguer:** The validator runs on each keystroke (or on submit, depending on dialoguer version). With `.validate_with(...).with_confirmation(...)`, the validator runs first — short PINs are rejected before the user sees the confirm prompt. This is correct behavior.
- **`validate_with()` uses chars().count() vs len():** Use `input.chars().count() >= 8` not `input.len() >= 8` to correctly handle multibyte Unicode characters. A 4-character emoji PIN would fail a `.len()` check at 8 bytes but correctly pass a `.chars().count()` check at 4.
- **Audit step configuration:** `backoff 0.4.0` (RUSTSEC-2025-0012) and `instant 0.1.13` (RUSTSEC-2024-0384) are both `informational/unmaintained` — not vulnerabilities. Configure `audit.toml` or use `ignore` parameter to suppress these, or configure the step to warn but not fail on `unmaintained` level.

---

## MVP Definition

### This Milestone: v1.2 (Dependency Audit & Code Quality)

All items are P1 — this is a quality/hardening milestone, not a feature milestone.

- [ ] **Minimum PIN length (8 chars)** — closes the usability gap where `--pin` silently accepts "1"; implements NIST 800-63B floor; uses `dialoguer::validate_with()` (no new deps)
- [ ] **`cargo fmt --check` in CI** — fix 2 divergent files first; add 1 step to `ci.yml`
- [ ] **`cargo clippy` in CI** — fix 2 doc-comment warnings in test files first; use `RUSTFLAGS: "-Dwarnings"` in workflow env
- [ ] **`cargo audit` in CI** — add `actions-rust-lang/audit@v1`; configure deny on `unsound`/`vulnerability`; allow `unmaintained`
- [ ] **ed25519-dalek `=3.0.0-pre.6`** — bump Cargo.toml; verify API compatibility; run full test suite
- [ ] **Fix `user/cclink` placeholder paths** — `Cargo.toml` repository/homepage fields; `install.sh` REPO variable and usage comment
- [ ] **Remove dead `LatestPointer`** — delete struct + test in `src/record/mod.rs`; no downstream callers

### Add After Validation (not this milestone)

- [ ] **Scheduled weekly `cargo audit`** — add `schedule:` cron trigger after CI hardening is stable; catches post-release advisories
- [ ] **QR + share bug** — verify actual behavior before fixing; may not be a real bug

### Future Consideration (v2+)

- [ ] **Stable ed25519-dalek 3.x** — no stable 3.x exists as of 2026-02-23; monitor `dalek-cryptography` for release
- [ ] **cargo-deny** — add if project grows complex dependency policies or multiple published crates

---

## Feature Prioritization Matrix

| Feature | User / Security Value | Implementation Cost | Priority |
|---------|----------------------|---------------------|----------|
| Minimum PIN length enforcement | HIGH — security correctness, NIST floor | LOW — 1 closure, 0 new deps | P1 |
| `cargo clippy` in CI | HIGH — catches real bugs | LOW — fix 2 warnings + YAML | P1 |
| `cargo audit` in CI | HIGH — security tool must audit itself | LOW — YAML only | P1 |
| `cargo fmt --check` in CI | MEDIUM — code consistency | LOW — fix 2 files + YAML | P1 |
| Fix placeholder repo paths | MEDIUM — breaks install.sh for real users | LOW — string replacement | P1 |
| ed25519-dalek pre.6 upgrade | MEDIUM — stays current on active pre-release | LOW — Cargo.toml bump + verify | P2 |
| Remove dead LatestPointer code | LOW — tech debt, no user impact | LOW — delete struct + test | P2 |
| Scheduled cargo audit | MEDIUM — catches post-release advisories | LOW — cron YAML | P2 |
| QR + share bug | LOW — edge case combination, may not be a bug | LOW — investigate first | P2 |

**Priority key:**
- P1: Must have for this milestone
- P2: Should have, add when possible
- P3: Nice to have, future consideration

---

## Implementation Notes

### PIN Minimum Length

**Standard:** NIST SP 800-63B requires memorized secrets chosen by subscribers to be at least 8 characters in length. OWASP Authentication Cheat Sheet recommends 10+, but 8 is the accepted floor for user-chosen secrets. For a security CLI, 8 is the right conservative floor — defensible, not arbitrary.

**Why 8 for cclink specifically:** The PIN derives a 256-bit key via Argon2id (t=3, m=64MB, p=1) + HKDF-SHA256. Argon2id at these parameters makes offline cracking expensive even for short PINs, but "1" with a random 32-byte salt is still brute-forceable through the entire 1-character PIN space (26 lowercase + 10 digit + symbols = ~100 attempts). At 8 characters, even a low-entropy passphrase like "aaaaaaaa" expands the search space sufficiently that Argon2id parameters dominate attack cost.

**Code location:** `src/commands/publish.rs`, lines 99-103.

**Correct implementation:**

```rust
let pin = dialoguer::Password::new()
    .with_prompt("Enter PIN for this handoff")
    .with_confirmation("Confirm PIN", "PINs don't match")
    .validate_with(|input: &String| -> Result<(), &str> {
        if input.chars().count() >= 8 {
            Ok(())
        } else {
            Err("PIN must be at least 8 characters")
        }
    })
    .interact()
    .map_err(|e| anyhow::anyhow!("PIN prompt failed: {}", e))?;
```

Note: Use `chars().count()` not `len()` to handle Unicode correctly. The validator placement before `interact()` is correct — dialoguer evaluates validators in chain order.

### CI Step Order

The correct order for `ci.yml` to minimize wasted time (fail fast, fail cheap):

1. `cargo fmt --check` — fast, purely structural
2. `cargo clippy --all-targets -- -D warnings` — catches logic issues, slightly slower
3. `cargo test --locked` — full test suite, slowest
4. `actions-rust-lang/audit@v1` — network call to advisory DB
5. OpenSSL absence check — keep existing step

**`RUSTFLAGS: "-Dwarnings"` approach:** Set as an env var in the job or step, not in source. This is the pattern recommended by the official Clippy docs and avoids the `#[deny(warnings)]` anti-pattern.

### cargo audit Configuration

Current advisories (confirmed 2026-02-23):
- `backoff 0.4.0` — RUSTSEC-2025-0012 (unmaintained, not a vulnerability)
- `instant 0.1.13` — RUSTSEC-2024-0384 (unmaintained, transitive dep of `backoff`)

Both are `informational` severity. The `actions-rust-lang/audit@v1` action accepts an `ignore` parameter for specific RUSTSEC IDs, or create `audit.toml` with `[informational_warnings]` to configure behavior. Do not hard-fail CI on unmaintained warnings — this would block every PR if a transitive dep gets flagged post-merge.

The `backoff` crate itself is worth evaluating for replacement or inlining, but that is a separate cleanup item.

### ed25519-dalek Upgrade Path

- Current: `=3.0.0-pre.5`
- Target: `=3.0.0-pre.6` (released 2026-02-04)
- No stable 3.x exists. The `=` exact-version pin is correct — do not relax to `^` or `~` for pre-release deps.
- Key API surface to verify after bump: `SigningKey`, `VerifyingKey`, `.to_montgomery()` (used in `src/crypto/mod.rs` for Ed25519→X25519 conversion)
- pkarr 5.0.3 bundles its own curve25519-dalek dependency; the dual-dalek situation (curve25519-dalek 4 for age, curve25519-dalek 5 for pkarr) already managed via raw bytes — this isolation should survive the bump, but verify with `cargo tree`

---

## Sources

- [NIST SP 800-63B Digital Identity Guidelines](https://pages.nist.gov/800-63-4/sp800-63b.html) — 8-character minimum for user-chosen memorized secrets (HIGH confidence, official NIST publication)
- [NIST SP 800-63B-4 Second Public Draft, September 2024](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-63B-4.2pd.pdf) — removal of mandatory complexity rules (HIGH confidence, official NIST draft)
- [dialoguer::Password API docs](https://docs.rs/dialoguer/latest/dialoguer/struct.Password.html) — `validate_with()` and `PasswordValidator` trait confirmed in 0.12 (HIGH confidence, verified via docs.rs fetch)
- [actions-rust-lang/audit GitHub Action](https://github.com/actions-rust-lang/audit) — maintained replacement for deprecated `actions-rs/audit-check` (HIGH confidence, official repo)
- [Clippy GitHub Actions CI documentation](https://doc.rust-lang.org/nightly/clippy/continuous_integration/github_actions.html) — `RUSTFLAGS: "-Dwarnings"` pattern (HIGH confidence, official Clippy docs)
- [Rust Design Patterns — deny(warnings) anti-pattern](https://rust-unofficial.github.io/patterns/anti_patterns/deny-warnings.html) — rationale for not using `#[deny(warnings)]` in source (HIGH confidence, community standard)
- [RUSTSEC Advisory Database](https://rustsec.org/) — advisory IDs for backoff and instant (HIGH confidence, confirmed via live `cargo audit` run)
- `cargo search ed25519-dalek` — live run 2026-02-23 confirmed `3.0.0-pre.6` as latest (HIGH confidence)
- Live codebase inspection — `cargo clippy --all-targets` (2 warnings), `cargo fmt --check` (2 divergent files), `cargo audit` (2 unmaintained warnings, 0 vulnerabilities) all confirmed 2026-02-23 (HIGH confidence)

---

*Feature research for: cclink v1.2 — PIN enforcement and CI hardening*
*Researched: 2026-02-23*
