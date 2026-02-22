# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-22)

**Core value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.
**Current focus:** Phase 7 — Code Quality and Transport

## Current Position

Phase: 7 of 9 (Code Quality and Transport)
Plan: 2 of 2 complete
Status: Phase 7 complete
Last activity: 2026-02-22 — 07-02 complete (lazy signin, get_all_records, list optimization)

Progress: [█████░░░░░] ~44% (v1.0 complete, phases 6-7 complete)

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

### Pending Todos

None.

### Blockers/Concerns

- HomeserverClient URL routing for multi-tenant PUT needs empirical verification against live pubky.app
- QR code content wrong when --share + --qr combined (minor tech debt, not in v1.1 scope)
- Cargo.toml/install.sh placeholder `user/cclink` repo path — must fix before next release

## Session Continuity

Last session: 2026-02-22
Stopped at: Completed 07-02-PLAN.md (lazy signin Cell<bool>, get_all_records, list command optimization)
Resume file: None
