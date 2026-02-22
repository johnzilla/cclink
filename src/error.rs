use thiserror::Error;

#[derive(Error, Debug)]
pub enum CclinkError {
    #[error("No keypair found. Run `cclink init` first.")]
    NoKeypairFound,

    #[error("Failed to write key file atomically")]
    AtomicWriteFailed(#[source] std::io::Error),

    #[error("Cannot determine home directory")]
    HomeDirNotFound,

    #[error("Signature verification failed: {0}")]
    SignatureVerificationFailed(String),

    #[error("No Claude Code session found. Start a session with 'claude' first.")]
    SessionNotFound,

    #[error("Record not found")]
    RecordNotFound,
}
