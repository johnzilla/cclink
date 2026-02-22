# Phase 1: Foundation and Key Management - Context

**Gathered:** 2026-02-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Rust project scaffold, Ed25519/PKARR keypair generation, secure key storage, key import, and identity display. This phase delivers `cclink init`, `cclink init --import`, and `cclink whoami`. No network communication, no encryption, no session discovery — those are later phases.

</domain>

<decisions>
## Implementation Decisions

### Key storage layout
- Store keys in `~/.pubky/` directory (reuse existing Pubky tool storage, not a cclink-specific directory)
- Separate files for public key and secret key
- File format: Claude's discretion based on what pkarr/pubky crates expect natively
- Private key file must have 0600 permissions
- Atomic writes: write to temp file then rename (prevent corruption on crash)
- No config file — all configuration via flags or environment variables

### Init experience
- If keys already exist: prompt to confirm overwrite, displaying an identifier for the existing key (fingerprint or short pubkey) so user knows which key they'd be replacing
- Homeserver: default to pubky.app, override with `--homeserver <URL>` flag
- After successful init: show detailed output — public key, homeserver, key file location, and next steps hint
- If user runs `cclink` (publish) without having run init: error with "No keypair found. Run `cclink init` first."

### Import workflow
- Accept key from file path OR stdin: `cclink init --import /path/to/key` or `echo key | cclink init --import -`
- Only accept pkarr-native key format (whatever the pkarr crate uses)
- If imported key is invalid/corrupted: fail with clear error message, don't write anything to disk
- If keys already exist during import: same prompt behavior as regular init (show existing key identifier, confirm overwrite)

### Whoami output
- Display: PKARR public key, homeserver URL, key file path, and short key fingerprint
- Format: labeled human-readable output (e.g., `Public Key: pk:abc123...`)
- Auto-copy public key to clipboard with confirmation message
- If no keys configured: error with "No keypair found. Run `cclink init` first."

### Claude's Discretion
- Key file format on disk (match pkarr crate expectations)
- Exact fingerprint format/length for key identification
- Clipboard library choice
- Exact output formatting and spacing

</decisions>

<specifics>
## Specific Ideas

- Keys should live alongside any existing Pubky tooling keys in `~/.pubky/` — the user wants cclink to feel like part of the Pubky ecosystem, not a standalone tool
- The overwrite prompt should make it obvious which key exists (show some identifier) so the user doesn't accidentally destroy a key they care about

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation-and-key-management*
*Context gathered: 2026-02-21*
