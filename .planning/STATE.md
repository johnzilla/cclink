# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-22)

**Core value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.
**Current focus:** Phase 6 — Signed Record Format (v1.1)

## Current Position

Phase: 6 of 9 (Signed Record Format)
Plan: 2 of 2 complete
Status: Phase complete
Last activity: 2026-02-22 — 06-02 complete (0600 permission enforcement on secret key file)

Progress: [████░░░░░░] ~33% (v1.0 complete, phase 6 complete)

## Performance Metrics

**Velocity (v1.0):**
- Total plans completed: 14
- Total execution time: 2 days

**By Phase (v1.0):**

| Phase | Plans | Status |
|-------|-------|--------|
| 1. Foundation | 2 | Complete |
| 2. Crypto and Transport | 3 | Complete |
| 3. Core Commands | 4 | Complete |
| 4. Adv. Encryption | 3 | Complete |
| 5. Release | 2 | Complete |

*v1.1 metrics start fresh from Phase 6*

## Accumulated Context

### Decisions

Key decisions carried forward from v1.0:
- Rust single binary, pubky 0.6.0 + pkarr 5.0.3
- age encryption via Ed25519-to-X25519 derivation
- Pubky homeserver transport (no custom relay)
- Key storage at ~/.pubky/secret_key

v1.1 decisions:
- Clean break on signed metadata — no v1/v2 version negotiation (v1.0 records expire via TTL)
- --burn + --share mutually exclusive (CLI error, not silent skip)
- SEC-03 PIN mode is real feature (earlier deferral reversed)
- HandoffRecordSignable v1.1: burn and recipient now in signed envelope (SEC-01 complete)
- Field order: blob, burn, created_at, hostname, project, pubkey, recipient, ttl (alphabetical, enforced by struct declaration order)
- check_key_permissions integrated into load_keypair — enforces 0600 at read time (SEC-02)
- write_keypair_atomic explicitly sets 0600 after rename — cclink owns permission guarantee, not pkarr

### Pending Todos

None.

### Blockers/Concerns

- HomeserverClient URL routing for multi-tenant PUT needs empirical verification against live pubky.app
- QR code content wrong when --share + --qr combined (minor tech debt, not in v1.1 scope)
- Cargo.toml/install.sh placeholder `user/cclink` repo path — must fix before next release

## Session Continuity

Last session: 2026-02-22
Stopped at: Completed 06-02-PLAN.md (0600 permission enforcement — phase 6 complete)
Resume file: None
