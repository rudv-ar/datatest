/// wrap/fetch.rs — Command execution and output detection
///
/// Runs the user-supplied command as a subprocess, waits for it to finish,
/// and returns the working directory so the snapshot diff can find what
/// was produced. Also handles stdout-capturing for commands like curl that
/// write to stdout rather than disk.
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::error::{DendecError, Result};

/// Result of running a wrapped command.
pub struct FetchResult {
    /// Directory to scan for produced files.
    /// This is a future rollout feature, now a dead code 
    #[allow(dead_code)]
    pub scan_root: PathBuf,
    /// Raw stdout bytes if the command wrote to stdout (curl without -o, etc.)
    pub stdout_bytes: Option<Vec<u8>>,
}

/// Run the command and return what it produced.
///
/// If the command writes to disk (git clone, wget -O, curl -o) the files
/// will appear in the snapshot diff. If the command writes to stdout
/// (bare curl, cat, etc.) the bytes are captured and returned separately
/// so the caller can decode them directly.
pub fn run_command(args: &[String], capture_stdout: bool) -> Result<FetchResult> {
    if args.is_empty() {
        return Err(DendecError::WrapNoFilesFound);
    }

    let program = &args[0];
    let rest = &args[1..];

    let scan_root = std::env::current_dir().map_err(DendecError::Io)?;

    eprintln!("  Running: {}", args.join(" "));

    if capture_stdout {
        // Capture stdout — used when command is expected to write to stdout
        let output = Command::new(program)
            .args(rest)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // let stderr through so user sees progress
            .output()
            .map_err(DendecError::Io)?;

        check_exit(&output.status, args)?;

        Ok(FetchResult {
            scan_root,
            stdout_bytes: Some(output.stdout),
        })
    } else {
        // Inherit stdout — command writes to disk (git clone, wget, etc.)
        let status = Command::new(program)
            .args(rest)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(DendecError::Io)?;

        check_exit(&status, args)?;

        Ok(FetchResult {
            scan_root,
            stdout_bytes: None,
        })
    }
}

fn check_exit(status: &std::process::ExitStatus, args: &[String]) -> Result<()> {
    if !status.success() {
        return Err(DendecError::WrapCommandFailed {
            cmd: args.join(" "),
            code: status.code().unwrap_or(-1),
        });
    }
    Ok(())
}

/// Heuristic: does this command write to disk rather than stdout?
///
/// Returns true for commands that create files as their primary output.
/// Returns false for commands that write to stdout by default.
pub fn writes_to_disk(args: &[String]) -> bool {
    let program = args.first().map(|s| s.as_str()).unwrap_or("");

    match program {
        // git always writes to disk
        "git" => true,
        // wget writes to disk by default
        "wget" => true,
        // curl writes to stdout by default unless -o or --output is present
        "curl" => args
            .iter()
            .any(|a| a == "-o" || a == "--output" || a == "-O"),
        // conservative default: assume disk
        _ => true,
    }
}

/// Extract the target directory name for git clone.
///
/// `git clone https://github.com/user/repo` → `repo`
/// `git clone https://github.com/user/repo mydir` → `mydir`
pub fn git_clone_target(args: &[String]) -> Option<PathBuf> {
    // Find "clone" subcommand
    let clone_pos = args.iter().position(|a| a == "clone")?;
    let clone_args = &args[clone_pos + 1..];

    // Skip flags (anything starting with -)
    let positional: Vec<&String> = clone_args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .collect();

    match positional.len() {
        // git clone <url> → directory name is last segment of url
        1 => {
            let url = positional[0];
            let name = url
                .trim_end_matches('/')
                .rsplit('/')
                .next()?
                .trim_end_matches(".git");
            Some(PathBuf::from(name))
        }
        // git clone <url> <dir>
        2 => Some(PathBuf::from(positional[1])),
        _ => None,
    }
}
