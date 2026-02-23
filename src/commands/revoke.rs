/// Revoke command — publishes an empty SignedPacket to revoke the active handoff.
use std::io::IsTerminal;

use owo_colors::{OwoColorize, Stream::Stdout};

/// Revoke the active handoff record from the DHT.
///
/// Resolves the current record to show details in the confirmation prompt,
/// then publishes an empty SignedPacket to overwrite it. The `token` and `--all`
/// args are accepted but ignored (one record per identity on the DHT).
pub fn run_revoke(args: crate::cli::RevokeArgs) -> anyhow::Result<()> {
    // ── 1. Load keypair ──────────────────────────────────────────────────
    let keypair = crate::keys::store::load_keypair()?;
    let own_z32 = keypair.public_key().to_z32();
    let client = crate::transport::DhtClient::new()?;

    // ── 2. Resolve current record ────────────────────────────────────────
    let record = match client.resolve_record(&own_z32) {
        Ok(r) => r,
        Err(e) => {
            if e.downcast_ref::<crate::error::CclinkError>()
                .is_some_and(|ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
            {
                println!("No active handoffs.");
                return Ok(());
            }
            return Err(e);
        }
    };

    // ── 3. Confirmation prompt ───────────────────────────────────────────
    let skip_confirm = args.yes || !std::io::stdin().is_terminal();
    if !skip_confirm {
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Revoke handoff for {}?",
                record.project
            ))
            .default(false)
            .interact()
            .map_err(|e| anyhow::anyhow!("prompt failed: {}", e))?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    // ── 4. Revoke by publishing empty packet ─────────────────────────────
    client.revoke(&keypair)?;
    println!(
        "{} ({})",
        "Revoked.".if_supports_color(Stdout, |t| t.green()),
        record.project
    );

    Ok(())
}
