---
phase: 05-release-and-distribution
plan: 01
subsystem: testing
tags: [age, encryption, round-trip, plaintext-leak, integration-tests, rust, httpmock]

# Dependency graph
requires:
  - phase: 04-advanced-encryption-and-management
    provides: age encryption functions (age_encrypt, age_decrypt, age_identity, age_recipient, ed25519_to_x25519_secret, ed25519_to_x25519_public) in src/crypto/mod.rs
provides:
  - Integration test suite: 4 round-trip tests covering all encryption code paths (self, shared, burn, shared+burn)
  - Plaintext leak detection: 3 tests asserting session IDs never appear in ciphertext or base64 blob
  - src/lib.rs exposing crypto, record, transport, error, keys modules for integration test access
  - httpmock dev-dependency for future HTTP-level integration tests
affects: [05-02, ci, release]

# Tech tracking
tech-stack:
  added: [httpmock = "0.8" (dev-dependency)]
  patterns: [lib.rs re-exports internal modules for integration test access, integration tests use #[test] (no async) for deterministic no-network execution]

key-files:
  created:
    - tests/integration_round_trip.rs
    - tests/plaintext_leak.rs
    - src/lib.rs
  modified:
    - Cargo.toml

key-decisions:
  - "src/lib.rs re-exports pub mod crypto/record/transport/error/keys — does not include cli/commands/session (not needed by tests)"
  - "Integration tests use pkarr::Keypair::from_secret_key with fixed seeds ([42u8;32], [99u8;32]) for deterministic, reproducible test vectors"
  - "Burn flag confirmed as metadata-only: crypto path in test_burn_encrypt_round_trip is identical to self-encrypt path"
  - "Plaintext leak byte-window scan plus UTF-8 lossy scan provides defense-in-depth: catches both raw bytes and string reinterpretation"

patterns-established:
  - "Round-trip test pattern: fixed keypair → derive identity+recipient → encrypt → decrypt → assert eq original"
  - "Sender-cannot-decrypt assertion: shared-encrypt tests include negative case to confirm encryption is asymmetric"
  - "Base64 blob leak test: mirrors actual HandoffRecord.blob storage format to catch leaks in the real publish path"

requirements-completed: []

# Metrics
duration: 2min
completed: 2026-02-22
---

# Phase 5 Plan 01: Integration Tests for Encryption Round-Trips and Plaintext Leak Detection Summary

**7 integration tests covering all 4 age-encryption code paths with byte-level plaintext leak detection, using fixed-seed keypairs for deterministic test vectors**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-22T18:24:46Z
- **Completed:** 2026-02-22T18:26:54Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Created `src/lib.rs` to expose internal modules for integration test access (no CLI/commands exposure — only crypto, record, transport, error, keys)
- 4 round-trip tests pass covering self-encrypt, shared-encrypt (with sender-cannot-decrypt assertion), burn, and shared+burn
- 3 plaintext leak tests pass with UTF-8 lossy scan + byte-window scan + base64-encoded blob check
- Added `httpmock = "0.8"` dev-dependency for future HTTP-level integration tests
- All 40 total tests pass (33 existing unit tests + 7 new integration tests), zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Add httpmock dev-dependency and create round-trip encryption integration tests** - `8857e67` (feat)
2. **Task 2: Create plaintext leak detection test** - `4b0111e` (feat)

**Plan metadata:** (this SUMMARY commit — see final commit below)

## Files Created/Modified
- `src/lib.rs` - Library crate root re-exporting crypto, record, transport, error, keys as pub for integration tests
- `tests/integration_round_trip.rs` - 4 tests: self-encrypt, shared-encrypt, burn, shared+burn round-trips
- `tests/plaintext_leak.rs` - 3 tests: self-encrypt leak, shared-encrypt leak, base64-blob leak
- `Cargo.toml` - Added `[dev-dependencies] httpmock = "0.8"`

## Decisions Made
- `src/lib.rs` intentionally omits `cli`, `commands`, and `session` modules — they depend on filesystem/stdin/stdout and are not needed by crypto integration tests
- Used fixed seeds (`[42u8; 32]` for self/sender, `[99u8; 32]` for recipient) to make tests deterministic and reproducible across environments
- Plaintext leak tests use two complementary checks: `String::from_utf8_lossy` scan (catches string reinterpretation) and `.windows(n)` byte scan (catches raw byte presence) — defense-in-depth

## Deviations from Plan

None - plan executed exactly as written.

The plan specified creating `src/lib.rs` if no lib.rs existed. None existed, so it was created as specified. The httpmock dependency was in Cargo.toml already when the plan was executed (added by the plan spec). No architectural changes, no bug fixes required.

## Issues Encountered
None.

## Next Phase Readiness
- All encryption code paths now have verified round-trip tests — CI will catch any regression in crypto functions
- Plaintext leak tests will catch any future refactor that accidentally stores unencrypted session data
- `httpmock` dev-dependency is ready for Plan 02 if HTTP-level integration tests are added
- 33 existing unit tests continue to pass — no regressions

---
*Phase: 05-release-and-distribution*
*Completed: 2026-02-22*

## Self-Check: PASSED

- FOUND: tests/integration_round_trip.rs
- FOUND: tests/plaintext_leak.rs
- FOUND: src/lib.rs
- FOUND: 05-01-SUMMARY.md
- FOUND: commit 8857e67 (Task 1)
- FOUND: commit 4b0111e (Task 2)
