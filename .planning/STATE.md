# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-21)

**Core value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.
**Current focus:** Phase 4 (Advanced Encryption and Management) — Complete; Phase 5 (Polish and Release) is next

## Current Position

Phase: 4 of 5 (Advanced Encryption and Management) — COMPLETE (all 3 plans done)
Plan: 3 of 3 in current phase — ALL COMPLETE
Status: All Phase 4 plans complete: 04-01 (primitives), 04-02 (share/burn publish/pickup), 04-03 (list/revoke)
Last activity: 2026-02-22 — Plan 04-02 complete (--share + --burn publish/pickup, 4 pickup scenarios, burn-after-read)

Progress: [██████████] 95%

## Performance Metrics

**Velocity:**
- Total plans completed: 7
- Average duration: 2.3 min
- Total execution time: 16 min

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-foundation-and-key-management | 2 | 5 min | 2.5 min |
| 02-crypto-and-transport | 3 | 9 min | 3 min |
| 03-core-commands | 3 | 6 min | 2 min |

**Recent Trend:**
- Last 5 plans: 2.2 min
- Trend: stable

*Updated after each plan completion*
| Phase 03-core-commands P04 | 2 | 2 tasks | 3 files |
| Phase 04-advanced-encryption-and-management P01 | 4 | 3 tasks | 9 files |
| Phase 04-advanced-encryption-and-management P03 | 3 | 2 tasks | 2 files |
| Phase 04-advanced-encryption-and-management P02 | 2 | 2 tasks | 2 files |

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
- [Phase 03-core-commands]: Session file UUID stem IS the session_id — no decoding of encoded directory names needed (03-01)
- [Phase 03-core-commands]: cwd must be read from JSONL progress record; directory names use lossy encoding (03-01)
- [Phase 03-core-commands]: 24-hour mtime cutoff defines active sessions — consistent with TTL default 86400s (03-01)
- [Phase 03-core-commands]: SessionInfo derives Debug — required for test assertions with {:?} format (03-01)
- [Phase 03-core-commands]: owo_colors chained methods (.green().bold()) return references to temporaries — use single color method per if_supports_color call (03-02)
- [Phase 03-core-commands]: Publish path uses only ed25519_to_x25519_public (recipient); ed25519_to_x25519_secret only needed for decrypt in pickup (03-02)
- [Phase 03-core-commands]: Self-pickup signs in via session cookie; cross-user uses public multi-tenant path without sign-in (03-03)
- [Phase 03-core-commands]: Retry wraps full get_latest+get_record sequence; 404/not-found are permanent errors; network failures are transient (03-03)
- [Phase 03-core-commands]: launch_claude_resume() uses Unix exec() to replace cclink process; non-Unix falls back to status() wait (03-03)
- [Phase 03-core-commands]: [Phase 03-core-commands]: discover_sessions() filter uses starts_with on canonicalized paths — handles symlinks and relative paths correctly
- [Phase 03-core-commands]: [Phase 03-core-commands]: cwd scoping — pass Option<&Path> to discover_sessions, filter inside discovery function, not in callers
- [Phase 04-advanced-encryption-and-management]: burn and recipient are unsigned metadata — excluded from HandoffRecordSignable to preserve Phase 3 signature compatibility
- [Phase 04-advanced-encryption-and-management]: list_record_tokens uses parse::<u64>().is_ok() filter to exclude latest LatestPointer key from results
- [Phase 04-advanced-encryption-and-management]: delete_record treats 404 as success — idempotent deletion for burn-after-read and revoke flows
- [Phase 04-advanced-encryption-and-management]: recipient_from_z32 reuses existing age_recipient() + pkarr PublicKey::try_from path — no new crypto deps needed
- [Phase 04-advanced-encryption-and-management]: human_duration is module-private in each command file (list.rs, pickup.rs) — not shared, per plan spec
- [Phase 04-advanced-encryption-and-management]: Corrupt record in single-token revoke path uses delete-anyway prompt rather than hard-fail
- [Phase 04-advanced-encryption-and-management]: burn-after-read only on self-pickup: recipient cannot auth to delete publisher record; cross-user burn records expire via TTL
- [Phase 04-advanced-encryption-and-management]: token derived from record.created_at.to_string() in pickup — consistent with transport publish() convention, avoids restructuring retry closure return

### Pending Todos

None yet.

### Blockers/Concerns

- Phase 4 (Advanced Encryption): Argon2id parameters for PIN mode and pkarr DHT recipient resolution API are MEDIUM confidence — research may be warranted before planning (ENC-03 deferred to v2 but --share uses similar DHT lookup)
- Phase 3 (publish/pickup): HomeserverClient URL routing for multi-tenant PUT needs empirical verification against live pubky.app (researched but not integration-tested)

## Session Continuity

Last session: 2026-02-22
Stopped at: Completed 04-02-PLAN.md (--share + --burn publish/pickup: 4 pickup scenarios, burn-after-read DELETE, 33 tests pass) and 04-03-PLAN.md (cclink list with comfy-table, cclink revoke with single/batch confirmation prompts, 34 tests pass)
Resume file: None
