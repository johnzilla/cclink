# Roadmap: CCLink

## Overview

CCLink ships as a single Rust binary that lets you publish an encrypted Claude Code session reference from one machine and resume it on another.

## Milestones

- ✅ **v1.0 MVP** -- Phases 1-5 (shipped 2026-02-22)
- ✅ **v1.1 Security Hardening & Code Review Fixes** -- Phases 6-10 (shipped 2026-02-22)
- ✅ **v1.2 Dependency Audit & Code Quality** -- Phases 11-13 (shipped 2026-02-24)
- 🚧 **v1.3 Key Security Hardening** -- Phases 14-16 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1-5) -- SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation and Key Management (2/2 plans) -- completed 2026-02-22
- [x] Phase 2: Crypto and Transport (3/3 plans) -- completed 2026-02-22
- [x] Phase 3: Core Commands (4/4 plans) -- completed 2026-02-22
- [x] Phase 4: Advanced Encryption and Management (3/3 plans) -- completed 2026-02-22
- [x] Phase 5: Release and Distribution (2/2 plans) -- completed 2026-02-22

Full details: `milestones/v1.0-ROADMAP.md`

</details>

<details>
<summary>✅ v1.1 Security Hardening (Phases 6-10) -- SHIPPED 2026-02-22</summary>

- [x] Phase 6: Signed Record Format (2/2 plans) -- completed 2026-02-22
- [x] Phase 7: Code Quality and Transport (2/2 plans) -- completed 2026-02-22
- [x] Phase 8: CLI Fixes and Documentation (1/1 plan) -- completed 2026-02-22
- [x] Phase 9: PIN-Protected Handoffs (2/2 plans) -- completed 2026-02-22
- [x] Phase 10: Pubky Homeserver Transport Fix (2/2 plans) -- completed 2026-02-22

Full details: `milestones/v1.1-ROADMAP.md`

</details>

<details>
<summary>✅ v1.2 Dependency Audit & Code Quality (Phases 11-13) -- SHIPPED 2026-02-24</summary>

- [x] Phase 11: Prerequisites (2/2 plans) -- completed 2026-02-23
- [x] Phase 12: CI Hardening (1/1 plan) -- completed 2026-02-24
- [x] Phase 13: Code Quality and Security (2/2 plans) -- completed 2026-02-24

Full details: `milestones/v1.2-ROADMAP.md`

</details>

### 🚧 v1.3 Key Security Hardening (In Progress)

**Milestone Goal:** Protect the Ed25519 secret key at rest with passphrase encryption and zeroize all sensitive key material from memory after use.

- [x] **Phase 14: Memory Zeroization** - Zeroize X25519 scalar, decrypted key bytes, and passphrase/PIN strings from memory after use (completed 2026-02-24)
- [x] **Phase 15: Encrypted Key Crypto Layer** - Implement the CCLINKEK binary envelope format with encrypt/decrypt functions and unit tests (completed 2026-02-24)
- [ ] **Phase 16: Encrypted Key Storage and CLI Integration** - Wire passphrase-protected init, format-detecting load, and backward-compatible plaintext fallback into the full user-facing flow

## Phase Details

### Phase 14: Memory Zeroization
**Goal**: All sensitive secret material is zeroized from memory immediately after use
**Depends on**: Phase 13
**Requirements**: ZERO-01, ZERO-02, ZERO-03
**Success Criteria** (what must be TRUE):
  1. The derived X25519 secret scalar is wrapped in `Zeroizing<[u8;32]>` and is zeroed when it goes out of scope after publish and pickup
  2. The raw decrypted key file bytes are zeroized from memory after the keypair is parsed
  3. Passphrase and PIN strings collected from user prompts are wrapped in `Zeroizing<String>` and zeroed on drop
**Plans**: 2 plans
Plans:
- [ ] 14-01-PLAN.md -- Add zeroize dep, wrap crypto return types and reimplement load_keypair with zeroizing buffers (ZERO-01, ZERO-02)
- [ ] 14-02-PLAN.md -- Wrap PIN prompt strings in Zeroizing at call sites in publish.rs and pickup.rs (ZERO-03)

### Phase 15: Encrypted Key Crypto Layer
**Goal**: A tested, correct crypto layer can encrypt and decrypt an Ed25519 seed into the CCLINKEK binary envelope format
**Depends on**: Phase 14
**Requirements**: KEYS-05
**Success Criteria** (what must be TRUE):
  1. `encrypt_key_envelope` produces a binary blob with the `CCLINKEK` magic header, version byte, Argon2 parameters, salt, and age ciphertext
  2. `decrypt_key_envelope` with the correct passphrase round-trips back to the original 32-byte seed
  3. `decrypt_key_envelope` with a wrong passphrase returns a clear error (not a panic or corrupt-data error)
  4. Argon2 parameters are read from the file header on decryption, not from hardcoded constants
  5. The HKDF info string `"cclink-key-v1"` is distinct from the PIN derivation info string `"cclink-pin-v1"`
**Plans**: 1 plan
Plans:
- [ ] 15-01-PLAN.md -- TDD: implement encrypt_key_envelope, decrypt_key_envelope, and key_derive_key with CCLINKEK binary envelope format (KEYS-05)

### Phase 16: Encrypted Key Storage and CLI Integration
**Goal**: Users can create passphrase-protected keypairs with `cclink init` and all commands transparently prompt for the passphrase when needed, while existing plaintext v1.2 key files continue to work
**Depends on**: Phase 15
**Requirements**: KEYS-01, KEYS-02, KEYS-03, KEYS-04, KEYS-06
**Success Criteria** (what must be TRUE):
  1. `cclink init` prompts for a passphrase with confirmation and writes an encrypted key file; `cclink init --no-passphrase` skips the prompt and writes a plaintext key file
  2. Any command that loads an encrypted keypair prompts for the passphrase before proceeding
  3. Entering the wrong passphrase prints "Wrong passphrase" and exits with code 1 (no retry, no ambiguous error)
  4. An existing v1.2 plaintext key file loads without any passphrase prompt in the v1.3 binary
  5. The encrypted key file has 0600 permissions and is written atomically (no partial file left on interrupted write)
**Plans**: 2 plans
Plans:
- [ ] 16-01-PLAN.md -- TDD: encrypted key store layer — write_encrypted_keypair_atomic and format-detecting load_keypair (KEYS-03, KEYS-04, KEYS-06)
- [ ] 16-02-PLAN.md -- Wire --no-passphrase CLI flag and passphrase prompt into cclink init (KEYS-01, KEYS-02)

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation and Key Management | v1.0 | 2/2 | Complete | 2026-02-22 |
| 2. Crypto and Transport | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Core Commands | v1.0 | 4/4 | Complete | 2026-02-22 |
| 4. Advanced Encryption and Management | v1.0 | 3/3 | Complete | 2026-02-22 |
| 5. Release and Distribution | v1.0 | 2/2 | Complete | 2026-02-22 |
| 6. Signed Record Format | v1.1 | 2/2 | Complete | 2026-02-22 |
| 7. Code Quality and Transport | v1.1 | 2/2 | Complete | 2026-02-22 |
| 8. CLI Fixes and Documentation | v1.1 | 1/1 | Complete | 2026-02-22 |
| 9. PIN-Protected Handoffs | v1.1 | 2/2 | Complete | 2026-02-22 |
| 10. Pubky Homeserver Transport Fix | v1.1 | 2/2 | Complete | 2026-02-22 |
| 11. Prerequisites | v1.2 | 2/2 | Complete | 2026-02-23 |
| 12. CI Hardening | v1.2 | 1/1 | Complete | 2026-02-24 |
| 13. Code Quality and Security | v1.2 | 2/2 | Complete | 2026-02-24 |
| 14. Memory Zeroization | 2/2 | Complete    | 2026-02-24 | - |
| 15. Encrypted Key Crypto Layer | 1/1 | Complete    | 2026-02-24 | - |
| 16. Encrypted Key Storage and CLI Integration | v1.3 | 0/2 | Not started | - |
