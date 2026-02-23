---
phase: 10-pubky-homeserver-transport-fix
plan: "01"
subsystem: transport
tags: [transport, homeserver, host-header, virtual-hosting, signup, pubky]
dependency_graph:
  requires: []
  provides: [HomeserverClient-Host-header, signup-fallback, cross-user-host-routing]
  affects: [publish, pickup, list, revoke]
tech_stack:
  added: []
  patterns:
    - "Pubky homeserver virtual hosting via Host header (not URL path)"
    - "signin() -> 404 -> signup fallback with 409 conflict retry"
key_files:
  created: []
  modified:
    - src/transport/mod.rs
    - src/commands/publish.rs
    - src/commands/pickup.rs
    - src/commands/list.rs
    - src/commands/revoke.rs
decisions:
  - "Host header on every HTTP request — no pubky-host fallback needed"
  - "get_record_by_pubkey() uses URL /pub/cclink/{token} with Host: {target_pubkey_z32} (not /{pubkey}/pub/cclink/{token})"
  - "get_latest() uses /pub/cclink/latest with Host header for both self and cross-user"
  - "Signup 409 conflict triggers retry of /session with fresh token"
  - "Command callers updated in 10-01 (not deferred to 10-02) due to compile-time signature change"
metrics:
  duration_seconds: 240
  completed_date: "2026-02-22"
  tasks_completed: 3
  files_modified: 5
---

# Phase 10 Plan 01: Pubky Homeserver Transport Fix Summary

**One-liner:** HomeserverClient with pubkey_z32 field and Host header on all requests, signup fallback for first-time users, and Host-header-based cross-user record retrieval replacing broken path-based routing.

## What Was Built

Fixed the `HomeserverClient` transport layer to correctly interact with the Pubky homeserver's virtual hosting API. The homeserver uses the `Host` header to identify tenants — without it, every request returns 404. Three core problems were fixed:

1. **Missing Host header** — Every HTTP request now includes `Host: {pubkey_z32}` so the homeserver can identify the tenant namespace.

2. **No signup flow** — First-time users previously got 404 on `POST /session` and the error propagated. Now `signin()` automatically falls back to `POST /signup`, with a 409 conflict retry path for race conditions.

3. **Wrong cross-user URL routing** — `get_record_by_pubkey()` previously used `/{pubkey_z32}/pub/cclink/{token}` (path-based routing). The correct API uses `/pub/cclink/{token}` with the target pubkey in the `Host` header.

## Tasks Completed

### Task 1: Add pubkey_z32 field and Host header to HomeserverClient
- Added `pubkey_z32: String` field to `HomeserverClient` struct
- Updated `new()` to require `pubkey_z32: &str` parameter
- Added `host_header(pubkey_z32: Option<&str>) -> String` private helper
- Added `Host` header to: `signin()`, `put_record()`, `put_latest()`, `delete_record()`, `list_record_tokens()`, `get_bytes()`
- Updated `get_bytes()` signature to accept `host_pubkey: Option<&str>`
- Fixed `get_record_by_pubkey()` — URL is now `/pub/cclink/{token}`, Host header carries `pubkey_z32`
- Fixed `get_latest()` — URL is now always `/pub/cclink/latest`, Host header carries pubkey
- Commit: `1b23ea9`

### Task 2: Add signup fallback when signin returns 404
- Modified `signin()` to check for 404 status after `POST /session`
- On 404: automatically `POST /signup` with same auth token and Host header
- On signup 409 (Conflict): retry `POST /session` once with fresh token
- Added `test_signin_url_construction` unit test verifying URL format
- Updated `signin()` doc comment documenting session-then-signup fallback
- Commit: `1b23ea9` (included in same commit as Task 1)

### Task 3: Update integration tests and callers
- Updated `test_integration_signin_put_get` to pass `keypair.public_key().to_z32()`
- Added `test_integration_signup_new_keypair` (ignored) for first-time signup flow
- Added `test_integration_cross_user_get` (ignored) for Host-header cross-user retrieval
- Added `test_homeserver_client_stores_pubkey_z32` unit test
- Added `test_host_header_self_operation` unit test
- Added `test_host_header_cross_user_operation` unit test
- Commit: `92e2fc8` (callers in command modules)

## Verification Results

- `cargo build`: zero errors, zero warnings
- `cargo test --lib transport::tests`: 14 passed, 3 ignored (integration tests), 0 failed
- All `.send()` calls preceded by `.header("Host", ...)`
- `signin()` contains `NOT_FOUND` check and `/signup` fallback path
- `get_record_by_pubkey()` no longer embeds pubkey in URL path
- `get_latest()` with `Some(pubkey)` uses Host header, not path routing
- `HomeserverClient::new()` requires `pubkey_z32` parameter

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated command module callers in 10-01**
- **Found during:** Task 3 planning review
- **Issue:** Changing `HomeserverClient::new()` signature to require `pubkey_z32` immediately breaks `publish.rs`, `pickup.rs`, `list.rs`, and `revoke.rs` — the build cannot succeed without updating them
- **Fix:** Updated all four command module callers to pass `keypair.public_key().to_z32()` as the second argument. Each file already had `keypair` loaded, making the change a one-liner.
- **Files modified:** `src/commands/publish.rs`, `src/commands/pickup.rs`, `src/commands/list.rs`, `src/commands/revoke.rs`
- **Commit:** `92e2fc8`
- **Note:** The plan indicated these would be updated in Plan 02, but the compile-time break meant they had to be updated now. Plan 02 can still refine transport usage in command modules.

## Self-Check: PASSED

- src/transport/mod.rs: FOUND
- src/commands/publish.rs: FOUND
- src/commands/pickup.rs: FOUND
- src/commands/list.rs: FOUND
- src/commands/revoke.rs: FOUND
- 10-01-SUMMARY.md: FOUND
- commit 1b23ea9: FOUND
- commit 92e2fc8: FOUND
