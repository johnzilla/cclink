# Phase 11: Prerequisites - Research

**Researched:** 2026-02-23
**Domain:** Rust code quality — clippy, rustfmt, cargo-audit, dependency replacement
**Confidence:** HIGH

## Summary

Phase 11 brings the codebase to a clean baseline before CI gates are introduced in Phase 12. Three categories of work are required: (1) fix two clippy errors in test files caused by `///` doc comments that should be `//!` inner doc comments or plain `//` comments, (2) run `cargo fmt` to reformat 12 source files that have accumulated drift, and (3) resolve two RUSTSEC unmaintained advisories — the `backoff` crate (RUSTSEC-2025-0012) and its transitive dependency `instant` (RUSTSEC-2024-0384) — by replacing `backoff` with `backon 1.6.0`.

The `backoff` → `backon` migration requires a targeted rewrite of `src/commands/pickup.rs` (the only file that uses it). The API differs: `backon` uses a fluent `.retry(builder).when(predicate).call()` pattern for blocking code instead of `backoff::retry(config, closure)`. Additionally, Cargo.toml must have a comment added documenting the `ed25519-dalek = "=3.0.0-pre.5"` exact pin constraint (the pkarr 5.0.3 dependency forces it; no stable ed25519-dalek 3.x exists). Formatting is achieved with a single `cargo fmt` invocation — no manual editing required.

The key decision documented in STATE.md is: replace `backoff` with `backon` now (do not use `audit.toml` ignores as a workaround). This is the right call since the replacement is straightforward and eliminates both advisories at once (removing `backoff` removes its transitive `instant` dependency too).

**Primary recommendation:** Fix clippy errors manually, run `cargo fmt` for formatting, replace `backoff` with `backon 1.6.0` in pickup.rs, add Cargo.toml comment for ed25519-dalek pin. All four gates will then pass.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CI-01 | Fix existing clippy warnings in test files (doc comment style `///` → inner doc comments `//!` or plain `//` in test files) | Clippy lint is `empty-line-after-doc-comments`. Fix: convert file-level `///` blocks to `//!` inner doc comments in `tests/integration_round_trip.rs` and `tests/plaintext_leak.rs` |
| DEP-01 | Document ed25519-dalek pre-release constraint in Cargo.toml comment (pkarr 5.0.3 forces `=3.0.0-pre.5`) | Add inline TOML comment on the `ed25519-dalek` line explaining the exact-pin constraint |
| DEP-02 | Replace unmaintained `backoff` crate (RUSTSEC-2025-0012) and transitive `instant` crate (RUSTSEC-2024-0384) | Replace `backoff = "0.4"` with `backon = "1.6"` in Cargo.toml; rewrite backoff usage in `src/commands/pickup.rs` using `BlockingRetryable` trait |
</phase_requirements>

## Standard Stack

### Core

| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| rustfmt | (bundled with toolchain) | Format all Rust source files | Official formatter; `cargo fmt` fixes all style drift in one command |
| clippy | (bundled with toolchain) | Lint Rust code for style and correctness | Official linter; `-D warnings` makes warnings into errors |
| cargo-audit | 0.22.x | Scan Cargo.lock for known RUSTSEC advisories | Standard security scanning for Rust; used by RustSec advisory DB authors |
| backon | 1.6.0 | Retry with exponential backoff for blocking and async | Recommended replacement in RUSTSEC-2025-0012 advisory itself |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| backon ExponentialBuilder | 1.6.0 | Configurable exponential backoff strategy | Default backoff strategy; matches what backoff 0.4 ExponentialBackoff provided |
| backon BlockingRetryable | 1.6.0 | Adds `.retry()` method to sync `FnMut() -> Result` closures | pickup.rs runs synchronously (not async); use BlockingRetryable not Retryable |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| backon (replace) | audit.toml ignore entries | Ignoring keeps unmaintained code in the tree; replacement is ~15 lines of code and eliminates both advisories permanently |
| cargo fmt (whole codebase) | Manual file-by-file edits | cargo fmt is idempotent and safe; manual edits are error-prone and unnecessary |

## Architecture Patterns

### Clippy Fix Pattern: Doc Comments in Test Files

Clippy lint: `empty-line-after-doc-comments` (implied by `-D warnings` under `clippy::empty-line-after-doc-comments`).

**Root cause:** Test files use `///` (outer doc comments) at the file top level. These become doc comments on the next item (`use` imports), causing the lint when there is a blank line between the comment block and the item. The fix is to convert file-level documentation to `//!` inner doc comments, which document the enclosing module (the test file itself).

**Pattern — before (broken):**
```rust
/// Integration tests: encryption round-trip...
///
/// Tests cover: ...
///
/// All tests are `#[test]`...

use cclink::crypto::{...};
```

**Pattern — after (correct):**
```rust
//! Integration tests: encryption round-trip...
//!
//! Tests cover: ...
//!
//! All tests are `#[test]`...

use cclink::crypto::{...};
```

Files to fix:
- `tests/integration_round_trip.rs` — lines 1-12 (file header block)
- `tests/plaintext_leak.rs` — lines 1-8 (file header block)

Note: Function-level `///` doc comments inside test files (e.g., `/// Fixed keypair with seed...` on `fn keypair_a()`) are valid outer doc comments and must NOT be changed.

### Formatting Fix Pattern

`cargo fmt` is the correct and complete solution. It rewrites files in-place to match rustfmt conventions. No manual editing is required.

**Files with formatting drift (12 files):**
- `src/cli.rs`
- `src/commands/init.rs`
- `src/commands/pickup.rs`
- `src/commands/publish.rs`
- `src/commands/revoke.rs`
- `src/crypto/mod.rs`
- `src/keys/store.rs`
- `src/record/mod.rs`
- `src/session/mod.rs`
- `src/transport/mod.rs`
- `tests/integration_round_trip.rs`
- `tests/plaintext_leak.rs`

Most drift is line-length reformatting of long expressions and alphabetical import ordering. `cargo fmt` handles all of it.

**Order matters:** Run `cargo fmt` AFTER fixing the clippy doc-comment issues (converting `///` to `//!`), because rustfmt will also reformat those files. Doing fmt first then fixing clippy avoids a second fmt run.

### Cargo.toml Comment Pattern for ed25519-dalek Pin (DEP-01)

Inline TOML comments go on the same line or the line above. The convention for constraint explanations is:

```toml
# pkarr 5.0.3 requires ed25519-dalek 3.x pre-release; no stable 3.x exists.
# Do not change this pin until pkarr publishes a release that depends on a stable ed25519-dalek 3.x.
ed25519-dalek = "=3.0.0-pre.5"
```

The exact-version pin (`=3.0.0-pre.5`) is required because `pkarr = "5.0.3"` itself depends on `ed25519-dalek` without an exact pin, but the only compatible version is the pre-release. Without the exact pin, Cargo may fail to resolve or resolve to an incompatible version.

### backon Migration Pattern (DEP-02)

**Current code in `src/commands/pickup.rs`:**

```rust
use backoff::{retry, ExponentialBackoff, Error as BackoffError};

let backoff_config = ExponentialBackoff {
    max_elapsed_time: Some(std::time::Duration::from_secs(30)),
    max_interval: std::time::Duration::from_secs(8),
    initial_interval: std::time::Duration::from_secs(2),
    ..Default::default()
};

let record = retry(backoff_config, || {
    match client.resolve_record(&target_z32_owned) {
        Ok(r) => Ok(r),
        Err(e) => {
            if e.downcast_ref::<crate::error::CclinkError>()
                .is_some_and(|ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
            {
                Err(BackoffError::permanent(e))
            } else {
                Err(BackoffError::transient(e))
            }
        }
    }
})
.map_err(|e| anyhow::anyhow!("Failed to retrieve handoff after retries: {}", e))?;
```

**Replacement pattern with backon 1.6.0:**

```rust
use backon::{BlockingRetryable, ExponentialBuilder};

let backoff_builder = ExponentialBuilder::default()
    .with_min_delay(std::time::Duration::from_secs(2))
    .with_max_delay(std::time::Duration::from_secs(8))
    .with_max_times(6); // ~30 seconds total at these delays

let record = (|| client.resolve_record(&target_z32_owned))
    .retry(backoff_builder)
    .sleep(std::thread::sleep)
    .when(|e| {
        // Only retry on transient errors; permanent errors (RecordNotFound) stop immediately
        !e.downcast_ref::<crate::error::CclinkError>()
            .is_some_and(|ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
    })
    .call()
    .map_err(|e| anyhow::anyhow!("Failed to retrieve handoff after retries: {}", e))?;
```

**Key differences:**
- `backoff::retry(config, closure)` → `closure.retry(builder).sleep(std::thread::sleep).call()`
- `BackoffError::permanent(e)` → `.when(predicate)` where predicate returns `false` to stop retrying
- `BackoffError::transient(e)` → `.when(predicate)` where predicate returns `true` to continue
- `max_elapsed_time` is not directly available in backon ExponentialBuilder; use `with_max_times` to approximate. `with_max_times(6)` gives approximately 2+4+8+8+8+8 = 38 seconds worst case with these delays — close enough to the original 30s cap.
- `.sleep(std::thread::sleep)` is required for blocking use; without it backon defaults to a no-op sleep (no actual delay between retries)

**Cargo.toml change:**
```toml
# Before
backoff = "0.4"

# After
backon = "1.6"
```

### audit.toml Pattern (fallback only — NOT used for DEP-02)

If there were advisories that could NOT be resolved by replacement (e.g., a transitive advisory in a deep dependency with no maintainable replacement), the pattern is `.cargo/audit.toml`:

```toml
[advisories]
ignore = [
    # RUSTSEC-XXXX-YYYY: <crate> is unmaintained.
    # Rationale: <reason why replacement is not feasible>.
    # Tracked in: <issue or PR link>.
    "RUSTSEC-XXXX-YYYY",
]
```

This is documented here for completeness but is NOT the path for DEP-02 since `backoff` → `backon` is straightforward.

### Anti-Patterns to Avoid

- **Fixing fmt manually:** Never hand-edit files to match rustfmt output. Run `cargo fmt` and let the tool do it.
- **Changing function-level `///` to `//!`:** Only the file-level header blocks need to change. Inner function doc comments with `///` are correct.
- **Using `audit.toml` ignores for DEP-02:** The advisory recommends `backon` as a replacement. Using ignores would leave the unmaintained code in the tree and would not resolve the `instant` transitive advisory.
- **Approximating `max_elapsed_time` via jitter:** Use `with_max_times` for a deterministic upper bound. Jitter adds randomness that makes the total time unpredictable.
- **Forgetting `.sleep(std::thread::sleep)` in backon:** Without this, backon will not actually sleep between retries in blocking code — the retries would hammer the network with no delay.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Exponential backoff with jitter | Custom retry loop with `std::thread::sleep` | `backon::ExponentialBuilder` | Edge cases: overflow on large delays, missing jitter, no max-times cap |
| Code formatting | Manual whitespace fixes | `cargo fmt` | Idempotent, consistent, zero effort, handles 12 files at once |

## Common Pitfalls

### Pitfall 1: Clippy Passes on Main Targets but Fails on Test Targets

**What goes wrong:** Running `cargo clippy` (without `--all-targets`) does not check integration test files in `tests/`. The CI requirement is `cargo clippy --all-targets -- -D warnings`. Running clippy without `--all-targets` will show green, then CI will fail.

**Why it happens:** Integration tests in `tests/` are compiled as separate crates. Without `--all-targets`, clippy skips them.

**How to avoid:** Always verify with `cargo clippy --all-targets -- -D warnings`, exactly as specified in the success criteria.

**Warning signs:** Clippy exits 0 when run without `--all-targets` but the test files still have `///` doc comments.

### Pitfall 2: cargo fmt Changes Are Cosmetic but Must Happen Before Clippy

**What goes wrong:** `cargo fmt` and clippy are independent. The order of operations matters for the commit: fix clippy issues (the `///` → `//!` changes) first, then run `cargo fmt` once to clean up formatting across all files. If you run fmt before fixing clippy, the clippy fix will make a formatting change that isn't yet normalized.

**Why it happens:** Both tools touch the same files. Running fmt after clippy fixes means one clean pass.

**How to avoid:** Sequence: (1) fix `///` → `//!` in test files, (2) run `cargo fmt`, (3) verify `cargo clippy --all-targets -- -D warnings` exits 0, (4) verify `cargo fmt --check` exits 0.

### Pitfall 3: backon `.when()` Predicate Semantics Are Inverted Relative to backoff

**What goes wrong:** In `backoff`, you return `Err(BackoffError::permanent(e))` to STOP retrying. In `backon`, `.when(predicate)` returns `true` to CONTINUE retrying and `false` to STOP.

**Why it happens:** The API semantic is opposite: backoff signals "stop" explicitly, backon signals "continue" via predicate.

**How to avoid:** The `.when()` predicate logic must be the inverse of the original permanent-error check. If old code did `if is_permanent { Err(BackoffError::permanent(e)) }`, new code does `.when(|e| !is_permanent(e))`.

**Warning signs:** RecordNotFound errors are retried indefinitely instead of failing immediately.

### Pitfall 4: backon max_elapsed_time Has No Direct Equivalent

**What goes wrong:** `ExponentialBackoff { max_elapsed_time: Some(Duration::from_secs(30)) }` is straightforward in `backoff`. `backon::ExponentialBuilder` does not have a `with_total_delay` method by name in the public API; `with_max_times` gives an attempt count bound, not a wall-clock bound.

**Why it happens:** Different design philosophy. `backoff` uses wall-clock timeout; `backon` uses attempt count.

**How to avoid:** Use `with_max_times` and calculate the approximate worst-case elapsed time from `min_delay + ... + max_delay * (max_times - N)`. For the current config (min=2s, max=8s, target ~30s total), `with_max_times(6)` gives a reasonable bound.

### Pitfall 5: Formatting Drift Accumulates Silently Without CI

**What goes wrong:** `cargo fmt --check` currently fails on 12 files. This drift likely happened gradually because there was no CI gate. After Phase 11, Phase 12 will add that gate.

**Why it happens:** No enforcement mechanism.

**How to avoid:** Run `cargo fmt` once now. The resulting diff is large but cosmetic (no semantic changes). Commit it as a single formatting commit distinct from logic changes.

## Code Examples

Verified patterns from official sources and codebase analysis:

### Verify All Three Gates Pass

```bash
# Run these in order to confirm the phase is complete
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo audit
```

All three must exit 0 with no errors or warnings.

### Fix Test File Doc Comments (CI-01)

```rust
// tests/integration_round_trip.rs — change lines 1-12
// BEFORE (causes clippy lint empty-line-after-doc-comments):
/// Integration tests: encryption round-trip for all four code paths...
///
/// ...

// AFTER (inner doc comments for the module/file):
//! Integration tests: encryption round-trip for all four code paths...
//!
//! ...
```

### Add ed25519-dalek Pin Comment (DEP-01)

```toml
# In Cargo.toml [dependencies]:
# pkarr 5.0.3 requires ed25519-dalek 3.x pre-release; no stable 3.x exists yet.
# This exact pin must remain until pkarr publishes a release depending on a stable ed25519-dalek 3.x.
ed25519-dalek = "=3.0.0-pre.5"
```

### backon Replacement (DEP-02)

```toml
# Cargo.toml: remove backoff, add backon
backon = "1.6"
```

```rust
// src/commands/pickup.rs
use backon::{BlockingRetryable, ExponentialBuilder};

let record = (|| client.resolve_record(&target_z32_owned))
    .retry(
        ExponentialBuilder::default()
            .with_min_delay(std::time::Duration::from_secs(2))
            .with_max_delay(std::time::Duration::from_secs(8))
            .with_max_times(6),
    )
    .sleep(std::thread::sleep)
    .when(|e| {
        !e.downcast_ref::<crate::error::CclinkError>()
            .is_some_and(|ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
    })
    .call()
    .map_err(|e| anyhow::anyhow!("Failed to retrieve handoff after retries: {}", e))?;
```

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| `backoff 0.4` (unmaintained since ~2023) | `backon 1.6` (actively maintained, recommended by advisory) | Eliminates RUSTSEC-2025-0012 and transitive RUSTSEC-2024-0384 |
| `///` at file top level in test files | `//!` inner doc comments | Clippy `empty-line-after-doc-comments` lint no longer fires |
| No formatting enforcement | `cargo fmt --check` as CI gate (Phase 12) | Phase 11 establishes the clean baseline Phase 12 will guard |

**Deprecated/outdated:**
- `backoff::ExponentialBackoff` struct with field initialization: replaced by `backon::ExponentialBuilder` fluent builder API
- `backoff::Error::permanent` / `backoff::Error::transient`: replaced by `backon` `.when()` predicate

## Open Questions

1. **backon `with_max_times` vs `max_elapsed_time` equivalence**
   - What we know: `backoff` had `max_elapsed_time: Some(Duration::from_secs(30))`. `backon` does not have a wall-clock total-elapsed cap in `ExponentialBuilder`.
   - What's unclear: Whether `with_total_delay` exists in backon 1.6 (docs mention it in the method list but exact behavior with blocking retries is not verified).
   - Recommendation: Use `with_max_times(6)` conservatively. Verify by running `cargo doc --open` on the installed backon crate and checking `ExponentialBuilder` methods before committing.

2. **Whether `cargo fmt` changes will conflict with clippy fix**
   - What we know: Both tools touch `tests/integration_round_trip.rs` and `tests/plaintext_leak.rs`.
   - What's unclear: Whether rustfmt reformats `//!` vs `///` (it does not — comment content is not reformatted by rustfmt).
   - Recommendation: Fix doc comments first, then fmt. Both changes will coexist cleanly.

## Sources

### Primary (HIGH confidence)

- `cargo clippy --all-targets -- -D warnings` run on actual codebase — 2 errors confirmed in test files
- `cargo fmt --check` run on actual codebase — 12 files confirmed needing formatting
- `cargo audit` run on actual codebase — 2 advisories: RUSTSEC-2025-0012 (backoff) and RUSTSEC-2024-0384 (instant, transitive)
- https://docs.rs/backon/1.6.0/backon/ — BlockingRetryable API, ExponentialBuilder methods
- https://docs.rs/backon/1.6.0/backon/struct.ExponentialBuilder.html — with_min_delay, with_max_delay, with_max_times, with_jitter, defaults
- https://rustsec.org/advisories/RUSTSEC-2025-0012.html — backoff advisory, backon as recommended replacement
- https://github.com/RustSec/rustsec/blob/main/.cargo/audit.toml — real-world audit.toml with inline comment pattern

### Secondary (MEDIUM confidence)

- WebSearch + docs.rs verification: audit.toml `[advisories] ignore = [...]` syntax confirmed across multiple sources
- backon `BlockingRetryable::when()` predicate semantics verified from docs.rs examples

### Tertiary (LOW confidence)

- `with_total_delay` on `ExponentialBuilder`: mentioned in docs but behavior with blocking retries not fully verified — check before using

## Metadata

**Confidence breakdown:**
- Clippy issues: HIGH — directly observed via tool run on the actual codebase
- Formatting issues: HIGH — directly observed via `cargo fmt --check` on actual codebase
- cargo audit advisories: HIGH — directly observed via `cargo audit` run
- backon API: HIGH — verified against official docs.rs documentation
- backon `.when()` semantics: MEDIUM — verified from examples but exact edge cases not tested
- audit.toml syntax: HIGH — verified against RustSec's own repository usage

**Research date:** 2026-02-23
**Valid until:** 2026-03-25 (stable tooling, 30-day window)
