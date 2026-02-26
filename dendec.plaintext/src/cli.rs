use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// dendec — DNA Encode/Decode
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

    /// Run a command and encode or decode all files it produces
    ///
    /// wrap intercepts the output of any shell command and applies a DNA
    /// transform to every appropriate file. Directory structure is preserved
    /// exactly. Binary files are skipped automatically.
    ///
    /// Examples:
    ///   dendec wrap -e git clone https://github.com/user/repo
    ///   dendec wrap -d git clone https://github.com/user/repo
    ///   dendec wrap -e curl -o config.toml https://example.com/config.toml
    ///   dendec wrap -d curl -o config.toml.dna https://example.com/config.toml.dna
    Wrap {
        /// Encode mode — transform files to .dna
        #[arg(short = 'e', long = "encode")]
        encode: bool,

        /// Decode mode — restore files from .dna
        #[arg(short = 'd', long = "decode")]
        decode: bool,

        /// The command to run (everything after the flags)
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
}
