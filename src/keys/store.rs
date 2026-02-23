use anyhow::Context;
use std::path::{Path, PathBuf};

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

pub fn write_homeserver(homeserver: &str) -> anyhow::Result<()> {
    let path = homeserver_path()?;
    std::fs::write(&path, homeserver)
        .with_context(|| format!("Failed to write homeserver to {}", path.display()))?;
    Ok(())
}

pub fn read_homeserver() -> anyhow::Result<String> {
    let default_pk = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";
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

/// Load the keypair from the default secret key path.
///
/// Performs a permission check before reading the key file: if the file has permissions
/// other than 0600 the load is rejected with a clear error message that includes the
/// remediation command. This check is cclink's own enforcement (SEC-02) and is not
/// delegated to pkarr.
pub fn load_keypair() -> anyhow::Result<pkarr::Keypair> {
    let path = secret_key_path()?;
    if !path.exists() {
        return Err(CclinkError::NoKeypairFound.into());
    }
    // Enforce 0600 permissions before reading key material (SEC-02).
    check_key_permissions(&path)?;
    pkarr::Keypair::from_secret_key_file(&path)
        .map_err(|e| anyhow::anyhow!("Failed to load keypair: {}", e))
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
        assert!(result.is_ok(), "Expected Ok for 0600 permissions, got: {:?}", result);
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
            mode,
            0o600,
            "Expected 0600 permissions after atomic write, got {:04o}",
            mode
        );
    }
}
