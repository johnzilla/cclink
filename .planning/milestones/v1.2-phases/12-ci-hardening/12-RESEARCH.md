# Phase 12: CI Hardening - Research

**Researched:** 2026-02-23
**Domain:** GitHub Actions CI workflow — Rust lint, format, and security audit jobs
**Confidence:** HIGH

## Summary

Phase 12 adds three CI quality gates to the existing GitHub Actions pipeline: a `lint` job running `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`, an `audit` job running `cargo-audit` via the `actions-rust-lang/audit@v1` action, and requires that these run in parallel with the existing `test` job rather than appended to it.

The current `ci.yml` has a single `test` job using `dtolnay/rust-toolchain@stable` and `Swatinem/rust-cache@v2`. The Phase 11 work (now complete) ensures that clippy, fmt, and audit all exit 0 locally before this CI harness is added — meaning CI will be green on day one.

The implementation is pure YAML surgery on `.github/workflows/ci.yml`. No Rust source changes are required. The critical design point is that GitHub Actions jobs are parallel by default when they share no `needs:` dependency — so adding `lint` and `audit` as top-level jobs alongside `test` achieves the parallelism requirement automatically. The `actions-rust-lang/audit@v1` action (latest patch: v1.2.7) handles tool installation internally; no separate `cargo install cargo-audit` step is needed.

**Primary recommendation:** Add `lint` and `audit` as independent top-level jobs in `ci.yml`, matching the `on:` triggers and toolchain setup already used in the existing `test` job.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CI-02 | Add `cargo clippy --all-targets -- -D warnings` job to CI pipeline | Confirmed: add top-level `lint` job with `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` steps |
| CI-03 | Add `cargo audit` job to CI via `actions-rust-lang/audit@v1` | Confirmed: add top-level `audit` job using `actions-rust-lang/audit@v1` with `permissions: contents: read` |
| CI-04 | Add `cargo fmt --check` job to CI pipeline | Confirmed: fold into same `lint` job as CI-02 (clippy and fmt checking are collocated by convention) |
</phase_requirements>

## Standard Stack

### Core

| Tool/Action | Version | Purpose | Why Standard |
|-------------|---------|---------|--------------|
| `dtolnay/rust-toolchain` | `@stable` | Install Rust stable with components | Already in project CI; minimal, reliable |
| `Swatinem/rust-cache` | `@v2` | Cache cargo registry and build artifacts | Already in project CI; cuts lint job time significantly |
| `actions-rust-lang/audit` | `@v1` (v1.2.7 latest) | Run `cargo audit` against RustSec advisory DB | Official action maintained by actions-rust-lang org; handles tool install internally |
| `actions/checkout` | `@v4` | Checkout source | Required by all jobs |

### Components Required for Lint Job

The `dtolnay/rust-toolchain@stable` action accepts a `components:` input. The `lint` job must explicitly request `clippy` and `rustfmt`:

```yaml
- uses: dtolnay/rust-toolchain@stable
  with:
    components: clippy, rustfmt
```

Without this, `cargo clippy` may fail if the component is not installed (stable includes them by default, but explicit declaration is safer and self-documenting).

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `dtolnay/rust-toolchain@stable` | `actions-rust-lang/setup-rust-toolchain@v1` | setup-rust-toolchain has built-in caching + problem matchers but changes the action used vs. what test job uses — creates inconsistency; not worth the churn |
| `actions-rust-lang/audit@v1` | Manual `cargo install cargo-audit && cargo audit` | Manual install is slower (compiles audit tool each run) and loses the issue-creation and summary features |
| Single combined job | Separate parallel jobs | Combined job is simpler but means a clippy failure also blocks seeing audit results; parallel provides clearer attribution per success criterion 3 |

## Architecture Patterns

### Recommended CI Structure

```
.github/workflows/ci.yml
  jobs:
    test:     # existing — unchanged
    lint:     # new — clippy + fmt
    audit:    # new — cargo audit
```

All three jobs share the same `on:` triggers. No `needs:` relationships between them — they run in parallel automatically.

### Pattern 1: Independent Parallel Jobs

**What:** In GitHub Actions, top-level jobs without `needs:` dependencies run concurrently on separate runners.
**When to use:** When failures in one gate should not prevent another gate from reporting — satisfies success criterion 4 ("run in parallel, not appended").

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --locked

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo fmt --check

  audit:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/audit@v1
```

**Source:** GitHub Actions docs on job parallelism; actions-rust-lang/audit README.

### Pattern 2: `audit` Job Permissions

**What:** `actions-rust-lang/audit@v1` can create GitHub issues for discovered vulnerabilities. The `issues: write` permission enables this; `contents: read` is the minimum. For this project the `createIssues` behavior defaults to main/master branch only, so issue creation activates automatically on merges to main.

**When to use:** Always include explicit permissions block on `audit` job even if only `contents: read` — principle of least privilege.

### Pattern 3: Cache Sharing Between Jobs

**What:** `Swatinem/rust-cache@v2` uses a cache key based on `Cargo.lock` hash. Multiple concurrent jobs will each attempt to restore the same cache. The first to finish writes back; others read only. This is safe — cache is read-only during a run.

**When to use:** Include `Swatinem/rust-cache@v2` in both `lint` and `test` jobs for build speed. The `audit` job does not compile code, so cache is not needed there.

### Anti-Patterns to Avoid

- **Appending lint steps to the `test` job:** This makes lint failures indistinguishable from test failures in the GitHub UI, and fails success criterion 3 (clippy warning should fail `lint`, not `test`).
- **Running `cargo fmt` without `--check`:** Without `--check`, fmt silently reformats files in the runner's workspace and reports success. The `--check` flag makes it exit non-zero if files would change.
- **Using `actions-rs/*` actions:** The `actions-rs` organization's actions (e.g., `actions-rs/clippy-check`) are unmaintained. Multiple web sources confirm this. Use bare `run:` steps with `cargo clippy` instead.
- **Not declaring `components: clippy, rustfmt`:** Omitting the components declaration works today (stable includes them) but is fragile for toolchain overrides. Be explicit.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Installing cargo-audit | `cargo install cargo-audit` in a run step | `actions-rust-lang/audit@v1` | Action caches the tool binary, generates a step summary, and creates issues; manual install recompiles on every run (~2 min penalty) |
| Parallel job orchestration | `needs:` chains or matrix tricks | Top-level jobs with no `needs:` | Native GHA parallelism is automatic; no custom logic needed |

**Key insight:** `actions-rust-lang/audit@v1` does not require Rust to be installed — it ships with cargo-audit pre-bundled. Do not add `dtolnay/rust-toolchain` as a prerequisite step in the `audit` job.

## Common Pitfalls

### Pitfall 1: `cargo fmt --check` vs `cargo fmt`

**What goes wrong:** Workflow author forgets `--check`; `cargo fmt` succeeds silently on the runner, job is always green.
**Why it happens:** Muscle memory from local use where you want fmt to reformat.
**How to avoid:** Command must be `cargo fmt --check` (exits 1 if files would be modified). The `--check` flag is not `--dry-run`; both exist but `--check` is the correct one for CI.
**Warning signs:** The fmt job always passes even when you introduce messy formatting.

### Pitfall 2: `audit` Job Needing `issues: write` Permission

**What goes wrong:** `createIssues` is enabled (it is by default on main/master pushes) but the job lacks `issues: write` permission; audit job fails with a permissions error.
**Why it happens:** Default GitHub Actions `GITHUB_TOKEN` permissions vary by repo settings; not always write-enabled.
**How to avoid:** Add explicit `permissions:` block. Either `issues: write` (to allow issue creation) or disable `createIssues: false` input if issue tracking is not desired. Since this project has no existing advisory issues to manage, `issues: write` is the safest default.
**Warning signs:** `Error: Resource not accessible by integration` in audit job logs.

### Pitfall 3: Lint Job Clippy Cache Miss on First Run

**What goes wrong:** `Swatinem/rust-cache@v2` in the `lint` job hits a cold cache on first run, making it look slow. Not a failure, but worth knowing.
**Why it happens:** Cache key includes workspace hash; first run always writes.
**How to avoid:** No action needed; subsequent runs warm. Worth noting in plan so the implementer does not debug a non-issue.

### Pitfall 4: `--all-targets` Requires Test Dependencies

**What goes wrong:** `cargo clippy --all-targets` compiles `[dev-dependencies]` (e.g., `tempfile`). Any new dev-dep that introduces a clippy warning will fail the lint job.
**Why it happens:** `--all-targets` explicitly includes test, bench, and example targets.
**How to avoid:** Desired behavior — keep dev-deps lint-clean. Phase 11 already cleaned all existing warnings (CI-01 complete), so day-one should be green.

### Pitfall 5: Job Named `lint` vs `check`

**What goes wrong:** Confusion about what the job is called. Success criterion 1 says "CI runs a `lint` job."
**Why it happens:** Some templates name it `check`, some `clippy`, some `lint`.
**How to avoid:** Name the job `lint` to match the success criterion exactly.

## Code Examples

Verified patterns from official sources:

### Complete Updated `ci.yml`

```yaml
# Source: GitHub Actions docs (parallel jobs), actions-rust-lang/audit README
name: CI
on:
  push:
    branches: ["*"]
  pull_request:
    branches: ["*"]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --locked
      - name: Verify no OpenSSL dependency
        run: |
          cargo tree 2>/dev/null | grep -i openssl && echo "ERROR: OpenSSL found in dependency tree" && exit 1 || echo "OK: No OpenSSL dependency"

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo fmt --check

  audit:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      issues: write
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/audit@v1
```

### Verifying Parallelism Locally

There is no local equivalent — parallelism is a GHA concept. To verify: after push, check the workflow run's job graph in GitHub UI. All three jobs (`test`, `lint`, `audit`) should show as running concurrently without a dependency arrow.

### Verifying Success Criterion 3 (clippy fails lint, not test)

To validate that a clippy warning fails `lint` while `test` stays green:
1. Introduce a trivial clippy-triggering change (e.g., `let x = String::new(); let _ = x;` — triggers `clippy::redundant_closure_for_method_calls` or similar)
2. Push to a branch
3. Observe: `lint` job fails, `test` job passes
4. Revert

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `actions-rs/clippy-check@v1` | Bare `run: cargo clippy` step | ~2022 | actions-rs is unmaintained; bare step is simpler and more reliable |
| `actions-rs/cargo@v1 with command: audit` | `actions-rust-lang/audit@v1` | ~2022 | Dedicated action with summaries and issue creation |
| Appending lint to test job | Separate parallel jobs | Ongoing best practice | Clearer attribution of failures; faster feedback |

**Deprecated/outdated:**
- `actions-rs/*` family: Unmaintained as of ~2022. Do not use `actions-rs/toolchain`, `actions-rs/clippy-check`, or `actions-rs/cargo`.

## Open Questions

1. **Should `audit` job also trigger on `schedule:` (daily cron)?**
   - What we know: The `actions-rust-lang/audit` README shows a common pattern of scheduled daily audits so new advisories are caught between dependency changes.
   - What's unclear: The phase success criteria only require "every push" triggering, not scheduled. The current `on:` block in `ci.yml` uses `push: branches: ["*"]` and `pull_request: branches: ["*"]`.
   - Recommendation: Keep the `audit` job under the same `on:` triggers as `test` and `lint` for this phase. A daily schedule is a future enhancement outside CI-03 scope.

2. **`issues: write` permission for `audit` — is it needed?**
   - What we know: `createIssues` defaults to `true` on main/master pushes. Without `issues: write`, the job will error on main branch pushes.
   - What's unclear: Whether the project owner wants GitHub issues auto-created for advisories.
   - Recommendation: Include `issues: write` in the permissions block. This is the safest default; it enables the feature without forcing it. The default behavior only creates issues on main, which is useful for a security tool.

## Sources

### Primary (HIGH confidence)
- `actions-rust-lang/audit` README (fetched 2026-02-23) — inputs, permissions, usage examples — https://github.com/actions-rust-lang/audit
- `actions-rust-lang/setup-rust-toolchain` README (fetched 2026-02-23) — comparison with dtolnay — https://github.com/actions-rust-lang/setup-rust-toolchain
- `dtolnay/rust-toolchain` README (fetched 2026-02-23) — components input — https://github.com/dtolnay/rust-toolchain
- Project `ci.yml` (read directly) — existing job structure, action versions in use

### Secondary (MEDIUM confidence)
- WebSearch: "GitHub Actions Rust CI parallel jobs clippy rustfmt cargo audit 2025 2026" — confirmed actions-rs deprecation, parallel job pattern

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — Actions versions confirmed from official GitHub repos fetched 2026-02-23
- Architecture: HIGH — Pattern is direct YAML extension of existing ci.yml; no ambiguity
- Pitfalls: HIGH — Derived from official docs and direct inspection of existing project state

**Research date:** 2026-02-23
**Valid until:** 2026-09-23 (stable GitHub Actions, changes slowly)
