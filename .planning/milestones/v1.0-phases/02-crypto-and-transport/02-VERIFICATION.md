---
phase: 02-crypto-and-transport
verified: 2026-02-22T00:00:00Z
status: passed
score: 13/13 must-haves verified
re_verification: false
---

# Phase 2: Crypto and Transport Verification Report

**Phase Goal:** Encrypted payloads can be written to and read from the Pubky homeserver
**Verified:** 2026-02-22
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth                                                                                          | Status     | Evidence                                                                                     |
|----|-----------------------------------------------------------------------------------------------|------------|----------------------------------------------------------------------------------------------|
| 1  | Ed25519 keypair can derive a valid X25519 secret scalar and public Montgomery point            | VERIFIED   | `ed25519_to_x25519_secret` / `ed25519_to_x25519_public` in `src/crypto/mod.rs`; 2 unit tests pass |
| 2  | age ciphertext round-trips correctly (encrypt then decrypt yields identical plaintext)          | VERIFIED   | `age_encrypt` / `age_decrypt` in `src/crypto/mod.rs`; `test_age_encrypt_decrypt_round_trip` passes |
| 3  | age ciphertext varies per encryption (ephemeral key randomness)                                | VERIFIED   | `test_age_encrypt_produces_different_ciphertext` passes                                      |
| 4  | Decryption with wrong key fails with an error                                                  | VERIFIED   | `test_age_decrypt_wrong_key_fails` passes                                                    |
| 5  | HandoffRecord serializes to canonical compact JSON with alphabetical key ordering              | VERIFIED   | `canonical_json()` in `src/record/mod.rs`; `test_handoff_record_signable_serializes_alphabetical_keys` and `test_canonical_json_is_compact_no_whitespace` pass |
| 6  | Ed25519 sign + verify round-trip succeeds; wrong key and tampered content both fail            | VERIFIED   | `sign_record()` / `verify_record()` in `src/record/mod.rs`; 3 tests pass                   |
| 7  | Signature field is excluded from canonical JSON used for signing (no circular dependency)      | VERIFIED   | `HandoffRecordSignable` struct excludes `signature`; `From<&HandoffRecord>` copies all other fields |
| 8  | AuthToken binary matches pubky-common 0.5.4 layout: varint(64)+sig+namespace+version+ts+pubkey+caps | VERIFIED | `build_auth_token()` in `src/transport/mod.rs`; `test_auth_token_structure` confirms bytes[0]=0x40, bytes[65..75]=b"PUBKY:AUTH", bytes[75]=0 |
| 9  | AuthToken Ed25519 signature verifies over signable region bytes[65..]                         | VERIFIED   | `test_auth_token_signature_verifies` passes                                                  |
| 10 | HomeserverClient uses cookie_store(true) for session persistence                               | VERIFIED   | `reqwest::blocking::Client::builder().cookie_store(true)` in `HomeserverClient::new()`      |
| 11 | URL construction follows /pub/cclink/{token} and /pub/cclink/latest patterns                  | VERIFIED   | `put_record`, `put_latest`, `get_record`, `get_latest` construct correct URLs; `test_homeserver_client_strips_https_prefix` passes |
| 12 | publish() generates timestamp-based tokens and writes both record and latest pointer payloads  | VERIFIED   | `publish()` calls `record.created_at.to_string()` for token, then `put_record` + `put_latest`; `test_publish_token_is_created_at_timestamp` passes |
| 13 | Retrieved records have Ed25519 signature verified before bytes are returned to caller          | VERIFIED   | `deserialize_and_verify()` called in `get_record` and `get_record_by_pubkey`; `test_get_record_deserialization_and_verification_pipeline` and `test_get_record_wrong_pubkey_fails` pass |

**Score:** 13/13 truths verified

---

### Required Artifacts

| Artifact                   | Plan    | Provides                                                         | Exists | Substantive          | Wired       | Status     |
|---------------------------|---------|------------------------------------------------------------------|--------|----------------------|-------------|------------|
| `Cargo.toml`              | 02-01   | All Phase 2 dependencies                                         | YES    | YES — 13 deps listed | YES — compiled | VERIFIED |
| `src/crypto/mod.rs`       | 02-01   | Key derivation + age encrypt/decrypt                             | YES    | YES — 164 lines, 6 pub fns, 5 tests | YES — `mod crypto;` in main.rs | VERIFIED |
| `src/record/mod.rs`       | 02-02   | HandoffRecord, canonical JSON, Ed25519 signing/verification      | YES    | YES — 306 lines, 3 pub structs, 5 pub fns, 7 tests | YES — `mod record;` in main.rs | VERIFIED |
| `src/error.rs`            | 02-02   | Extended error enum with SignatureVerificationFailed             | YES    | YES — both variants present | YES — used in `verify_record` | VERIFIED |
| `src/transport/mod.rs`    | 02-03   | AuthToken builder, HomeserverClient with signin/put/get/publish  | YES    | YES — 616 lines, 8 pub fns, 9 unit tests + 1 ignored integration test | YES — `mod transport;` in main.rs | VERIFIED |

---

### Key Link Verification

| From                      | To                        | Via                                                                      | Status   | Evidence                                                                    |
|--------------------------|---------------------------|--------------------------------------------------------------------------|----------|-----------------------------------------------------------------------------|
| `src/crypto/mod.rs`      | `pkarr::Keypair`          | `keypair.secret_key()` -> `SigningKey` -> `to_scalar_bytes()`            | WIRED    | Line 18: `ed25519_dalek::SigningKey::from_bytes(&keypair.secret_key())` then `.to_scalar_bytes()` |
| `src/crypto/mod.rs`      | `age::x25519::Identity`   | bech32 encode of X25519 scalar bytes with "age-secret-key-" HRP          | WIRED    | Lines 35-41: `bech32::encode("age-secret-key-", ...)` then `.parse::<age::x25519::Identity>()` |
| `src/record/mod.rs`      | `pkarr::Keypair`          | `keypair.sign(canonical_json_bytes)`                                     | WIRED    | Line 99: `keypair.sign(json.as_bytes())`                                    |
| `src/record/mod.rs`      | `pkarr::PublicKey`        | `pubkey.verify(canonical_json_bytes, &signature)`                        | WIRED    | Line 128: `pubkey.verify(json.as_bytes(), &sig)`                            |
| `src/record/mod.rs`      | `serde_json`              | `serde_json::to_string` for canonical compact JSON                       | WIRED    | Line 90: `serde_json::to_string(signable)?`                                 |
| `src/transport/mod.rs`   | `reqwest::blocking::Client` | `cookie_store(true)` for session persistence across PUT calls          | WIRED    | Line 124: `.cookie_store(true)`                                             |
| `src/transport/mod.rs`   | postcard                  | AuthToken binary serialization (manual varint construction)              | WIRED    | Lines 60-93: manual postcard byte layout with varint encoding               |
| `src/transport/mod.rs`   | `src/record/mod.rs`       | `verify_record` called before returning HandoffRecord from GET           | WIRED    | Line 330: `crate::record::verify_record(&record, pubkey)?`                  |

---

### Requirements Coverage

| Requirement | Source Plan | Description                                                                      | Status      | Evidence                                                                          |
|-------------|------------|---------------------------------------------------------------------------------|-------------|-----------------------------------------------------------------------------------|
| PUB-02      | 02-02      | Handoff record includes hostname, project path, creation timestamp, and TTL      | SATISFIED   | `HandoffRecord` has `hostname`, `project`, `created_at`, `ttl` fields (all pub, required in construction) |
| PUB-03      | 02-01      | Session ID is age-encrypted to creator's own X25519 key (derived from Ed25519)  | SATISFIED   | `age_encrypt` / `age_decrypt` via `ed25519_to_x25519_secret` / `ed25519_to_x25519_public`; round-trip test passes |
| PUB-05      | 02-03      | A `latest.json` pointer is updated on each publish                              | SATISFIED   | `publish()` calls `put_latest()` with `LatestPointer` JSON; URL is `/pub/cclink/latest` |
| UX-02       | 02-02, 02-03 | Ed25519 signature verification on all retrieved records                       | SATISFIED   | `verify_record()` in record module; `deserialize_and_verify()` called in all GET paths; hard fail enforced |

No orphaned requirements. All four Phase 2 requirement IDs (PUB-02, PUB-03, PUB-05, UX-02) are claimed by plans and have implementation evidence.

---

### Anti-Patterns Found

| File                      | Line    | Pattern                                   | Severity | Impact                               |
|--------------------------|---------|-------------------------------------------|----------|--------------------------------------|
| `src/error.rs`           | 9, 12   | `InvalidKeyFormat`, `KeyCorrupted` unused | Info     | Dead code warning; Phase 1 variants not yet exercised from production paths — not a Phase 2 concern |
| `src/error.rs`           | 24      | `RecordDeserializationFailed` unused      | Info     | Defined but not yet used in error propagation paths (transport uses `anyhow` directly); does not block goal |
| `src/transport/mod.rs`   | 234, 252 | `get_record_by_pubkey`, `get_latest` unused | Info  | Compiler warnings; both functions are fully implemented and will be used in Phase 3 pickup command |

No blockers found. No stub implementations. No placeholder returns. No TODO-only handlers.

---

### Human Verification Required

#### 1. Live Homeserver Round-Trip

**Test:** Run `cargo test -- --ignored` with a real Pubky homeserver accessible (e.g., pubky.app). Requires a valid Pubky account or local homeserver instance.
**Expected:** `test_integration_signin_put_get` passes — signin succeeds, record PUT returns 2xx, GET retrieves the correct record, and signature verification passes.
**Why human:** Requires a live Pubky homeserver; cannot verify network auth, cookie forwarding, and correct URL routing against the real homeserver in an automated offline check.

---

### Test Run Summary

```
running 22 tests
21 passed; 0 failed; 1 ignored (integration test — requires live homeserver)
```

All 21 unit tests pass:
- Crypto module: 5 tests (key derivation determinism, age round-trip, ephemeral uniqueness, wrong-key rejection)
- Record module: 7 tests (alphabetical JSON keys, compactness, determinism, sign/verify, wrong-key rejection, tamper detection, LatestPointer serialization)
- Transport module: 9 tests (AuthToken length/structure/byte-layout/signature verification/multi-keypair, HomeserverClient construction/prefix-stripping, token derivation, deserialize+verify pipeline, wrong-pubkey rejection)

### Commit Verification

All documented commits confirmed present in git history:
- `2f551ca` — chore(02-01): add all Phase 2 dependencies to Cargo.toml
- `fa8a4b3` — feat(02-01): implement crypto module with key derivation and age encryption
- `2e2f41a` — feat(02-02): implement HandoffRecord with canonical JSON and Ed25519 signing
- `970e13f` — feat(02-03): implement transport module with AuthToken and HomeserverClient

---

## Phase Goal Verdict

The phase goal — **encrypted payloads can be written to and read from the Pubky homeserver** — is achieved:

1. **Encryption layer** (`src/crypto/mod.rs`): Ed25519 keypairs derive X25519 keys; age encrypts plaintext to the derived recipient; age decrypts with the derived identity. Round-trip is verified in tests.

2. **Record layer** (`src/record/mod.rs`): `HandoffRecord` carries the encrypted blob plus all metadata. Canonical JSON signing ensures tamper detection. `verify_record` hard-fails on any mismatch.

3. **Transport layer** (`src/transport/mod.rs`): `HomeserverClient` authenticates with a postcard-serialized AuthToken (byte layout confirmed against pubky-common 0.5.4), uses cookie_store for session persistence, PUTs records to `/pub/cclink/{token}`, updates `latest.json`, and verifies signatures on all GET operations before returning records to callers.

The only gap is a live homeserver integration test (1 `#[ignore]` test) — this is expected and flagged for human verification.

---

_Verified: 2026-02-22_
_Verifier: Claude (gsd-verifier)_
