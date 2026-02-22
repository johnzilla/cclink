---
phase: 02-crypto-and-transport
plan: 03
subsystem: transport
tags: [transport, auth-token, homeserver, reqwest, postcard, ed25519, cookie-session, pubky, rust]

# Dependency graph
requires:
  - phase: 02-crypto-and-transport
    plan: 01
    provides: pkarr::Keypair.sign() and pkarr::PublicKey.verify() for Ed25519 signing; postcard for binary serialization
  - phase: 02-crypto-and-transport
    plan: 02
    provides: HandoffRecord, LatestPointer, verify_record for transport serialization and verification

provides:
  - build_auth_token: postcard binary AuthToken compatible with pubky-common 0.5.4 (bytes[65..] signable region)
  - HomeserverClient: reqwest::blocking client with cookie_store(true) for session persistence
  - signin: POST binary AuthToken to /session, stores session cookie
  - put_record: PUT HandoffRecord JSON bytes to /pub/cclink/{token}
  - put_latest: PUT LatestPointer JSON bytes to /pub/cclink/latest
  - get_record: GET + Ed25519 signature verification (hard fail) from /pub/cclink/{token}
  - get_record_by_pubkey: GET + verify from /{pubkey_z32}/pub/cclink/{token}
  - get_latest: GET latest pointer (own or other user)
  - publish: sign in + PUT record + PUT latest in one call, returns timestamp-based token

affects:
  - 03+ phases (publish/pickup commands use HomeserverClient.publish() and get_record())

# Tech tracking
tech-stack:
  added: []  # All dependencies were added in 02-01 (reqwest, postcard already in Cargo.toml)
  patterns:
    - Manual postcard byte construction for AuthToken (serde 1.0.228 lacks Serialize for [u8; 64])
    - Signable region at bytes[65..] confirmed from pubky-common-0.5.4 source (varint prefix makes offset 65 not 64)
    - reqwest::blocking::Client with cookie_store(true) for automatic session cookie forwarding
    - Varint encoding: values < 128 encode as single byte; values >= 128 need continuation bits

key-files:
  created:
    - src/transport/mod.rs
  modified:
    - src/main.rs

key-decisions:
  - "serde 1.0.228 does not implement Serialize for [u8; 64] — AuthToken bytes built manually instead of via postcard::to_allocvec on a derived struct"
  - "AuthToken signable region confirmed as bytes[65..] from pubky-common 0.5.4 source: signature is serialized as varint(64)+[64 bytes] = 65 bytes total"
  - "Capabilities string is '/:rw' (root read+write) serialized as postcard String: varint(4) + UTF-8 bytes"
  - "Timestamp is microseconds since UNIX_EPOCH in big-endian [u8; 8] format matching pubky-timestamp Serialize impl"
  - "publish() calls signin() on every invocation (stateless — no persistent session across calls)"

patterns-established:
  - "Pattern: build_auth_token manually encodes varint + sig + namespace + version + timestamp + pubkey + capabilities"
  - "Pattern: HomeserverClient.deserialize_and_verify() is the shared deserialization+verification pipeline for all GET methods"
  - "Pattern: token = record.created_at.to_string() for timestamp-based sortable record paths"

requirements-completed: [PUB-05, UX-02]

# Metrics
duration: 4min
completed: 2026-02-22
---

# Phase 2 Plan 03: Transport Module Summary

**AuthToken manual postcard construction with correct bytes[65..] signable region, HomeserverClient with cookie session persistence, and hard-fail Ed25519 verification on all GET operations**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-22T05:17:09Z
- **Completed:** 2026-02-22T05:21:19Z
- **Tasks:** 2 (implemented in one pass)
- **Files modified:** 2

## Accomplishments

- build_auth_token() produces correct postcard binary matching pubky-common 0.5.4 layout: varint(64)+signature+namespace+version+timestamp+pubkey+capabilities, with signable region at bytes[65..]
- AuthToken byte layout verified empirically: reading pubky-common source confirmed signable=bytes[65..], bytes[0]=0x40 (varint for 64-byte signature slice), namespace at bytes[65..75]
- HomeserverClient wraps reqwest::blocking::Client with cookie_store(true) for automatic session cookie forwarding on PUT operations
- publish() generates timestamp-based tokens (record.created_at as string), signs in, PUTs record and latest pointer in one call
- get_record() and get_record_by_pubkey() hard-fail on Ed25519 verification mismatch before returning records to callers
- 9 unit tests pass: AuthToken length/structure/byte layout, signature verification over signable region, multi-keypair token differences, client construction/URL handling, token derivation, full deserialize+verify pipeline, wrong-key rejection
- 1 #[ignore] integration test provided for live homeserver round-trip testing

## Task Commits

Each task was committed atomically:

1. **Tasks 1+2: AuthToken builder + HomeserverClient (combined)** - `970e13f` (feat)

Note: Tasks 1 and 2 were both implemented in a single file creation pass since they share `src/transport/mod.rs`. Both tasks are fully complete in this commit.

## Files Created/Modified

- `src/transport/mod.rs` - build_auth_token, HomeserverClient struct with signin/put_record/put_latest/get_record/get_record_by_pubkey/get_latest/publish; 9 unit tests + 1 ignored integration test
- `src/main.rs` - Added `mod transport;`

## Decisions Made

- serde 1.0.228 does not implement Serialize for [u8; 64]. The AuthToken intermediate struct approach (using #[derive(Serialize)]) failed to compile. Fixed by building the postcard bytes manually: varint(64) prefix + 64 sig bytes + raw namespace/version/timestamp/pubkey + varint-prefixed capabilities string.
- Signable region is bytes[65..] not bytes[64..]. This was confirmed by reading pubky-common-0.5.4/src/auth.rs directly: the Signature type serializes as a byte slice in postcard (varint(64) = 0x40 prefix + 64 bytes = 65 bytes total), so signable starts at offset 65.
- Capabilities string "/:rw" matches pubky-common's `Capability::root().to_string()` (scope="/" + actions=[r,w] = "/:rw"). Serialized by postcard as varint(4) + UTF-8 bytes.
- Timestamp is microseconds since UNIX_EPOCH in big-endian [u8; 8] — confirmed from pubky-timestamp 0.4.1 Serialize impl which calls `self.to_bytes()` returning `self.0.to_be_bytes()`.
- publish() calls signin() on each invocation. No session state is persisted between calls. This is intentional: simple and correct for Phase 3 command invocations.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] serde 1.0.228 does not implement Serialize/Deserialize for [u8; 64]**
- **Found during:** Task 1 (AuthToken implementation)
- **Issue:** Attempted to use `#[derive(Serialize, Deserialize)]` on a struct with `signature: [u8; 64]` field. serde supports fixed arrays up to [u8; 32] by default; [u8; 64] requires the `serde_arrays` crate or manual implementation.
- **Fix:** Abandoned the struct approach entirely. Implemented `build_auth_token()` by manually constructing the postcard bytes: read pubky-common source to confirm exact byte layout, then write varint + sig + fixed-array fields directly. This is actually simpler and avoids the serde dependency.
- **Files modified:** src/transport/mod.rs (removed the intermediate struct, kept manual byte construction)
- **Verification:** All 9 transport tests pass; AuthToken structure test confirms bytes[0]=0x40, bytes[65..75]=b"PUBKY:AUTH", bytes[75]=0
- **Committed in:** 970e13f

---

**Total deviations:** 1 auto-fixed (1 blocking: serde array size limitation)
**Impact on plan:** The manual byte construction approach is actually more transparent and maintainable than the struct-derive approach. No scope creep; same functionality delivered.

## Issues Encountered

- serde 1.0.228 does not support Serialize for arrays larger than [u8; 32] — [u8; 64] for the Ed25519 signature fails to compile. Identified from rustc error message, fixed by switching to manual byte construction which also made the signable region calculation more explicit.

## User Setup Required

None — no external service configuration required for unit tests. The `#[ignore]` integration test requires a live Pubky homeserver.

## Next Phase Readiness

- `src/transport/mod.rs` exports: `build_auth_token`, `HomeserverClient` (with signin/put_record/put_latest/get_record/get_record_by_pubkey/get_latest/publish) — all ready for Phase 3 publish/pickup commands
- All three Phase 2 modules (crypto, record, transport) compile and all 21 tests pass
- No blockers for Phase 3

---
*Phase: 02-crypto-and-transport*
*Completed: 2026-02-22*

## Self-Check: PASSED

- src/transport/mod.rs: FOUND
- src/main.rs (contains `mod transport;`): FOUND
- 02-03-SUMMARY.md: FOUND
- Commit 970e13f (Tasks 1+2): FOUND
- cargo test: 21 passed, 0 failed, 1 ignored
