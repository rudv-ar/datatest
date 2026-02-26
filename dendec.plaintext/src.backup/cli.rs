use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// dendec — DNA Encode/Decode
///
/// Encodes arbitrary Unicode text or raw binary files into a DNA base
/// sequence (A/T/G/C) and decodes them back. All data is password-protected
/// with modern authenticated encryption (ChaCha20-Poly1305 + Argon2id).
#[derive(Parser, Debug)]
#[command(
    name = "dendec",
    author,
    version,
    about = "Password-based encrypted Unicode ↔ DNA encoding",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Encode text or a file into an encrypted DNA sequence
    ///
    /// Provide either inline text as a positional argument, or a file
    /// path via --file. Using --file reads raw bytes directly, preserving
    /// exact content including trailing newlines and binary data.
    ///
    /// Examples:
    ///   dendec encode "Hello"
    ///   dendec encode --file src/main.rs --as main.rs.dna
    Encode {
        /// Inline text to encode. Omit when using --file.
        text: Option<String>,

        /// Read input from this file path (binary-safe, raw bytes)
        #[arg(short, long, value_name = "PATH")]
        file: Option<PathBuf>,

        /// Write DNA output to this file instead of stdout
        #[arg(long = "as", value_name = "PATH")]
        save_as: Option<PathBuf>,

        /// Display DNA output in groups of N bases (default: continuous)
        #[arg(short, long, value_name = "N")]
        group: Option<usize>,
    },

    /// Decode an encrypted DNA sequence back to text or a file
    ///
    /// Provide either the DNA string as a positional argument, or a file
    /// path via --file. Using --as writes the decoded bytes directly to a
    /// file, preserving exact byte content including trailing newlines.
    ///
    /// Examples:
    ///   dendec decode "ATGC..."
    ///   dendec decode --file main.rs.dna --as main.rs
    Decode {
        /// Inline DNA sequence to decode. Omit when using --file.
        dna: Option<String>,

        /// Read DNA input from this file path
        #[arg(short, long, value_name = "PATH")]
        file: Option<PathBuf>,

        /// Write decoded output to this file instead of stdout
        #[arg(long = "as", value_name = "PATH")]
        save_as: Option<PathBuf>,
    },
}
