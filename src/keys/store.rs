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

pub fn write_keypair_atomic(keypair: &pkarr::Keypair, dest: &Path) -> anyhow::Result<()> {
    let parent = dest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Key destination path has no parent directory"))?;

    let tmp = parent.join(".secret_key.tmp");

    keypair
        .write_secret_key_file(&tmp)
        .map_err(|e| CclinkError::AtomicWriteFailed(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;

    if let Err(e) = std::fs::rename(&tmp, dest) {
        // Attempt cleanup of temp file on rename failure
        let _ = std::fs::remove_file(&tmp);
        return Err(CclinkError::AtomicWriteFailed(e).into());
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
    let path = homeserver_path()?;
    if !path.exists() {
        return Ok("https://pubky.app".to_string());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read homeserver from {}", path.display()))?;
    Ok(content.trim().to_string())
}

pub fn load_keypair() -> anyhow::Result<pkarr::Keypair> {
    let path = secret_key_path()?;
    if !path.exists() {
        return Err(CclinkError::NoKeypairFound.into());
    }
    pkarr::Keypair::from_secret_key_file(&path)
        .map_err(|e| anyhow::anyhow!("Failed to load keypair: {}", e))
}

pub fn keypair_exists() -> anyhow::Result<bool> {
    let path = secret_key_path()?;
    Ok(path.exists())
}
