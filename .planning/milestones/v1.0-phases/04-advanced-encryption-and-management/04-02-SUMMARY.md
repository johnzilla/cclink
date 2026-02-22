---
phase: 04-advanced-encryption-and-management
plan: 02
subsystem: commands
tags: [rust, age, pkarr, owo-colors, burn-after-read, share]

# Dependency graph
requires:
  - phase: 04-advanced-encryption-and-management
    plan: 01
    provides: recipient_from_z32, delete_record, burn/recipient fields on HandoffRecord, --share/--burn CLI flags

provides:
  - publish.rs with --share (encrypt to recipient's X25519 key) and --burn (set flag + yellow warning)
  - pickup.rs with four pickup scenarios: self/self-share-error/cross-shared-decrypt/cross-unshared-metadata
  - burn-after-read: DELETE called after successful self-pickup, before exec
  - wrong-recipient metadata display with clear explanation message

affects:
  - 04-03 (list/revoke complete Phase 4; pickup/publish now fully implemented)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "recipient dispatch: if share.is_some() -> recipient_from_z32 else ed25519_to_x25519_public"
    - "four pickup scenarios: converge on single session_id variable, common burn+confirm+exec tail"
    - "token from created_at: record.created_at.to_string() matches transport publish() convention"
    - "burn-after-read guard: record.burn && !is_cross_user — recipients cannot DELETE publisher records"
    - "wrong-key metadata: age_decrypt Err branch shows host/project/age + explanation"

key-files:
  created: []
  modified:
    - src/commands/publish.rs (--share recipient dispatch, --burn field + warning, pickup instructions)
    - src/commands/pickup.rs (four scenario handling, burn-after-read delete, token from created_at)

key-decisions:
  - "burn-after-read only on self-pickup: recipient cannot auth to delete publisher's record; cross-user --burn records expire via TTL"
  - "token derived from record.created_at.to_string(): consistent with transport::publish() which uses created_at as token"
  - "cross-user decryption attempt-first: try own key, show metadata on failure; no pre-check needed"

# Metrics
duration: 2min
completed: 2026-02-22
---

# Phase 4 Plan 2: --share and --burn Publish/Pickup Summary

**--share encrypts to recipient's X25519 key via recipient_from_z32; --burn flags record for deletion after self-pickup; four pickup scenarios handled with correct decryption and metadata fallback**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-22T17:38:20Z
- **Completed:** 2026-02-22
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Extended `publish.rs` section 4 to dispatch on `cli.share`: `recipient_from_z32` for --share, `ed25519_to_x25519_public` for self-encrypt
- Extended `publish.rs` section 5 to set `burn: cli.burn` and `recipient: cli.share.clone()` on `HandoffRecord`
- Added yellow burn warning in section 7 (before "Published!") and context-sensitive pickup instructions (recipient's `cclink pickup <own_pubkey>` vs self's `cclink pickup <token>`)
- Reworked `pickup.rs` section 4 into a unified `session_id: String` variable populated by two branches with a common burn+confirm+QR+exec tail
- Cross-user branch: attempts age decrypt with own key; on success continues; on failure shows cleartext metadata with appropriate explanation
- Self-pickup branch: checks `record.recipient.is_some()` first (cannot decrypt, show error); otherwise self-decrypts
- Added burn-after-read DELETE in new section 5: `if record.burn && !is_cross_user` with non-fatal warning on failure
- Token derived as `record.created_at.to_string()` — consistent with transport publish convention

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend publish command with --share and --burn support** - `a582592` (feat)
2. **Task 2: Extend pickup command with shared-record handling and burn-after-read** - `d7a579a` (feat)

## Files Created/Modified

- `src/commands/publish.rs` — --share recipient dispatch, --burn field/warning, pickup instruction branching
- `src/commands/pickup.rs` — four pickup scenarios, burn-after-read DELETE, token from created_at, unified session_id

## Decisions Made

- **burn-after-read only on self-pickup:** The recipient performing a cross-user --share pickup cannot authenticate as the publisher to call DELETE. Cross-user burn records expire naturally via TTL. This is documented in the code with a comment.
- **token = record.created_at.to_string():** The retry closure returns `record` but not `latest.token`. Rather than restructuring the closure to return a tuple, we re-derive the token from `created_at` which matches the transport layer convention (publish stores the token as the created_at timestamp string).
- **attempt-first decryption for cross-user:** No pre-check on `record.recipient` for cross-user path — we simply try decryption and handle failure. This is simpler and handles both cases (record shared with us, or not) without needing to compare pubkeys.

## Deviations from Plan

None - plan executed exactly as written. The four-scenario structure described in the plan was implemented as specified.

## Issues Encountered

None — both cargo check and cargo test (33 tests, 1 ignored) passed cleanly on first attempt.

## User Setup Required

None.

## Self-Check: PASSED

All files verified present. All commits verified in git log.

## Next Phase Readiness

Phase 4 Plan 2 complete. Plan 04-03 (list and revoke commands) can proceed:
- `delete_record` and `list_record_tokens` from 04-01 are available
- All 34 test cases (33 passing + 1 integration ignored) verified
- ENC-01 (--share) and ENC-02 (--burn) requirements fulfilled
