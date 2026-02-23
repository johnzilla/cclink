/// Revoke command — publishes an empty SignedPacket to revoke the active handoff.
use base64::Engine;
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

    // ── 3. Decrypt project for display ────────────────────────────────────
    let project_display = if record.pin_salt.is_some() {
        "(PIN-protected)".to_string()
    } else if record.recipient.is_some() {
        "(shared)".to_string()
    } else {
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&record.blob)
            .unwrap_or_default();
        let x25519_secret = crate::crypto::ed25519_to_x25519_secret(&keypair);
        let identity = crate::crypto::age_identity(&x25519_secret);
        match crate::crypto::age_decrypt(&ciphertext, &identity) {
            Ok(plaintext) => match serde_json::from_slice::<crate::record::Payload>(&plaintext) {
                Ok(payload) => payload.project,
                Err(_) => record.project.clone(),
            },
            Err(_) => "(encrypted)".to_string(),
        }
    };

    // ── 4. Confirmation prompt ───────────────────────────────────────────
    let skip_confirm = args.yes || !std::io::stdin().is_terminal();
    if !skip_confirm {
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!("Revoke handoff for {}?", project_display))
            .default(false)
            .interact()
            .map_err(|e| anyhow::anyhow!("prompt failed: {}", e))?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    // ── 5. Revoke by publishing empty packet ─────────────────────────────
    client.revoke(&keypair)?;
    println!(
        "{} ({})",
        "Revoked.".if_supports_color(Stdout, |t| t.green()),
        project_display
    );

    Ok(())
}
