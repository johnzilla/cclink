---
phase: 02-crypto-and-transport
plan: 02
subsystem: record
tags: [handoff-record, canonical-json, ed25519, signing, verification, serde_json, base64, rust]

# Dependency graph
requires:
  - phase: 02-crypto-and-transport
    plan: 01
    provides: pkarr::Keypair.sign() and pkarr::PublicKey.verify() for Ed25519 signing/verification; base64 crate for signature encoding

provides:
  - HandoffRecord struct with all metadata fields (blob, created_at, hostname, project, pubkey, signature, ttl)
  - HandoffRecordSignable struct for canonical signing (excludes signature field)
  - LatestPointer struct for latest.json with token, project, hostname, created_at
  - canonical_json() for deterministic compact JSON serialization with alphabetically sorted keys
  - sign_record() returning base64-encoded Ed25519 signature over canonical JSON
  - verify_record() verifying Ed25519 signature using pkarr::PublicKey — hard fail on mismatch

affects:
  - 02-03-transport (uses HandoffRecord, sign_record, verify_record for PUT/GET operations)
  - 03+ phases (HandoffRecord is the core data model for all transport and pickup)

# Tech tracking
tech-stack:
  added: []  # All dependencies were added in 02-01
  patterns:
    - Alphabetical struct field ordering for deterministic serde_json serialization without preserve_order feature
    - Signable/record split pattern: HandoffRecordSignable excludes signature to prevent circular signing dependency
    - base64::Engine trait must be imported explicitly in scope for encode/decode methods on GeneralPurpose engine

key-files:
  created:
    - src/record/mod.rs
  modified:
    - src/error.rs
    - src/main.rs

key-decisions:
  - "serde_json serializes struct fields in declaration order — alphabetical field ordering in struct definitions ensures canonical JSON without preserve_order feature"
  - "HandoffRecordSignable is a separate struct (not a derived view) — avoids circular dependency when computing signature"
  - "Hard fail on signature verification failure — no bypass flag, no graceful degradation (per user requirement)"
  - "base64::Engine trait must be in scope explicitly (use base64::Engine) for GeneralPurpose encode/decode methods"

patterns-established:
  - "Pattern: HandoffRecordSignable From<&HandoffRecord> conversion copies all fields except signature"
  - "Pattern: verify_record extracts signable, computes canonical JSON, decodes base64 sig, reconstructs ed25519_dalek::Signature from [u8; 64]"
  - "Pattern: pkarr::Keypair.sign() returns ed25519_dalek::Signature — call .to_bytes() then base64 encode"

requirements-completed: [PUB-02, UX-02]

# Metrics
duration: 2min
completed: 2026-02-22
---

# Phase 2 Plan 02: HandoffRecord Summary

**HandoffRecord with deterministic canonical JSON (alphabetical fields, compact serde_json) and Ed25519 sign/verify round-trip using pkarr keys and base64-encoded signatures**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-22T05:13:04Z
- **Completed:** 2026-02-22T05:14:39Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments

- HandoffRecord and HandoffRecordSignable structs with alphabetically-ordered fields ensure deterministic canonical JSON via serde_json default serialization order
- sign_record() uses pkarr::Keypair.sign() over canonical JSON bytes and base64-encodes the 64-byte Ed25519 signature
- verify_record() decodes base64 signature, reconstructs ed25519_dalek::Signature, verifies with pkarr::PublicKey — returns CclinkError::SignatureVerificationFailed on mismatch
- LatestPointer struct for latest.json indirection (token + summary metadata)
- All 7 TDD tests pass: alphabetical key ordering, compactness, determinism, sign/verify round-trip, wrong-key rejection, tamper detection, LatestPointer serialization

## Task Commits

Each task was committed atomically:

1. **Task 1: HandoffRecord + canonical JSON + Ed25519 signing (TDD)** - `2e2f41a` (feat)

## Files Created/Modified

- `src/record/mod.rs` - HandoffRecord, HandoffRecordSignable, LatestPointer structs; canonical_json, sign_record, verify_record functions; 7 unit tests
- `src/error.rs` - Added SignatureVerificationFailed and RecordDeserializationFailed error variants
- `src/main.rs` - Added `mod record;`

## Decisions Made

- serde_json serializes struct fields in declaration order. Alphabetical field ordering in the struct definition ensures canonical JSON without enabling the `preserve_order` feature. This is a zero-cost approach that keeps serialization deterministic.
- HandoffRecordSignable is a separate struct (not a field-masked view) — this avoids the circular dependency where the signature is included in the data being signed.
- Hard fail on verification failure per user requirement — no bypass flag, no soft error mode.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] base64::Engine trait must be explicitly imported for encode/decode**
- **Found during:** Task 1 (HandoffRecord implementation)
- **Issue:** `base64::engine::general_purpose::STANDARD.encode()` and `.decode()` require `use base64::Engine` in scope — Rust trait methods are not auto-imported when the trait is not in scope
- **Fix:** Added `use base64::Engine;` at the top of `src/record/mod.rs`
- **Files modified:** src/record/mod.rs
- **Verification:** All 7 tests pass, cargo check clean
- **Committed in:** 2e2f41a (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking: missing trait import)
**Impact on plan:** Single-line fix required for compilation. No scope creep.

## Issues Encountered

- base64::Engine trait not in scope — `STANDARD.encode()` / `.decode()` would not resolve without the explicit `use base64::Engine` import. Diagnosed immediately from rustc error message with suggested fix.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `src/record/mod.rs` exports: `HandoffRecord`, `HandoffRecordSignable`, `LatestPointer`, `canonical_json`, `sign_record`, `verify_record` — all ready for 02-03 (transport)
- CclinkError extended with SignatureVerificationFailed and RecordDeserializationFailed for use in transport layer
- No blockers

---
*Phase: 02-crypto-and-transport*
*Completed: 2026-02-22*

## Self-Check: PASSED

- src/record/mod.rs: FOUND
- src/error.rs: FOUND
- 02-02-SUMMARY.md: FOUND
- Commit 2e2f41a (Task 1): FOUND
- cargo test record: 7 passed, 0 failed
