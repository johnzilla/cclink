# Phase 3: Core Commands - Context

**Gathered:** 2026-02-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Implement the full publish-to-pickup loop: `cclink` discovers and publishes an encrypted Claude Code session handoff to the Pubky homeserver, and `cclink pickup` retrieves, decrypts, and resumes it on another machine. Includes session discovery, TTL enforcement, QR code (opt-in), retry/backoff, colored output, and `--exec` via `claude --resume`.

Advanced features like `--share`, `--burn`, `cclink list`, and `cclink revoke` belong in Phase 4.

</domain>

<decisions>
## Implementation Decisions

### Session discovery
- Auto-detect the most recent Claude Code session — no explicit session ID argument
- If multiple active sessions exist, list them and prompt the user to pick one
- After discovering a session, show the session ID and project/directory before publishing

### Publish output
- Default output: print a copyable `cclink pickup <token>` command and show TTL expiry ("Expires in 24h")
- QR code is opt-in via `--qr` flag — renders as Unicode block characters in the terminal when requested
- Default TTL is 24 hours

### Pickup behavior
- Default action: decrypt the handoff and run `claude --resume <session-id>` automatically
- Before launching, show a confirmation prompt with session ID, project name, and how long ago it was published — user confirms with Y/n
- `--yes` / `-y` flag skips the confirmation and launches immediately

### Error & edge cases
- Expired handoff: clear message — "This handoff expired 3h ago. Publish a new one with cclink."
- No session found: helpful error — "No Claude Code session found. Start a session with 'claude' first."
- Network failures: retry 3 times with exponential backoff, then fail with a clear message

### Colored output
- Auto-detect TTY: colors when outputting to a terminal, plain text when piped
- Green for success, red for errors, yellow for warnings

### Claude's Discretion
- Exact retry backoff intervals
- QR code library/implementation
- Session discovery file paths and detection logic
- Exact layout/formatting of the publish success output
- Confirmation prompt styling

</decisions>

<specifics>
## Specific Ideas

- The publish flow should feel fast and confident — discover session, show what you found, publish, done
- Pickup should feel safe — show what you're about to resume before doing it
- Error messages should guide the user to the fix, not just report the problem

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 03-core-commands*
*Context gathered: 2026-02-22*
