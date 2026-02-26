use thiserror::Error;

#[derive(Debug, Error)]
pub enum DendecError {
    #[error("Password mismatch: confirmation did not match")]
    PasswordMismatch,

    #[error("Invalid DNA sequence: unexpected character '{0}' at position {1}")]
    InvalidDnaChar(char, usize),

    #[error("Invalid DNA sequence: length {0} is not a multiple of 2")]
    InvalidDnaLength(usize),

    #[error("Missing or corrupted header: magic bytes not found")]
    BadMagic,

    #[error("Unsupported version: expected {expected}, got {got}")]
    UnsupportedVersion { expected: u8, got: u8 },

    #[error("Decryption failed: wrong password or corrupted data")]
    DecryptionFailed,

    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("Data is not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Payload length mismatch: header says {header}, actual {actual}")]
    LengthMismatch { header: usize, actual: usize },
}

pub type Result<T> = std::result::Result<T, DendecError>;

