---
phase: 10-pubky-homeserver-transport-fix
verified: 2026-02-22T00:00:00Z
status: passed
score: 9/9 must-haves verified
re_verification: false
human_verification:
  - test: "Live pubky.app round-trip: cclink init, publish, pickup on two machines"
    expected: "Pickup succeeds, session resumes in Claude Code"
    why_human: "Requires live pubky.app homeserver — integration tests are marked #[ignore]"
  - test: "First-time user signup flow against live pubky.app"
    expected: "New keypair triggers POST /signup, session established, publish succeeds"
    why_human: "Requires a brand-new keypair and live homeserver; cannot automate without network"
  - test: "Cross-user pickup with --share: user_b picks up user_a's shared record"
    expected: "Host header routes to user_a's namespace; user_b decrypts with own key"
    why_human: "Requires two separate machine configurations and live homeserver"
---

# Phase 10: Pubky Homeserver Transport Fix Verification Report

**Phase Goal:** The transport layer works correctly against the real Pubky homeserver API — Host header identifies tenants, signup flow handles first-time users, and all CRUD operations succeed against live pubky.app
**Verified:** 2026-02-22
**Status:** PASSED (automated), HUMAN NEEDED (live verification)
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Every HTTP request to the homeserver includes a Host header with a z32-encoded pubkey | VERIFIED | 8 `.header("Host", ...)` calls at transport/mod.rs:190, 203, 216, 274, 303, 415, 439, 512. Every `.send()` at lines 192, 205, 218, 277, 306, 416, 440, 513 is preceded by a Host header |
| 2 | First-time users are automatically signed up when POST /session returns 404 | VERIFIED | `signin()` checks `NOT_FOUND` at line 197, falls back to POST `/signup` at lines 199-237, with 409 conflict retry at lines 210-229. Unit test `test_signin_url_construction` covers URL format |
| 3 | Cross-user GET uses Host header for tenant identification instead of embedding pubkey in URL path | VERIFIED | `get_record_by_pubkey()` at line 351: URL is `/pub/cclink/{token}` (no pubkey in path); passes `Some(pubkey_z32)` to `get_bytes()` which sets Host header. `get_latest()` at line 365: URL is always `/pub/cclink/latest` |
| 4 | Self-operations (publish, list, revoke, self-pickup) use the client's own pubkey in the Host header | VERIFIED | `publish.rs:143`, `list.rs:17`, `revoke.rs:20`, `pickup.rs:51` all call `HomeserverClient::new(&homeserver, &keypair.public_key().to_z32())` |
| 5 | Cross-user operations (shared pickup) use the target user's pubkey in the Host header | VERIFIED | `pickup.rs:66`: `client.get_latest(pk_z32_opt.as_deref())` passes target pubkey to Host header override. `pickup.rs:90`: `client.get_record_by_pubkey(pk_z32, ...)` passes target pubkey explicitly to `get_bytes()` |
| 6 | All command modules pass the keypair's z32 pubkey when constructing HomeserverClient | VERIFIED | All 4 callers verified: `publish.rs:143`, `list.rs:17`, `revoke.rs:20`, `pickup.rs:51` — each passes `&keypair.public_key().to_z32()` as second argument |
| 7 | List parsing correctly handles full pubky:// URI format from homeserver directory listings | VERIFIED | `list_record_tokens()` at lines 455-462 splits on `/pub/cclink/`, filters non-numeric. 7 unit tests in `parse_record_tokens` cover: full pubky:// URIs, plain paths, mixed formats, empty body, latest-only, trailing slashes, multiple tokens |
| 8 | Full end-to-end flow compiles and unit tests pass | VERIFIED | `cargo build`: 0 errors, 0 warnings. `cargo test`: 51+53+8+3 = 115 tests pass, 3 ignored (integration), 0 failed. `cargo clippy -- -D warnings`: exits 0 with no output |
| 9 | HomeserverClient::new() requires a pubkey_z32 parameter | VERIFIED | `HomeserverClient::new(homeserver: &str, pubkey_z32: &str)` at line 135. `pubkey_z32` stored in struct at line 151, used in all self-operation Host headers |

**Score:** 9/9 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/transport/mod.rs` | HomeserverClient with pubkey_z32 field and Host header on all requests | VERIFIED | Struct has `pubkey_z32: String` field (line 120), `new()` takes `pubkey_z32: &str` (line 135), `host_header()` helper (line 161), Host on all 8 HTTP calls |
| `src/commands/publish.rs` | Updated HomeserverClient construction with pubkey_z32 | VERIFIED | Line 143: `HomeserverClient::new(&homeserver, &keypair.public_key().to_z32())` |
| `src/commands/pickup.rs` | Updated HomeserverClient construction and cross-user Host header routing | VERIFIED | Line 51: own z32 in constructor. Line 66: `get_latest(pk_z32_opt.as_deref())` passes target pubkey. Line 90: `get_record_by_pubkey(pk_z32, ...)` |
| `src/commands/list.rs` | Updated HomeserverClient construction with pubkey_z32 | VERIFIED | Line 17: `HomeserverClient::new(&homeserver, &keypair.public_key().to_z32())` |
| `src/commands/revoke.rs` | Updated HomeserverClient construction with pubkey_z32 | VERIFIED | Line 20: `HomeserverClient::new(&homeserver, &keypair.public_key().to_z32())` |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `HomeserverClient::new` | `pubkey_z32 field` | constructor parameter stored in struct | WIRED | `pubkey_z32: pubkey_z32.to_string()` at line 151 |
| All HTTP request methods | Host header | `.header("Host", ...)` | WIRED | All 8 `.send()` calls preceded by `.header("Host", ...)` — confirmed at lines 190, 203, 216, 274, 303, 415, 439, 512 |
| `signin` | signup fallback | 404 status check then POST /signup | WIRED | `NOT_FOUND` check at line 197, `signup_url` POST at lines 199-206 |
| `src/commands/publish.rs` | `HomeserverClient::new` | `keypair.public_key().to_z32()` | WIRED | Line 143 — two-argument constructor call verified |
| `src/commands/pickup.rs` | `HomeserverClient::new` | `keypair.public_key().to_z32()` | WIRED | Line 51 — own z32, plus pk_z32_opt override for cross-user |
| `list_record_tokens` | pubky:// URI parsing | split on `/pub/cclink/` | WIRED | Lines 455-462, 7 unit tests passing |

---

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| FUNC-04 | 10-01-PLAN.md, 10-02-PLAN.md | Transport layer uses correct Pubky homeserver API (Host header for tenant identification, signup/session flow, pubky:// URI parsing) | SATISFIED | All three sub-requirements implemented and verified: (1) Host header on all requests, (2) signup/session flow with fallback, (3) pubky:// URI parsing. 21 unit tests pass. REQUIREMENTS.md status table shows "Not started" — stale; the checkbox `[x] FUNC-04` at line 21 reflects the actual completed state |

**Note:** The REQUIREMENTS.md tracking table (`| FUNC-04 | Phase 10 | Not started |`) was not updated when the phase completed. The implementation is verified as complete in the codebase. The table entry is a documentation gap, not an implementation gap.

---

### Anti-Patterns Found

No blockers or stubs detected.

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `.planning/REQUIREMENTS.md:68` | Status table shows FUNC-04 as "Not started" | Info | Documentation only — stale tracking table. Checkbox at line 21 shows `[x]` correctly |

---

### Human Verification Required

#### 1. Live pubky.app round-trip (self-pickup)

**Test:** On machine A: `cclink init && cclink publish`. On machine B: `cclink pickup`
**Expected:** Session ID is retrieved, decrypted, and `claude --resume <id>` launches
**Why human:** Requires live pubky.app — the 3 integration tests (`test_integration_signin_put_get`, `test_integration_signup_new_keypair`, `test_integration_cross_user_get`) are correctly marked `#[ignore]` and cannot run in CI without network access

#### 2. First-time user signup flow

**Test:** Generate a fresh keypair with `cclink init` on a machine that has never connected, then `cclink publish`
**Expected:** signin() triggers POST /signup transparently; publish succeeds; no 404 error surfaced to user
**Why human:** Requires a brand-new keypair that has never been registered at pubky.app

#### 3. Cross-user pickup (--share flow)

**Test:** Machine A: `cclink publish --share <pubkey_of_B>`. Machine B: `cclink pickup <pubkey_of_A>`
**Expected:** Machine B retrieves record using pubkey_A in Host header; decrypts with own key; session resumes
**Why human:** Requires two separate configured machines and live homeserver

---

### Commit Verification

All commits claimed in SUMMARY files verified to exist in git history:

| Commit | Message | Status |
|--------|---------|--------|
| `1b23ea9` | feat(10-01): add pubkey_z32 field and Host header to HomeserverClient | FOUND |
| `92e2fc8` | fix(10-01): update command callers to pass pubkey_z32 to HomeserverClient::new | FOUND |
| `4fe4884` | feat(10-02): harden list_record_tokens with comment and unit tests | FOUND |
| `504e78f` | fix(10-02): resolve all clippy warnings for clean -D warnings build | FOUND |

---

### Summary

Phase 10 goal is **achieved in the codebase**. All automated verifications pass:

- `HomeserverClient` correctly uses the `Host` header on every HTTP request to identify the Pubky homeserver tenant. No path-based pubkey embedding remains.
- `signin()` implements the session-then-signup fallback flow: POST `/session` first, 404 triggers POST `/signup`, 409 conflict retries `/session`. The complete 3-path flow is implemented with doc comments.
- All four command modules (`publish`, `pickup`, `list`, `revoke`) pass `keypair.public_key().to_z32()` to `HomeserverClient::new()`.
- Cross-user pickup correctly overrides the Host header with the target user's pubkey via `get_latest(Some(pk))` and `get_record_by_pubkey(pk_z32, ...)`.
- `list_record_tokens()` parses `pubky://` URI format via split on `/pub/cclink/`, with 7 unit tests covering all edge cases.
- Full test suite: 115 tests pass, 0 fail, 3 ignored (live integration tests correctly gated behind `#[ignore]`).
- Clippy clean: `cargo clippy -- -D warnings` exits 0.

Live verification against pubky.app requires human testing (3 items above).

The REQUIREMENTS.md status table entry for FUNC-04 remains "Not started" — this is a stale documentation artifact; the implementation is complete.

---

_Verified: 2026-02-22_
_Verifier: Claude (gsd-verifier)_
