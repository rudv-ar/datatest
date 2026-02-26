/// main.rs — dendec entry point
///
/// Handles all I/O decisions:
///   - Inline text argument vs --file input
///   - stdout output vs --as file output
///   - Password prompting
///   - Binary-safe file reading and writing
///
/// No crypto or encoding logic lives here.

mod cli;
mod crypto;
mod dna;
mod encoding;
mod error;

use std::fs;
use std::io::Write;
use clap::Parser;
use cli::{Cli, Command};
use error::DendecError;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run() -> error::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Encode { text, file, save_as, group } => {
            // Resolve input: --file reads raw bytes, inline text falls back
            let plaintext: Vec<u8> = match (&file, &text) {
                (Some(path), _) => {
                    // Binary-safe read — preserves trailing newlines,
                    // binary content, and exact byte sequences
                    fs::read(path).map_err(DendecError::Io)?
                }
                (None, Some(t)) => t.as_bytes().to_vec(),
                (None, None) => {
                    eprintln!("Error: provide text as an argument or use --file <PATH>");
                    std::process::exit(1);
                }
            };

            let password = rpassword::prompt_password("Enter password: ")
                .map_err(DendecError::Io)?;
            let confirm = rpassword::prompt_password("Confirm password: ")
                .map_err(DendecError::Io)?;

            if password != confirm {
                return Err(DendecError::PasswordMismatch);
            }
            if password.is_empty() {
                eprintln!("Warning: using an empty password provides no security.");
            }

            eprintln!("Encoding… (Argon2id key derivation may take a moment)");

            let dna = encoding::encode_raw(&plaintext, &password, group)?;

            // Resolve output: --as writes to file, otherwise stdout
            match &save_as {
                Some(path) => {
                    fs::write(path, dna.as_bytes()).map_err(DendecError::Io)?;
                    eprintln!("Written to {}", path.display());
                }
                None => println!("{dna}"),
            }
        }

        Command::Decode { dna, file, save_as } => {
            // Resolve input: --file reads DNA from file, inline arg otherwise
            let dna_string: String = match (&file, &dna) {
                (Some(path), _) => {
                    fs::read_to_string(path).map_err(DendecError::Io)?
                }
                (None, Some(d)) => d.clone(),
                (None, None) => {
                    eprintln!("Error: provide a DNA sequence as an argument or use --file <PATH>");
                    std::process::exit(1);
                }
            };

            let password = rpassword::prompt_password("Enter password: ")
                .map_err(DendecError::Io)?;

            eprintln!("Decoding… (Argon2id key derivation may take a moment)");

            // Always decode to raw bytes — preserves exact content
            let decoded_bytes = encoding::decode_raw(&dna_string, &password)?;

            // Resolve output: --as writes raw bytes to file, otherwise print
            match &save_as {
                Some(path) => {
                    // Write raw bytes — trailing newlines preserved exactly
                    let mut f = fs::File::create(path).map_err(DendecError::Io)?;
                    f.write_all(&decoded_bytes).map_err(DendecError::Io)?;
                    eprintln!("Written to {}", path.display());
                }
                None => {
                    // Print to stdout — interpret as UTF-8 for terminal output
                    let text = String::from_utf8(decoded_bytes)
                        .map_err(DendecError::Utf8)?;
                    print!("{text}");
                }
            }
        }
    }

    Ok(())
}