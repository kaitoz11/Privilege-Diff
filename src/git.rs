use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("failed to invoke git while {operation}: {source}")]
    Invocation {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("git {operation} failed: {message}")]
    Command {
        operation: &'static str,
        message: String,
    },
    #[error("git {operation} returned non-UTF-8 {field}")]
    NonUtf8 {
        operation: &'static str,
        field: &'static str,
    },
}

/// Reads GitHub Actions workflow YAML files directly from a local Git revision.
pub fn workflows_at(repo: &Path, revision: &str) -> Result<BTreeMap<PathBuf, String>, GitError> {
    let revision = git(
        repo,
        "resolve revision",
        &["rev-parse", "--verify", revision],
    )?;
    let revision = String::from_utf8(revision.stdout)
        .map_err(|_| GitError::NonUtf8 {
            operation: "resolve revision",
            field: "revision",
        })?
        .trim()
        .to_owned();

    let paths = git(
        repo,
        "list workflow files",
        &[
            "ls-tree",
            "-r",
            "--name-only",
            &revision,
            "--",
            ".github/workflows",
        ],
    )?;
    let paths = String::from_utf8(paths.stdout).map_err(|_| GitError::NonUtf8 {
        operation: "list workflow files",
        field: "path",
    })?;

    let mut workflows = BTreeMap::new();
    for path in paths
        .lines()
        .filter(|path| path.ends_with(".yml") || path.ends_with(".yaml"))
    {
        let contents = git(
            repo,
            "read workflow file",
            &["show", &format!("{revision}:{path}")],
        )?;
        let contents = String::from_utf8(contents.stdout).map_err(|_| GitError::NonUtf8 {
            operation: "read workflow file",
            field: "workflow contents",
        })?;
        workflows.insert(PathBuf::from(path), contents);
    }

    Ok(workflows)
}

fn git(
    repo: &Path,
    operation: &'static str,
    args: &[&str],
) -> Result<std::process::Output, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|source| GitError::Invocation { operation, source })?;

    if output.status.success() {
        return Ok(output);
    }

    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GitError::Command { operation, message })
}
