/// wrap/mod.rs — Orchestration for the wrap subcommand
///
/// Ties together fetch, snapshot, classify, and transform into the
/// full wrap pipeline:
///
///   encode:
///     snapshot → run command → diff → encode new files → report
///
///   decode:
///     snapshot → run command → diff → decode .dna files → report
pub mod classify;
pub mod fetch;
pub mod snapshot;
pub mod transform;

use std::path::PathBuf;

use crate::error::{DendecError, Result};
use fetch::{git_clone_target, run_command, writes_to_disk};
use snapshot::Snapshot;
use transform::{decode_files, encode_files, print_summary};

/// Entry point for `dendec wrap -e <command>` and `dendec wrap -d <command>`.
pub fn run_wrap(encode_mode: bool, command: &[String], password: &str) -> Result<()> {
    let to_disk = writes_to_disk(command);

    // ── Determine scan root ───────────────────────────────────────
    // For git clone, the scan root is the cloned directory.
    // For everything else, scan the current working directory.
    let cwd = std::env::current_dir().map_err(DendecError::Io)?;

    let is_git_clone = command.first().map(|s| s == "git").unwrap_or(false)
        && command.get(1).map(|s| s == "clone").unwrap_or(false);

    // ── Snapshot before ──────────────────────────────────────────
    let before = Snapshot::capture(&cwd);

    // ── Run the command ──────────────────────────────────────────
    let result = run_command(command, !to_disk)?;

    // ── Handle stdout-output commands ────────────────────────────
    // If the command wrote to stdout (e.g. bare curl), handle inline.
    if let Some(stdout_bytes) = result.stdout_bytes {
        return handle_stdout_output(encode_mode, stdout_bytes, password);
    }

    // ── Snapshot after ───────────────────────────────────────────
    let after = Snapshot::capture(&cwd);
    let changed: Vec<PathBuf> = before.diff(&after).into_iter().cloned().collect();

    if changed.is_empty() {
        return Err(DendecError::WrapNoFilesFound);
    }

    // ── For git clone, narrow scan to the cloned directory ───────
    let files_to_process: Vec<PathBuf> = if is_git_clone {
        if let Some(target) = git_clone_target(command) {
            let target_abs = cwd.join(&target);
            changed
                .into_iter()
                .filter(|p| p.starts_with(&target_abs))
                .collect()
        } else {
            changed
        }
    } else {
        changed
    };

    eprintln!();

    // ── Transform ─────────────────────────────────────────────────
    if encode_mode {
        eprintln!("Encoding {} file(s)...", files_to_process.len());
        eprintln!();
        let summary = encode_files(&files_to_process, password);
        print_summary(&summary, "encode");

        if summary.failed > 0 {
            return Err(DendecError::WrapFileFailed {
                path: PathBuf::from("<multiple>"),
                reason: format!("{} file(s) failed to encode", summary.failed),
            });
        }
    } else {
        eprintln!("Decoding {} file(s)...", files_to_process.len());
        eprintln!();
        let summary = decode_files(&files_to_process, password);
        print_summary(&summary, "decode");

        if summary.failed > 0 {
            return Err(DendecError::WrapFileFailed {
                path: PathBuf::from("<multiple>"),
                reason: format!("{} file(s) failed to decode", summary.failed),
            });
        }
    }

    Ok(())
}

/// Handle the case where the wrapped command wrote to stdout.
///
/// Encode mode: the stdout bytes are plain text — encode and print as DNA.
/// Decode mode: the stdout bytes should be a DNA string — decode and print.
fn handle_stdout_output(encode_mode: bool, bytes: Vec<u8>, password: &str) -> Result<()> {
    use crate::encoding::{decode_raw, encode_raw};

    if encode_mode {
        eprintln!("Encoding stdout output...");
        let dna = encode_raw(&bytes, password, None)?;
        println!("{dna}");
    } else {
        eprintln!("Decoding stdout output...");
        let dna_string = String::from_utf8(bytes)
            .map_err(|e| DendecError::Utf8(e))?;
        let plaintext = decode_raw(&dna_string, password)?;
        let text = String::from_utf8(plaintext)
            .map_err(DendecError::Utf8)?;
        print!("{text}");
    }

    Ok(())
}
