# Requirements: CCLink

**Defined:** 2026-02-24
**Core Value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.

## v1.3 Requirements

Requirements for v1.3 Key Security Hardening. Each maps to roadmap phases.

### Encrypted Key Storage

- [ ] **KEYS-01**: User can create a passphrase-protected keypair with `cclink init` (passphrase prompt with confirmation, min 8 chars)
- [ ] **KEYS-02**: User can create an unprotected keypair with `cclink init --no-passphrase`
- [ ] **KEYS-03**: User is prompted for passphrase when any command loads an encrypted keypair
- [ ] **KEYS-04**: User sees clear "Wrong passphrase" error on incorrect passphrase (exit 1, no retry)
- [ ] **KEYS-05**: Encrypted key file uses self-describing format (JSON envelope with version, salt, ciphertext)
- [ ] **KEYS-06**: Encrypted key file preserves 0600 permissions

### Memory Zeroization

- [ ] **ZERO-01**: Derived X25519 secret scalar is zeroized from memory after use
- [ ] **ZERO-02**: Decrypted key file bytes are zeroized from memory after parsing
- [ ] **ZERO-03**: Passphrase and PIN strings from user prompts are zeroized from memory after use

## Future Requirements

Deferred to future release. Tracked but not in current roadmap.

### Key Management

- **KMGT-01**: Auto-detect plaintext v1.0-v1.2 keys and offer one-time migration to encrypted format
- **KMGT-02**: User can change key passphrase without regenerating keypair (`cclink rekey`)
- **KMGT-03**: User can provide passphrase via `CCLINK_PASSPHRASE` env var for CI/scripting

### System Keystore

- **KSTR-01**: Passphrase cached in macOS Keychain for session duration
- **KSTR-02**: Passphrase cached in Freedesktop Secret Service (Linux) for session duration
- **KSTR-03**: Passphrase cached in Windows Credential Store for session duration
- **KSTR-04**: Graceful fallback to passphrase prompt when keystore unavailable (headless Linux)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| In-process passphrase caching | Increases attack surface; system keystore handles this correctly |
| Encrypt key with user's SSH key | Cross-dependency on SSH key lifecycle; passphrase encryption is self-contained |
| Zeroize `pkarr::Keypair` struct | `ed25519_dalek::SigningKey` already implements `ZeroizeOnDrop` internally |
| 256 MB Argon2id memory | 64 MB (existing params) is already overkill; 256 MB stalls 3-4s on modest hardware |
| Derive key-encryption key from Ed25519 key itself | Circular — if you have the key, you don't need to decrypt it |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| KEYS-01 | Phase 16 | Pending |
| KEYS-02 | Phase 16 | Pending |
| KEYS-03 | Phase 16 | Pending |
| KEYS-04 | Phase 16 | Pending |
| KEYS-05 | Phase 15 | Pending |
| KEYS-06 | Phase 16 | Pending |
| ZERO-01 | Phase 14 | Pending |
| ZERO-02 | Phase 14 | Pending |
| ZERO-03 | Phase 14 | Pending |

**Coverage:**
- v1.3 requirements: 9 total
- Mapped to phases: 9
- Unmapped: 0

---
*Requirements defined: 2026-02-24*
*Last updated: 2026-02-24 after roadmap creation*
