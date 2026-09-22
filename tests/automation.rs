use std::{fs, path::Path};

use serde_yaml::Value;

fn read(relative: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

fn yaml(relative: &str) -> Value {
    serde_yaml::from_str(&read(relative)).expect("automation YAML must parse")
}

#[test]
fn pull_request_ci_has_no_write_permissions_or_secrets() {
    let workflow = yaml(".github/workflows/ci.yml");
    assert!(workflow["permissions"].as_mapping().unwrap().is_empty());
    let events = workflow["on"].as_mapping().unwrap();
    assert!(events.contains_key("pull_request"));
    assert!(!events.contains_key("pull_request_target"));
    assert_eq!(workflow["on"]["push"]["branches"][0], "main");

    for (_, job) in workflow["jobs"].as_mapping().unwrap() {
        let permissions = job["permissions"].as_mapping().unwrap();
        assert_eq!(permissions.len(), 1, "checkout only needs contents: read");
        assert_eq!(job["permissions"]["contents"], "read");
        assert!(job["environment"].is_null());
        for step in job["steps"].as_sequence().unwrap() {
            if step["uses"]
                .as_str()
                .is_some_and(|s| s.starts_with("actions/checkout@"))
            {
                assert_eq!(step["with"]["persist-credentials"], false);
            }
        }
    }
    assert!(!read(".github/workflows/ci.yml").contains("secrets"));
}

#[test]
fn ci_actions_are_immutable_and_required_checks_cannot_be_skipped() {
    let workflow = yaml(".github/workflows/ci.yml");
    for (_, job) in workflow["jobs"].as_mapping().unwrap() {
        assert!(job["if"].is_null());
        assert!(job["continue-on-error"].is_null());
        for step in job["steps"].as_sequence().unwrap() {
            assert!(step["if"].is_null());
            assert!(step["continue-on-error"].is_null());
            if let Some(action) = step["uses"].as_str() {
                let (_, revision) = action.rsplit_once('@').expect("action must be pinned");
                assert_eq!(revision.len(), 40, "{action} must use a full commit SHA");
                assert!(revision.bytes().all(|byte| byte.is_ascii_hexdigit()));
            }
        }
    }

    for (job, required) in [
        (
            "quality",
            vec![
                "cargo fmt --all --check",
                "cargo test --locked",
                "cargo clippy --locked --all-targets -- -D warnings",
            ],
        ),
        (
            "dependencies",
            vec!["cargo audit", "cargo deny --locked check"],
        ),
    ] {
        let steps = workflow["jobs"][job]["steps"].as_sequence().unwrap();
        let commands: Vec<_> = steps
            .iter()
            .filter_map(|step| step["run"].as_str())
            .collect();
        for command in required {
            assert!(commands.contains(&command), "{job} must run {command}");
        }
    }
}

#[test]
fn dependency_tools_use_exact_versions_and_locked_installation() {
    let workflow = yaml(".github/workflows/ci.yml");
    let steps = workflow["jobs"]["dependencies"]["steps"]
        .as_sequence()
        .unwrap();
    for tool in ["cargo-audit", "cargo-deny"] {
        let prefix = format!("cargo install --locked {tool} --version ");
        let version = steps
            .iter()
            .filter_map(|step| step["run"].as_str())
            .find_map(|command| command.strip_prefix(&prefix))
            .expect("tool installation must be locked and versioned");
        let components: Vec<_> = version.trim().trim_start_matches('=').split('.').collect();
        assert_eq!(components.len(), 3);
        assert!(components
            .iter()
            .all(|component| component.parse::<u64>().is_ok()));
    }
}

#[test]
fn cargo_dependencies_have_a_weekly_update_and_restrictive_review_policy() {
    let dependabot = yaml(".github/dependabot.yml");
    assert_eq!(dependabot["version"], 2);
    assert!(dependabot["updates"]
        .as_sequence()
        .unwrap()
        .iter()
        .any(|update| {
            update["package-ecosystem"] == "cargo"
                && update["directory"] == "/"
                && update["schedule"]["interval"] == "weekly"
        }));
    let deny = read("deny.toml");
    for requirement in [
        "[advisories]",
        "[licenses]",
        "[bans]",
        "[sources]",
        "multiple-versions = \"deny\"",
        "wildcards = \"deny\"",
        "unknown-registry = \"deny\"",
        "unknown-git = \"deny\"",
    ] {
        assert!(
            deny.lines().any(|line| line.trim() == requirement),
            "dependency policy must include {requirement}"
        );
    }
}
