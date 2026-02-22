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

