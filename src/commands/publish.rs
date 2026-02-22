/// Publish command — discovers or uses a specified Claude Code session, encrypts it,
/// signs the record, publishes to the homeserver, and prints colored output.
use std::io::IsTerminal;
use std::time::SystemTime;

use base64::Engine;
use owo_colors::{OwoColorize, Stream::Stderr, Stream::Stdout};

use crate::error::CclinkError;

/// Run the publish flow.
///
/// If `cli.session_id` is `Some`, publish that session directly.
/// Otherwise, discover active sessions and prompt if multiple exist.
pub fn run_publish(cli: &crate::cli::Cli) -> anyhow::Result<()> {
    // ── 1. Load keypair and homeserver ────────────────────────────────────
    let keypair = crate::keys::store::load_keypair()?;
    let homeserver = crate::keys::store::read_homeserver()?;

    // ── 2. Resolve session ────────────────────────────────────────────────
    let session = if let Some(ref id) = cli.session_id {
        // Explicit session ID provided — use it directly
        let project = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        crate::session::SessionInfo {
            session_id: id.clone(),
            project,
            mtime: SystemTime::now(),
        }
    } else {
        // Auto-discover sessions from ~/.claude/projects/, scoped to the
        // current working directory so unrelated project sessions are excluded.
        let cwd = std::env::current_dir().ok();
        let mut sessions = crate::session::discover_sessions(cwd.as_deref())?;
        match sessions.len() {
            0 => {
                // No active session found
                eprintln!(
                    "{} No Claude Code session found. Start a session with 'claude' first.",
                    "Error:".if_supports_color(Stderr, |t| t.red())
                );
                return Err(CclinkError::SessionNotFound.into());
            }
            1 => sessions.remove(0),
            _ => {
                // Multiple sessions — prompt unless stdin is not a TTY
                if !std::io::stdin().is_terminal() {
                    // Non-interactive: use the most recent (index 0, already sorted desc)
                    sessions.remove(0)
                } else {
                    let items: Vec<String> = sessions
                        .iter()
                        .map(|s| {
                            let id_prefix: String = s.session_id.chars().take(8).collect();
                            format!("{} ({})", id_prefix, s.project)
                        })
                        .collect();

                    let selection = dialoguer::Select::new()
                        .with_prompt("Multiple sessions found — pick one")
                        .items(&items)
                        .default(0)
                        .interact()
                        .map_err(|e| anyhow::anyhow!("session selection failed: {}", e))?;

                    sessions.remove(selection)
                }
            }
        }
    };

    // ── 3. Display discovered session ─────────────────────────────────────
    println!(
        "Session: {} in {}",
        session
            .session_id
            .if_supports_color(Stdout, |t| t.cyan()),
        session.project.if_supports_color(Stdout, |t| t.cyan())
    );

    // ── 4. Encrypt session ID with age ────────────────────────────────────
    // --share: encrypt to the specified recipient's X25519 key (ENC-01)
    // Otherwise: self-encrypt to own X25519 key (existing behavior)
    let recipient = if let Some(ref share_pubkey) = cli.share {
        crate::crypto::recipient_from_z32(share_pubkey)?
    } else {
        let x25519_pubkey = crate::crypto::ed25519_to_x25519_public(&keypair);
        crate::crypto::age_recipient(&x25519_pubkey)
    };
    let ciphertext = crate::crypto::age_encrypt(session.session_id.as_bytes(), &recipient)?;
    let blob = base64::engine::general_purpose::STANDARD.encode(&ciphertext);

    // ── 5. Build and sign record ──────────────────────────────────────────
    let created_at = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    let hostname = gethostname::gethostname().to_string_lossy().into_owned();

    let signable = crate::record::HandoffRecordSignable {
        blob,
        burn: cli.burn,
        created_at,
        hostname,
        project: session.project.clone(),
        pubkey: keypair.public_key().to_z32(),
        recipient: cli.share.clone(),
        ttl: cli.ttl,
    };
    let signature = crate::record::sign_record(&signable, &keypair)?;
    let record = crate::record::HandoffRecord {
        blob: signable.blob,
        burn: cli.burn,
        created_at: signable.created_at,
        hostname: signable.hostname,
        project: signable.project,
        pubkey: signable.pubkey,
        recipient: cli.share.clone(),
        signature,
        ttl: signable.ttl,
    };

    // ── 6. Publish to homeserver ──────────────────────────────────────────
    let client = crate::transport::HomeserverClient::new(&homeserver)?;
    let token = client.publish(&keypair, &record)?;

    // ── 7. Output success ─────────────────────────────────────────────────
    if cli.burn {
        println!(
            "{}",
            "Warning: This handoff will be deleted after the first successful pickup."
                .if_supports_color(Stdout, |t| t.yellow())
        );
    }
    println!(
        "\n{}",
        "Published!".if_supports_color(Stdout, |t| t.green())
    );
    if cli.share.is_some() {
        // Shared: recipient needs to specify the publisher's pubkey to pick up
        let own_pubkey = keypair.public_key().to_z32();
        println!("  Recipient pickup command:");
        println!(
            "  {}",
            format!("cclink pickup {}", own_pubkey).if_supports_color(Stdout, |t| t.bold())
        );
    } else {
        // Self: existing pickup command using token
        println!("  Run on another machine:");
        println!(
            "  {}",
            format!("cclink pickup {}", token).if_supports_color(Stdout, |t| t.bold())
        );
    }
    let hours = cli.ttl / 3600;
    println!("  Expires in {}h", hours);

    // ── 8. Optional QR code ───────────────────────────────────────────────
    if cli.qr {
        println!();
        qr2term::print_qr(&format!("cclink pickup {}", token))
            .map_err(|e| anyhow::anyhow!("QR code render failed: {}", e))?;
    }

    Ok(())
}
