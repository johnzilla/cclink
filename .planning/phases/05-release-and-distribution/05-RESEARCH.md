# Phase 5: Release and Distribution - Research

**Researched:** 2026-02-22
**Domain:** Rust binary distribution, GitHub Actions CI/CD, cross-compilation, musl static linking, integration testing with mock HTTP
**Confidence:** HIGH (core stack verified against official docs/repos; a few specifics flagged as MEDIUM)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Build targets:**
- Full platform coverage: Linux (x86_64-musl + aarch64-musl), macOS (universal binary combining Intel + Apple Silicon), Windows (x86_64)
- Linux binaries statically linked via musl — no runtime dependencies
- macOS ships as a single universal (fat) binary
- Artifact naming: `cclink-{os}-{arch}` format (e.g., cclink-linux-x86_64, cclink-darwin-universal, cclink-windows-x86_64.exe)

**CI pipeline behavior:**
- Release triggered by tag push only (e.g., v1.0.0) — deliberate releases
- Test suite (cargo test) runs on every push and pull request — catches issues early
- Version: tag IS the version, manual sync with Cargo.toml
- CI creates a GitHub Release with auto-generated changelog from commits since last tag
- SHA256 checksums generated alongside binaries

**Installation experience:**
- Primary: curl | sh installer that auto-detects platform and downloads the correct binary
- Installer path: try ~/.local/bin first (no sudo), fall back to /usr/local/bin with sudo prompt
- Installer verifies downloads with SHA256 checksums before installing
- Secondary: publish to crates.io for `cargo install cclink`

**Round-trip test strategy:**
- All encryption code paths tested: self-encrypt, shared (--share), burn (--burn), and shared+burn
- Dedicated plaintext leak test: encrypts a known value, inspects ciphertext blob, asserts plaintext is absent
- Integration tests use an in-process mock HTTP server (wiremock or similar) — full publish/pickup flow without real homeserver
- No external dependencies in CI (no Docker, no real servers)

### Claude's Discretion
- Specific CI matrix configuration and runner selection
- Mock server implementation crate choice (wiremock, axum, etc.)
- Installer script language (bash, POSIX sh, etc.)
- Cross-compilation toolchain setup details

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

---

## Summary

Phase 5 delivers CCLink as a polished, distributable binary with full CI/CD automation. The dominant tool for building and uploading multi-platform Rust binaries from GitHub Actions is `taiki-e/upload-rust-binary-action` combined with `taiki-e/create-gh-release-action` — both are well-maintained, support musl and `universal-apple-darwin`, handle SHA256 checksums, and use the `cross` toolchain internally for cross-compilation. The project's existing use of `reqwest` with `default-features = false, features = ["rustls"]` means there are no OpenSSL complications for musl builds — this is the single most common musl pitfall and it's already avoided.

For integration tests with the mock HTTP server, use `httpmock 0.8.3`, not wiremock. The reason: `httpmock::MockServer::start()` is synchronous and works in regular `#[test]` without a tokio runtime, which is essential because the entire transport layer uses `reqwest::blocking`. Wiremock requires `MockServer::start().await` inside `#[tokio::test]`, which conflicts with the blocking reqwest client.

For crates.io publishing, the modern approach is Trusted Publishing (OIDC-based, announced July 2025) — no long-lived API tokens. The installer script should be POSIX sh (not bash) for maximum portability; the pattern from rustup and similar tools is a single-file script that detects `uname -s` + `uname -m`, constructs the download URL, downloads with `curl -fsSL`, verifies SHA256 with `sha256sum` or `shasum -a 256`, then installs to `~/.local/bin` with a `$PATH` check.

**Primary recommendation:** Use `taiki-e/upload-rust-binary-action@v1` for release builds, `httpmock 0.8.3` for integration tests, and crates.io Trusted Publishing for cargo install support.

---

## Standard Stack

### Core

| Library/Tool | Version | Purpose | Why Standard |
|---|---|---|---|
| `taiki-e/upload-rust-binary-action` | v1 (latest) | Build + upload release binaries to GitHub Releases | Native `universal-apple-darwin` support, SHA256 checksums, musl via cross, widely used in Rust ecosystem |
| `taiki-e/create-gh-release-action` | v1 (latest) | Create GitHub Release on tag push with auto-generated notes | Companion to upload-rust-binary-action; built-in changelog generation |
| `httpmock` | 0.8.3 | In-process mock HTTP server for integration tests | Synchronous API works with `reqwest::blocking` without tokio runtime; parallel test support; released 2026-02-04 |
| `cross` (via action) | handled by upload-rust-binary-action | Cross-compilation toolchain | Zero-config Docker-based cross-compilation; used internally by the upload action |
| `softprops/action-gh-release` | v2 | Alternative release action (if not using taiki-e pair) | Most popular release action; v2.5.0 current |

### Supporting

| Library/Tool | Version | Purpose | When to Use |
|---|---|---|---|
| `dtolnay/rust-toolchain` | latest | Install specific Rust toolchain in GHA | CI test jobs where you need a specific toolchain |
| `Swatinem/rust-cache` | v2 | Cache Cargo registry and build artifacts | Every CI job — dramatically speeds up builds |
| `crates.io Trusted Publishing` | N/A (OIDC, no version) | Publish to crates.io without long-lived tokens | The modern approach since July 2025; replaces `CARGO_REGISTRY_TOKEN` secret |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|---|---|---|
| `taiki-e/upload-rust-binary-action` | `houseabsolute/actions-rust-release` v0.0.6 | houseabsolute action is newer (2025), less mature, 24 stars vs taiki-e's broad adoption |
| `httpmock` | `wiremock 0.6.5` | wiremock requires async runtime — incompatible with `reqwest::blocking` in `#[test]` |
| `httpmock` | `mockito` | mockito is simpler but runs tests serially; httpmock supports parallel test execution |
| `cross` (via action) | `cargo-zigbuild` | zigbuild is better for glibc version targeting; for pure musl, cross is simpler |
| Trusted Publishing | `CARGO_REGISTRY_TOKEN` secret | Token approach still works but requires manual rotation; Trusted Publishing is preferred |

### Installation

```toml
# In Cargo.toml [dev-dependencies]
httpmock = "0.8"
```

No other new dependencies needed. The release toolchain lives entirely in GitHub Actions YAML.

---

## Architecture Patterns

### Recommended GitHub Actions Structure

```
.github/
├── workflows/
│   ├── ci.yml           # Runs cargo test on every push/PR (all platforms)
│   └── release.yml      # Triggered on tag push: builds binaries, creates GH release, publishes to crates.io
```

### Pattern 1: Two-Workflow Split (CI + Release)

**What:** Separate the "always runs" test job from the "tag-triggered" release job. Keeps CI fast and release logic isolated.

**When to use:** This project — tests run on every push; release artifacts only on `v*` tags.

**CI workflow example:**
```yaml
# .github/workflows/ci.yml
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
```

**Release workflow structure:**
```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ["v[0-9]*"]
jobs:
  create-release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: taiki-e/create-gh-release-action@v1
        with:
          changelog: CHANGELOG.md
          token: ${{ secrets.GITHUB_TOKEN }}

  upload-assets:
    needs: create-release
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-musl
            os: ubuntu-latest
          - target: aarch64-unknown-linux-musl
            os: ubuntu-latest
          - target: universal-apple-darwin
            os: macos-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: taiki-e/upload-rust-binary-action@v1
        with:
          bin: cclink
          target: ${{ matrix.target }}
          archive: cclink-$target           # $target is a built-in variable
          checksum: sha256
          token: ${{ secrets.GITHUB_TOKEN }}

  publish-crates-io:
    needs: upload-assets
    runs-on: ubuntu-latest
    permissions:
      id-token: write   # Required for Trusted Publishing OIDC
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo publish
```

**Note on artifact naming:** The `archive` parameter's `$target` produces the Rust triple (e.g., `cclink-x86_64-unknown-linux-musl`). The user decision calls for human-readable names (`cclink-linux-x86_64`). This requires either a rename step or using a matrix `name` field + the `$bin` variable. See Pitfalls section.

### Pattern 2: Integration Tests with httpmock

**What:** Replace the existing `#[ignore]` integration test (which hits live pubky.app) with an in-process mock server. `httpmock::MockServer::start()` binds to a random local port — no network required.

**When to use:** All HTTP round-trip tests in CI (publish flow, pickup flow, burn-after-read).

**Example:**
```rust
// Source: docs.rs/httpmock/latest
use httpmock::prelude::*;
use serde_json::json;

#[test]
fn test_publish_pickup_round_trip() {
    let server = MockServer::start();  // Synchronous — works in #[test]

    // Mock the signin endpoint
    let _signin = server.mock(|when, then| {
        when.method(POST).path("/session");
        then.status(200);
    });

    // Mock the PUT record endpoint
    let _put_record = server.mock(|when, then| {
        when.method(PUT).path_matches(Regex::new(r"/pub/cclink/\d+").unwrap());
        then.status(201);
    });

    // Build client pointed at mock server
    let client = HomeserverClient::new(&server.address().to_string()).unwrap();
    // ... exercise publish flow ...
}
```

**Key imports:**
```rust
use httpmock::prelude::*;
// Provides: MockServer, GET, POST, PUT, DELETE, PATCH
```

### Pattern 3: Plaintext Leak Test

**What:** After encryption, deserialize the JSON blob, base64-decode the ciphertext, and assert the known session ID string is absent.

**When to use:** Dedicated test in `crypto/mod.rs` or a separate `tests/plaintext_leak.rs` integration test.

**Example:**
```rust
#[test]
fn test_encrypted_blob_contains_no_plaintext_session_id() {
    let keypair = pkarr::Keypair::from_secret_key(&[42u8; 32]);
    let known_session_id = "abc123-known-session-id";

    // Encrypt through the full publish path
    let blob_b64 = encrypt_session_id(known_session_id, &keypair).unwrap();

    // Decode ciphertext
    let ciphertext = base64::engine::general_purpose::STANDARD.decode(&blob_b64).unwrap();

    // Assert plaintext is absent in both raw bytes and as string
    let ciphertext_str = String::from_utf8_lossy(&ciphertext);
    assert!(
        !ciphertext_str.contains(known_session_id),
        "Plaintext session ID found in ciphertext!"
    );
    assert!(
        !ciphertext.windows(known_session_id.len())
            .any(|w| w == known_session_id.as_bytes()),
        "Plaintext session ID bytes found in ciphertext!"
    );
}
```

### Pattern 4: POSIX sh Installer Script

**What:** A single `install.sh` that detects platform, builds the download URL from GitHub Releases, verifies SHA256, and installs to `~/.local/bin` or `/usr/local/bin`.

**When to use:** The primary install method: `curl -fsSL https://raw.githubusercontent.com/.../install.sh | sh`

**Structure:**
```sh
#!/bin/sh
set -eu

REPO="username/cclink"
VERSION="${CCLINK_VERSION:-latest}"  # Allow version override via env

# Detect OS and arch
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  linux)  OS_NAME="linux" ;;
  darwin) OS_NAME="darwin" ;;
  *)      echo "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
  x86_64)         ARCH_NAME="x86_64" ;;
  aarch64|arm64)  ARCH_NAME="aarch64" ;;
  *)              echo "Unsupported arch: $ARCH"; exit 1 ;;
esac

# macOS ships as universal — override arch
if [ "$OS_NAME" = "darwin" ]; then
  ARTIFACT="cclink-darwin-universal"
else
  ARTIFACT="cclink-${OS_NAME}-${ARCH_NAME}"
fi

# ... fetch release URL, download, verify SHA256, install ...

# Install path: prefer ~/.local/bin, fall back to /usr/local/bin
INSTALL_DIR="$HOME/.local/bin"
if [ ! -d "$INSTALL_DIR" ]; then
  mkdir -p "$INSTALL_DIR"
fi
# Check if ~/.local/bin is in PATH
if ! echo "$PATH" | grep -q "$HOME/.local/bin"; then
  echo "Note: Add $HOME/.local/bin to your PATH"
fi
```

**SHA256 verification:**
```sh
# Cross-platform SHA256
if command -v sha256sum > /dev/null 2>&1; then
  echo "$EXPECTED_SHA256  $ARTIFACT" | sha256sum --check --quiet
elif command -v shasum > /dev/null 2>&1; then
  echo "$EXPECTED_SHA256  $ARTIFACT" | shasum -a 256 --check --quiet
else
  echo "Warning: cannot verify checksum (no sha256sum or shasum)"
fi
```

### Anti-Patterns to Avoid

- **Using `CARGO_REGISTRY_TOKEN` secret:** The modern approach is crates.io Trusted Publishing (OIDC). Avoid storing long-lived tokens.
- **Building all targets on a single runner:** Linux musl cross-compilation and macOS must run on matching runner OS (ubuntu for musl, macos for universal-apple-darwin).
- **Wiremock for blocking reqwest tests:** Wiremock is async-only; `MockServer::start().await` in `#[tokio::test]` conflicts with `reqwest::blocking` — use httpmock instead.
- **Running clipboard tests without xvfb:** `arboard::Clipboard::new()` fails in headless CI. The existing code already handles this gracefully via `match`; tests must NOT call clipboard code directly unless DISPLAY is set.
- **Hardcoding glibc in musl targets:** musl targets produce static binaries with no libc dependency — do not add glibc version suffixes to musl target triples.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Multi-platform binary upload to GH Releases | Custom upload scripts | `taiki-e/upload-rust-binary-action@v1` | Handles universal macOS, cross-compilation selection, archive format, SHA256, asset naming |
| In-process mock HTTP server | Custom axum/warp test server | `httpmock 0.8.3` | Built-in sync API, parallel tests, path/method matching, request inspection |
| SHA256 checksum generation | `sha256sum` in shell script | `checksum: sha256` in upload action input | Action generates `.sha256` files and uploads them alongside binaries |
| Release notes from commits | git log parsing | `generate_release_notes: true` in `softprops/action-gh-release` or `taiki-e/create-gh-release-action` | GitHub's built-in release notes pull in commit messages automatically |
| crates.io token management | `CARGO_REGISTRY_TOKEN` rotation | Trusted Publishing (OIDC) | Short-lived tokens, no secret management, announced July 2025 |

**Key insight:** The entire release pipeline — cross-compilation, universal binary merging, checksum generation, GitHub Release creation, asset uploads — is handled by two taiki-e actions. The only custom code needed is the installer shell script and the integration tests.

---

## Common Pitfalls

### Pitfall 1: Artifact Naming Mismatch (CRITICAL for this project)

**What goes wrong:** `taiki-e/upload-rust-binary-action` defaults to `$bin-$target` (e.g., `cclink-x86_64-unknown-linux-musl.tar.gz`). The user decision requires `cclink-linux-x86_64` format.

**Why it happens:** The `archive` parameter uses built-in variables; there is no built-in `$os` or human-readable arch variable.

**How to avoid:** Two options:
1. Add a `name` field to each matrix entry and use `archive: cclink-${{ matrix.name }}`. The matrix `name` value can be `linux-x86_64`, `linux-aarch64`, `darwin-universal`, `windows-x86_64`.
2. Add a rename step after `upload-rust-binary-action` runs (more complex).

**Recommended approach:**
```yaml
matrix:
  include:
    - target: x86_64-unknown-linux-musl
      os: ubuntu-latest
      artifact_name: cclink-linux-x86_64
    - target: aarch64-unknown-linux-musl
      os: ubuntu-latest
      artifact_name: cclink-linux-aarch64
    - target: universal-apple-darwin
      os: macos-latest
      artifact_name: cclink-darwin-universal
    - target: x86_64-pc-windows-msvc
      os: windows-latest
      artifact_name: cclink-windows-x86_64
```
Then in the action: `archive: ${{ matrix.artifact_name }}`.

**Warning signs:** SHA256 checksum file name won't match either if archive name is wrong — the installer script will fail checksum verification.

### Pitfall 2: reqwest blocking + wiremock = tokio runtime panic

**What goes wrong:** `wiremock::MockServer::start().await` requires `#[tokio::test]`. Inside a tokio runtime, calling `reqwest::blocking::Client` panics with "Cannot drop a runtime in a context where blocking is not allowed."

**Why it happens:** reqwest blocking internally creates its own tokio runtime; nesting runtimes panics.

**How to avoid:** Use `httpmock::MockServer::start()` (synchronous, no tokio) in regular `#[test]` functions. The entire `HomeserverClient` is synchronous — keep tests synchronous too.

**Warning signs:** Test output showing "Cannot start a runtime from within a runtime" or tests hanging indefinitely.

### Pitfall 3: arboard Clipboard Failure in CI Tests

**What goes wrong:** `arboard::Clipboard::new()` returns `Err` in headless GitHub Actions runners (no display). If any test directly calls clipboard code, the test panics.

**Why it happens:** The publish command calls `try_copy_to_clipboard()` which uses `match on Clipboard::new()` — graceful fallback is already implemented. But integration tests that exercise the full publish path may hit this.

**How to avoid:**
- Tests that exercise `run_publish()` must not assert on clipboard state.
- If a test needs clipboard, either mock it or skip on CI: `#[cfg_attr(not(target_os = "linux"), test)]` or check `std::env::var("DISPLAY").is_err()` at test start.
- Note: arboard *compiles* fine for musl targets — `x11rb` is a pure-Rust X11 protocol implementation and does NOT require `libxcb.so` at compile time. The failure is purely at runtime when no display is available.

**Warning signs:** Tests passing locally but failing in CI with "No display server available" or similar.

### Pitfall 4: musl Cross-Compilation with OpenSSL

**What goes wrong:** If a dependency pulls in `openssl-sys`, musl builds fail because `libssl.a` is not available in the cross-compilation container.

**Why it happens:** Most C SSL libraries assume dynamic linking or have complex musl build requirements.

**How to avoid:** The project already uses `reqwest` with `default-features = false, features = ["rustls"]`. This is already correct — no OpenSSL dependency. Verify with `cargo tree | grep openssl` before finalizing the release workflow.

**Warning signs:** `cargo build --target x86_64-unknown-linux-musl` failing with "openssl-sys" linker errors.

### Pitfall 5: Cargo.toml Version Not Synced with Tag

**What goes wrong:** Release tag is `v1.2.0` but `Cargo.toml` still says `version = "0.1.0"`. The `cargo publish` step fails because crates.io uses Cargo.toml version, not the git tag.

**Why it happens:** User decision says "tag IS the version, manual sync with Cargo.toml." Manual sync is a process discipline issue.

**How to avoid:** Add a CI check step that verifies `grep "^version" Cargo.toml | grep "${GITHUB_REF_NAME#v}"` before uploading assets. Document the release process: update Cargo.toml version → tag → push.

**Warning signs:** `cargo publish` failing with "version already exists" or a published version mismatching the GitHub Release.

### Pitfall 6: aarch64 musl Runner Availability

**What goes wrong:** GitHub's free runners don't include Linux ARM64 runners; building for `aarch64-unknown-linux-musl` requires either cross-compilation from x86_64 or a paid ARM64 runner.

**Why it happens:** `cross` uses Docker to cross-compile — it runs on ubuntu-latest (x86_64) and produces ARM64 musl binaries. The upload action handles this automatically.

**How to avoid:** Use `ubuntu-latest` for both musl targets; `cross` handles the AArch64 cross-compilation transparently via Docker. Do NOT try to run tests on an ARM64 runner for the release job.

**Warning signs:** Workflow trying to run `cargo test` for `aarch64-unknown-linux-musl` on ubuntu-latest — this would try to execute ARM binaries on x86_64.

---

## Code Examples

Verified patterns from official sources:

### httpmock Basic Setup (Synchronous)

```rust
// Source: docs.rs/httpmock/0.8.3/httpmock/
use httpmock::prelude::*;

#[test]
fn test_homeserver_mock() {
    let server = MockServer::start();  // No await, no tokio::test

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/session");
        then.status(200)
            .header("content-type", "application/json");
    });

    // Use server.base_url() or server.address() to point your client
    let client = HomeserverClient::new(&server.address().to_string()).unwrap();
    client.signin(&some_keypair).unwrap();

    mock.assert();  // Verify the endpoint was called
}
```

### httpmock Request Body Matching

```rust
// Match PUT requests to cclink record paths
let put_mock = server.mock(|when, then| {
    when.method(PUT)
        .path_matches(Regex::new(r"^/pub/cclink/\d+$").unwrap());
    then.status(201);
});
```

### upload-rust-binary-action with Custom Names

```yaml
# Source: github.com/taiki-e/upload-rust-binary-action README
- uses: taiki-e/upload-rust-binary-action@v1
  with:
    bin: cclink
    target: ${{ matrix.target }}
    archive: ${{ matrix.artifact_name }}   # Custom name per matrix entry
    checksum: sha256
    token: ${{ secrets.GITHUB_TOKEN }}
```

### crates.io Trusted Publishing

```yaml
# Source: crates.io/docs/trusted-publishing
publish:
  runs-on: ubuntu-latest
  permissions:
    id-token: write  # Required for OIDC token request
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo publish
      env:
        CARGO_REGISTRY_TOKEN: ""  # Empty — OIDC token used automatically
```

**Note:** Trusted Publishing must be configured in the crates.io UI first. The crate owner links the GitHub repository before the first OIDC-based publish. For a first-ever publish (`cargo publish` before the crate exists on crates.io), the initial publish can use a traditional token, then Trusted Publishing is enabled for subsequent releases.

### Cargo.toml Version Check in CI

```yaml
- name: Verify Cargo.toml version matches tag
  run: |
    TAG_VERSION="${GITHUB_REF_NAME#v}"
    CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/')
    if [ "$TAG_VERSION" != "$CARGO_VERSION" ]; then
      echo "Error: Tag version $TAG_VERSION != Cargo.toml version $CARGO_VERSION"
      exit 1
    fi
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| `CARGO_REGISTRY_TOKEN` secret | Trusted Publishing (OIDC) | July 2025 | No secrets to rotate; short-lived tokens |
| Manual release + asset upload | `taiki-e/upload-rust-binary-action@v1` | 2022-2024 (matured) | Universal macOS, SHA256, cross in one action |
| `cross` separate install step | Built-in via `upload-rust-binary-action` | 2023 | Zero-config cross-compilation in release workflow |
| Separate Intel + ARM macOS binaries | `universal-apple-darwin` target | ~2022 | Single binary works on all modern Macs |
| `actions-rs/cargo` (deprecated) | `dtolnay/rust-toolchain` | 2022 | actions-rs org is unmaintained |

**Deprecated/outdated:**
- `actions-rs/toolchain` and `actions-rs/cargo`: The `actions-rs` GitHub org is unmaintained. Use `dtolnay/rust-toolchain` instead.
- `CARGO_REGISTRY_TOKEN` as a long-lived secret: Still functional but superseded by Trusted Publishing for new setups.

---

## Open Questions

1. **First crates.io publish requires traditional token**
   - What we know: Trusted Publishing requires the crate to already exist on crates.io before OIDC linking can be configured.
   - What's unclear: Whether there's a chicken-and-egg problem for the very first `cargo publish`.
   - Recommendation: Do the initial `cargo publish` manually from the developer machine or with a temporary `CARGO_REGISTRY_TOKEN` secret, then configure Trusted Publishing for all subsequent releases.

2. **httpmock DELETE method matching**
   - What we know: The transport layer has `delete_record()` which the burn-after-read flow calls. httpmock supports all HTTP methods via `when.method(DELETE)`.
   - What's unclear: Whether httpmock's regex path matching handles the exact URL format used by `delete_record()`.
   - Recommendation: Write the DELETE mock test early; the format `DELETE /pub/cclink/{token}` is straightforward. Mark LOW confidence until verified in a test run.

3. **arboard compilation on aarch64-unknown-linux-musl**
   - What we know: x11rb (arboard's Linux dependency) is pure-Rust and does not require libxcb at compile time. The `allow-unsafe-code` feature (which adds libxcb FFI) is not used by arboard's default build.
   - What's unclear: Whether the `cross` Docker image for `aarch64-unknown-linux-musl` has any issues with parking_lot or percent-encoding (also arboard deps) on that target.
   - Recommendation: HIGH confidence arboard compiles fine for musl. Verify with a `cargo build --target aarch64-unknown-linux-musl` check in CI before release.

---

## Sources

### Primary (HIGH confidence)
- `github.com/taiki-e/upload-rust-binary-action` — Verified: universal-apple-darwin support, archive naming syntax, SHA256 checksum config, build-tool option, current version v1
- `docs.rs/httpmock/latest/httpmock/struct.MockServer.html` — Verified: `start()` is synchronous, no tokio required, `start_async()` is the async variant, version 0.8.3
- `raw.githubusercontent.com/1Password/arboard/master/Cargo.toml` — Verified: uses `x11rb` on Linux, no libxcb FFI by default, version 3.6.1
- `github.com/psychon/x11rb` — Verified: pure-Rust X11 protocol implementation; libxcb only used via optional `allow-unsafe-code` feature
- `crates.io/docs/trusted-publishing` (via WebSearch) — Verified: OIDC Trusted Publishing available for GitHub Actions since July 2025

### Secondary (MEDIUM confidence)
- `blog.urth.org` (Cross Compiling Rust Projects in GitHub Actions) — Verified cross/actions-rust-cross patterns against official action docs
- `reemus.dev/tldr/rust-cross-compilation-github-actions` — Matrix workflow structure; consistent with official GitHub docs
- Rust Blog `blog.rust-lang.org/2026/01/21/crates-io-development-update/` — Trusted Publishing details

### Tertiary (LOW confidence — flag for validation)
- arboard + musl + aarch64 compilation: Not empirically tested in this project; inferred from x11rb being pure-Rust. Validate with actual `cargo build --target aarch64-unknown-linux-musl`.
- httpmock DELETE mock path regex behavior: Inferred from the library's general regex support. Validate in first integration test writing.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — Verified taiki-e action capabilities, httpmock sync API, arboard deps, Trusted Publishing availability
- Architecture: HIGH — Two-workflow split is industry standard; patterns verified against official action docs
- Pitfalls: HIGH for known issues (OpenSSL already resolved, artifact naming, tokio conflict); MEDIUM for arboard musl runtime behavior

**Research date:** 2026-02-22
**Valid until:** 2026-05-22 (90 days — stable tooling, slow-moving ecosystem)
