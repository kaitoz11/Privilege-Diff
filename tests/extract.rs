use std::path::PathBuf;

use privilege_diff::extract_workflow;

const SOURCE: &str = r#"
on:
  pull_request_target:

permissions:
  contents: read

jobs:
  release:
    permissions:
      contents: write
      id-token: write
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm publish --token "${{ secrets.NPM_TOKEN }}"
"#;

#[test]
fn extracts_job_privileges_from_a_workflow() {
    let capability = extract_workflow(PathBuf::from(".github/workflows/release.yml"), SOURCE);
    let release = &capability.jobs["release"];

    assert!(capability.triggers.contains("pull_request_target"));
    assert_eq!(release.permissions["contents"], "write");
    assert!(release.oidc);
    assert!(release.secrets.contains("NPM_TOKEN"));
    assert_eq!(release.action_refs, vec!["actions/checkout@v4"]);
}

#[test]
fn warns_for_dynamic_runners_and_keeps_static_group_labels() {
    let capability = extract_workflow(
        PathBuf::from(".github/workflows/runners.yml"),
        r#"
on: push
jobs:
  dynamic:
    runs-on: "${{ matrix.runner }}"
  grouped:
    runs-on:
      group: deployment
      labels:
        - ubuntu-latest
        - "${{ matrix.os }}"
"#,
    );

    assert!(capability.jobs["dynamic"].runners.is_empty());
    assert_eq!(
        capability.jobs["grouped"]
            .runners
            .iter()
            .collect::<Vec<_>>(),
        vec!["ubuntu-latest"]
    );
    assert!(capability
        .warnings
        .iter()
        .any(|warning| warning.contains("dynamic")));
}

#[test]
fn warns_for_unknown_permission_access_values() {
    let capability = extract_workflow(
        PathBuf::from(".github/workflows/permissions.yml"),
        r#"
on: push
permissions:
  contents: administer
jobs:
  check:
    runs-on: ubuntu-latest
"#,
    );

    assert!(capability
        .warnings
        .iter()
        .any(|warning| warning.contains("unsupported access level")));
}

#[test]
fn warns_for_malformed_trigger_configuration() {
    let capability = extract_workflow(
        PathBuf::from(".github/workflows/trigger.yml"),
        r#"
on:
  push: invalid
jobs: {}
"#,
    );

    assert!(capability.triggers.contains("push"));
    assert!(capability
        .warnings
        .iter()
        .any(|warning| warning.contains("push") && warning.contains("configuration")));
}

#[test]
fn capability_models_serialize_for_json_rendering() {
    let capability = extract_workflow(
        PathBuf::from(".github/workflows/check.yml"),
        "on: push\njobs: {}\n",
    );

    let serialized = serde_yaml::to_string(&capability).unwrap();

    assert!(serialized.contains("triggers"));
}

#[test]
fn warns_for_static_runner_groups_that_the_model_cannot_represent() {
    let capability = extract_workflow(
        PathBuf::from(".github/workflows/runners.yml"),
        r#"
on: push
jobs:
  deploy:
    runs-on:
      group: trusted-group
"#,
    );

    assert!(capability.jobs["deploy"].runners.is_empty());
    assert!(capability
        .warnings
        .iter()
        .any(|warning| warning.contains("runner group") && warning.contains("unsupported")));
}

#[test]
fn warns_when_reusable_workflow_inherits_all_secrets() {
    let capability = extract_workflow(
        PathBuf::from(".github/workflows/reusable.yml"),
        r#"
on: push
jobs:
  publish:
    uses: owner/repository/.github/workflows/publish.yml@v1
    secrets: inherit
"#,
    );

    assert!(capability.jobs["publish"].secrets.is_empty());
    assert!(capability
        .warnings
        .iter()
        .any(|warning| warning.contains("unknown secret access")));
}
