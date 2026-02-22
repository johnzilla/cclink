# Requirements: CCLink

**Defined:** 2026-02-22
**Core Value:** Effortless, secure session handoff between devices: `cclink` on one machine, `cclink pickup` on another, you're back in your session.

## v1.1 Requirements

Requirements for v1.1 Security Hardening & Code Review Fixes. Each maps to roadmap phases.

### Security

- [x] **SEC-01**: Handoff payload signs burn and recipient fields (clean break from v1.0 unsigned format)
- [x] **SEC-02**: Key file permissions (0600) enforced explicitly in cclink code, not just delegated to pkarr
- [ ] **SEC-03**: User can publish PIN-protected handoff (`--pin`) with Argon2id+HKDF-derived encryption key

### Functional

- [ ] **FUNC-01**: `--burn` and `--share` are mutually exclusive (CLI errors if both specified)
- [ ] **FUNC-02**: Self-publish success message shows correct pickup command (not raw token)
- [ ] **FUNC-03**: HomeserverClient reuses session cookies instead of signing in on every operation

### Code Quality

- [x] **QUAL-01**: `human_duration` extracted to shared utility (no duplication across commands)
- [x] **QUAL-02**: Error handling uses structured `CclinkError` variants instead of string matching on "404"/"not found"
- [x] **QUAL-03**: Dead `CclinkError` variants removed (InvalidKeyFormat, KeyCorrupted, RecordDeserializationFailed, HandoffExpired, NetworkRetryExhausted)
- [ ] **QUAL-04**: List command fetches records efficiently (not N+1 individual HTTP requests)

### Documentation

- [ ] **DOCS-01**: PRD updated to reflect `~/.pubky/` paths instead of stale `~/.cclink/keys` references

## Future Requirements

### Deferred from v1.0

- **PUB-07**: Override inferred project label via `--project` flag

## Out of Scope

| Feature | Reason |
|---------|--------|
| Burn-after-read for shared records | Homeserver can't support delegated delete; --burn + --share now mutually exclusive |
| Team/shared namespace handoffs | v2, not needed for single-user flow |
| Web UI at cclink.dev | Optional polish, CLI-first |
| Claude Code hook/plugin integration | Future consideration |
| v1/v2 record version negotiation | Clean break chosen; v1.0 records expire via TTL |
| --project flag override | Deferred, not in code review scope |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| SEC-01 | Phase 6 | Complete |
| SEC-02 | Phase 6 | Complete |
| SEC-03 | Phase 9 | Pending |
| FUNC-01 | Phase 8 | Pending |
| FUNC-02 | Phase 8 | Pending |
| FUNC-03 | Phase 7 | Pending |
| QUAL-01 | Phase 7 | Complete |
| QUAL-02 | Phase 7 | Complete |
| QUAL-03 | Phase 7 | Complete |
| QUAL-04 | Phase 7 | Pending |
| DOCS-01 | Phase 8 | Pending |

**Coverage:**
- v1.1 requirements: 11 total
- Mapped to phases: 11
- Unmapped: 0 ✓

---
*Requirements defined: 2026-02-22*
*Last updated: 2026-02-22 — traceability populated after roadmap creation*
