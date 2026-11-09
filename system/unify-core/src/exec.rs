//! Running helper programs.
//!
//! UnifyOS system components deliberately shell out to the well-tested system
//! tools (`nmcli`, `wpctl`, `bluetoothctl`, `systemctl`) instead of
//! re-implementing D-Bus protocols, but every invocation goes through this
//! module so error handling and output capture stay consistent.

use std::env;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Captured result of running a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub program: String,
    pub args: Vec<String>,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    pub fn stdout_trimmed(&self) -> &str {
        self.stdout.trim()
    }

    pub fn first_line(&self) -> Option<&str> {
        self.stdout
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
    }

    /// Human-readable reason the command failed.
    pub fn failure_message(&self) -> String {
        let detail = self.stderr.trim();
        if !detail.is_empty() {
            return detail.to_string();
        }
        match self.code {
            Some(code) => format!("{} exited with status {code}", self.program),
            None => format!("{} was terminated by a signal", self.program),
        }
    }
}

/// Run a program, capture stdout/stderr, and never block on stdin.
pub fn run(program: &str, args: &[&str]) -> io::Result<Output> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()?;
    Ok(Output {
        program: program.to_string(),
        args: args.iter().map(|arg| arg.to_string()).collect(),
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Run a program with the caller's stdout/stderr, e.g. a package manager whose
/// progress the user should see live. Returns the exit code.
pub fn run_inherit(program: &str, args: &[&str]) -> io::Result<i32> {
    let status = Command::new(program).args(args).status()?;
    Ok(status.code().unwrap_or(-1))
}

/// Resolve a program name against `PATH` (or accept an explicit path).
pub fn which(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let path = PathBuf::from(program);
        return is_executable(&path).then_some(path);
    }
    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        let candidate = directory.join(program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub fn have(program: &str) -> bool {
    which(program).is_some()
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_captures_stdout_and_status() {
        let output = run("sh", &["-c", "echo unify"]).expect("run sh");
        assert!(output.success());
        assert_eq!(output.stdout_trimmed(), "unify");
        assert_eq!(output.first_line(), Some("unify"));
    }

    #[test]
    fn run_reports_failure_status_and_stderr() {
        let output = run("sh", &["-c", "echo boom >&2; exit 3"]).expect("run sh");
        assert!(!output.success());
        assert_eq!(output.code, Some(3));
        assert_eq!(output.failure_message(), "boom");
    }

    #[test]
    fn which_finds_existing_programs_only() {
        assert!(have("sh"));
        assert!(!have("unify-program-that-does-not-exist"));
        assert!(which("/bin/sh").is_some());
        assert!(which("/nonexistent/binary").is_none());
        let sh = which("sh").expect("sh on PATH");
        assert!(sh.is_absolute());
    }

    #[test]
    fn missing_program_is_an_io_error() {
        let error = run("unify-program-that-does-not-exist", &[]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
