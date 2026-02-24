# Milestones

## v1.0 MVP (Shipped: 2026-02-22)

**Phases:** 1-5 (14 plans) | **Rust LOC:** 2,851 | **Timeline:** 2 days

**Delivered:** Secure CLI tool for publishing and resuming encrypted Claude Code session handoffs via the Pubky protocol.

**Key accomplishments:**
- PKARR Ed25519 keypair management with atomic write and import support
- age encryption with Ed25519-to-X25519 derivation and round-trip verification
- Pubky homeserver transport with AuthToken signing, session cookies, and signature verification
- Full publish-to-pickup loop with session discovery, QR codes, retry/backoff, and Unix exec()
- Share-to-recipient encryption, burn-after-read, list/revoke management commands
- CI/CD pipeline with 4-platform release builds, SHA256 checksums, and curl installer

**Git range:** `c381479` → `4180df6` (66 files, 15,361 insertions)

**Known tech debt:**
- QR code content wrong when `--share` + `--qr` combined (printed text correct)
- Cargo.toml/install.sh use placeholder `user/cclink` repo path
- 5 dead `CclinkError` variants (compiler warnings)

**Archive:** `milestones/v1.0-ROADMAP.md`, `milestones/v1.0-REQUIREMENTS.md`

---

## v1.1 Security Hardening & Code Review Fixes (Shipped: 2026-02-22)

**Phases:** 6-10 (9 plans) | **Rust LOC:** 2,633 (src) + 590 (tests) | **Timeline:** 1 day

**Delivered:** Comprehensive security hardening: signed metadata envelopes, key permission enforcement, PIN-protected handoffs, structured error handling, and migration from homeserver to direct PKARR Mainline DHT transport with full metadata encryption.

**Key accomplishments:**
- Ed25519-signed burn and recipient fields with tamper detection (clean break from v1.0)
- Key file permission enforcement (0600) at load and write time
- PIN-protected handoffs with Argon2id+HKDF-SHA256 key derivation
- Structured error handling (CclinkError::RecordNotFound, dead variant cleanup)
- Replaced homeserver transport with PKARR Mainline DHT (no accounts, no tokens, no signup)
- Encrypted all sensitive metadata (hostname, project path, session ID) into blob -- zero cleartext metadata on DHT

**Git range:** `49c9586` → `d2b65d3` (97 files, 6,140 insertions, 11,880 deletions)

**Known tech debt:**
- LatestPointer struct is dead code after DHT migration
- Age ciphertext size non-deterministic (budget relies on skip_serializing_if)
- QR code content wrong when `--share` + `--qr` combined

**Archive:** `milestones/v1.1-ROADMAP.md`, `milestones/v1.1-REQUIREMENTS.md`

---


## v1.2 Dependency Audit & Code Quality (Shipped: 2026-02-24)

**Phases:** 11-13 (5 plans) | **Rust LOC:** 2,867 | **Timeline:** 2 days

**Delivered:** Code quality hardening -- dependency audit, CI enforcement gates, PIN strength validation, and tech debt cleanup.

**Key accomplishments:**
- Fixed all clippy warnings, applied rustfmt, documented ed25519-dalek pre-release pin constraint
- Replaced unmaintained `backoff` crate with `backon` -- eliminated RUSTSEC-2025-0012 and RUSTSEC-2024-0384
- Added parallel lint (clippy + fmt) and audit jobs to CI -- three-job pipeline on every push/PR
- PIN strength validation at publish time -- 4 rules (min 8 chars, all-same, sequential, common word), 15 tests
- Removed dead LatestPointer code and fixed placeholder repo paths to johnzilla/cclink

**Git range:** `3a90895` → `c6d3858` (36 files, 3,547 insertions, 230 deletions)

**Known tech debt:**
- Age ciphertext size non-deterministic (budget relies on skip_serializing_if)
- QR code content wrong when `--share` + `--qr` combined

**Archive:** `milestones/v1.2-ROADMAP.md`, `milestones/v1.2-REQUIREMENTS.md`

---


## v1.3 Key Security Hardening (Shipped: 2026-02-24)

**Phases:** 14-16 (5 plans) | **Rust LOC:** 4,003 | **Timeline:** 1 day

**Delivered:** Secret key material protected at rest with passphrase-encrypted CCLINKEK envelopes and in memory with automatic zeroization on drop.

**Key accomplishments:**
- Memory zeroization of all secret key material (X25519 scalars, decrypted key bytes, passphrase/PIN strings) via `Zeroizing<T>` wrappers
- CCLINKEK binary envelope format with embedded Argon2id params, HKDF-SHA256 domain separation, and age encryption
- `cclink init` passphrase prompt with confirmation, min 8 chars, and `--no-passphrase` plaintext bypass
- Transparent format detection in `load_keypair` -- auto-detects encrypted vs plaintext keys, prompts only when needed
- Full backward compatibility -- existing v1.2 plaintext key files load without any passphrase prompt

**Git range:** `3f46dd2` → `44bc477` (6 files, 583 insertions, 29 deletions)

**Known tech debt:**
- Age ciphertext size non-deterministic (budget relies on skip_serializing_if)
- QR code content wrong when `--share` + `--qr` combined
- Two pre-existing `#[allow(dead_code)]` on homeserver utility functions

**Archive:** `milestones/v1.3-ROADMAP.md`, `milestones/v1.3-REQUIREMENTS.md`

---

