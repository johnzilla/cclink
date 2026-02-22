/// Pickup command — retrieves the latest handoff from the homeserver, verifies its
/// signature, checks TTL, decrypts the session ID, shows a confirmation prompt,
/// and execs `claude --resume`.
///
/// Self-pickup (no pubkey arg): decrypts with own key. Shows error if record was
/// encrypted for a different recipient (--share record).
/// Cross-user pickup (pubkey arg): attempts decryption with own key. On success,
/// record was shared with us. On failure, shows cleartext metadata and exits.
/// Burn-after-read: on self-pickup of a --burn record, DELETE is called after
/// successful decryption and before exec.
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

/// Run the pickup flow.
pub fn run_pickup(args: crate::cli::PickupArgs) -> anyhow::Result<()> {
    use backoff::{retry, ExponentialBackoff, Error as BackoffError};

    // ── 1. Load keypair and homeserver ────────────────────────────────────
    let keypair = crate::keys::store::load_keypair()?;
    let homeserver = crate::keys::store::read_homeserver()?;
    let client = crate::transport::HomeserverClient::new(&homeserver)?;

    // ── 2. Retrieve record with retry/backoff (RET-06) ────────────────────
    let backoff_config = ExponentialBackoff {
        max_elapsed_time: Some(std::time::Duration::from_secs(30)),
        max_interval: std::time::Duration::from_secs(8),
        initial_interval: std::time::Duration::from_secs(2),
        ..Default::default()
    };

    let is_cross_user = args.pubkey.is_some();
    let pk_z32_opt = args.pubkey.clone();

    let record = retry(backoff_config, || {
        // Get the latest pointer
        let latest_bytes = match client.get_latest(pk_z32_opt.as_deref()) {
            Ok(bytes) => bytes,
            Err(e) => {
                if e.downcast_ref::<crate::error::CclinkError>()
                    .map_or(false, |ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
                {
                    return Err(BackoffError::permanent(e));
                }
                return Err(BackoffError::transient(e));
            }
        };

        // Deserialize the latest pointer to get the token
        let latest: crate::record::LatestPointer =
            serde_json::from_slice(&latest_bytes)
                .map_err(|e| BackoffError::permanent(anyhow::anyhow!("failed to parse latest pointer: {}", e)))?;

        let token = &latest.token;

        // Fetch and verify the full record
        if let Some(ref pk_z32) = pk_z32_opt {
            let parsed_pubkey = pkarr::PublicKey::try_from(pk_z32.as_str())
                .map_err(|e| BackoffError::permanent(anyhow::anyhow!("invalid pubkey: {}", e)))?;

            match client.get_record_by_pubkey(pk_z32, token, &parsed_pubkey) {
                Ok(r) => Ok(r),
                Err(e) => {
                    if e.downcast_ref::<crate::error::CclinkError>()
                        .map_or(false, |ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
                    {
                        Err(BackoffError::permanent(e))
                    } else {
                        Err(BackoffError::transient(e))
                    }
                }
            }
        } else {
            // Self-pickup: sign in first (session cookie needed), then get record
            if let Err(e) = client.signin(&keypair) {
                return Err(BackoffError::transient(e));
            }
            match client.get_record(token, &keypair.public_key()) {
                Ok(r) => Ok(r),
                Err(e) => {
                    if e.downcast_ref::<crate::error::CclinkError>()
                        .map_or(false, |ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
                    {
                        Err(BackoffError::permanent(e))
                    } else {
                        Err(BackoffError::transient(e))
                    }
                }
            }
        }
    })
    .map_err(|e| anyhow::anyhow!("Failed to retrieve handoff after retries: {}", e))?;

    // ── 3. TTL expiry check (RET-03) ──────────────────────────────────────
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

    // ── 4. Decrypt or show metadata ───────────────────────────────────────
    let age_secs = now_secs.saturating_sub(record.created_at);
    let human_age = human_duration(age_secs);

    // Derive token from created_at (matches transport publish() convention)
    let token = record.created_at.to_string();

    let session_id: String;

    if is_cross_user {
        // Cross-user pickup: attempt decryption with own key.
        // If the record was encrypted for us (--share), decryption succeeds.
        // If not (self-encrypted or shared with someone else), show metadata.
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&record.blob)
            .map_err(|e| anyhow::anyhow!("failed to decode blob: {}", e))?;
        let x25519_secret = crate::crypto::ed25519_to_x25519_secret(&keypair);
        let identity = crate::crypto::age_identity(&x25519_secret);

        match crate::crypto::age_decrypt(&ciphertext, &identity) {
            Ok(plaintext) => {
                // Decryption succeeded — this record was shared with us
                session_id = String::from_utf8(plaintext)
                    .map_err(|e| anyhow::anyhow!("session ID is not valid UTF-8: {}", e))?;
            }
            Err(_) => {
                // Cannot decrypt — show cleartext metadata
                println!(
                    "Handoff from {}",
                    record.pubkey.if_supports_color(Stdout, |t| t.cyan())
                );
                println!(
                    "  Host: {}",
                    record.hostname.if_supports_color(Stdout, |t| t.cyan())
                );
                println!(
                    "  Project: {}",
                    record.project.if_supports_color(Stdout, |t| t.cyan())
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

        // Check if this is the publisher's own --share record (encrypted for a different recipient)
        if let Some(ref intended_recipient) = record.recipient {
            // Publisher trying to pick up their own --share record — cannot decrypt
            eprintln!(
                "{}",
                format!(
                    "Error: This handoff was shared with {}. Only the recipient can decrypt it.",
                    intended_recipient
                )
                .if_supports_color(Stdout, |t| t.red())
            );
            println!("  Host: {}", record.hostname);
            println!("  Project: {}", record.project);
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
        session_id = String::from_utf8(plaintext)
            .map_err(|e| anyhow::anyhow!("session ID is not valid UTF-8: {}", e))?;
    }

    // ── 5. Burn-after-read (ENC-02) ───────────────────────────────────────
    // Only attempt DELETE on self-pickup: we have auth (session cookie from signin above).
    // Cross-user pickup cannot auth to delete the publisher's record.
    // DELETE must happen BEFORE exec (which replaces the process).
    if record.burn && !is_cross_user {
        if let Err(e) = client.delete_record(&token) {
            eprintln!(
                "{}",
                format!("Warning: burn deletion failed: {}", e)
                    .if_supports_color(Stdout, |t| t.yellow())
            );
        }
    }

    // ── 6. Confirmation prompt (RET-04) ───────────────────────────────────
    let skip_confirm = args.yes || !std::io::stdin().is_terminal();
    if !skip_confirm {
        let confirmed = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Resume session {} ({}) published {} ago?",
                &session_id[..8.min(session_id.len())],
                record.project,
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

    // ── 7. Optional QR code (RET-05) ──────────────────────────────────────
    if args.qr {
        qr2term::print_qr(&session_id)
            .map_err(|e| anyhow::anyhow!("QR code render failed: {}", e))?;
    }

    // ── 8. Launch claude --resume (RET-04) ────────────────────────────────
    println!(
        "{}",
        format!("Resuming session {}...", &session_id[..8.min(session_id.len())])
            .if_supports_color(Stdout, |t| t.green())
    );
    launch_claude_resume(&session_id)?;

    Ok(())
}

