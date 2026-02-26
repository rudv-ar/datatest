use std::path::PathBuf;
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

    // ── wrap errors ───────────────────────────────────────────────────
    #[error("Wrap command failed with exit code {code}: {cmd}")]
    WrapCommandFailed { cmd: String, code: i32 },

    #[error("Wrap command produced no transformable files")]
    WrapNoFilesFound,

    #[error("Wrap failed on {path}: {reason}")]
    WrapFileFailed { path: PathBuf, reason: String },

    #[error("Wrap requires either -e (encode) or -d (decode), not both")]
    WrapConflictingFlags,

    #[error("Wrap requires either -e or -d flag")]
    WrapMissingFlag,

    // ── refer errors ──────────────────────────────────────────────────

    /// The embedded table.bin failed magic/version checks or was truncated.
    #[error("Reference table is corrupt or incompatible — reinstall dendec")]
    ReferTableCorrupt,

    /// A BED file line could not be parsed.
    #[error("Invalid BED file: {0}")]
    ReferInvalidBed(String),

    /// A chunk index expected during decode was not found.
    #[error("Chunk {chunk} not found during refer decode — BED file may be incomplete")]
    ReferChunkNotFound { chunk: usize },

    /// A base in the DNA string is not A, T, G, or C.
    #[error("Invalid base in DNA string at position {position}: only A/T/G/C are permitted")]
    ReferInvalidBases { position: usize },

    /// A BED file references an accession not present in the embedded table.
    #[error("Assembly mismatch: expected {expected}, got '{got}' — BED file may be from a different genome build")]
    ReferAssemblyMismatch { expected: String, got: String },
}

pub type Result<T> = std::result::Result<T, DendecError>;

