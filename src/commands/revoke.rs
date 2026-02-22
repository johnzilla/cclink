/// Revoke command — deletes a handoff record (or all records) from the homeserver.
use std::io::IsTerminal;

use owo_colors::{OwoColorize, Stream::Stdout};

/// Revoke a single handoff record or all records on the homeserver.
///
/// Single-token revoke (MGT-02): fetches record details for the confirmation prompt,
/// then deletes the record. If the record is not found or fails verification, still
/// offers to delete (covers corrupted/partially-written records).
///
/// Batch revoke (MGT-03): lists all tokens, shows count in confirmation prompt,
/// then deletes all records in sequence.
///
/// `--yes` / `-y` skips the confirmation prompt in both modes.
pub fn run_revoke(args: crate::cli::RevokeArgs) -> anyhow::Result<()> {
    // ── 1. Load keypair and homeserver ────────────────────────────────────
    let keypair = crate::keys::store::load_keypair()?;
    let homeserver = crate::keys::store::read_homeserver()?;
    let client = crate::transport::HomeserverClient::new(&homeserver)?;
    client.signin(&keypair)?;

    // ── 2. Validate args ──────────────────────────────────────────────────
    if args.token.is_none() && !args.all {
        anyhow::bail!("Provide a token to revoke, or use --all to revoke all handoffs.");
    }

    // ── 3. Handle --all mode (MGT-03) ─────────────────────────────────────
    if args.all {
        let tokens = client.list_record_tokens()?;
        if tokens.is_empty() {
            println!("No active handoffs.");
            return Ok(());
        }
        let count = tokens.len();
        let skip_confirm = args.yes || !std::io::stdin().is_terminal();
        if !skip_confirm {
            let confirmed = dialoguer::Confirm::new()
                .with_prompt(format!(
                    "This will revoke {} active handoff{}. Continue?",
                    count,
                    if count == 1 { "" } else { "s" }
                ))
                .default(false)
                .interact()
                .map_err(|e| anyhow::anyhow!("prompt failed: {}", e))?;
            if !confirmed {
                println!("Aborted.");
                return Ok(());
            }
        }
        for token in &tokens {
            client.delete_record(token)?;
        }
        println!(
            "{}",
            format!(
                "Revoked {} handoff{}.",
                count,
                if count == 1 { "" } else { "s" }
            )
            .if_supports_color(Stdout, |t| t.green())
        );
        return Ok(());
    }

    // ── 4. Handle single-token revoke (MGT-02) ────────────────────────────
    let token = args.token.as_ref().unwrap(); // Safe: validated above

    // Fetch the record to show details in the confirmation prompt
    match client.get_record(token, &keypair.public_key()) {
        Ok(record) => {
            let token_prefix = if token.len() > 8 { &token[..8] } else { token.as_str() };
            let skip_confirm = args.yes || !std::io::stdin().is_terminal();
            if !skip_confirm {
                let confirmed = dialoguer::Confirm::new()
                    .with_prompt(format!(
                        "Revoke handoff {}... ({})?",
                        token_prefix, record.project
                    ))
                    .default(false)
                    .interact()
                    .map_err(|e| anyhow::anyhow!("prompt failed: {}", e))?;
                if !confirmed {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            client.delete_record(token)?;
            println!(
                "{} {} ({})",
                "Revoked.".if_supports_color(Stdout, |t| t.green()),
                token_prefix,
                record.project
            );
        }
        Err(_) => {
            // Record not found or verification failed — try deleting anyway
            // (might be a corrupted record the user still wants removed)
            let skip_confirm = args.yes || !std::io::stdin().is_terminal();
            if !skip_confirm {
                let token_prefix = if token.len() > 8 { &token[..8] } else { token.as_str() };
                let confirmed = dialoguer::Confirm::new()
                    .with_prompt(format!(
                        "Record {}... not found or corrupt. Delete anyway?",
                        token_prefix
                    ))
                    .default(false)
                    .interact()
                    .map_err(|e| anyhow::anyhow!("prompt failed: {}", e))?;
                if !confirmed {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            client.delete_record(token)?;
            println!(
                "{}",
                "Revoked.".if_supports_color(Stdout, |t| t.green())
            );
        }
    }

    Ok(())
}
