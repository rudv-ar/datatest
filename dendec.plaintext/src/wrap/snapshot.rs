/// wrap/snapshot.rs — Filesystem snapshot and diff
///
/// To detect what files a command produced, we snapshot the directory
/// tree before and after running it. The diff gives us exactly the set
/// of files that appeared or changed — regardless of what the command is
/// or where it puts its output.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

/// A snapshot of the filesystem at a point in time.
/// Maps each file path to its last-modified timestamp.
pub struct Snapshot {
    files: HashMap<PathBuf, SystemTime>,
}

impl Snapshot {
    /// Capture a snapshot of all files under `root`.
    pub fn capture(root: &Path) -> Self {
        let mut files = HashMap::new();
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        files.insert(entry.path().to_path_buf(), mtime);
                    }
                }
            }
        }
        Self { files }
    }

    /// Return all paths in `after` that are new or modified relative to `self`.
    ///
    /// A path is "new" if it did not exist in the before snapshot.
    /// A path is "modified" if its mtime changed.
    pub fn diff<'a>(&self, after: &'a Snapshot) -> Vec<&'a PathBuf> {
        after
            .files
            .iter()
            .filter(|(path, &after_mtime)| {
                match self.files.get(*path) {
                    // New file — not in before snapshot
                    None => true,
                    // Modified file — mtime changed
                    Some(&before_mtime) => after_mtime != before_mtime,
                }
            })
            .map(|(path, _)| path)
            .collect()
    }

    /// Return all paths currently in this snapshot.
    /// This might be a part of future rollouts of dendec, currently a dead code 
    #[allow(dead_code)]
    pub fn all_paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_new_file_detected() {
        let dir = tempdir().unwrap();
        let before = Snapshot::capture(dir.path());

        fs::write(dir.path().join("new.txt"), b"hello").unwrap();
        let after = Snapshot::capture(dir.path());

        let diff = before.diff(&after);
        assert_eq!(diff.len(), 1);
        assert!(diff[0].ends_with("new.txt"));
    }

    #[test]
    fn test_unchanged_file_not_in_diff() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("existing.txt"), b"data").unwrap();

        let before = Snapshot::capture(dir.path());
        let after = Snapshot::capture(dir.path());

        let diff = before.diff(&after);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_empty_dir_snapshot() {
        let dir = tempdir().unwrap();
        let snap = Snapshot::capture(dir.path());
        assert_eq!(snap.all_paths().count(), 0);
    }
}
