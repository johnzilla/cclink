# Roadmap: CCLink

## Overview

CCLink ships as a single Rust binary that lets you publish an encrypted Claude Code session reference from one machine and resume it on another. The roadmap follows the hard dependency chain in the codebase: keypair management comes first because everything else depends on it, encryption comes second because transport requires it, the core publish/pickup loop comes third to deliver the product's central value, advanced encryption modes and management commands come fourth once the core loop is proven, and release tooling closes it out.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation and Key Management** - Rust project scaffold, keypair generation, atomic key storage, and `cclink init` / `cclink whoami` (completed 2026-02-22)
- [x] **Phase 2: Crypto and Transport** - age self-encryption, Ed25519-to-X25519 conversion, Pubky homeserver integration, and HandoffRecord serialization (completed 2026-02-22)
- [x] **Phase 3: Core Commands** - `cclink publish`, `cclink pickup`, session discovery, TTL enforcement, QR code, retry/backoff, and colored output (completed 2026-02-22)
- [ ] **Phase 4: Advanced Encryption and Management** - `--share`, `--burn`, `--exec`, signature verification, `cclink list`, and `cclink revoke`
- [ ] **Phase 5: Release and Distribution** - musl static binary, GitHub release artifacts, CI pipeline with round-trip tests

## Phase Details

### Phase 1: Foundation and Key Management
**Goal**: Users have a working identity: keypair generated, stored safely, and inspectable
**Depends on**: Nothing (first phase)
**Requirements**: KEY-01, KEY-02, KEY-03, KEY-04
**Success Criteria** (what must be TRUE):
  1. User runs `cclink init` and a keypair is generated and stored at `~/.cclink/keys` with 0600 permissions
  2. User runs `cclink whoami` and sees their PKARR public key and homeserver info
  3. User runs `cclink init --import` and an existing keypair is loaded without loss
  4. The key file on disk survives a simulated crash during write (atomic write-to-temp-then-rename verified)
**Plans:** 2/2 plans complete
Plans:
- [ ] 01-01-PLAN.md — Scaffold Rust project, key store module, and `cclink init` (generate + import + atomic write)
- [ ] 01-02-PLAN.md — Implement `cclink whoami` with identity display and clipboard support

### Phase 2: Crypto and Transport
**Goal**: Encrypted payloads can be written to and read from the Pubky homeserver
**Depends on**: Phase 1
**Requirements**: PUB-02, PUB-03, PUB-05, UX-02
**Success Criteria** (what must be TRUE):
  1. A session payload encrypted with the user's own X25519 key (derived from Ed25519) can be decrypted by the same key in a round-trip test
  2. Session data is published to the Pubky homeserver via PUT (not PKARR DNS records) and retrieved via GET
  3. A `latest.json` pointer is written to the homeserver on each publish
  4. All retrieved records have their Ed25519 signature verified before being returned to the caller
**Plans:** 3/3 plans complete
Plans:
- [ ] 02-01-PLAN.md — Add Phase 2 dependencies and implement crypto module (Ed25519-to-X25519 key derivation + age encrypt/decrypt)
- [ ] 02-02-PLAN.md — Implement HandoffRecord struct with canonical JSON serialization and Ed25519 signing/verification
- [ ] 02-03-PLAN.md — Implement transport module with AuthToken, homeserver signin, PUT/GET, and latest.json pointer

### Phase 3: Core Commands
**Goal**: Users can complete the full publish-to-pickup loop from two different machines
**Depends on**: Phase 2
**Requirements**: SESS-01, SESS-02, PUB-01, PUB-04, PUB-06, RET-01, RET-02, RET-03, RET-04, RET-05, RET-06, UX-01
**Success Criteria** (what must be TRUE):
  1. User runs `cclink` with no arguments and the most recent Claude Code session is discovered, encrypted, and published; a QR code appears in the terminal
  2. User runs `cclink pickup` on a second machine and retrieves the session ID, decrypted and printed
  3. Pickup refuses a record whose TTL has expired
  4. User runs `cclink pickup --exec` and `claude --resume <id>` executes automatically
  5. Colored terminal output clearly distinguishes success states from error states
**Plans:** 4/4 plans complete
Plans:
- [x] 03-01-PLAN.md — Add Phase 3 dependencies, session discovery module, and new error variants (completed 2026-02-22)
- [ ] 03-02-PLAN.md — Restructure CLI for default publish and implement the publish command end-to-end
- [ ] 03-03-PLAN.md — Implement the pickup command with retrieval, TTL check, retry/backoff, and exec
- [ ] 03-04-PLAN.md — Gap closure: Claude Code help text and cwd-filtered session discovery

### Phase 4: Advanced Encryption and Management
**Goal**: Users can share handoffs with specific recipients, burn records after read, and manage their published records
**Depends on**: Phase 3
**Requirements**: ENC-01, ENC-02, MGT-01, MGT-02, MGT-03
**Success Criteria** (what must be TRUE):
  1. User runs `cclink --share <pubkey>` and only the holder of the corresponding private key can decrypt the handoff
  2. User runs `cclink --burn` and the record is deleted from the homeserver after the first successful retrieval
  3. User runs `cclink list` and sees token, project, age, TTL remaining, and burn status for all active records
  4. User runs `cclink revoke <token>` and the specific record is removed; `cclink revoke --all` removes all records
**Plans:** 1/3 plans executed
Plans:
- [ ] 04-01-PLAN.md — Extend record/crypto/transport/CLI with Phase 4 primitives (burn/recipient fields, z32 recipient, DELETE/LIST, new subcommands)
- [ ] 04-02-PLAN.md — Implement --share and --burn in publish, extend pickup for shared records and burn-after-read
- [ ] 04-03-PLAN.md — Implement cclink list and cclink revoke commands

### Phase 5: Release and Distribution
**Goal**: CCLink is distributable as a self-contained binary with automated release artifacts
**Depends on**: Phase 4
**Requirements**: (none — cross-cutting delivery concern)
**Success Criteria** (what must be TRUE):
  1. `cargo build --release` produces a musl-linked static binary that runs on a fresh Linux machine with no installed dependencies
  2. GitHub Actions triggers a release on tag push and publishes platform binaries (Linux musl, macOS, Windows) as release artifacts
  3. CI runs round-trip encryption tests (encrypt-then-decrypt for every code path) and fails the build if any key material appears in plaintext in test output
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation and Key Management | 1/2 | Complete    | 2026-02-22 |
| 2. Crypto and Transport | 3/3 | Complete   | 2026-02-22 |
| 3. Core Commands | 4/4 | Complete   | 2026-02-22 |
| 4. Advanced Encryption and Management | 1/3 | In Progress|  |
| 5. Release and Distribution | 0/TBD | Not started | - |
