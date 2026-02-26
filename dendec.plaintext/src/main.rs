/// main.rs — dendec entry point

mod cli;
mod crypto;
mod dna;
mod encoding;
mod error;
mod wrap;

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
            let plaintext: Vec<u8> = match (&file, &text) {
                (Some(path), _) => fs::read(path).map_err(DendecError::Io)?,
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

            match &save_as {
                Some(path) => {
                    fs::write(path, dna.as_bytes()).map_err(DendecError::Io)?;
                    eprintln!("Written to {}", path.display());
                }
                None => println!("{dna}"),
            }
        }

        Command::Decode { dna, file, save_as } => {
            let dna_string: String = match (&file, &dna) {
                (Some(path), _) => fs::read_to_string(path).map_err(DendecError::Io)?,
                (None, Some(d)) => d.clone(),
                (None, None) => {
                    eprintln!("Error: provide a DNA sequence as an argument or use --file <PATH>");
                    std::process::exit(1);
                }
            };

            let password = rpassword::prompt_password("Enter password: ")
                .map_err(DendecError::Io)?;

            eprintln!("Decoding… (Argon2id key derivation may take a moment)");
            let decoded_bytes = encoding::decode_raw(&dna_string, &password)?;

            match &save_as {
                Some(path) => {
                    let mut f = fs::File::create(path).map_err(DendecError::Io)?;
                    f.write_all(&decoded_bytes).map_err(DendecError::Io)?;
                    eprintln!("Written to {}", path.display());
                }
                None => {
                    let text = String::from_utf8(decoded_bytes)
                        .map_err(DendecError::Utf8)?;
                    print!("{text}");
                }
            }
        }

        Command::Wrap { encode, decode, command } => {
            // Validate flags
            if encode && decode {
                return Err(DendecError::WrapConflictingFlags);
            }
            if !encode && !decode {
                return Err(DendecError::WrapMissingFlag);
            }

            let password = rpassword::prompt_password("Enter password: ")
                .map_err(DendecError::Io)?;

            if encode {
                let confirm = rpassword::prompt_password("Confirm password: ")
                    .map_err(DendecError::Io)?;
                if password != confirm {
                    return Err(DendecError::PasswordMismatch);
                }
            }

            wrap::run_wrap(encode, &command, &password)?;
        }
    }

    Ok(())
}
