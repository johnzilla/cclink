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

