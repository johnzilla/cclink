# Roadmap: CCLink

## Overview

CCLink ships as a single Rust binary that lets you publish an encrypted Claude Code session reference from one machine and resume it on another.

## Milestones

- ✅ **v1.0 MVP** — Phases 1-5 (shipped 2026-02-22)
- 🚧 **v1.1 Security Hardening & Code Review Fixes** — Phases 6-9 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) — SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation and Key Management (2/2 plans) — completed 2026-02-22
- [x] Phase 2: Crypto and Transport (3/3 plans) — completed 2026-02-22
- [x] Phase 3: Core Commands (4/4 plans) — completed 2026-02-22
- [x] Phase 4: Advanced Encryption and Management (3/3 plans) — completed 2026-02-22
- [x] Phase 5: Release and Distribution (2/2 plans) — completed 2026-02-22

Full details: `milestones/v1.0-ROADMAP.md`

</details>

### 🚧 v1.1 Security Hardening & Code Review Fixes (In Progress)

**Milestone Goal:** Address all findings from external code review — fix security gaps, resolve functional discrepancies, and improve code quality. Every v1.1 requirement addressed before v1.1 ships.

- [ ] **Phase 6: Signed Record Format** — Sign burn and recipient fields; enforce key file permissions
- [ ] **Phase 7: Code Quality and Transport** — Structured errors, dead variant removal, lazy signin, list optimization, human_duration dedup
- [ ] **Phase 8: CLI Fixes and Documentation** — Mutual exclusion for --burn/--share, correct pickup help text, PRD path cleanup
- [ ] **Phase 9: PIN-Protected Handoffs** — New --pin flag with Argon2id+HKDF-derived encryption

## Phase Details

### Phase 6: Signed Record Format
**Goal**: Handoff records are cryptographically honest — burn and recipient intent is signed into the payload and key permissions are enforced by cclink itself
**Depends on**: Phase 5 (v1.0 complete)
**Requirements**: SEC-01, SEC-02
**Success Criteria** (what must be TRUE):
  1. A published handoff payload includes burn and recipient fields inside the signed envelope (verifiable in raw record bytes)
  2. Picking up a v1.1 record that was tampered to flip the burn flag fails signature verification and errors out
  3. On any cclink operation that loads the key, the code explicitly checks and enforces 0600 permissions rather than relying on pkarr
  4. Existing v1.0 records (unsigned burn/recipient) expire via TTL without any migration step required
**Plans:** 1/2 plans executed
Plans:
- [ ] 06-01-PLAN.md — Sign burn and recipient into HandoffRecordSignable (TDD)
- [ ] 06-02-PLAN.md — Enforce 0600 key file permissions on load and write (TDD)

### Phase 7: Code Quality and Transport
**Goal**: The codebase is clean — no dead error variants, no stringly-typed 404 detection, no duplicated utilities, and the homeserver client reuses sessions for efficient transport
**Depends on**: Phase 6
**Requirements**: QUAL-01, QUAL-02, QUAL-03, QUAL-04, FUNC-03
**Success Criteria** (what must be TRUE):
  1. `cargo build` produces zero compiler warnings (no unused variant warnings from CclinkError)
  2. Error handling in pickup and list never matches on the string "404" or "not found" — uses typed CclinkError variants instead
  3. `cclink list` with N records makes one batch HTTP request, not N individual fetches
  4. HomeserverClient signs in once per process and reuses the session cookie for subsequent operations
  5. `human_duration` exists in exactly one place in the codebase (utility module shared by all commands)
**Plans**: TBD

### Phase 8: CLI Fixes and Documentation
**Goal**: The CLI surface is correct and honest — flag combinations that cannot work are rejected at parse time, help text shows valid commands, and the PRD reflects actual filesystem paths
**Depends on**: Phase 7
**Requirements**: FUNC-01, FUNC-02, DOCS-01
**Success Criteria** (what must be TRUE):
  1. Running `cclink --burn --share <pubkey>` immediately errors with a clear message before any network call
  2. The success message after self-publish shows `cclink pickup` (not a raw token) as the retrieval command
  3. The PRD contains no references to `~/.cclink/` paths — all key storage references say `~/.pubky/`
**Plans**: TBD

### Phase 9: PIN-Protected Handoffs
**Goal**: Users can protect a handoff with a PIN so only the recipient who knows the PIN can decrypt the session ID
**Depends on**: Phase 8
**Requirements**: SEC-03
**Success Criteria** (what must be TRUE):
  1. Running `cclink --pin` prompts for a PIN and publishes a record encrypted with an Argon2id+HKDF-derived key
  2. Running `cclink pickup` on a PIN-protected record prompts for the PIN before decryption succeeds
  3. Providing the wrong PIN during pickup produces a clear decryption failure error (not a panic or silent wrong result)
  4. A PIN-protected record cannot be decrypted by the owner's keypair alone — the PIN is required
**Plans**: TBD

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation and Key Management | v1.0 | 2/2 | Complete | 2026-02-22 |
| 2. Crypto and Transport | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Core Commands | v1.0 | 4/4 | Complete | 2026-02-22 |
| 4. Advanced Encryption and Management | v1.0 | 3/3 | Complete | 2026-02-22 |
| 5. Release and Distribution | v1.0 | 2/2 | Complete | 2026-02-22 |
| 6. Signed Record Format | 1/2 | In Progress|  | - |
| 7. Code Quality and Transport | v1.1 | 0/? | Not started | - |
| 8. CLI Fixes and Documentation | v1.1 | 0/? | Not started | - |
| 9. PIN-Protected Handoffs | v1.1 | 0/? | Not started | - |
