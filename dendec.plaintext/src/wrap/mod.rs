/// wrap/mod.rs — Orchestration for the wrap subcommand
///
/// Ties together fetch, snapshot, classify, and transform into the
/// full wrap pipeline:
///
///   encode (local dir):
///     walk directory → encode files → report
///
///   encode (command):
///     snapshot → run command → diff → encode new files → report
///
///   decode (local dir):
///     walk directory → decode .dna files → report
///
///   decode (command):
///     snapshot → run command → diff → decode .dna files → report
pub mod classify;
pub mod fetch;
pub mod snapshot;
pub mod transform;

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::{DendecError, Result};
use fetch::{git_clone_target, run_command, writes_to_disk};
use snapshot::Snapshot;
use transform::{decode_files, encode_files, print_summary};

/// Entry point for `dendec wrap -e <command>` and `dendec wrap -d <command>`.
pub fn run_wrap(encode_mode: bool, command: &[String], password: &str) -> Result<()> {

    // ── Local directory shortcut ──────────────────────────────────
    // If the entire "command" is just a single path to an existing
    // directory, skip subprocess execution and transform it directly.
    // This handles:
    //   dendec wrap -e ./myproject
    //   dendec wrap -d ./myproject
    if command.len() == 1 {
        let candidate = Path::new(&command[0]);
        if candidate.is_dir() {
            return transform_directory(encode_mode, candidate, password);
        }
    }

    // ── Determine command behaviour ───────────────────────────────
    let to_disk = writes_to_disk(command);
    let cwd = std::env::current_dir().map_err(DendecError::Io)?;

    let is_git_clone = command.first().map(|s| s == "git").unwrap_or(false)
        && command.get(1).map(|s| s == "clone").unwrap_or(false);

    // ── Snapshot before ──────────────────────────────────────────
    let before = Snapshot::capture(&cwd);

    // ── Run the command ──────────────────────────────────────────
    let result = run_command(command, !to_disk)?;

    // ── Handle stdout-output commands ────────────────────────────
    // If the command wrote to stdout (e.g. bare curl without -o),
    // handle the bytes directly without touching the filesystem.
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
    // git clone creates a new subdirectory. We only want to process
    // files inside that directory, not anything else that happened
    // to change in cwd during the clone.
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
    run_transform(encode_mode, &files_to_process, password)
}

/// Walk a local directory and transform all appropriate files.
///
/// Used when the user passes a directory path directly instead of a
/// shell command:
///   dendec wrap -e ./myproject
///   dendec wrap -d ./myproject
fn transform_directory(
    encode_mode: bool,
    dir: &Path,
    password: &str,
) -> Result<()> {
    eprintln!("  Scanning {}...", dir.display());

    let files: Vec<PathBuf> = WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    if files.is_empty() {
        return Err(DendecError::WrapNoFilesFound);
    }

    eprintln!();
    run_transform(encode_mode, &files, password)
}

/// Common transform dispatch used by both the command and directory paths.
///
/// Encodes or decodes the given file list, prints progress per file,
/// and prints a summary at the end. Returns an error if any files failed.
fn run_transform(encode_mode: bool, files: &[PathBuf], password: &str) -> Result<()> {
    if encode_mode {
        eprintln!("Encoding {} file(s)...", files.len());
        eprintln!();
        let summary = encode_files(files, password);
        print_summary(&summary, "encode");

        if summary.failed > 0 {
            return Err(DendecError::WrapFileFailed {
                path: PathBuf::from("<multiple>"),
                reason: format!("{} file(s) failed to encode", summary.failed),
            });
        }
    } else {
        eprintln!("Decoding {} file(s)...", files.len());
        eprintln!();
        let summary = decode_files(files, password);
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
/// Encode mode: the stdout bytes are plain content — encode and print as DNA.
/// Decode mode: the stdout bytes should be a DNA string — decode and print.
///
/// This handles bare curl usage:
///   dendec wrap -d curl https://example.com/file.rs.dna
fn handle_stdout_output(encode_mode: bool, bytes: Vec<u8>, password: &str) -> Result<()> {
    use crate::encoding::{decode_raw, encode_raw};

    if encode_mode {
        eprintln!("Encoding stdout output...");
        let dna = encode_raw(&bytes, password, None)?;
        println!("{dna}");
    } else {
        eprintln!("Decoding stdout output...");
        let dna_string = String::from_utf8(bytes)
            .map_err(DendecError::Utf8)?;
        let plaintext = decode_raw(&dna_string, password)?;
        let text = String::from_utf8(plaintext)
            .map_err(DendecError::Utf8)?;
        print!("{text}");
    }

    Ok(())
}
