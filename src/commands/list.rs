/// List command — displays all active handoff records from the homeserver.
use owo_colors::{OwoColorize, Stream::Stdout};

/// Convert a duration in seconds to a human-readable string.
///
/// >= 3600s → "Xh", >= 60s → "Xm", otherwise → "Xs".
fn human_duration(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h", secs / 3600)
    } else if secs >= 60 {
        format!("{}m", secs / 60)
    } else {
        format!("{}s", secs)
    }
}

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
    let client = crate::transport::HomeserverClient::new(&homeserver)?;

    // ── 2. Sign in and list tokens ────────────────────────────────────────
    client.signin(&keypair)?;
    let tokens = client.list_record_tokens()?;

    if tokens.is_empty() {
        println!(
            "{}",
            "No active handoffs. Publish one with cclink."
                .if_supports_color(Stdout, |t| t.yellow())
        );
        return Ok(());
    }

    // ── 3. Fetch each record, filter expired ─────────────────────────────
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut active_records: Vec<(String, crate::record::HandoffRecord)> = Vec::new();
    for token in &tokens {
        match client.get_record(token, &keypair.public_key()) {
            Ok(record) => {
                let expires_at = record.created_at.saturating_add(record.ttl);
                if now_secs < expires_at {
                    active_records.push((token.clone(), record));
                }
            }
            Err(_) => {
                // Skip records that fail to fetch or verify — they may have been
                // tampered with or partially written. Silent skip is correct.
                continue;
            }
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_duration_seconds() {
        assert_eq!(human_duration(0), "0s");
        assert_eq!(human_duration(1), "1s");
        assert_eq!(human_duration(59), "59s");
    }

    #[test]
    fn test_human_duration_minutes() {
        assert_eq!(human_duration(60), "1m");
        assert_eq!(human_duration(90), "1m");
        assert_eq!(human_duration(3599), "59m");
    }

    #[test]
    fn test_human_duration_hours() {
        assert_eq!(human_duration(3600), "1h");
        assert_eq!(human_duration(7200), "2h");
        assert_eq!(human_duration(86400), "24h");
    }
}
