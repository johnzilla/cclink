/// Pickup command — retrieves the handoff from the PKARR DHT, verifies its
/// signature, checks TTL, decrypts the session ID, shows a confirmation prompt,
/// and execs `claude --resume`.
///
/// Self-pickup (no pubkey arg): resolves own public key from the DHT.
/// Cross-user pickup (pubkey arg): resolves the specified public key.
/// Burn-after-read: on self-pickup of a --burn record, publishes an empty packet
/// to revoke the record before exec.
use std::io::IsTerminal;
use std::time::SystemTime;

use base64::Engine;
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::util::human_duration;

/// Launch `claude --resume <session_id>`.
///
/// On Unix, replaces the current process via `exec()` so the shell history entry
/// is for `cclink`, not `claude`. On non-Unix, spawns a child and waits.
fn launch_claude_resume(session_id: &str) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new("claude");
    cmd.arg("--resume").arg(session_id);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        // exec() only returns if it failed
        Err(anyhow::anyhow!("failed to exec claude: {}", err))
    }
    #[cfg(not(unix))]
    {
        let status = cmd.status()?;
        if !status.success() {
            anyhow::bail!("claude exited with status {}", status);
        }
        Ok(())
    }
}

/// Parse decrypted blob as Payload JSON (new format) or raw session_id (old format).
fn parse_decrypted(
    plaintext: Vec<u8>,
    record: &crate::record::HandoffRecord,
) -> anyhow::Result<(String, String)> {
    if let Ok(payload) = serde_json::from_slice::<crate::record::Payload>(&plaintext) {
        Ok((payload.session_id, payload.project))
    } else {
        // Old format: raw session_id string, metadata in outer record
        let session_id = String::from_utf8(plaintext)
            .map_err(|e| anyhow::anyhow!("session ID is not valid UTF-8: {}", e))?;
        Ok((session_id, record.project.clone()))
    }
}

/// Run the pickup flow.
pub fn run_pickup(args: crate::cli::PickupArgs) -> anyhow::Result<()> {
    use backoff::{retry, Error as BackoffError, ExponentialBackoff};

    // ── 1. Load keypair ──────────────────────────────────────────────────
    let keypair = crate::keys::store::load_keypair()?;
    let own_z32 = keypair.public_key().to_z32();

    let is_cross_user = args.pubkey.is_some();
    let target_z32 = args.pubkey.as_deref().unwrap_or(&own_z32);

    let client = crate::transport::DhtClient::new()?;

    // ── 2. Retrieve record with retry/backoff ────────────────────────────
    let backoff_config = ExponentialBackoff {
        max_elapsed_time: Some(std::time::Duration::from_secs(30)),
        max_interval: std::time::Duration::from_secs(8),
        initial_interval: std::time::Duration::from_secs(2),
        ..Default::default()
    };

    let target_z32_owned = target_z32.to_string();
    let record = retry(backoff_config, || {
        match client.resolve_record(&target_z32_owned) {
            Ok(r) => Ok(r),
            Err(e) => {
                if e.downcast_ref::<crate::error::CclinkError>()
                    .is_some_and(|ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
                {
                    Err(BackoffError::permanent(e))
                } else {
                    Err(BackoffError::transient(e))
                }
            }
        }
    })
    .map_err(|e| anyhow::anyhow!("Failed to retrieve handoff after retries: {}", e))?;

    // ── 3. TTL expiry check ──────────────────────────────────────────────
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let expires_at = record.created_at.saturating_add(record.ttl);
    if now_secs >= expires_at {
        let expired_secs = now_secs.saturating_sub(expires_at);
        let expired_human = human_duration(expired_secs);
        eprintln!(
            "{}",
            format!(
                "Error: This handoff expired {} ago. Publish a new one with cclink.",
                expired_human
            )
            .if_supports_color(Stdout, |t| t.red())
        );
        anyhow::bail!(
            "This handoff expired {} ago. Publish a new one with cclink.",
            expired_human
        );
    }

    // ── 4. Decrypt or show metadata ──────────────────────────────────────
    let age_secs = now_secs.saturating_sub(record.created_at);
    let human_age = human_duration(age_secs);

    let session_id: String;
    let display_project: String;

    // ── PIN-protected record detection ───────────────────────────────────
    if let Some(ref pin_salt_b64) = record.pin_salt {
        // Non-interactive guard: PIN prompt requires a terminal
        if !std::io::stdin().is_terminal() {
            anyhow::bail!("PIN-protected handoff requires interactive terminal for PIN entry");
        }

        // PIN-protected record: prompt for PIN and decrypt
        let salt_bytes = base64::engine::general_purpose::STANDARD
            .decode(pin_salt_b64)
            .map_err(|e| anyhow::anyhow!("invalid pin_salt base64: {}", e))?;
        let salt: [u8; 32] = salt_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("pin_salt must be exactly 32 bytes"))?;

        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&record.blob)
            .map_err(|e| anyhow::anyhow!("failed to decode blob: {}", e))?;

        let pin = dialoguer::Password::new()
            .with_prompt("Enter PIN")
            .interact()
            .map_err(|e| anyhow::anyhow!("PIN prompt failed: {}", e))?;

        match crate::crypto::pin_decrypt(&ciphertext, &pin, &salt) {
            Ok(plaintext) => {
                let (sid, proj) = parse_decrypted(plaintext, &record)?;
                session_id = sid;
                display_project = proj;
            }
            Err(_) => {
                eprintln!(
                    "{}",
                    "Error: Incorrect PIN. Cannot decrypt this handoff."
                        .if_supports_color(Stdout, |t| t.red())
                );
                anyhow::bail!("Incorrect PIN — decryption failed");
            }
        }
    } else if is_cross_user {
        // Cross-user pickup: attempt decryption with own key.
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&record.blob)
            .map_err(|e| anyhow::anyhow!("failed to decode blob: {}", e))?;
        let x25519_secret = crate::crypto::ed25519_to_x25519_secret(&keypair);
        let identity = crate::crypto::age_identity(&x25519_secret);

        match crate::crypto::age_decrypt(&ciphertext, &identity) {
            Ok(plaintext) => {
                let (sid, proj) = parse_decrypted(plaintext, &record)?;
                session_id = sid;
                display_project = proj;
            }
            Err(_) => {
                // Cannot decrypt — metadata is encrypted in the blob
                println!(
                    "Handoff from {}",
                    record.pubkey.if_supports_color(Stdout, |t| t.cyan())
                );
                println!("  Created: {} ago", human_age);
                if record.recipient.is_some() {
                    println!(
                        "{}",
                        "This handoff was encrypted for a specific recipient. Your key cannot decrypt it."
                            .if_supports_color(Stdout, |t| t.yellow())
                    );
                } else {
                    println!(
                        "{}",
                        "This handoff is self-encrypted. Only the publisher can decrypt it."
                            .if_supports_color(Stdout, |t| t.yellow())
                    );
                }
                return Ok(());
            }
        }
    } else {
        // Self-pickup path

        // Check if this is the publisher's own --share record
        if let Some(ref intended_recipient) = record.recipient {
            eprintln!(
                "{}",
                format!(
                    "Error: This handoff was shared with {}. Only the recipient can decrypt it.",
                    intended_recipient
                )
                .if_supports_color(Stdout, |t| t.red())
            );
            println!("  Created: {} ago", human_age);
            return Ok(());
        }

        // Self-encrypt path: decrypt with own key
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&record.blob)
            .map_err(|e| anyhow::anyhow!("failed to decode blob: {}", e))?;
        let x25519_secret = crate::crypto::ed25519_to_x25519_secret(&keypair);
        let identity = crate::crypto::age_identity(&x25519_secret);
        let plaintext = crate::crypto::age_decrypt(&ciphertext, &identity)?;
        let (sid, proj) = parse_decrypted(plaintext, &record)?;
        session_id = sid;
        display_project = proj;
    }

    // ── 5. Burn-after-read ───────────────────────────────────────────────
    // Only attempt revoke on self-pickup: we have the keypair to sign a new packet.
    // Cross-user pickup cannot revoke the publisher's record.
    if record.burn && !is_cross_user {
        if let Err(e) = client.revoke(&keypair) {
            eprintln!(
                "{}",
                format!("Warning: burn revocation failed: {}", e)
                    .if_supports_color(Stdout, |t| t.yellow())
            );
        }
    }

    // ── 6. Confirmation prompt ───────────────────────────────────────────
    let skip_confirm = args.yes || !std::io::stdin().is_terminal();
    if !skip_confirm {
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Resume session {} ({}) published {} ago?",
                &session_id[..8.min(session_id.len())],
                display_project,
                human_age
            ))
            .default(true)
            .interact()
            .map_err(|e| anyhow::anyhow!("prompt failed: {}", e))?;

        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    // ── 7. Optional QR code ──────────────────────────────────────────────
    if args.qr {
        qr2term::print_qr(&session_id)
            .map_err(|e| anyhow::anyhow!("QR code render failed: {}", e))?;
    }

    // ── 8. Launch claude --resume ────────────────────────────────────────
    println!(
        "{}",
        format!(
            "Resuming session {}...",
            &session_id[..8.min(session_id.len())]
        )
        .if_supports_color(Stdout, |t| t.green())
    );
    launch_claude_resume(&session_id)?;

    Ok(())
}
