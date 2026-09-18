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
            "-z",
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
        .split('\0')
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
    let output = git_command(repo, args)
        .output()
        .map_err(|source| GitError::Invocation { operation, source })?;

    if output.status.success() {
        return Ok(output);
    }

    let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(GitError::Command { operation, message })
}

fn git_command(repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_NO_LAZY_FETCH", "1")
        .arg("--no-lazy-fetch")
        .arg("-C")
        .arg(repo)
        .args(args);
    command
}

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn every_git_invocation_disables_lazy_fetching() {
        let repo = tempfile::tempdir().unwrap();
        let output = super::git(
            repo.path(),
            "inspect local-only environment",
            &[
                "-c",
                "alias.inspect-local-only=!printf '%s' \"$GIT_NO_LAZY_FETCH\"",
                "inspect-local-only",
            ],
        )
        .unwrap();
        assert_eq!(output.stdout, b"1");
    }

    #[test]
    fn every_git_invocation_uses_the_global_no_lazy_fetch_option() {
        let repo = tempfile::tempdir().unwrap();
        let command = super::git_command(repo.path(), &["rev-parse", "HEAD"]);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(args.first(), Some(&"--no-lazy-fetch".to_owned()));
    }
}
