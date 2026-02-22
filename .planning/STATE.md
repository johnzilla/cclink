# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-22)

**Core value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.
**Current focus:** Phase 8 — CLI Fixes and Documentation

## Current Position

Phase: 8 of 9 (CLI Fixes and Documentation)
Plan: 1 of 1 complete
Status: Phase 8 complete
Last activity: 2026-02-22 — 08-01 complete (--burn/--share conflict, self-publish message, PRD path fixes)

Progress: [██████░░░░] ~55% (v1.0 complete, phases 6-8 complete)

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

**By Phase (v1.1):**

| Phase | Plans | Status |
|-------|-------|--------|
| 6. Signed Record Format | 2 | Complete |
| 7. Code Quality and Transport | 2/2 | Complete |
| 8. CLI Fixes and Documentation | 1/1 | Complete |

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

v1.1 phase 7 decisions:
- CclinkError::RecordNotFound carries no payload — URL context added by anyhow context chain at call site
- Dead CclinkError variants removed immediately (no deprecation period) — private binary crate with no external API
- Shared utilities live in src/util.rs, exported as pub mod util for integration test access
- ensure_signed_in() uses Cell<bool> interior mutability so &self methods can set session state without &mut self
- get_all_records() is architectural encapsulation — N individual HTTP fetches are transport implementation detail, not visible to callers
- list.rs retains explicit client.signin() call for clarity; signed_in flag prevents actual double-signin

v1.1 phase 8 decisions:
- --burn/--share mutual exclusion implemented via clap conflicts_with, not runtime validation — parse-time rejection is more correct
- Self-publish message shows "cclink pickup" with no token; QR section retains token for concrete identifier
- PRD updated only for ~/.cclink -> ~/.pubky/secret_key; other stale references left intentionally (historical planning doc)

### Pending Todos

None.

### Blockers/Concerns

- HomeserverClient URL routing for multi-tenant PUT needs empirical verification against live pubky.app
- QR code content wrong when --share + --qr combined (minor tech debt, not in v1.1 scope)
- Cargo.toml/install.sh placeholder `user/cclink` repo path — must fix before next release

## Session Continuity

Last session: 2026-02-22
Stopped at: Completed 08-01-PLAN.md (--burn/--share conflict, self-publish message, PRD path fixes)
Resume file: None
