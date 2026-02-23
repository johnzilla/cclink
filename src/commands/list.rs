/// List command — displays the active handoff record from the DHT.
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::util::human_duration;

/// Show the active handoff record on the DHT.
///
/// Resolves the current identity's SignedPacket, extracts the HandoffRecord,
/// checks expiry, and renders a comfy-table with one row.
pub fn run_list() -> anyhow::Result<()> {
    use comfy_table::{Cell, Color, Table};

    // ── 1. Load keypair ──────────────────────────────────────────────────
    let keypair = crate::keys::store::load_keypair()?;
    let own_z32 = keypair.public_key().to_z32();
    let client = crate::transport::DhtClient::new()?;

    // ── 2. Resolve record from DHT ───────────────────────────────────────
    let record = match client.resolve_record(&own_z32) {
        Ok(r) => r,
        Err(e) => {
            if e.downcast_ref::<crate::error::CclinkError>()
                .is_some_and(|ce| matches!(ce, crate::error::CclinkError::RecordNotFound))
            {
                println!(
                    "{}",
                    "No active handoffs. Publish one with cclink."
                        .if_supports_color(Stdout, |t| t.yellow())
                );
                return Ok(());
            }
            return Err(e);
        }
    };

    // ── 3. Check expiry ──────────────────────────────────────────────────
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let expires_at = record.created_at.saturating_add(record.ttl);
    if now_secs >= expires_at {
        println!(
            "{}",
            "No active handoffs. Publish one with cclink."
                .if_supports_color(Stdout, |t| t.yellow())
        );
        return Ok(());
    }

    // ── 4. Build and render comfy-table ──────────────────────────────────
    let mut table = Table::new();
    table.set_header(vec!["Project", "Age", "TTL Left", "Burn", "Recipient"]);

    let age_secs = now_secs.saturating_sub(record.created_at);
    let ttl_left = expires_at.saturating_sub(now_secs);
    let burn_display = if record.burn { "yes" } else { "" };
    let recipient_display = record.recipient.as_deref().unwrap_or("");
    let recipient_short = if recipient_display.len() > 8 {
        &recipient_display[..8]
    } else {
        recipient_display
    };

    table.add_row(vec![
        Cell::new(&record.project),
        Cell::new(human_duration(age_secs)),
        Cell::new(human_duration(ttl_left)),
        if record.burn {
            Cell::new(burn_display).fg(Color::Yellow)
        } else {
            Cell::new(burn_display)
        },
        Cell::new(recipient_short),
    ]);

    println!("{table}");

    Ok(())
}
