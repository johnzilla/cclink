use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;

use anyhow::Context;
use zeroize::Zeroizing;

use crate::cli::InitArgs;
use crate::keys::{fingerprint, store};

pub fn run_init(args: InitArgs) -> anyhow::Result<()> {
    // Step 1: Ensure ~/.pubky/ directory exists
    store::ensure_key_dir().context("Failed to create ~/.pubky/ directory")?;

    // Step 2: Get the destination path
    let secret_key_path = store::secret_key_path()?;

    // Step 3: Overwrite guard
    if store::keypair_exists()? && !args.yes {
        let should_overwrite = prompt_overwrite(&secret_key_path)?;
        if !should_overwrite {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Step 4: Generate or import keypair
    let (keypair, action) = if let Some(import_path) = &args.import {
        if import_path == "-" {
            let kp = import_from_stdin(&secret_key_path)?;
            (kp, "imported")
        } else {
            let kp = import_from_file(import_path)?;
            (kp, "imported")
        }
    } else {
        let kp = pkarr::Keypair::random();
        (kp, "generated")
    };

    // Step 5: Write keypair — encrypted (default) or plaintext (--no-passphrase)
    if args.no_passphrase {
        // Plaintext path (v1.2-compatible)
        store::write_keypair_atomic(&keypair, &secret_key_path)
            .context("Failed to write keypair")?;
    } else {
        // Encrypted path (v1.3 default)
        if !io::stdin().is_terminal() {
            anyhow::bail!("Use --no-passphrase for non-interactive init");
        }
        let passphrase = Zeroizing::new(
            dialoguer::Password::new()
                .with_prompt("Enter key passphrase (min 8 chars)")
                .with_confirmation("Confirm passphrase", "Passphrases don't match")
                .interact()
                .map_err(|e| anyhow::anyhow!("Passphrase prompt failed: {}", e))?,
        );
        if passphrase.len() < 8 {
            eprintln!("Error: Passphrase must be at least 8 characters");
            std::process::exit(1);
        }
        let seed: [u8; 32] = keypair.secret_key();
        let envelope = crate::crypto::encrypt_key_envelope(&seed, &passphrase)?;
        store::write_encrypted_keypair_atomic(&envelope, &secret_key_path)
            .context("Failed to write encrypted keypair")?;
    }

    // Step 6: Success output
    let pub_key = keypair.public_key();
    let success_verb = if action == "generated" {
        "Keypair generated successfully."
    } else {
        "Keypair imported successfully."
    };

    println!("{}", success_verb);
    println!();
    println!("Public Key:  {}", pub_key.to_uri_string());
    if args.no_passphrase {
        println!("Key file:    {} (plaintext)", secret_key_path.display());
    } else {
        println!(
            "Key file:    {} (passphrase-protected)",
            secret_key_path.display()
        );
    }

    println!();
    println!("Next: run 'cclink' to publish your first session handoff.");

    Ok(())
}

fn prompt_overwrite(existing_key_path: &Path) -> anyhow::Result<bool> {
    // Check if stdin is a terminal — if not, we can't prompt
    if !io::stdin().is_terminal() {
        eprintln!("Use --yes to confirm overwrite in non-interactive mode");
        return Ok(false);
    }

    // Try to load existing key to get a fingerprint identifier
    // If the file starts with the CCLINKEK magic bytes, show "(encrypted)" instead of
    // falling through to the unreadable fallback.
    let identifier = if std::fs::read(existing_key_path)
        .map(|raw| raw.starts_with(b"CCLINKEK"))
        .unwrap_or(false)
    {
        "(encrypted)".to_string()
    } else {
        match pkarr::Keypair::from_secret_key_file(existing_key_path) {
            Ok(kp) => fingerprint::short_fingerprint(&kp.public_key()),
            Err(_) => "(unreadable)".to_string(),
        }
    };

    eprint!(
        "Key {} already exists at {}. Overwrite? [y/N]: ",
        identifier,
        existing_key_path.display()
    );
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

fn import_from_file(path_str: &str) -> anyhow::Result<pkarr::Keypair> {
    let path = Path::new(path_str);
    pkarr::Keypair::from_secret_key_file(path)
        .map_err(|e| anyhow::anyhow!("Invalid key file at {}: {}", path_str, e))
}

fn import_from_stdin(dest_parent: &Path) -> anyhow::Result<pkarr::Keypair> {
    // Read all of stdin
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .context("Failed to read from stdin")?;

    let hex = buf.trim();

    if hex.is_empty() {
        anyhow::bail!("No key data received from stdin");
    }

    // Validate: must be 64 hex characters (32 bytes)
    if hex.len() != 64 {
        anyhow::bail!(
            "Invalid hex format — expected 64 hex characters, got {}",
            hex.len()
        );
    }

    // Ensure all characters are valid hex
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("Invalid hex format — expected 64 hex characters");
    }

    // Write to a temp file and use from_secret_key_file to validate (avoids type ambiguity)
    let parent = dest_parent.parent().unwrap_or_else(|| Path::new("/tmp"));
    let tmp_path = parent.join(".stdin_import.tmp");

    std::fs::write(&tmp_path, hex)
        .with_context(|| format!("Failed to write temp file at {}", tmp_path.display()))?;

    let result = pkarr::Keypair::from_secret_key_file(&tmp_path)
        .map_err(|e| anyhow::anyhow!("Invalid key data from stdin: {}", e));

    // Clean up temp file regardless of success or failure
    let _ = std::fs::remove_file(&tmp_path);

    result
}
