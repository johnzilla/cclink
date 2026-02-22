# Phase 5: Release and Distribution - Context

**Gathered:** 2026-02-22
**Status:** Ready for planning

<domain>
## Phase Boundary

CCLink ships as a distributable binary with automated CI/CD. Produces platform binaries, a curl installer, crates.io publishing, and round-trip encryption tests in CI.

</domain>

<decisions>
## Implementation Decisions

### Build targets
- Full platform coverage: Linux (x86_64-musl + aarch64-musl), macOS (universal binary combining Intel + Apple Silicon), Windows (x86_64)
- Linux binaries statically linked via musl — no runtime dependencies
- macOS ships as a single universal (fat) binary
- Artifact naming: `cclink-{os}-{arch}` format (e.g., cclink-linux-x86_64, cclink-darwin-universal, cclink-windows-x86_64.exe)

### CI pipeline behavior
- Release triggered by tag push only (e.g., v1.0.0) — deliberate releases
- Test suite (cargo test) runs on every push and pull request — catches issues early
- Version: tag IS the version, manual sync with Cargo.toml
- CI creates a GitHub Release with auto-generated changelog from commits since last tag
- SHA256 checksums generated alongside binaries

### Installation experience
- Primary: curl | sh installer that auto-detects platform and downloads the correct binary
- Installer path: try ~/.local/bin first (no sudo), fall back to /usr/local/bin with sudo prompt
- Installer verifies downloads with SHA256 checksums before installing
- Secondary: publish to crates.io for `cargo install cclink`

### Round-trip test strategy
- All encryption code paths tested: self-encrypt, shared (--share), burn (--burn), and shared+burn
- Dedicated plaintext leak test: encrypts a known value, inspects ciphertext blob, asserts plaintext is absent
- Integration tests use an in-process mock HTTP server (wiremock or similar) — full publish/pickup flow without real homeserver
- No external dependencies in CI (no Docker, no real servers)

### Claude's Discretion
- Specific CI matrix configuration and runner selection
- Mock server implementation crate choice (wiremock, axum, etc.)
- Installer script language (bash, POSIX sh, etc.)
- Cross-compilation toolchain setup details

</decisions>

<specifics>
## Specific Ideas

- Artifact naming should be human-readable (cclink-linux-x86_64), not Rust target triples
- The curl installer is the primary user-facing install method — it should feel polished
- Auto-detect install path avoids forcing sudo on users who have ~/.local/bin in PATH

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 05-release-and-distribution*
*Context gathered: 2026-02-22*
