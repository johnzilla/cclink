use anyhow::Context;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use crate::error::CclinkError;

pub fn key_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or(CclinkError::HomeDirNotFound)?;
    Ok(home.join(".pubky"))
}

pub fn secret_key_path() -> anyhow::Result<PathBuf> {
    Ok(key_dir()?.join("secret_key"))
}

pub fn homeserver_path() -> anyhow::Result<PathBuf> {
    Ok(key_dir()?.join("cclink_homeserver"))
}

pub fn ensure_key_dir() -> anyhow::Result<()> {
    let dir = key_dir()?;
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create {} directory", dir.display()))?;
    Ok(())
}

/// Write a keypair to disk atomically (write to temp then rename) and set 0600 permissions.
///
/// Uses a temp file in the same directory to ensure atomic replacement on POSIX systems.
/// After a successful rename, the file permissions are explicitly set to 0600 so that
/// the secret key is only readable by the owner — cclink enforces this directly rather
/// than relying on pkarr or the OS umask.
pub fn write_keypair_atomic(keypair: &pkarr::Keypair, dest: &Path) -> anyhow::Result<()> {
    let parent = dest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Key destination path has no parent directory"))?;

    let tmp = parent.join(".secret_key.tmp");

    keypair
        .write_secret_key_file(&tmp)
        .map_err(|e| CclinkError::AtomicWriteFailed(std::io::Error::other(e.to_string())))?;

    if let Err(e) = std::fs::rename(&tmp, dest) {
        // Attempt cleanup of temp file on rename failure
        let _ = std::fs::remove_file(&tmp);
        return Err(CclinkError::AtomicWriteFailed(e).into());
    }

    // Explicitly enforce 0600 after the atomic rename. We do not rely on pkarr or
    // the OS umask to set the correct permissions — cclink owns this guarantee (SEC-02).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set 0600 permissions on {}", dest.display()))?;
    }

    Ok(())
}

#[allow(dead_code)]
pub fn write_homeserver(homeserver: &str) -> anyhow::Result<()> {
    let path = homeserver_path()?;
    std::fs::write(&path, homeserver)
        .with_context(|| format!("Failed to write homeserver to {}", path.display()))?;
    Ok(())
}

#[allow(dead_code)]
pub fn read_homeserver() -> anyhow::Result<String> {
    let default_pk = "ufibwbmed6jeq9k4p583go95wofakh9fwpp4k734trq79pd9u1uy";
    let path = homeserver_path()?;
    if !path.exists() {
        return Ok(default_pk.to_string());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read homeserver from {}", path.display()))?;
    let value = content.trim().to_string();
    // Migration: old installs stored a URL like "https://pubky.app"
    if value.starts_with("http") || value.contains('/') {
        return Ok(default_pk.to_string());
    }
    Ok(value)
}

/// Write a raw binary envelope (e.g. CCLINKEK) to disk atomically with 0600 permissions.
///
/// Follows the same atomic-write pattern as `write_keypair_atomic`: write to a temp
/// file, set permissions, then rename. The envelope bytes are written verbatim (not hex).
// Plan 02 wires this into `cclink init`; suppress dead_code until then.
#[allow(dead_code)]
pub fn write_encrypted_keypair_atomic(envelope: &[u8], dest: &Path) -> anyhow::Result<()> {
    let parent = dest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Key destination path has no parent directory"))?;

    let tmp = parent.join(".secret_key.tmp");

    std::fs::write(&tmp, envelope).map_err(CclinkError::AtomicWriteFailed)?;

    // Set 0600 on temp file BEFORE rename to minimize the insecure window (SEC-02).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).with_context(
            || {
                format!(
                    "Failed to set 0600 permissions on tmp file {}",
                    tmp.display()
                )
            },
        )?;
    }

    if let Err(e) = std::fs::rename(&tmp, dest) {
        // Attempt cleanup of temp file on rename failure
        let _ = std::fs::remove_file(&tmp);
        return Err(CclinkError::AtomicWriteFailed(e).into());
    }

    // Defense-in-depth: set 0600 again on the final dest after rename (same as write_keypair_atomic).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set 0600 permissions on {}", dest.display()))?;
    }

    Ok(())
}

/// Load the keypair from the default secret key path.
///
/// Performs a permission check before reading the key file: if the file has permissions
/// other than 0600 the load is rejected with a clear error message that includes the
/// remediation command. This check is cclink's own enforcement (SEC-02) and is not
/// delegated to pkarr.
///
/// Transparently detects the file format:
/// - CCLINKEK magic bytes → encrypted envelope → prompts for passphrase (interactive)
/// - Otherwise → plaintext hex key → decoded directly with no passphrase prompt
///
/// This provides backward compatibility: existing hex key files load without any
/// change in behavior.
pub fn load_keypair() -> anyhow::Result<pkarr::Keypair> {
    let path = secret_key_path()?;
    if !path.exists() {
        return Err(CclinkError::NoKeypairFound.into());
    }
    // Enforce 0600 permissions before reading key material (SEC-02).
    check_key_permissions(&path)?;

    // Read as raw bytes — CCLINKEK envelopes are binary, not valid UTF-8.
    let raw = std::fs::read(&path)
        .with_context(|| format!("Failed to read key file: {}", path.display()))?;

    if raw.starts_with(b"CCLINKEK") {
        load_encrypted_keypair(&raw)
    } else {
        load_plaintext_keypair(&raw)
    }
}

/// Decode a plaintext hex key file (64 hex chars) into a Keypair.
///
/// Extracted from the original `load_keypair` logic. The raw bytes are decoded
/// to a UTF-8 string, trimmed, validated for length, and hex-decoded into a
/// `Zeroizing<[u8;32]>` seed with no intermediate Vec<u8> allocation.
fn load_plaintext_keypair(raw: &[u8]) -> anyhow::Result<pkarr::Keypair> {
    // Convert bytes to string, wrapping in Zeroizing so the heap buffer is wiped on drop.
    let hex_string =
        Zeroizing::new(String::from_utf8(raw.to_vec()).context("Key file is not valid UTF-8")?);
    let hex_trimmed = hex_string.trim();

    // Validate length: a 32-byte secret key encodes to exactly 64 hex characters.
    if hex_trimmed.len() != 64 {
        anyhow::bail!(
            "Invalid secret key file: expected 64 hex chars, got {}",
            hex_trimmed.len()
        );
    }

    // Decode hex into a Zeroizing seed array — no intermediate Vec<u8> allocation.
    let mut seed = Zeroizing::new([0u8; 32]);
    for i in 0..32 {
        let byte_str = &hex_trimmed[i * 2..i * 2 + 2];
        seed[i] = u8::from_str_radix(byte_str, 16)
            .with_context(|| format!("Invalid hex byte at position {}: '{}'", i * 2, byte_str))?;
    }

    // Construct the keypair from the zeroizing seed; seed and hex_string are zeroed on drop.
    Ok(pkarr::Keypair::from_secret_key(&seed))
}

/// Prompt for a passphrase interactively and decrypt a CCLINKEK envelope.
///
/// Requires an interactive terminal — rejects piped/redirected stdin with a clear
/// error message. On wrong passphrase, prints a user-facing message and exits(1)
/// so the caller never receives an incorrect keypair silently.
fn load_encrypted_keypair(envelope: &[u8]) -> anyhow::Result<pkarr::Keypair> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("Encrypted keypair requires interactive terminal for passphrase entry");
    }
    let passphrase = Zeroizing::new(
        dialoguer::Password::new()
            .with_prompt("Enter key passphrase")
            .interact()
            .map_err(|e| anyhow::anyhow!("Passphrase prompt failed: {}", e))?,
    );
    match load_encrypted_keypair_with_passphrase(envelope, &passphrase) {
        Ok(kp) => Ok(kp),
        Err(_) => {
            eprintln!("Wrong passphrase");
            std::process::exit(1);
        }
    }
}

/// Decrypt a CCLINKEK envelope with the given passphrase and return the Keypair.
///
/// This is the testable core: no I/O, no terminal dependency. Returns `Ok(Keypair)`
/// on success or propagates the `Err` from `decrypt_key_envelope` on failure.
/// The interactive wrapper (`load_encrypted_keypair`) converts the `Err` to an exit(1).
fn load_encrypted_keypair_with_passphrase(
    envelope: &[u8],
    passphrase: &str,
) -> anyhow::Result<pkarr::Keypair> {
    let seed = crate::crypto::decrypt_key_envelope(envelope, passphrase)?;
    Ok(pkarr::Keypair::from_secret_key(&seed))
}

pub fn keypair_exists() -> anyhow::Result<bool> {
    let path = secret_key_path()?;
    Ok(path.exists())
}

/// Check that the key file has exactly 0600 permissions (Unix only).
///
/// Returns an error if the file permissions allow group or other access.
/// This is a security check — secret key files must not be readable by
/// other users on the system. The error message includes the remediation
/// command (`chmod 600 <path>`) so users can fix the issue immediately.
#[cfg(unix)]
pub fn check_key_permissions(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let mode = metadata.permissions().mode() & 0o777;
    if mode != 0o600 {
        anyhow::bail!(
            "Key file {} has insecure permissions {:04o} (expected 0600). Fix with: chmod 600 {}",
            path.display(),
            mode,
            path.display()
        );
    }
    Ok(())
}

/// No-op permission check on non-Unix platforms (Windows, WASM, etc.).
#[cfg(not(unix))]
pub fn check_key_permissions(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::encrypt_key_envelope;

    // ── Encrypted key store tests (Phase 16) ────────────────────────────────

    #[test]
    fn test_write_encrypted_keypair_atomic_creates_cclinkek_file() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("secret_key");
        let keypair = pkarr::Keypair::random();
        let envelope = encrypt_key_envelope(&keypair.secret_key(), "testpass1234")
            .expect("encrypt should succeed");
        write_encrypted_keypair_atomic(&envelope, &path).expect("write should succeed");
        let contents = std::fs::read(&path).expect("Failed to read file");
        assert!(
            contents.starts_with(b"CCLINKEK"),
            "File must start with CCLINKEK magic bytes"
        );
        assert_eq!(
            contents, envelope,
            "File contents must equal the original envelope bytes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_encrypted_keypair_atomic_sets_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("secret_key");
        let keypair = pkarr::Keypair::random();
        let envelope = encrypt_key_envelope(&keypair.secret_key(), "testpass1234")
            .expect("encrypt should succeed");
        write_encrypted_keypair_atomic(&envelope, &path).expect("write should succeed");
        let metadata = std::fs::metadata(&path).expect("Failed to read metadata");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "Expected 0600 permissions, got {:04o}", mode);
    }

    #[test]
    fn test_load_keypair_format_detection_plaintext() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("secret_key");
        let keypair = pkarr::Keypair::random();
        keypair
            .write_secret_key_file(&path)
            .expect("Failed to write key file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .expect("Failed to set permissions");
        }
        let raw_bytes = std::fs::read(&path).expect("Failed to read file");
        let loaded =
            load_plaintext_keypair(&raw_bytes).expect("load_plaintext_keypair should succeed");
        assert_eq!(
            loaded.public_key().to_z32(),
            keypair.public_key().to_z32(),
            "Loaded keypair's public key must match original"
        );
    }

    #[test]
    fn test_load_encrypted_keypair_round_trip() {
        let keypair = pkarr::Keypair::random();
        let envelope = encrypt_key_envelope(&keypair.secret_key(), "testpass1234")
            .expect("encrypt should succeed");
        let loaded = load_encrypted_keypair_with_passphrase(&envelope, "testpass1234")
            .expect("load_encrypted_keypair_with_passphrase should succeed");
        assert_eq!(
            loaded.public_key().to_z32(),
            keypair.public_key().to_z32(),
            "Loaded keypair's public key must match original"
        );
    }

    #[test]
    fn test_load_encrypted_keypair_wrong_passphrase() {
        let keypair = pkarr::Keypair::random();
        let envelope = encrypt_key_envelope(&keypair.secret_key(), "testpass1234")
            .expect("encrypt should succeed");
        let result = load_encrypted_keypair_with_passphrase(&envelope, "wrongpass999");
        assert!(
            result.is_err(),
            "Wrong passphrase must return Err (not a panic)"
        );
    }

    #[test]
    fn test_load_keypair_format_detection_encrypted() {
        let keypair = pkarr::Keypair::random();
        let envelope = encrypt_key_envelope(&keypair.secret_key(), "testpass1234")
            .expect("encrypt should succeed");
        assert!(
            envelope.starts_with(b"CCLINKEK"),
            "Encrypted envelope must start with CCLINKEK magic bytes"
        );
        let plaintext = "ab".repeat(32);
        assert!(
            !plaintext.as_bytes().starts_with(b"CCLINKEK"),
            "Plaintext hex must not start with CCLINKEK (no false positive)"
        );
    }

    // ── Permission tests (existing) ──────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn test_enforce_permissions_rejects_0644() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("secret_key");
        let keypair = pkarr::Keypair::random();
        keypair
            .write_secret_key_file(&path)
            .expect("Failed to write keypair");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("Failed to set permissions");
        let result = check_key_permissions(&path);
        assert!(result.is_err(), "Expected error for 0644 permissions");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("permissions"),
            "Error message should contain 'permissions', got: {}",
            err_msg
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_enforce_permissions_accepts_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("secret_key");
        let keypair = pkarr::Keypair::random();
        keypair
            .write_secret_key_file(&path)
            .expect("Failed to write keypair");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("Failed to set permissions");
        let result = check_key_permissions(&path);
        assert!(
            result.is_ok(),
            "Expected Ok for 0600 permissions, got: {:?}",
            result
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_keypair_atomic_sets_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("secret_key");
        let keypair = pkarr::Keypair::random();
        write_keypair_atomic(&keypair, &path).expect("Failed to write keypair atomically");
        let metadata = std::fs::metadata(&path).expect("Failed to read metadata");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "Expected 0600 permissions after atomic write, got {:04o}",
            mode
        );
    }
}
