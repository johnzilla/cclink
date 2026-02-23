/// List command — displays all active handoff records from the homeserver.
use owo_colors::{OwoColorize, Stream::Stdout};

use crate::util::human_duration;

/// List all active handoff records on the homeserver.
///
/// Fetches all tokens, retrieves each record, filters out expired ones, and renders
/// a comfy-table with token (truncated), project, age, TTL remaining, burn flag,
/// and recipient pubkey. Empty state prints a friendly message.
pub fn run_list() -> anyhow::Result<()> {
    use comfy_table::{Cell, Color, Table};

    // ── 1. Load keypair and homeserver ────────────────────────────────────
    let keypair = crate::keys::store::load_keypair()?;
    let homeserver = crate::keys::store::read_homeserver()?;
    let client = crate::transport::HomeserverClient::new(&homeserver, &keypair.public_key().to_z32())?;

    // ── 2. Sign in (lazy — will only POST /session once per client) ──────
    client.signin(&keypair)?;

    // ── 3. Fetch all records in one transport call, filter expired ────────
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let all_records = client.get_all_records(&keypair.public_key())?;

    let active_records: Vec<(String, crate::record::HandoffRecord)> = all_records
        .into_iter()
        .filter(|(_, record)| {
            let expires_at = record.created_at.saturating_add(record.ttl);
            now_secs < expires_at
        })
        .collect();

    if active_records.is_empty() {
        println!(
            "{}",
            "No active handoffs. Publish one with cclink."
                .if_supports_color(Stdout, |t| t.yellow())
        );
        return Ok(());
    }

    // ── 4. Build and render comfy-table ───────────────────────────────────
    let mut table = Table::new();
    table.set_header(vec!["Token", "Project", "Age", "TTL Left", "Burn", "Recipient"]);

    for (token, record) in &active_records {
        let token_display = if token.len() > 8 { &token[..8] } else { token.as_str() };
        let age_secs = now_secs.saturating_sub(record.created_at);
        let ttl_left = record
            .created_at
            .saturating_add(record.ttl)
            .saturating_sub(now_secs);
        let burn_display = if record.burn { "yes" } else { "" };
        let recipient_display = record.recipient.as_deref().unwrap_or("");
        let recipient_short = if recipient_display.len() > 8 {
            &recipient_display[..8]
        } else {
            recipient_display
        };

        table.add_row(vec![
            Cell::new(token_display),
            Cell::new(&record.project),
            Cell::new(&human_duration(age_secs)),
            Cell::new(&human_duration(ttl_left)),
            if record.burn {
                Cell::new(burn_display).fg(Color::Yellow)
            } else {
                Cell::new(burn_display)
            },
            Cell::new(recipient_short),
        ]);
    }

    println!("{table}");

    Ok(())
}

