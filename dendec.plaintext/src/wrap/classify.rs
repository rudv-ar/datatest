/// wrap/classify.rs — File classification for wrap
///
/// Determines whether a file should be encoded, decoded, or skipped.
/// Binary detection samples the first 512 bytes and checks the ratio
/// of non-UTF-8-safe bytes. This mirrors the approach used by git
/// and most editors to detect binary files.
use std::fs;
use std::path::Path;

/// Known binary extensions — fast path to skip obvious binaries
/// without reading file contents.
const BINARY_EXTENSIONS: &[&str] = &[
    // images
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "tiff", "svg",
    // archives
    "zip", "tar", "gz", "bz2", "xz", "zst", "7z", "rar",
    // compiled
    "wasm", "bin", "exe", "dll", "so", "dylib", "a", "o",
    // documents
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
    // media
    "mp3", "mp4", "wav", "ogg", "flac", "avi", "mkv", "mov",
    // other
    "db", "sqlite", "pyc", "class",
];

/// A .dna file produced by dendec
const DNA_EXTENSION: &str = "dna";

/// Classification decision for a file.
#[derive(Debug, PartialEq)]
pub enum FileClass {
    /// Encode this file to .dna
    Encode,
    /// Decode this .dna file
    Decode,
    /// Skip — binary, already processed, or in excluded directory
    Skip(SkipReason),
}

#[derive(Debug, PartialEq)]
pub enum SkipReason {
    Binary,
    AlreadyDna,
    NotDna,
    ExcludedDir,
    #[allow(dead_code)]
    // This is preserved for future rollouts
    ReadError,
}

/// Classify a file for encode mode.
pub fn classify_for_encode(path: &Path) -> FileClass {
    if is_excluded_dir(path) {
        return FileClass::Skip(SkipReason::ExcludedDir);
    }
    // Skip files that are already .dna
    if has_extension(path, DNA_EXTENSION) {
        return FileClass::Skip(SkipReason::AlreadyDna);
    }
    // Fast path: known binary extension
    if has_known_binary_extension(path) {
        return FileClass::Skip(SkipReason::Binary);
    }
    // Content inspection: sample first 512 bytes
    if is_binary_content(path) {
        return FileClass::Skip(SkipReason::Binary);
    }
    FileClass::Encode
}

/// Classify a file for decode mode.
pub fn classify_for_decode(path: &Path) -> FileClass {
    if is_excluded_dir(path) {
        return FileClass::Skip(SkipReason::ExcludedDir);
    }
    // Only decode .dna files
    if has_extension(path, DNA_EXTENSION) {
        return FileClass::Decode;
    }
    FileClass::Skip(SkipReason::NotDna)
}

/// Check if path is inside an excluded directory (.git, target, node_modules).
fn is_excluded_dir(path: &Path) -> bool {
    path.components().any(|c| {
        matches!(
            c.as_os_str().to_str().unwrap_or(""),
            ".git" | "target" | "node_modules" | ".svn" | ".hg"
        )
    })
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

fn has_known_binary_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            BINARY_EXTENSIONS
                .iter()
                .any(|&b| e.eq_ignore_ascii_case(b))
        })
        .unwrap_or(false)
}

/// Sample up to 512 bytes of the file.
/// If more than 10% of bytes are non-printable non-whitespace, treat as binary.
///
/// This mirrors git's binary detection heuristic.
fn is_binary_content(path: &Path) -> bool {
    let sample = match read_sample(path, 512) {
        Ok(b) => b,
        Err(_) => return false, // if we can't read it, try to encode anyway
    };

    if sample.is_empty() {
        return false;
    }

    // Null byte is a definitive binary indicator
    if sample.contains(&0u8) {
        return true;
    }

    let non_text = sample
        .iter()
        .filter(|&&b| b < 0x08 || (b > 0x0D && b < 0x20 && b != 0x1B))
        .count();

    // More than 10% suspicious bytes → binary
    non_text * 10 > sample.len()
}

fn read_sample(path: &Path, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut f = fs::File::open(path)?;
    let mut buf = vec![0u8; max_bytes];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_dna_file_skipped_in_encode() {
        let p = PathBuf::from("file.rs.dna");
        assert_eq!(classify_for_encode(&p), FileClass::Skip(SkipReason::AlreadyDna));
    }

    #[test]
    fn test_non_dna_skipped_in_decode() {
        let p = PathBuf::from("file.rs");
        assert_eq!(classify_for_decode(&p), FileClass::Skip(SkipReason::NotDna));
    }

    #[test]
    fn test_dna_file_decoded_in_decode() {
        let p = PathBuf::from("file.rs.dna");
        assert_eq!(classify_for_decode(&p), FileClass::Decode);
    }

    #[test]
    fn test_git_dir_excluded() {
        let p = PathBuf::from(".git/config");
        assert_eq!(classify_for_encode(&p), FileClass::Skip(SkipReason::ExcludedDir));
    }

    #[test]
    fn test_binary_extension_skipped() {
        let p = PathBuf::from("image.png");
        assert_eq!(classify_for_encode(&p), FileClass::Skip(SkipReason::Binary));
    }

    #[test]
    fn test_text_file_encoded() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("hello.rs");
        std::fs::write(&path, b"fn main() {}").unwrap();
        assert_eq!(classify_for_encode(&path), FileClass::Encode);
    }

    #[test]
    fn test_null_byte_is_binary() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bin.dat");
        std::fs::write(&path, b"hello\x00world").unwrap();
        assert_eq!(classify_for_encode(&path), FileClass::Skip(SkipReason::Binary));
    }
}
