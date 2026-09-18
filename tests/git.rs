use std::fs;
use std::path::PathBuf;
use std::process::Command;

use privilege_diff::workflows_at;
use tempfile::TempDir;

fn git(repo: &TempDir, args: &[&str]) {
    let status = Command::new("git")
        .args(["-C", repo.path().to_str().unwrap()])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn repository() -> TempDir {
    let repo = tempfile::tempdir().unwrap();
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["config", "user.name", "Privilege Diff test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);

    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    fs::write(
        repo.path().join(".github/workflows/check.yml"),
        "name: check\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".github/workflows/release.yaml"),
        "name: release\n",
    )
    .unwrap();
    fs::write(repo.path().join(".github/workflows/notes.txt"), "ignore\n").unwrap();
    fs::write(repo.path().join("README.md"), "ignore\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "test fixture"]);
    repo
}

#[test]
fn reads_only_workflow_yaml_files_at_a_revision() {
    let repo = repository();
    let files = workflows_at(repo.path(), "HEAD").unwrap();
    assert_eq!(
        files.keys().collect::<Vec<_>>(),
        vec![
            &PathBuf::from(".github/workflows/check.yml"),
            &PathBuf::from(".github/workflows/release.yaml"),
        ]
    );
}

#[test]
fn reads_workflows_with_unicode_and_quoted_filenames_losslessly() {
    let repo = repository();
    let names = ["triển-khai.yml", "quoted\"name.yaml", "line\nbreak.yml"];
    for name in names {
        fs::write(
            repo.path().join(".github/workflows").join(name),
            "on: push\n",
        )
        .unwrap();
    }
    git(&repo, &["add", "."]);
    git(
        &repo,
        &["commit", "--quiet", "-m", "unusual workflow paths"],
    );

    let files = workflows_at(repo.path(), "HEAD").unwrap();
    for name in names {
        assert_eq!(
            files.get(&PathBuf::from(".github/workflows").join(name)),
            Some(&"on: push\n".to_owned()),
            "missing workflow {name:?}"
        );
    }
}

#[test]
fn missing_promised_workflow_blobs_error_without_fetching() {
    let origin = repository();
    git(&origin, &["config", "uploadpack.allowFilter", "true"]);
    let clone = tempfile::tempdir().unwrap();
    let output = Command::new("git")
        .args(["clone", "--quiet", "--no-checkout", "--filter=blob:none"])
        .arg(format!("file://{}", origin.path().display()))
        .arg(clone.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");

    let error = workflows_at(clone.path(), "HEAD").unwrap_err();
    assert!(matches!(
        error,
        privilege_diff::GitError::Command {
            operation: "read workflow file",
            ..
        }
    ));
}
