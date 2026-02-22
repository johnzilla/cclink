# Phase 2: Crypto and Transport - Context

**Gathered:** 2026-02-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the encryption and transport layer: age self-encryption with Ed25519-to-X25519 key derivation, HandoffRecord serialization, Pubky homeserver PUT/GET, latest.json pointer, and Ed25519 signature verification. This is infrastructure consumed by Phase 3's commands. No user-facing CLI commands are added in this phase.

</domain>

<decisions>
## Implementation Decisions

### HandoffRecord design
- No version field — keep minimal, handle format evolution later if needed
- JSON serialization — human-readable, easy to debug
- Split structure: metadata (hostname, project, timestamp, TTL) in cleartext, session ID age-encrypted as a separate blob field
- Include creator's public key as a field in the record — record is self-describing for authorship verification

### Homeserver path layout
- Flat layout: records at `/pub/cclink/<token>`
- Token is timestamp-based (Unix timestamp) — naturally sortable
- Latest pointer at `/pub/cclink/latest`
- latest.json contains token + summary metadata (project, hostname, created_at) so callers can show info without fetching the full record

### Signature & verification model
- Sign everything — signature covers the full record (metadata + encrypted blob) to prevent tampering with any field
- Signature is a base64-encoded field in the JSON record itself
- Signed content is canonical JSON (sorted keys, no whitespace) for deterministic verification regardless of serialization differences
- Hard fail on verification failure — treat record as nonexistent, print error, exit. No bypass flag.

### Crate & dependency choices
- `age` crate (str4d/age) for X25519 age encryption
- Prefer pkarr/pubky crates for Ed25519 signing and Ed25519-to-X25519 key conversion if available, fall back to ed25519-dalek ecosystem
- Prefer pubky-homeserver client crate for HTTP transport if available, fall back to reqwest
- Blocking/synchronous execution — no tokio async runtime. Use ureq or reqwest::blocking if pubky client isn't available.

### Claude's Discretion
- Exact canonical JSON implementation details
- Error type design for the crypto/transport layer
- Internal module organization
- Test fixture design for round-trip encryption tests

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-crypto-and-transport*
*Context gathered: 2026-02-21*
