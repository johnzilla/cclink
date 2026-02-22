# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-21)

**Core value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.
**Current focus:** Phase 2 (Crypto and Transport) — COMPLETE (all 3 plans done)

## Current Position

Phase: 2 of 5 (Crypto and Transport) — COMPLETE
Plan: 3 of 3 in current phase — COMPLETE
Status: Ready for Phase 3
Last activity: 2026-02-22 — Plan 02-03 complete (transport module: AuthToken + HomeserverClient, all 21 tests pass)

Progress: [██████░░░░] 56%

## Performance Metrics

**Velocity:**
- Total plans completed: 4
- Average duration: 2.25 min
- Total execution time: 10 min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-foundation-and-key-management | 2 | 5 min | 2.5 min |
| 02-crypto-and-transport | 3 | 9 min | 3 min |

**Recent Trend:**
- Last 5 plans: 2.6 min
- Trend: stable

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Rust single binary with pubky 0.6.0 + pkarr 5.0.3 (must match — released together 2026-01-15)
- age encryption via ssh-to-age 0.2.0 for Ed25519-to-X25519 conversion (eliminates manual curve arithmetic)
- Atomic key write (write-to-temp-then-rename) required from Phase 1 — cannot be retrofitted
- pkarr 5.0.3 requires features = ["keys"] when default-features = false to access Keypair/PublicKey (01-01)
- Stdin import uses temp file + from_secret_key_file to avoid ed25519_dalek::SecretKey type ambiguity (01-01)
- Homeserver stored as plain text at ~/.pubky/cclink_homeserver — read by whoami and later phases (01-01)
- arboard 3.6 for clipboard; graceful fallback via match on Clipboard::new() — never unwrap in clipboard ops (01-02)
- try_copy_to_clipboard returns bool — clean separation of clipboard attempt from display logic (01-02)
- ed25519-dalek must be listed explicitly in Cargo.toml even though it is a pkarr transitive dep — Rust requires direct Cargo.toml declaration for direct crate imports (02-01)
- reqwest 0.13 feature name is 'rustls' not 'rustls-tls' (renamed in the 0.13 release) (02-01)
- curve25519-dalek 4 (age) and 5-pre.6 (pkarr) coexist safely — convert at [u8; 32] boundary only; never pass types between them (02-01)
- serde_json serializes struct fields in declaration order — alphabetical field ordering ensures canonical JSON without preserve_order feature (02-02)
- HandoffRecordSignable is a separate struct (not a field-masked view) — avoids circular signing dependency (02-02)
- Hard fail on signature verification failure — no bypass flag, no graceful degradation (02-02)
- base64::Engine trait must be in scope explicitly (use base64::Engine) for GeneralPurpose encode/decode methods (02-02)
- [Phase 02-crypto-and-transport]: serde 1.0.228 does not implement Serialize for [u8; 64] — AuthToken bytes built manually instead of via postcard::to_allocvec on a derived struct
- [Phase 02-crypto-and-transport]: AuthToken signable region confirmed as bytes[65..] from pubky-common 0.5.4 source: Signature serializes as varint(64)+[64 bytes]=65 bytes total
- [Phase 02-crypto-and-transport]: publish() calls signin() on every invocation — stateless, no persistent session across calls

### Pending Todos

None yet.

### Blockers/Concerns

- Phase 4 (Advanced Encryption): Argon2id parameters for PIN mode and pkarr DHT recipient resolution API are MEDIUM confidence — research may be warranted before planning (ENC-03 deferred to v2 but --share uses similar DHT lookup)
- Phase 3 (publish/pickup): HomeserverClient URL routing for multi-tenant PUT needs empirical verification against live pubky.app (researched but not integration-tested)

## Session Continuity

Last session: 2026-02-22
Stopped at: Completed 02-03-PLAN.md (transport module: AuthToken binary encoding + HomeserverClient, all 21 tests pass — Phase 2 COMPLETE)
Resume file: None
