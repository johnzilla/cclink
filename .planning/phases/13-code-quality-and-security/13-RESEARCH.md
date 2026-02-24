# Phase 13: Code Quality and Security - Research

**Researched:** 2026-02-24
**Domain:** Rust CLI validation, dead code removal, metadata placeholder replacement
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**PIN rejection UX:**
- Direct and minimal error message style, consistent with existing CLI patterns
- Show the actual character count: e.g. `Error: PIN must be at least 8 characters (got 7)`
- Validation at publish time only — pickup accepts any PIN since the record already exists
- Exit code 1 on rejection (standard error exit)
- Do not publish when PIN is rejected — exit before any network call

**PIN policy rules:**
- Minimum length: 8 characters
- Block all-same character PINs (e.g. `00000000`, `aaaaaaaa`)
- Block sequential patterns (e.g. `12345678`, `abcdefgh`)
- Block a small hardcoded list of common words (~10-20 entries: `password`, `qwerty`, etc.)
- Error message includes specific rejection reason (e.g. `PIN rejected: common word`, `PIN rejected: sequential pattern`)

**Repository metadata:**
- Confirmed org/repo: `johnzilla/cclink`
- Update only the files specified in success criteria: `Cargo.toml` (repository + homepage fields) and `install.sh` (REPO variable)
- Homepage field: `https://github.com/johnzilla/cclink` (same as repository — no separate project site)
- Do not scan for or fix other placeholder references beyond these two files

**Dead code removal:**
- Remove `LatestPointer` struct and its test from `src/record/mod.rs` — exactly what's specified
- Cascade removal: if removing LatestPointer orphans helper functions or imports, remove those too
- Do not perform a broader dead code audit — keep scope to LatestPointer and its dependencies

### Claude's Discretion
- Exact common PIN word list contents (within the ~10-20 entry constraint)
- How to implement the sequential/all-same pattern detection (regex, loop, etc.)
- Whether to extract PIN validation into its own module or keep it inline

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PIN-01 | Enforce minimum 8-character PIN length at publish time with clear error message | PIN validation inserts before the dialoguer::Password prompt in `run_publish`; `cli.pin` is the branch gate; validation logic is pure Rust with no new dependencies |
| DEBT-01 | Fix placeholder `user/cclink` → `johnzilla/cclink` in Cargo.toml and install.sh | Two-line text substitution in two files; no tooling required |
| DEBT-02 | Remove dead `LatestPointer` struct and its serialization test from `src/record/mod.rs` | Struct is annotated `#[allow(dead_code)]` (line 91); test is `test_latest_pointer_serialization` (line 394); no other code references either |
</phase_requirements>

---

## Summary

Phase 13 is composed of three independent, narrowly-scoped tasks: (1) adding PIN strength validation to `run_publish` in `src/commands/publish.rs`, (2) deleting the dead `LatestPointer` struct and its test from `src/record/mod.rs`, and (3) replacing two placeholder strings in `Cargo.toml` and `install.sh`. None of the tasks require new crate dependencies, new files, or structural refactoring.

The PIN validation work is the only task with non-trivial logic. The existing PIN branch in `run_publish` (lines 95-106) prompts for the PIN after reaching step 4 — validation must fire before that prompt so a weak PIN is rejected before any I/O occurs. The three validation rules (length, all-same, sequential, common-word list) are straightforward character-level checks implementable in a small pure function. The existing error reporting pattern in the codebase (`eprintln!` with colored "Error:" prefix + `return Err(...)`) is the correct template to follow.

**Primary recommendation:** Implement PIN validation as a standalone `fn validate_pin(pin: &str) -> Result<(), String>` function, called immediately after the PIN is read from `dialoguer::Password`, returning early with `anyhow::bail!` and a descriptive message matching the locked format. Keep it in `publish.rs` rather than a new module — the function is small and only called once.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust std | — | String/char iteration for pattern detection | No external deps needed for simple character checks |
| anyhow | 1.0 | Error propagation already used throughout the codebase | `anyhow::bail!` is the idiomatic early-return for this project |
| owo-colors | 4 | Colored "Error:" prefix already used in publish.rs | Consistent with existing error output pattern |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| dialoguer | 0.12 | Already handles the PIN prompt — validation inserts after `.interact()` | Already in dependency tree; no change needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Inline validation in publish.rs | New `src/pin_validation.rs` module | Inline is simpler for a single-use, small function; a module adds file overhead with no reuse benefit |
| Loop-based sequential check | Regex | Loop is more readable and avoids needing the `regex` crate |

**Installation:** No new dependencies needed.

---

## Architecture Patterns

### Where Validation Inserts (Exact Location)

In `src/commands/publish.rs`, the PIN branch begins at line 95. The current flow is:

```
Step 4: Build encrypted payload
  if cli.pin {
      let pin = dialoguer::Password::new()...interact()?;  ← PIN is read here
      let (ciphertext, salt) = crate::crypto::pin_encrypt(&payload_bytes, &pin)?;
      ...
  }
```

Validation must run immediately after `interact()` returns and before `pin_encrypt` is called. The correct insertion point is after line 101 (after the PIN is read).

### Pattern 1: Validation Helper Function

**What:** A small pure function that checks the PIN string and returns an informative error string.
**When to use:** Called once, immediately after `dialoguer::Password::interact()`.

```rust
// In src/commands/publish.rs (or extracted to a named fn at module level)

fn validate_pin(pin: &str) -> Result<(), String> {
    let len = pin.len();
    if len < 8 {
        return Err(format!("PIN must be at least 8 characters (got {})", len));
    }
    // All-same character check
    let first = pin.chars().next().unwrap();
    if pin.chars().all(|c| c == first) {
        return Err("PIN rejected: all characters are the same".to_string());
    }
    // Sequential character check (ascending)
    let chars: Vec<char> = pin.chars().collect();
    let is_ascending = chars.windows(2).all(|w| {
        (w[1] as i32) - (w[0] as i32) == 1
    });
    let is_descending = chars.windows(2).all(|w| {
        (w[0] as i32) - (w[1] as i32) == 1
    });
    if is_ascending || is_descending {
        return Err("PIN rejected: sequential pattern".to_string());
    }
    // Common PIN/word list
    const COMMON: &[&str] = &[
        "password", "qwerty", "letmein", "welcome", "monkey",
        "dragon", "master", "iloveyou", "sunshine", "princess",
        "football", "baseball", "123456789", "12345678",
        "87654321", "qwertyui", "asdfghjk",
    ];
    let lower = pin.to_lowercase();
    if COMMON.contains(&lower.as_str()) {
        return Err("PIN rejected: common word or pattern".to_string());
    }
    Ok(())
}
```

**Calling pattern (consistent with existing error style in publish.rs):**
```rust
let pin = dialoguer::Password::new()
    .with_prompt("Enter PIN for this handoff")
    .with_confirmation("Confirm PIN", "PINs don't match")
    .interact()
    .map_err(|e| anyhow::anyhow!("PIN prompt failed: {}", e))?;

if let Err(reason) = validate_pin(&pin) {
    eprintln!(
        "{} {}",
        "Error:".if_supports_color(Stderr, |t| t.red()),
        reason
    );
    std::process::exit(1);
}
```

Note: `std::process::exit(1)` is appropriate here because `anyhow::bail!` propagates an error that may print an additional backtrace-style message through the main error handler — the locked decision specifies "exit code 1 on rejection" with a direct minimal message. Using `process::exit(1)` after the `eprintln!` exactly matches the pattern for clean exit without double-printing. Alternatively, returning `Err(CclinkError::...)` works if main's error handler is transparent — inspect main.rs to confirm which approach is cleaner.

### Pattern 2: Dead Code Removal (LatestPointer)

**What to remove from `src/record/mod.rs`:**

1. Lines 91-106 — the entire `LatestPointer` struct with its `#[allow(dead_code)]` attribute and doc comment:
   ```rust
   #[allow(dead_code)]
   /// A pointer stored at `latest.json` ...
   #[derive(Serialize, Deserialize, Debug, Clone)]
   pub struct LatestPointer {
       pub created_at: u64,
       pub hostname: String,
       pub project: String,
       pub token: String,
   }
   ```

2. Lines 394-410 — the `test_latest_pointer_serialization` test function (within the `mod tests` block).

**Cascade check:** After removing `LatestPointer`, verify:
- No `use` import references `LatestPointer` (none found in the file)
- No other `.rs` file references `LatestPointer` (grep confirms it is not used elsewhere)
- The `Serialize`, `Deserialize`, `Debug`, `Clone` derives remain needed by other structs in the file (they do — `HandoffRecord`, `HandoffRecordSignable`, and `Payload` all use these derives via the existing `use serde::{Deserialize, Serialize}` import)

### Pattern 3: Metadata Placeholder Replacement

Two exact string replacements, no tooling ceremony needed:

**Cargo.toml (lines 7-8):**
```toml
# Before:
repository = "https://github.com/user/cclink"
homepage = "https://github.com/user/cclink"

# After:
repository = "https://github.com/johnzilla/cclink"
homepage = "https://github.com/johnzilla/cclink"
```

**install.sh (line 6):**
```sh
# Before:
REPO="user/cclink"

# After:
REPO="johnzilla/cclink"
```

Also update the comment on line 2 of install.sh:
```sh
# Before:
# Usage: curl -fsSL https://raw.githubusercontent.com/user/cclink/main/install.sh | sh

# After:
# Usage: curl -fsSL https://raw.githubusercontent.com/johnzilla/cclink/main/install.sh | sh
```

### Anti-Patterns to Avoid

- **Validate PIN before the prompt:** The locked decision says "exit before any network call" — but the PIN is collected interactively. Validation must happen after the user types the PIN but before `pin_encrypt` is called, not before the prompt (that would require accepting PIN via a flag, which is a different design).
- **Adding the `regex` crate for sequential detection:** Simple character arithmetic on `chars()` windows is cleaner, avoids a new dependency, and is just as readable.
- **Double error output on rejection:** If using `anyhow::bail!`, ensure main's error handler doesn't add a second "Error: ..." line. The `eprintln!` + `process::exit(1)` approach gives full control.
- **Removing more than LatestPointer:** The decision explicitly says do not perform a broader dead code audit. Only `LatestPointer` and `test_latest_pointer_serialization` are in scope.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Common PIN detection | Complex similarity scoring | Hardcoded `&[&str]` list | ~10-20 entries is tiny; a list is transparent, auditable, zero-overhead |
| Sequential detection | External pattern library | `chars().windows(2)` arithmetic | Two comparisons cover ascending and descending; no library warranted |

**Key insight:** All three validation rules are O(n) string walks with trivial logic. No external crate adds value here.

---

## Common Pitfalls

### Pitfall 1: Validation Fires Too Late (After Network Call)
**What goes wrong:** PIN validation placed after `pin_encrypt` or after `client.publish()` — user types PIN, network call happens, then error fires.
**Why it happens:** Placing validation at the top of step 6 instead of immediately after step 4's `interact()` call.
**How to avoid:** Validate immediately after `.interact()` returns, before any call to `crate::crypto::pin_encrypt`.
**Warning signs:** Test reveals publish succeeds before error is printed.

### Pitfall 2: Wrong Error Format
**What goes wrong:** Error message doesn't match locked format — e.g., "invalid PIN" instead of "PIN must be at least 8 characters (got 7)".
**Why it happens:** Using a generic error type instead of the prescribed message template.
**How to avoid:** Follow the exact format from CONTEXT.md decisions; success criterion 1 explicitly checks `cclink --pin 1234567` prints "a clear error."

### Pitfall 3: Removing Wrong Test Lines
**What goes wrong:** Accidentally removing adjacent test functions when deleting `test_latest_pointer_serialization`.
**Why it happens:** The test function spans lines 394-410 within the `mod tests` block — cutting too many lines.
**How to avoid:** Identify exact line boundaries of the test function; verify the `mod tests` closing brace is still in place after removal.

### Pitfall 4: Cargo.toml comment on line 2 of install.sh missed
**What goes wrong:** `REPO="johnzilla/cclink"` is updated on line 6 but the comment on line 2 still says `user/cclink`.
**Why it happens:** Only searching for `REPO=` not for all occurrences of `user/cclink`.
**How to avoid:** After editing install.sh, grep the file for any remaining `user/cclink` occurrences.

### Pitfall 5: `process::exit(1)` vs `anyhow::bail!`
**What goes wrong:** Using `anyhow::bail!` causes main's error handler to print an additional "Error: ..." line, producing duplicate output.
**Why it happens:** The project's main.rs likely prints the anyhow error chain on non-zero return.
**How to avoid:** Check main.rs error handling behavior. If it prints the error, use `eprintln!` + `process::exit(1)` for PIN rejection to get exactly one clean line. If main is silent about the error value, `anyhow::bail!` is fine.

---

## Code Examples

### Current PIN Branch Location (publish.rs lines 95-106)

```rust
// Source: src/commands/publish.rs
let (blob, pin_salt_value) = if cli.pin {
    // PIN-protected: prompt for PIN, encrypt with PIN-derived key
    let pin = dialoguer::Password::new()
        .with_prompt("Enter PIN for this handoff")
        .with_confirmation("Confirm PIN", "PINs don't match")
        .interact()
        .map_err(|e| anyhow::anyhow!("PIN prompt failed: {}", e))?;

    let (ciphertext, salt) = crate::crypto::pin_encrypt(&payload_bytes, &pin)?;
    let blob = base64::engine::general_purpose::STANDARD.encode(&ciphertext);
    let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);
    (blob, Some(salt_b64))
} else {
    ...
```

Validation inserts between `.interact()?` and `crate::crypto::pin_encrypt(...)`.

### Existing Error Reporting Pattern (publish.rs line 38-42)

```rust
// Source: src/commands/publish.rs — template for PIN error output
eprintln!(
    "{} No Claude Code session found. Start a session with 'claude' first.",
    "Error:".if_supports_color(Stderr, |t| t.red())
);
return Err(CclinkError::SessionNotFound.into());
```

The PIN rejection message follows the same `"Error:".if_supports_color(Stderr, |t| t.red())` prefix pattern.

### LatestPointer Block to Remove (record/mod.rs lines 91-106)

```rust
// REMOVE THIS ENTIRE BLOCK:
#[allow(dead_code)]
/// A pointer stored at `latest.json` that references the most recent HandoffRecord.
///
/// Contains summary metadata so consumers can quickly check freshness without
/// fetching the full record.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LatestPointer {
    /// Unix timestamp (seconds) when the record was created.
    pub created_at: u64,
    /// Hostname of the machine that created the referenced record.
    pub hostname: String,
    /// Project path identifier.
    pub project: String,
    /// Unix timestamp token matching the record path (used to locate the full record).
    pub token: String,
}
```

### LatestPointer Test to Remove (record/mod.rs lines 394-410)

```rust
// REMOVE THIS ENTIRE TEST FUNCTION:
#[test]
fn test_latest_pointer_serialization() {
    let pointer = LatestPointer {
        created_at: 1_700_000_000,
        hostname: "testhost".to_string(),
        project: "/home/user/project".to_string(),
        token: "1700000000".to_string(),
    };

    let json = serde_json::to_string(&pointer).expect("LatestPointer should serialize");
    let deserialized: LatestPointer =
        serde_json::from_str(&json).expect("LatestPointer should deserialize");

    assert_eq!(deserialized.created_at, pointer.created_at);
    assert_eq!(deserialized.hostname, pointer.hostname);
    assert_eq!(deserialized.project, pointer.project);
    assert_eq!(deserialized.token, pointer.token);
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| NIST 800-63B-3 mandatory complexity rules | NIST 800-63B-4 recommends against complexity, recommends length + blocklist | ~2024 | This phase aligns with 800-63B-4: 8-char minimum + blocklist, no character-class requirements |

**Note from REQUIREMENTS.md:** "NIST 800-63B-4 explicitly recommends against mandatory complexity rules" — the current approach of length + blocklist is already aligned with current NIST guidance.

---

## Open Questions

1. **main.rs error handler behavior** — RESOLVED
   - `main()` is declared as `fn main() -> anyhow::Result<()>`, which means anyhow's default error formatter prints the error chain when an `Err` propagates out. Using `anyhow::bail!` for PIN rejection would produce a second line like `Error: PIN must be at least 8 characters (got 7)` from the runtime after the `eprintln!`.
   - **Conclusion:** Use `eprintln!` + `std::process::exit(1)` for PIN rejection. This gives exactly one clean error line and exits with code 1, matching the locked UX decision.

2. **Sequential pattern scope**
   - What we know: The locked decision says block `12345678` (numeric) and `abcdefgh` (alpha) sequential patterns.
   - What's unclear: Whether to block reverse sequences like `87654321` or `hgfedcba`.
   - Recommendation: Block both ascending and descending sequences (the proposed implementation above handles both). This is within Claude's discretion on implementation approach.

---

## Verification Checklist

After all tasks complete, these must be true:

- [ ] `cclink --pin 1234567` prints error with character count and exits non-zero without publishing
- [ ] `cclink --pin 12345678` (valid) proceeds to prompt and publishes normally
- [ ] `cargo test` passes (LatestPointer struct and test removed; remaining tests still green)
- [ ] `Cargo.toml` `repository` and `homepage` fields both contain `https://github.com/johnzilla/cclink`
- [ ] `install.sh` `REPO` variable is `"johnzilla/cclink"`
- [ ] `cargo build` succeeds without warnings about dead code
- [ ] No occurrences of `user/cclink` remain in Cargo.toml or install.sh

---

## Sources

### Primary (HIGH confidence)
- Direct code inspection: `/home/john/vault/projects/github.com/cclink/src/commands/publish.rs` — PIN branch location and error reporting pattern
- Direct code inspection: `/home/john/vault/projects/github.com/cclink/src/record/mod.rs` — LatestPointer struct (lines 91-106) and test (lines 394-410)
- Direct code inspection: `/home/john/vault/projects/github.com/cclink/Cargo.toml` — placeholder values on lines 7-8
- Direct code inspection: `/home/john/vault/projects/github.com/cclink/install.sh` — placeholder values on lines 2 and 6
- `cargo test` run: all 14 tests passing before any changes

### Secondary (MEDIUM confidence)
- REQUIREMENTS.md note: "NIST 800-63B-4 explicitly recommends against mandatory complexity rules" — confirms the approach is standards-aligned

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all implementation uses existing crate dependencies; nothing new needed
- Architecture: HIGH — exact file locations, line numbers, and code patterns verified by direct inspection
- Pitfalls: HIGH — derived from reading the actual code and locked decisions; not speculative

**Research date:** 2026-02-24
**Valid until:** Stable — this is a static codebase with no moving dependencies for this phase
