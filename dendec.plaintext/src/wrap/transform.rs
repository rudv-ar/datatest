/// wrap/transform.rs — Batch file encode/decode with progress reporting
///
/// Iterates a list of file paths, classifies each one, and applies
/// encode_raw or decode_raw. Reports per-file progress to stderr.
/// Original files are replaced by .dna files (encode) or vice versa (decode).
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::encoding::{decode_raw, encode_raw};
use crate::error::{DendecError, Result};
use crate::wrap::classify::{classify_for_decode, classify_for_encode, FileClass, SkipReason};

/// Summary of a batch transform operation.
pub struct TransformSummary {
    pub transformed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub failures: Vec<(PathBuf, String)>,
}

/// Encode all appropriate files in `paths` using `password`.
///
/// Each source file is read, encoded, written to `<original>.dna`,
/// and the original is deleted on success.
pub fn encode_files(paths: &[PathBuf], password: &str) -> TransformSummary {
    let mut summary = TransformSummary {
        transformed: 0,
        skipped: 0,
        failed: 0,
        failures: Vec::new(),
    };

    for path in paths {
        match classify_for_encode(path) {
            FileClass::Encode => {
                eprint!("  Encoding {}... ", path.display());
                match encode_file(path, password) {
                    Ok(dna_path) => {
                        let orig_size = fs::metadata(path)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        let dna_size = fs::metadata(&dna_path)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        eprintln!(
                            "ok  ({} → {})",
                            human_size(orig_size),
                            human_size(dna_size)
                        );
                        // Remove original after successful encode
                        if let Err(e) = fs::remove_file(path) {
                            eprintln!("  Warning: could not remove original {}: {e}", path.display());
                        }
                        summary.transformed += 1;
                    }
                    Err(e) => {
                        eprintln!("FAILED: {e}");
                        summary.failed += 1;
                        summary.failures.push((path.clone(), e.to_string()));
                    }
                }
            }
            FileClass::Skip(reason) => {
                let label = match reason {
                    SkipReason::Binary => "binary",
                    SkipReason::AlreadyDna => "already .dna",
                    SkipReason::ExcludedDir => "excluded dir",
                    SkipReason::NotDna => "not .dna",
                    SkipReason::ReadError => "read error",
                };
                eprintln!("  Skipping {}  ({})", path.display(), label);
                summary.skipped += 1;
            }
            FileClass::Decode => {
                // Should not happen in encode mode but handle gracefully
                summary.skipped += 1;
            }
        }
    }

    summary
}

/// Decode all `.dna` files in `paths` using `password`.
///
/// Each `.dna` file is decoded, written to the original path (extension
/// stripped), and the `.dna` file is deleted on success.
pub fn decode_files(paths: &[PathBuf], password: &str) -> TransformSummary {
    let mut summary = TransformSummary {
        transformed: 0,
        skipped: 0,
        failed: 0,
        failures: Vec::new(),
    };

    for path in paths {
        match classify_for_decode(path) {
            FileClass::Decode => {
                eprint!("  Decoding {}... ", path.display());
                match decode_file(path, password) {
                    Ok(out_path) => {
                        let dna_size = fs::metadata(path)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        let out_size = fs::metadata(&out_path)
                            .map(|m| m.len())
                            .unwrap_or(0);
                        eprintln!(
                            "ok  ({} → {})",
                            human_size(dna_size),
                            human_size(out_size)
                        );
                        // Remove .dna file after successful decode
                        if let Err(e) = fs::remove_file(path) {
                            eprintln!("  Warning: could not remove .dna file {}: {e}", path.display());
                        }
                        summary.transformed += 1;
                    }
                    Err(e) => {
                        eprintln!("FAILED: {e}");
                        summary.failed += 1;
                        summary.failures.push((path.clone(), e.to_string()));
                    }
                }
            }
            FileClass::Skip(reason) => {
                let label = match reason {
                    SkipReason::NotDna => "not .dna",
                    SkipReason::ExcludedDir => "excluded dir",
                    SkipReason::Binary => "binary",
                    SkipReason::AlreadyDna => "already .dna",
                    SkipReason::ReadError => "read error",
                };
                eprintln!("  Skipping {}  ({})", path.display(), label);
                summary.skipped += 1;
            }
            FileClass::Encode => {
                summary.skipped += 1;
            }
        }
    }

    summary
}

/// Encode a single file. Returns the path of the written .dna file.
fn encode_file(path: &Path, password: &str) -> Result<PathBuf> {
    let plaintext = fs::read(path).map_err(DendecError::Io)?;
    let dna = encode_raw(&plaintext, password, None)?;

    // Append .dna extension
    let mut dna_path = path.to_path_buf();
    let new_name = format!(
        "{}.dna",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
    );
    dna_path.set_file_name(new_name);

    let mut f = fs::File::create(&dna_path).map_err(DendecError::Io)?;
    f.write_all(dna.as_bytes()).map_err(DendecError::Io)?;

    Ok(dna_path)
}

/// Decode a single .dna file. Returns the path of the restored file.
fn decode_file(path: &Path, password: &str) -> Result<PathBuf> {
    let dna_string = fs::read_to_string(path).map_err(DendecError::Io)?;
    let plaintext = decode_raw(&dna_string, password)?;

    // Strip .dna extension to get original path
    let out_path = strip_dna_extension(path);

    let mut f = fs::File::create(&out_path).map_err(DendecError::Io)?;
    f.write_all(&plaintext).map_err(DendecError::Io)?;

    Ok(out_path)
}

/// Strip the trailing `.dna` extension from a path.
/// `src/main.rs.dna` → `src/main.rs`
fn strip_dna_extension(path: &Path) -> PathBuf {
    let stem = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    let stripped = stem.strip_suffix(".dna").unwrap_or(stem);

    let mut out = path.to_path_buf();
    out.set_file_name(stripped);
    out
}

/// Format byte count as human-readable string.
fn human_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Print a summary report to stderr.
pub fn print_summary(summary: &TransformSummary, mode: &str) {
    eprintln!();
    eprintln!(
        "  {} files {}d  |  {} skipped  |  {} failed",
        summary.transformed, mode, summary.skipped, summary.failed
    );
    if !summary.failures.is_empty() {
        eprintln!();
        eprintln!("  Failures:");
        for (path, reason) in &summary.failures {
            eprintln!("    {} — {}", path.display(), reason);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_encode_decode_file_roundtrip() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("hello.rs");
        let content = b"fn main() { println!(\"hello\"); }\n";
        fs::write(&src, content).unwrap();

        let password = "wraptest";

        // Encode
        let dna_path = encode_file(&src, password).unwrap();
        assert!(dna_path.exists());
        assert!(dna_path.to_str().unwrap().ends_with(".dna"));

        // Decode
        let out_path = decode_file(&dna_path, password).unwrap();
        let decoded = fs::read(&out_path).unwrap();
        assert_eq!(decoded, content);
    }

    #[test]
    fn test_strip_dna_extension() {
        let p = PathBuf::from("src/main.rs.dna");
        let stripped = strip_dna_extension(&p);
        assert_eq!(stripped, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_human_size() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
    }
}
