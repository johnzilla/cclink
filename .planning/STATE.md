# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-22)

**Core value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.
**Current focus:** Phase 9 complete — v1.1 complete

## Current Position

Phase: 9 of 9 (PIN-Protected Handoffs)
Plan: 2 of 2 complete
Status: Phase 9 complete — all plans done
Last activity: 2026-02-22 — 09-02 complete (--pin CLI flag, publish/pickup PIN flow, integration tests)

Progress: [██████████] ~100% (v1.1 complete)

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
| 9. PIN-Protected Handoffs | 2/2 | Complete |

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

v1.1 phase 9 decisions (09-01):
- PIN key derivation: Argon2id (t=3, m=64MB, p=1) + HKDF-SHA256 with info=cclink-pin-v1
- PIN-derived X25519 scalar fed directly into age_identity() — HKDF expansion ensures correct domain
- pin_salt: Option<String> field added alphabetically between project and pubkey in both HandoffRecord and HandoffRecordSignable
- Field order updated: blob, burn, created_at, hostname, pin_salt, project, pubkey, recipient, ttl
- #[allow(dead_code)] on pin_derive_key/pin_encrypt/pin_decrypt — temporary until wired in 09-02

v1.1 phase 9 decisions (09-02):
- --pin conflicts_with share (not burn): --pin + --burn is valid (burn-after-read PIN-protected record)
- PIN pickup path runs BEFORE is_cross_user check: PIN-derived key is independent of keypair identity
- Single-entry PIN prompt on pickup (no confirmation): pickup is read-only; confirmation prompt redundant
- Non-interactive guard on pickup: bail with clear message when pin_salt present but stdin is not a terminal
- #[allow(dead_code)] annotations removed from pin_derive_key/pin_encrypt/pin_decrypt — wired to binary

### Roadmap Evolution

- Phase 10 added: Pubky Homeserver Transport Fix (FUNC-04) — discovered during Phase 9 UAT that transport layer uses wrong API convention (missing Host header, no signup flow, wrong cross-user URL format, wrong list parsing)

### Pending Todos

None.

### Blockers/Concerns

- HomeserverClient URL routing for multi-tenant PUT needs empirical verification against live pubky.app → CONFIRMED BROKEN: Phase 10 created to fix
- QR code content wrong when --share + --qr combined (minor tech debt, not in v1.1 scope)
- Cargo.toml/install.sh placeholder `user/cclink` repo path — must fix before next release

## Session Continuity

Last session: 2026-02-22
Stopped at: Completed 09-02-PLAN.md (--pin CLI flag, publish/pickup PIN flow, integration tests) — v1.1 complete
Resume file: None
