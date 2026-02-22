# Phase 4: Advanced Encryption and Management - Context

**Gathered:** 2026-02-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can share handoffs with specific recipients, burn records after read, and manage their published records. Delivers `--share`, `--burn`, `cclink list`, and `cclink revoke`.

</domain>

<decisions>
## Implementation Decisions

### Shared handoffs (--share)
- Recipient specified by z32-encoded public key: `cclink --share <z32_pubkey>`
- Single recipient only per handoff — no multi-recipient support
- Only the recipient's private key can decrypt the handoff (encrypt to recipient's X25519, not publisher's)
- Wrong-recipient pickup shows clear error message plus cleartext metadata (project, hostname, age) so they know what it was even though they can't decrypt it
- Publisher cannot decrypt their own --share handoffs (they don't have the recipient's secret key)

### Burn-after-read (--burn)
- Publisher marks the burn flag at publish time: `cclink --burn`
- After first successful pickup, the record is deleted from the homeserver
- Burned records return "not found" on subsequent pickup — indistinguishable from never-existed or revoked
- Yellow warning printed at publish time so publisher knows it's burn-after-read
- Combinable with --share: `cclink --burn --share <z32_pubkey>` — single recipient, single read

### Record listing (cclink list)
- Columns: token (truncated), project, age, TTL remaining, burn flag, recipient pubkey (if shared)
- Active records only — expired records are excluded, no --all flag
- Colored table output with owo-colors, consistent with existing TTY-aware style
- Empty state: friendly message like "No active handoffs. Publish one with cclink."

### Revocation (cclink revoke)
- `cclink revoke <token>` shows record details and asks "Revoke this handoff? [y/N]" — skip with --yes/-y
- `cclink revoke --all` shows count: "This will revoke N active handoffs. Continue? [y/N]" — user sees blast radius
- Revoked records return "not found" on pickup — same as burned, same as never-existed
- Success output: green "Revoked." with token/project, consistent with publish success style

### Claude's Discretion
- Exact table formatting and column widths for `cclink list`
- How burn-after-read deletion is triggered (inline during pickup vs. deferred)
- Retry behavior for revoke network calls
- Internal data structures for tracking burn/share state in HandoffRecord

</decisions>

<specifics>
## Specific Ideas

- Burn + revoke + not-found should all be indistinguishable from the picker-upper's perspective — no information leakage about why a record is gone
- --yes/-y flag pattern already established in pickup; reuse for revoke consistency
- z32 pubkey format is already used throughout cclink (whoami, etc.) — no new encoding

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 04-advanced-encryption-and-management*
*Context gathered: 2026-02-22*
