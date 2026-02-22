---
phase: 06-signed-record-format
verified: 2026-02-22T20:00:00Z
status: passed
score: 7/7 must-haves verified
re_verification: false
---

# Phase 6: Signed Record Format — Verification Report

**Phase Goal:** Handoff records are cryptographically honest — burn and recipient intent is signed into the payload and key permissions are enforced by cclink itself
**Verified:** 2026-02-22
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A published handoff payload includes burn and recipient fields inside the signed envelope | VERIFIED | `HandoffRecordSignable` in `src/record/mod.rs` lines 58–75 declares `pub burn: bool` and `pub recipient: Option<String>`; `src/commands/publish.rs` lines 100–109 populates both fields (`burn: cli.burn`, `recipient: cli.share.clone()`) |
| 2 | Picking up a v1.1 record tampered to flip the burn flag fails signature verification | VERIFIED | `test_signed_burn_tamper_detection` in `tests/integration_round_trip.rs` signs with `burn=false`, tampers to `burn=true`, asserts `verify_record` returns `Err`; test passes |
| 3 | Tampering recipient field fails signature verification | VERIFIED | `test_signed_recipient_tamper_detection` in `tests/integration_round_trip.rs` signs with `recipient=None`, tampers to `Some("attacker-pubkey-z32encoded")`, asserts `verify_record` returns `Err`; test passes |
| 4 | On any cclink operation that loads the key, code explicitly checks and enforces 0600 permissions rather than relying on pkarr | VERIFIED | `src/keys/store.rs` line 90: `check_key_permissions(&path)?;` called in `load_keypair()` BEFORE `pkarr::Keypair::from_secret_key_file`; `check_key_permissions` defined at lines 107–127 with `#[cfg(unix)]` / `#[cfg(not(unix))]` variants |
| 5 | Existing v1.0 records (unsigned burn/recipient) expire via TTL without migration | VERIFIED | `test_phase3_record_backwards_compat` removed from codebase (clean break decision); no version negotiation code exists; ROADMAP and STATE.md document TTL expiry as the migration path |
| 6 | Writing a new keypair explicitly sets 0600 permissions after atomic rename | VERIFIED | `src/keys/store.rs` lines 51–56: `#[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(dest, Permissions::from_mode(0o600)) }` after `rename()` succeeds |
| 7 | Full test suite passes with all tamper-detection tests | VERIFIED | `cargo test` result: 38 unit + 6 integration + 3 plaintext_leak = 47 tests pass, 1 ignored (live network), 0 failures |

**Score:** 7/7 truths verified

---

### Required Artifacts

#### Plan 06-01 (SEC-01)

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/record/mod.rs` | `HandoffRecordSignable` with `burn: bool` and `recipient: Option<String>` | VERIFIED | Lines 58–75: both fields declared in correct alphabetical position (burn after blob, recipient after pubkey). `From<&HandoffRecord>` impl at lines 93–108 copies both fields. |
| `src/record/mod.rs` | `From<&HandoffRecord>` includes `burn: record.burn` and `recipient: record.recipient.clone()` | VERIFIED | Lines 99–104 confirmed |
| `tests/integration_round_trip.rs` | Integration tests proving tamper detection | VERIFIED | Tests 5 and 6 at lines 178–272 cover burn and recipient tampering end-to-end |

#### Plan 06-02 (SEC-02)

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/keys/store.rs` | `check_key_permissions()` function with `#[cfg(unix)]` and `#[cfg(not(unix))]` variants | VERIFIED | Lines 106–127: both variants present, `#[cfg(unix)]` enforces `mode == 0o600`, non-Unix is no-op |
| `src/keys/store.rs` | `load_keypair()` calls permission check before reading key file | VERIFIED | Line 90: `check_key_permissions(&path)?;` before `pkarr::Keypair::from_secret_key_file` at line 91 |
| `src/keys/store.rs` | `write_keypair_atomic()` sets 0600 after rename | VERIFIED | Lines 51–56: explicit `set_permissions` call in `#[cfg(unix)]` block after successful `rename()` |
| `src/keys/store.rs` | Error message includes `chmod 600` remediation | VERIFIED | Lines 113–119: `anyhow::bail!` message contains `"Fix with: chmod 600 {}"` |

---

### Key Link Verification

#### Plan 06-01

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/commands/publish.rs` | `src/record/mod.rs` | `HandoffRecordSignable` construction includes `burn: cli.burn` and `recipient: cli.share.clone()` | WIRED | Lines 100–109 of `publish.rs`: struct literal matches alphabetical order and includes both fields |
| `src/record/mod.rs` verify path | `src/record/mod.rs` signable | `verify_record` calls `HandoffRecordSignable::from(record)` which includes burn+recipient | WIRED | Line 140: `let signable = HandoffRecordSignable::from(record);` — burn and recipient flow into canonical JSON and signature check |

#### Plan 06-02

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/keys/store.rs` `load_keypair` | `src/keys/store.rs` `check_key_permissions` | Permission check before key read | WIRED | `check_key_permissions(&path)?;` on line 90 precedes `from_secret_key_file` on line 91 |
| `src/keys/store.rs` `write_keypair_atomic` | filesystem | `set_permissions` after rename | WIRED | Lines 51–56: explicit `#[cfg(unix)]` block sets 0600 unconditionally after successful rename |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SEC-01 | 06-01-PLAN.md | Handoff payload signs burn and recipient fields (clean break from v1.0 unsigned format) | SATISFIED | `HandoffRecordSignable` includes both fields; publish command populates them; tamper-detection tests pass; `test_phase3_record_backwards_compat` removed confirming clean break |
| SEC-02 | 06-02-PLAN.md | Key file permissions (0600) enforced explicitly in cclink code, not just delegated to pkarr | SATISFIED | `check_key_permissions()` exists in cclink's own code; called before pkarr in `load_keypair()`; `write_keypair_atomic()` calls `set_permissions` independently of pkarr; 3 unit tests cover reject-0644, accept-0600, write-produces-0600 |

No orphaned requirements: REQUIREMENTS.md traceability table maps only SEC-01 and SEC-02 to Phase 6, and both are covered by plans 06-01 and 06-02 respectively.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/error.rs` | 9–33 | Dead enum variants: `InvalidKeyFormat`, `KeyCorrupted`, `RecordDeserializationFailed`, `HandoffExpired`, `NetworkRetryExhausted` | Warning | Compiler emits `variants ... are never constructed` warning; tracked as QUAL-03 for Phase 7 — not a Phase 6 concern |

No blocker anti-patterns found. The compiler warning pre-dates Phase 6 and is explicitly assigned to Phase 7 (QUAL-03).

---

### Human Verification Required

None. All SEC-01 and SEC-02 behaviors are verifiable programmatically:
- Struct field presence: confirmed by reading source
- Signing pipeline wiring: confirmed by reading publish.rs
- Tamper detection: proven by passing automated tests
- Permission enforcement: proven by 3 unit tests in store.rs

---

### Gaps Summary

No gaps. All must-haves from both plans are fully implemented, wired, and tested.

**SEC-01 is structurally complete:**
- `HandoffRecordSignable` has `burn: bool` and `recipient: Option<String>` in the correct alphabetical position
- `From<&HandoffRecord>` copies both fields into the signable
- `publish.rs` populates both from CLI args (`burn: cli.burn`, `recipient: cli.share.clone()`)
- `verify_record` derives signable from record (including burn/recipient) before checking signature — tampering either field changes the canonical JSON and breaks the signature
- Two integration tests and one unit test prove tamper detection against actual crypto operations

**SEC-02 is structurally complete:**
- `check_key_permissions()` is cclink's own code (not pkarr's)
- Called before reading key material in `load_keypair()`
- Called after rename in `write_keypair_atomic()` (not relying on pkarr or umask)
- Non-Unix no-op ensures cross-platform compilation
- Error message includes the exact remediation command

**Test suite: 47 tests pass, 0 fail, 1 ignored (live network test).**

---

_Verified: 2026-02-22_
_Verifier: Claude (gsd-verifier)_
