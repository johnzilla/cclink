# Roadmap: CCLink

## Overview

CCLink ships as a single Rust binary that lets you publish an encrypted Claude Code session reference from one machine and resume it on another.

## Milestones

- ✅ **v1.0 MVP** -- Phases 1-5 (shipped 2026-02-22)
- ✅ **v1.1 Security Hardening & Code Review Fixes** -- Phases 6-10 (shipped 2026-02-22)
- 🚧 **v1.2 Dependency Audit & Code Quality** -- Phases 11-13 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) -- SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation and Key Management (2/2 plans) -- completed 2026-02-22
- [x] Phase 2: Crypto and Transport (3/3 plans) -- completed 2026-02-22
- [x] Phase 3: Core Commands (4/4 plans) -- completed 2026-02-22
- [x] Phase 4: Advanced Encryption and Management (3/3 plans) -- completed 2026-02-22
- [x] Phase 5: Release and Distribution (2/2 plans) -- completed 2026-02-22

Full details: `milestones/v1.0-ROADMAP.md`

</details>

<details>
<summary>✅ v1.1 Security Hardening (Phases 6-10) -- SHIPPED 2026-02-22</summary>

- [x] Phase 6: Signed Record Format (2/2 plans) -- completed 2026-02-22
- [x] Phase 7: Code Quality and Transport (2/2 plans) -- completed 2026-02-22
- [x] Phase 8: CLI Fixes and Documentation (1/1 plan) -- completed 2026-02-22
- [x] Phase 9: PIN-Protected Handoffs (2/2 plans) -- completed 2026-02-22
- [x] Phase 10: Pubky Homeserver Transport Fix (2/2 plans) -- completed 2026-02-22

Full details: `milestones/v1.1-ROADMAP.md`

</details>

### v1.2 Dependency Audit & Code Quality

**Milestone Goal:** Address code review findings -- audit crypto dependencies, harden CI, enforce PIN strength, and clean up tech debt.

- [ ] **Phase 11: Prerequisites** - Fix all issues that would cause CI failures before adding enforcement gates
- [ ] **Phase 12: CI Hardening** - Add clippy, audit, and fmt enforcement jobs to the CI pipeline
- [ ] **Phase 13: Code Quality and Security** - Enforce PIN length, remove dead code, fix placeholder paths

## Phase Details

### Phase 11: Prerequisites
**Goal**: The codebase passes `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and `cargo audit` locally before any CI gates exist
**Depends on**: Nothing (first phase of v1.2)
**Requirements**: CI-01, DEP-01, DEP-02
**Success Criteria** (what must be TRUE):
  1. `cargo clippy --all-targets -- -D warnings` exits 0 with no warnings or errors
  2. `cargo fmt --check` exits 0 on all source files
  3. `cargo audit` produces no unresolved vulnerability or unmaintained advisories (backoff replaced or explicitly ignored in audit.toml with documented rationale)
  4. ed25519-dalek pre-release pin is documented in Cargo.toml with a comment explaining the pkarr 5.0.3 constraint
**Plans**: 2 plans
Plans:
- [ ] 11-01-PLAN.md — Fix clippy warnings, apply rustfmt, document ed25519-dalek pin
- [ ] 11-02-PLAN.md — Replace backoff with backon to resolve RUSTSEC advisories

### Phase 12: CI Hardening
**Goal**: Every pull request and push to main runs clippy, rustfmt, and cargo-audit as separate parallel CI jobs, with failures attributed to the correct gate
**Depends on**: Phase 11
**Requirements**: CI-02, CI-03, CI-04
**Success Criteria** (what must be TRUE):
  1. CI runs a `lint` job with `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` on every push
  2. CI runs an `audit` job with `actions-rust-lang/audit@v1` on every push
  3. A commit that introduces a clippy warning causes the `lint` job to fail while the `test` job remains green
  4. Lint and audit jobs run in parallel with the existing test job (not appended to it)
**Plans**: TBD

### Phase 13: Code Quality and Security
**Goal**: PIN enforcement prevents weak PINs at publish time, dead DHT migration code is gone, and real repository metadata is in place for users who run the curl installer
**Depends on**: Phase 12
**Requirements**: PIN-01, DEBT-01, DEBT-02
**Success Criteria** (what must be TRUE):
  1. `cclink --pin 1234567` (7 chars) at publish time prints a clear error and exits non-zero without publishing
  2. `cclink --pin 12345678` (8 chars) proceeds normally
  3. `cargo test` passes with `LatestPointer` struct and its test removed from `src/record/mod.rs`
  4. `Cargo.toml` repository and homepage fields and `install.sh` REPO variable contain the real `johnzilla/cclink` path
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation and Key Management | v1.0 | 2/2 | Complete | 2026-02-22 |
| 2. Crypto and Transport | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Core Commands | v1.0 | 4/4 | Complete | 2026-02-22 |
| 4. Advanced Encryption and Management | v1.0 | 3/3 | Complete | 2026-02-22 |
| 5. Release and Distribution | v1.0 | 2/2 | Complete | 2026-02-22 |
| 6. Signed Record Format | v1.1 | 2/2 | Complete | 2026-02-22 |
| 7. Code Quality and Transport | v1.1 | 2/2 | Complete | 2026-02-22 |
| 8. CLI Fixes and Documentation | v1.1 | 1/1 | Complete | 2026-02-22 |
| 9. PIN-Protected Handoffs | v1.1 | 2/2 | Complete | 2026-02-22 |
| 10. Pubky Homeserver Transport Fix | v1.1 | 2/2 | Complete | 2026-02-22 |
| 11. Prerequisites | v1.2 | 0/2 | Not started | - |
| 12. CI Hardening | v1.2 | 0/TBD | Not started | - |
| 13. Code Quality and Security | v1.2 | 0/TBD | Not started | - |
