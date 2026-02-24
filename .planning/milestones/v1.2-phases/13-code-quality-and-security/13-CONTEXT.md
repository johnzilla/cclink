# Phase 13: Code Quality and Security - Context

**Gathered:** 2026-02-24
**Status:** Ready for planning

<domain>
## Phase Boundary

PIN enforcement prevents weak PINs at publish time, dead DHT migration code (`LatestPointer`) is removed, and real repository metadata replaces placeholders in Cargo.toml and install.sh. No new features — this is cleanup and hardening only.

</domain>

<decisions>
## Implementation Decisions

### PIN rejection UX
- Direct and minimal error message style, consistent with existing CLI patterns
- Show the actual character count: e.g. `Error: PIN must be at least 8 characters (got 7)`
- Validation at publish time only — pickup accepts any PIN since the record already exists
- Exit code 1 on rejection (standard error exit)
- Do not publish when PIN is rejected — exit before any network call

### PIN policy rules
- Minimum length: 8 characters
- Block all-same character PINs (e.g. `00000000`, `aaaaaaaa`)
- Block sequential patterns (e.g. `12345678`, `abcdefgh`)
- Block a small hardcoded list of common words (~10-20 entries: `password`, `qwerty`, etc.)
- Error message includes specific rejection reason (e.g. `PIN rejected: common word`, `PIN rejected: sequential pattern`)

### Repository metadata
- Confirmed org/repo: `johnzilla/cclink`
- Update only the files specified in success criteria: `Cargo.toml` (repository + homepage fields) and `install.sh` (REPO variable)
- Homepage field: `https://github.com/johnzilla/cclink` (same as repository — no separate project site)
- Do not scan for or fix other placeholder references beyond these two files

### Dead code removal
- Remove `LatestPointer` struct and its test from `src/record/mod.rs` — exactly what's specified
- Cascade removal: if removing LatestPointer orphans helper functions or imports, remove those too
- Do not perform a broader dead code audit — keep scope to LatestPointer and its dependencies

### Claude's Discretion
- Exact common PIN word list contents (within the ~10-20 entry constraint)
- How to implement the sequential/all-same pattern detection (regex, loop, etc.)
- Whether to extract PIN validation into its own module or keep it inline

</decisions>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches within the decisions above.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 13-code-quality-and-security*
*Context gathered: 2026-02-24*
