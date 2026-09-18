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
fn job_permission_maps_replace_workflow_permissions() {
    let capability = extract_workflow(
        PathBuf::from(".github/workflows/permissions.yml"),
        r#"
on: push
permissions:
  contents: write
  packages: write
  id-token: write
jobs:
  restricted:
    permissions:
      contents: read
    runs-on: ubuntu-latest
  disabled:
    permissions: {}
    runs-on: ubuntu-latest
  inherited:
    runs-on: ubuntu-latest
"#,
    );

    let restricted = &capability.jobs["restricted"];
    assert!(!restricted.oidc);
    assert_eq!(restricted.permissions["contents"], "read");
    assert!(!restricted.permissions.contains_key("packages"));
    assert!(!restricted.permissions.contains_key("id-token"));
    assert_eq!(restricted.permissions.len(), 1);

    let disabled = &capability.jobs["disabled"];
    assert!(disabled.permissions.is_empty());
    assert!(!disabled.oidc);

    let inherited = &capability.jobs["inherited"];
    assert_eq!(inherited.permissions["contents"], "write");
    assert_eq!(inherited.permissions["packages"], "write");
    assert_eq!(inherited.permissions["id-token"], "write");
    assert!(inherited.oidc);
    assert_eq!(capability.permissions, inherited.permissions);
    assert!(capability.warnings.is_empty());
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

#[test]
fn warns_for_dynamic_secret_access_without_recording_a_name() {
    let capability = extract_workflow(
        PathBuf::from(".github/workflows/dynamic-secret.yml"),
        r#"
on: push
jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - run: echo "${{ secrets[inputs.secret_name] }}"
"#,
    );

    assert!(capability.jobs["publish"].secrets.is_empty());
    assert!(capability.warnings.iter().any(|warning| {
        warning.contains("job publish") && warning.contains("unknown secret access")
    }));
}

#[test]
fn workflow_env_respects_job_overrides_and_warns_for_step_overrides() {
    let capability = extract_workflow(
        PathBuf::from("env.yml"),
        r#"
on: push
permissions: {}
env:
  TOKEN: ${{ secrets.DEPLOY_TOKEN }}
jobs:
  inherited:
    runs-on: ubuntu-latest
  overridden:
    env:
      TOKEN: literal
    runs-on: ubuntu-latest
  step_override:
    runs-on: ubuntu-latest
    steps:
      - run: deploy
        env:
          TOKEN: literal
  reusable:
    uses: ./.github/workflows/called.yml
"#,
    );
    assert!(capability.jobs["inherited"]
        .secrets
        .contains("DEPLOY_TOKEN"));
    assert!(capability.jobs["overridden"].secrets.is_empty());
    assert!(capability.jobs["reusable"].secrets.is_empty());
    assert!(capability
        .warnings
        .iter()
        .any(|warning| warning.contains("step_override") && warning.contains("env override")));
}

#[test]
fn whole_secret_context_expressions_warn_without_fabricating_names() {
    for expression in [
        "toJSON(secrets)",
        "secrets",
        "secrets.*",
        "secrets [inputs.name]",
        "toJSON(SECRETS)",
    ] {
        let source = format!("on: push\npermissions: {{}}\nenv:\n  PAYLOAD: ${{{{ {expression} }}}}\njobs:\n  check:\n    runs-on: ubuntu-latest\n");
        let capability = extract_workflow(PathBuf::from("secrets.yml"), &source);
        assert!(capability.jobs["check"].secrets.is_empty());
        assert!(
            capability
                .warnings
                .iter()
                .any(|warning| warning.contains("unknown secret access")),
            "missed {expression}"
        );
    }
}

#[test]
fn expression_scanning_ignores_literals_and_handles_expression_boundaries() {
    let capability = extract_workflow(
        PathBuf::from("expressions.yml"),
        r#"
on: push
permissions: {}
jobs:
  literals:
    runs-on: ubuntu-latest
    steps:
      - run: echo secrets.NAME github.event.title ${{ 'secrets.HIDDEN github.event' }} ${{ env.secrets.NAME }}
  expressions:
    runs-on: ubuntu-latest
    steps:
      - run: echo ${{ format('}} secrets.IGNORED', secrets . ACTUAL) }} ${{ github . event . issue . title }}
"#,
    );
    assert!(capability.jobs["literals"].secrets.is_empty());
    assert!(!capability.jobs["literals"].untrusted_script_context);
    assert_eq!(
        capability.jobs["expressions"]
            .secrets
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["ACTUAL"]
    );
    assert!(capability.jobs["expressions"].untrusted_script_context);
    assert!(capability.warnings.is_empty());
}

#[test]
fn missing_required_workflow_keys_are_visible_warnings() {
    for (source, missing) in [("name: example\n", "on"), ("on: push\n", "jobs")] {
        let capability = extract_workflow(PathBuf::from("missing.yml"), source);
        assert!(capability
            .warnings
            .iter()
            .any(|warning| { warning.contains("missing") && warning.contains(missing) }));
    }
}

#[test]
fn library_parse_warnings_never_retain_yaml_source_values() {
    let capability = extract_workflow(
        PathBuf::from("private.yml"),
        "env:\n  PRIVATE_LITERAL: first\n  PRIVATE_LITERAL: second\n",
    );
    let warnings = serde_json::to_string(&capability.warnings).unwrap();
    assert!(warnings.contains("could not parse workflow YAML"));
    assert!(!warnings.contains("PRIVATE_LITERAL"));
    assert!(warnings.contains("line") && warnings.contains("column"));
}
