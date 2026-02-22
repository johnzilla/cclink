---
phase: 04-advanced-encryption-and-management
plan: 01
subsystem: crypto
tags: [rust, age, pkarr, clap, comfy-table, serde, reqwest]

# Dependency graph
requires:
  - phase: 02-crypto-and-transport
    provides: HandoffRecord, crypto module (age_recipient, ed25519_to_x25519_public), HomeserverClient
  - phase: 03-core-commands
    provides: CLI structure (Cli, Commands enum), publish/pickup command patterns

provides:
  - HandoffRecord with burn (bool, serde default) and recipient (Option<String>, serde default) fields
  - recipient_from_z32() — converts z32 pubkey string to age X25519 Recipient for --share encryption
  - HomeserverClient::delete_record() — HTTP DELETE with 404-as-success idempotency
  - HomeserverClient::list_record_tokens() — directory listing parser, filters numeric tokens only
  - CLI --share <PUBKEY> and --burn flags on top-level Cli struct
  - CLI List and Revoke(RevokeArgs) subcommands with stub implementations
  - comfy-table 7.2.2 dependency for terminal table rendering

affects:
  - 04-02 (--share and --burn publish/pickup extensions use recipient_from_z32, burn field)
  - 04-03 (list and revoke commands use delete_record, list_record_tokens, comfy-table)

# Tech tracking
tech-stack:
  added:
    - comfy-table 7.2.2 (terminal table rendering for cclink list)
  patterns:
    - "burn/recipient as unsigned metadata: excluded from HandoffRecordSignable for Phase 3 backwards compatibility"
    - "serde defaults for new record fields: #[serde(default)] ensures old records deserialize gracefully"
    - "delete idempotency: 404 treated as success in delete_record()"
    - "numeric token filter: list_record_tokens uses parse::<u64>().is_ok() to exclude 'latest' key"
    - "stub commands: todo!() implementations compile but panic if called, gated behind later plans"

key-files:
  created:
    - src/commands/list.rs
    - src/commands/revoke.rs
  modified:
    - Cargo.toml (comfy-table 7.2.2 added)
    - src/record/mod.rs (burn + recipient fields, backwards-compat test)
    - src/crypto/mod.rs (recipient_from_z32 + 2 tests)
    - src/transport/mod.rs (delete_record + list_record_tokens)
    - src/cli.rs (--share, --burn, List, Revoke subcommands)
    - src/commands/mod.rs (list, revoke modules registered)
    - src/main.rs (List and Revoke wired in match)
    - src/commands/publish.rs (HandoffRecord construction updated with burn: false, recipient: None)

key-decisions:
  - "burn and recipient are unsigned metadata — excluded from HandoffRecordSignable to preserve Phase 3 signature compatibility"
  - "list_record_tokens uses parse::<u64>().is_ok() filter to exclude 'latest' LatestPointer key from results"
  - "delete_record treats 404 as success — idempotent deletion for both burn-after-read and revoke flows"
  - "recipient_from_z32 reuses existing age_recipient() + pkarr PublicKey::try_from path — no new crypto deps needed"

patterns-established:
  - "Unsigned metadata pattern: serde(default) fields on HandoffRecord that are excluded from HandoffRecordSignable"
  - "Stub command pattern: todo!() in stub files that compile cleanly; wired in main.rs for later plans to implement"

requirements-completed: [ENC-01, ENC-02, MGT-01, MGT-02, MGT-03]

# Metrics
duration: 4min
completed: 2026-02-22
---

# Phase 4 Plan 1: Phase 4 Primitives Summary

**HandoffRecord extended with burn/recipient fields, z32-to-age-recipient converter added, homeserver DELETE/LIST methods implemented, CLI wired with --share/--burn flags and List/Revoke subcommands**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-22T00:00:00Z
- **Completed:** 2026-02-22
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Extended HandoffRecord with `burn: bool` and `recipient: Option<String>` (both with `#[serde(default)]`) in alphabetical order, maintaining Phase 3 backwards compatibility
- Added `recipient_from_z32()` to crypto module — full round-trip tested (encrypt to derived recipient, decrypt with own key)
- Added `delete_record()` and `list_record_tokens()` to HomeserverClient with idempotent DELETE and numeric token filtering
- Extended CLI with `--share <PUBKEY>`, `--burn`, `list`, and `revoke <TOKEN>/--all` — all 30 existing tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add comfy-table dep and extend HandoffRecord** - `b6ba8e0` (feat)
2. **Task 2: Add recipient_from_z32 to crypto and delete/list to transport** - `5fc883d` (feat)
3. **Task 3: Extend CLI with --share, --burn flags and List/Revoke subcommands** - `3d99d31` (feat)

## Files Created/Modified

- `Cargo.toml` — comfy-table 7.2.2 added
- `src/record/mod.rs` — HandoffRecord with burn/recipient fields, backwards-compat test
- `src/crypto/mod.rs` — recipient_from_z32() with round-trip and invalid-key tests
- `src/transport/mod.rs` — delete_record() and list_record_tokens() methods
- `src/cli.rs` — --share, --burn flags; List and Revoke(RevokeArgs) subcommands
- `src/commands/mod.rs` — list and revoke modules registered
- `src/commands/list.rs` — stub run_list() (todo!)
- `src/commands/revoke.rs` — stub run_revoke() (todo!)
- `src/main.rs` — List and Revoke wired in main match dispatch
- `src/commands/publish.rs` — HandoffRecord constructor updated with burn: false, recipient: None

## Decisions Made

- **burn/recipient excluded from HandoffRecordSignable:** Phase 3 records were signed without these fields. Adding them to the signable struct would inject `burn: false, recipient: null` into the canonical JSON, breaking verification of all Phase 3 records. Pragmatic choice for a single-user tool where the user controls their own homeserver records.
- **list_record_tokens takes no keypair arg:** signin() must be called by the caller before list_record_tokens() — consistent with the existing transport pattern where delete_record() also assumes a pre-existing session.
- **Numeric token filter:** `t.parse::<u64>().is_ok()` cleanly excludes the "latest" LatestPointer key and any future non-numeric entries.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed compile errors in publish.rs and transport tests after HandoffRecord extension**
- **Found during:** Task 1 (extend HandoffRecord)
- **Issue:** Adding `burn` and `recipient` fields to HandoffRecord caused E0063 "missing fields" compile errors in `src/commands/publish.rs` and 4 HandoffRecord constructors in `src/transport/mod.rs` tests
- **Fix:** Updated all constructors to include `burn: false` and `recipient: None` defaults
- **Files modified:** `src/commands/publish.rs`, `src/transport/mod.rs`
- **Verification:** `cargo test -- record` passes all 10 tests
- **Committed in:** `b6ba8e0` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (blocking compile error)
**Impact on plan:** Necessary to fix all HandoffRecord constructors that pre-dated the new fields. No scope creep.

## Issues Encountered

None beyond the auto-fixed compile errors.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

All Phase 4 primitives are in place:
- Plans 04-02 and 04-03 can now proceed independently
- 04-02 (--share + --burn publish/pickup): uses `recipient_from_z32`, `record.burn`, `record.recipient` fields
- 04-03 (list + revoke commands): uses `delete_record`, `list_record_tokens`, comfy-table, stub command files

No blockers.

---
*Phase: 04-advanced-encryption-and-management*
*Completed: 2026-02-22*
