# Requirements: CCLink

**Defined:** 2026-02-23
**Core Value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.

## v1.2 Requirements

Requirements for v1.2 Dependency Audit & Code Quality. Each maps to roadmap phases.

### Dependency Audit

- [ ] **DEP-01**: Document ed25519-dalek pre-release constraint in Cargo.toml comment and PROJECT.md (pkarr 5.0.3 forces `=3.0.0-pre.5`, no stable 3.x exists)
- [ ] **DEP-02**: Replace unmaintained `backoff` crate to resolve RUSTSEC-2025-0012 and transitive `instant` advisory RUSTSEC-2024-0384

### CI Hardening

- [ ] **CI-01**: Fix existing clippy warnings in test files (doc comment style `///` → `//` in test files)
- [ ] **CI-02**: Add `cargo clippy --all-targets -- -D warnings` job to CI pipeline
- [ ] **CI-03**: Add `cargo audit` job to CI via `actions-rust-lang/audit@v1`
- [ ] **CI-04**: Add `cargo fmt --check` job to CI pipeline

### PIN Enforcement

- [ ] **PIN-01**: Enforce minimum 8-character PIN length at publish time with clear error message

### Tech Debt

- [ ] **DEBT-01**: Fix placeholder `user/cclink` → `johnzilla/cclink` in Cargo.toml and install.sh
- [ ] **DEBT-02**: Remove dead `LatestPointer` struct and its serialization test

## Future Requirements

### Key Management

- **KEY-01**: Encrypted key storage at rest using passphrase (Argon2-derived)
- **KEY-02**: System keystore integration (macOS Keychain, Freedesktop Secret Service)

### QR Improvements

- **QR-01**: Fix QR code content when --share + --qr combined

## Out of Scope

| Feature | Reason |
|---------|--------|
| ed25519-dalek upgrade to stable | pkarr 5.0.3 forces 3.x pre-release; no stable 3.x exists |
| Encrypted key storage at rest | New feature, not a security fix — deferred to future milestone |
| System keystore integration | Large effort, cross-platform complexity — deferred |
| QR+share fix | Needs product decision on intended UX — deferred |
| PIN complexity rules | NIST 800-63B-4 explicitly recommends against mandatory complexity rules |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DEP-01 | Phase 11 | Pending |
| DEP-02 | Phase 11 | Pending |
| CI-01 | Phase 11 | Pending |
| CI-02 | Phase 12 | Pending |
| CI-03 | Phase 12 | Pending |
| CI-04 | Phase 12 | Pending |
| PIN-01 | Phase 13 | Pending |
| DEBT-01 | Phase 13 | Pending |
| DEBT-02 | Phase 13 | Pending |

**Coverage:**
- v1.2 requirements: 9 total
- Mapped to phases: 9
- Unmapped: 0

---
*Requirements defined: 2026-02-23*
*Last updated: 2026-02-23 after roadmap creation (phases 11-13)*
