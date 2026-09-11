use std::fs;
use std::process::{Command, Output};

use tempfile::TempDir;

const SAFE: &str = "on: push\njobs:\n  check:\n    runs-on: ubuntu-latest\n    permissions:\n      contents: read\n    steps:\n      - run: echo ok\n";

fn git(repo: &TempDir, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo.path())
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

fn repository(base: &str, head: &str) -> TempDir {
    let repo = tempfile::tempdir().unwrap();
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["config", "user.name", "CLI test"]);
    git(&repo, &["config", "user.email", "cli@example.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    fs::create_dir_all(repo.path().join(".github/workflows")).unwrap();
    for (name, source) in [("base", base), ("head", head)] {
        fs::write(repo.path().join(".github/workflows/check.yml"), source).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "--quiet", "--allow-empty", "-m", name]);
    }
    repo
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_privilege-diff"))
}

fn compare(repo: &TempDir, extra: &[&str]) -> Output {
    cli()
        .arg("--repo")
        .arg(repo.path())
        .args(["--base", "HEAD~1", "--head", "HEAD"])
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn emits_json_and_fails_for_a_high_risk_delta() {
    let repo = repository(SAFE, &SAFE.replace("contents: read", "contents: write"));
    let output = compare(&repo, &["--format", "json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8(output.stdout.clone())
        .unwrap()
        .contains("\"severity\":\"high\""));
    let findings: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(findings.as_array().unwrap().len(), 1);
    assert_eq!(findings[0]["severity"], "high");
    assert_eq!(findings[0]["path"], ".github/workflows/check.yml");
    assert_eq!(findings[0]["job"], "check");
    assert_eq!(findings[0]["category"], "token-permission");
    assert!(findings[0]["message"]
        .as_str()
        .unwrap()
        .contains("contents: write"));
    assert!(!findings[0]["remediation"].as_str().unwrap().is_empty());
    assert_eq!(output.stdout, compare(&repo, &["--format", "json"]).stdout);
}

#[test]
fn defaults_to_text_with_finding_details_and_remediation() {
    let repo = repository(SAFE, &SAFE.replace("contents: read", "contents: write"));
    let output = compare(&repo, &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    for detail in [
        "HIGH",
        ".github/workflows/check.yml",
        "check",
        "token-permission",
        "contents: write",
        "read or none",
    ] {
        assert!(text.contains(detail), "missing {detail}: {text}");
    }
}

#[test]
fn unchanged_workflows_exit_zero_with_no_findings() {
    let repo = repository(SAFE, SAFE);
    let json = compare(&repo, &["--format", "json"]);
    assert_eq!(json.status.code(), Some(0));
    assert_eq!(json.stdout, b"[]\n");
    assert!(json.stderr.is_empty());
    let text = compare(&repo, &["--format", "text"]);
    assert_eq!(text.status.code(), Some(0));
    assert!(String::from_utf8(text.stdout)
        .unwrap()
        .contains("No privilege-increasing changes detected"));
}

#[test]
fn unsupported_structures_remain_visible_warnings_with_exit_zero() {
    let repo = repository(SAFE, "on: push\njobs: []\n");
    let output = compare(&repo, &[]);
    assert_eq!(output.status.code(), Some(0));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("WARNING"));
    assert!(text.contains("jobs must be a mapping"));
    assert!(!text.contains("No privilege-increasing changes"));
    let json = compare(&repo, &["--format", "json"]);
    assert_eq!(json.status.code(), Some(0));
    let findings: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(findings[0]["severity"], "warning");
    assert!(findings[0]["job"].is_null());
}

#[test]
fn invalid_arguments_are_input_errors() {
    for args in [
        vec![],
        vec![
            "--repo", ".", "--base", "HEAD", "--head", "HEAD", "--format", "xml",
        ],
        vec!["--unknown"],
    ] {
        let output = cli().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("error:"));
    }
}

#[test]
fn help_is_a_successful_request() {
    let output = cli().arg("--help").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("--format"));
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_revisions_and_repositories_are_input_errors() {
    let repo = repository(SAFE, SAFE);
    for (path, base, head) in [
        (repo.path().to_path_buf(), "missing-revision", "HEAD"),
        (repo.path().to_path_buf(), "HEAD", "missing-revision"),
        (repo.path().join("missing"), "HEAD", "HEAD"),
    ] {
        let output = cli()
            .arg("--repo")
            .arg(path)
            .args(["--base", base, "--head", head, "--format", "json"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("resolve revision"));
    }
}

#[test]
fn malformed_yaml_on_either_side_is_an_error_without_source_values() {
    let malformed =
        "on: push\nenv: {TOKEN: SUPER_SECRET_VALUE}\njobs:\n  check: [SUPER_SECRET_VALUE\n";
    for (base, head, side) in [(malformed, SAFE, "base"), (SAFE, malformed, "head")] {
        let repo = repository(base, head);
        let output = compare(&repo, &["--format", "json"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("could not parse workflow YAML"));
        assert!(error.contains(".github/workflows/check.yml"));
        assert!(error.contains(side));
        assert!(!error.contains("SUPER_SECRET_VALUE"));
    }
}

#[test]
fn reports_symbolic_secret_names_without_literal_values() {
    let head = format!("{SAFE}      - run: echo hi\n        env:\n          TOKEN: SUPER_SECRET_VALUE\n          OTHER: ${{{{ secrets.DEPLOY_TOKEN }}}}\n");
    let repo = repository(SAFE, &head);
    for format in ["text", "json"] {
        let output = compare(&repo, &["--format", format]);
        assert_eq!(output.status.code(), Some(2));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("DEPLOY_TOKEN"));
        assert!(!text.contains("SUPER_SECRET_VALUE"));
        assert!(output.stderr.is_empty());
    }
}
