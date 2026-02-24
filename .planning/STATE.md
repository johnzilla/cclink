# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-24)

**Core value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.
**Current focus:** Phase 16 — Encrypted Key Storage + CLI Integration (complete)

## Current Position

Phase: 16 of 16 in v1.3 (Encrypted Key Storage CLI Integration) — Complete
Plan: 2 of 2 in current phase (16-02 complete)
Status: Phase 16 complete — all v1.3 plans done (encrypted key storage CLI integration shipped)
Last activity: 2026-02-24 — 16-02 complete: cclink init now prompts for passphrase by default (CCLINKEK encrypted write) with --no-passphrase for plaintext fallback

Progress: [█████████░░░░░░░░░░░░░░░░░░░░░░░░░] 71% (v1.3 — 5 of 7 plans complete)

## Performance Metrics

**Velocity:**
- v1.0: 14 plans (Phases 1-5) | 2 days
- v1.1: 9 plans (Phases 6-10) | 1 day
- v1.2: 5 plans (Phases 11-13) | 2 days
- v1.3 so far: 4 plans (Phases 14-16) | <1 day
- Total: 32 plans across 16 phases

## Accumulated Context

### Decisions

All decisions documented in PROJECT.md Key Decisions table.

Key decisions relevant to v1.3:
- Zeroization before encrypted storage: Phase 14 is self-contained and validates Zeroizing<T> patterns before they appear in the encrypted load path
- Crypto layer before storage layer: Phase 15 must produce tested encrypt/decrypt functions before Phase 16 integrates them
- Bypass pkarr I/O for encrypted format: pkarr's write_secret_key_file writes hex only; own the read/write lifecycle via keypair.secret_key() -> encrypt -> write

Key decisions from 14-01:
- Use Zeroizing<[u8;32]> as return type (not a newtype) so callers auto-deref with no changes in pickup/revoke/list
- Wrap argon2_output and okm internally in pin_derive_key so intermediate secrets are also zeroed on drop
- Manual byte-by-byte hex decode in load_keypair avoids any intermediate Vec<u8> holding secret bytes on heap
- from_secret_key_file calls in init.rs (import path) deferred — outside ZERO-01/ZERO-02 scope
- [Phase 14-memory-zeroization]: Zeroizing<[u8;32]> as return type for secret derivation — auto-deref enables no-change callers

Key decisions from 14-02:
- Wrap at the interact() call site with Zeroizing::new() so no bare String copy escapes — Zeroizing<String> drops the heap buffer on scope exit
- No downstream changes needed — Zeroizing<String> Deref<Target=String> then String Deref<Target=str> means &pin passes where &str expected

Key decisions from 15-01:
- CCLINKEK binary envelope stores Argon2 params in header (not constants) for forward-compatible decryption on future param upgrades
- HKDF info b"cclink-key-v1" distinct from b"cclink-pin-v1" — domain separation is a named constant (KEY_HKDF_INFO)
- decrypt_key_envelope returns Zeroizing<[u8;32]> not Vec<u8> — Phase 16 passes directly to pkarr::Keypair::from_secret_key with auto-deref
- age decrypt error mapped to "Wrong passphrase or corrupted key envelope" — no raw age internals leak to user

Key decisions from 16-01:
- Testable core pattern: load_encrypted_keypair_with_passphrase returns Err for wrong passphrase; interactive wrapper load_encrypted_keypair converts Err to eprintln+exit(1) — enables test assertions and production UX
- write_encrypted_keypair_atomic sets 0600 before rename (minimize insecure window) and after rename (defense-in-depth)
- load_keypair uses std::fs::read (Vec<u8>) not read_to_string — CCLINKEK envelopes are binary and not valid UTF-8
- Binary format detection before dispatch: read raw bytes, check magic, branch to format-specific loader

Key decisions from 16-02:
- eprintln!+exit(1) for passphrase-too-short error (not anyhow::bail!) — consistent with Phase 13 PIN validation pattern, avoids double "Error:" prefix
- Passphrase length validated AFTER double-entry confirmation — user sees mistake once, not re-prompted
- --import path flows through same Step 5 branching as generated keys — --no-passphrase controls output encryption regardless of source

### Pending Todos

None.

### Blockers/Concerns

None.

## Session Continuity

Last session: 2026-02-24
Stopped at: Completed 16-02-PLAN.md — Phase 16 Plan 02 (CLI wiring for run_init passphrase prompt) complete; v1.3 encrypted key storage feature shipped end-to-end
Resume file: None
