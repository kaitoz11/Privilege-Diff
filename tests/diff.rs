use std::collections::BTreeMap;
use std::path::PathBuf;

use privilege_diff::{diff_workflows, extract_workflow, Severity};

fn workflows(source: &str) -> BTreeMap<PathBuf, privilege_diff::WorkflowCapability> {
    let path = PathBuf::from(".github/workflows/release.yml");
    BTreeMap::from([(path.clone(), extract_workflow(path, source))])
}

#[test]
fn reports_new_write_permission_on_an_existing_job() {
    let base = workflows(
        r#"
on: push
permissions:
  contents: read
jobs:
  release:
    runs-on: ubuntu-latest
"#,
    );
    let head = workflows(
        r#"
on: push
permissions:
  contents: write
jobs:
  release:
    runs-on: ubuntu-latest
"#,
    );

    let findings = diff_workflows(&base, &head);

    assert!(findings.iter().any(|finding| {
        finding.severity == Severity::High
            && finding.category == "token-permission"
            && finding.job.as_deref() == Some("release")
            && finding.message.contains("contents: write")
    }));
}

#[test]
fn reports_each_supported_risk_addition_with_remediation() {
    let base = workflows(
        r#"
on: push
permissions:
  contents: read
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: acme/publish@0123456789012345678901234567890123456789
      - run: echo safe
"#,
    );
    let head = workflows(
        r#"
on:
  - push
  - pull_request_target
permissions:
  contents: write
  id-token: write
jobs:
  release:
    runs-on: [self-hosted, linux]
    steps:
      - uses: acme/publish@v1
      - run: echo "${{ github.event.pull_request.title }} ${{ secrets.NPM_TOKEN }}"
"#,
    );

    let findings = diff_workflows(&base, &head);
    let categories = findings
        .iter()
        .map(|finding| finding.category.as_str())
        .collect::<Vec<_>>();

    for category in [
        "trigger-trust",
        "token-permission",
        "secret-access",
        "oidc",
        "self-hosted-runner",
        "mutable-action-reference",
        "untrusted-context",
    ] {
        assert!(categories.contains(&category), "missing {category}");
    }
    assert!(findings
        .iter()
        .all(|finding| !finding.remediation.is_empty()));
}

#[test]
fn reports_new_unknown_capabilities_as_warnings() {
    let base = workflows(
        r#"
on: push
jobs:
  release:
    runs-on: ubuntu-latest
"#,
    );
    let head = workflows(
        r#"
on: push
jobs:
  release:
    runs-on: "${{ matrix.runner }}"
"#,
    );

    let findings = diff_workflows(&base, &head);

    assert!(findings.iter().any(|finding| {
        finding.severity == Severity::Warning
            && finding.category == "unknown-capability"
            && finding.message.contains("dynamic")
    }));
}

#[test]
fn uses_effective_job_permissions_when_a_job_replaces_workflow_permissions() {
    let base = workflows(
        r#"
on: push
permissions:
  contents: write
jobs:
  release:
    runs-on: ubuntu-latest
"#,
    );
    let head = workflows(
        r#"
on: push
permissions:
  contents: write
jobs:
  release:
    permissions:
      contents: read
      packages: write
    runs-on: ubuntu-latest
"#,
    );

    let findings = diff_workflows(&base, &head);

    assert!(findings.iter().any(|finding| {
        finding.category == "token-permission" && finding.message.contains("packages: write")
    }));
    assert!(!findings.iter().any(|finding| {
        finding.category == "token-permission" && finding.message.contains("contents: write")
    }));
}

#[test]
fn findings_have_a_stable_path_job_category_and_message_order() {
    let base = workflows(
        r#"
on: push
jobs:
  release:
    runs-on: ubuntu-latest
"#,
    );
    let head = workflows(
        r#"
on: pull_request_target
jobs:
  release:
    permissions:
      zeta: write
      alpha: write
    runs-on: ubuntu-latest
"#,
    );

    let findings = diff_workflows(&base, &head);
    let keys = findings
        .iter()
        .map(|finding| {
            (
                finding.path.clone(),
                finding.job.clone(),
                finding.category.clone(),
                finding.message.clone(),
            )
        })
        .collect::<Vec<_>>();
    let mut sorted = keys.clone();
    sorted.sort();

    assert_eq!(keys, sorted);
}

#[test]
fn does_not_report_risk_reductions() {
    let base = workflows(
        r#"
on: pull_request_target
permissions:
  contents: write
jobs:
  release:
    runs-on: self-hosted
    steps:
      - uses: acme/publish@v1
      - run: echo "${{ github.event.pull_request.title }} ${{ secrets.NPM_TOKEN }}"
"#,
    );
    let head = workflows(
        r#"
on: push
permissions:
  contents: read
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: acme/publish@0123456789012345678901234567890123456789
      - run: echo safe
"#,
    );

    assert!(diff_workflows(&base, &head).is_empty());
}

#[test]
fn retains_head_uncertainty_when_dynamic_runner_selector_changes() {
    let base = workflows(
        r#"
on: push
jobs:
  release:
    runs-on: "${{ inputs.runner }}"
"#,
    );
    let head = workflows(
        r#"
on: push
jobs:
  release:
    runs-on: "${{ matrix.runner }}"
"#,
    );

    let findings = diff_workflows(&base, &head);

    assert!(findings.iter().any(|finding| {
        finding.severity == Severity::Warning
            && finding.category == "unknown-capability"
            && finding.message.contains("runner label is dynamic")
    }));
}

#[test]
fn does_not_report_a_write_added_after_a_write_all_baseline() {
    let base = workflows(
        r#"
on: push
permissions: write-all
jobs:
  release:
    runs-on: ubuntu-latest
"#,
    );
    let head = workflows(
        r#"
on: push
permissions:
  contents: write
jobs:
  release:
    runs-on: ubuntu-latest
"#,
    );

    let findings = diff_workflows(&base, &head);

    assert!(!findings
        .iter()
        .any(|finding| finding.category == "token-permission"));
}

#[test]
fn recommends_a_digest_for_a_mutable_docker_reference() {
    let base = workflows(
        r#"
on: push
jobs:
  release:
    runs-on: ubuntu-latest
"#,
    );
    let head = workflows(
        r#"
on: push
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: docker://alpine:latest
"#,
    );

    let findings = diff_workflows(&base, &head);
    let docker_finding = findings
        .iter()
        .find(|finding| finding.category == "mutable-action-reference")
        .expect("a mutable Docker image should be reported");

    assert!(docker_finding.remediation.contains("digest"));
}

#[test]
fn reports_workflow_environment_secret_exposure_in_existing_jobs() {
    let base = workflows("on: push\npermissions: {}\njobs:\n  release:\n    runs-on: ubuntu-latest\n    steps:\n      - run: deploy\n");
    let head = workflows("on: push\npermissions: {}\nenv:\n  TOKEN: ${{ secrets.DEPLOY_TOKEN }}\njobs:\n  release:\n    runs-on: ubuntu-latest\n    steps:\n      - run: deploy\n");
    assert!(diff_workflows(&base, &head).iter().any(|finding| {
        finding.category == "secret-access"
            && finding.job.as_deref() == Some("release")
            && finding.message.contains("DEPLOY_TOKEN")
    }));
}

#[test]
fn warns_when_explicit_permission_restrictions_are_removed() {
    for restriction in ["permissions: read-all\n", "permissions: {}\n"] {
        let body = "jobs:\n  release:\n    runs-on: ubuntu-latest\n";
        let base = workflows(&format!("on: push\n{restriction}{body}"));
        let head = workflows(&format!("on: push\n{body}"));
        assert!(diff_workflows(&base, &head).iter().any(|finding| {
            finding.severity == Severity::Warning
                && finding.category == "unknown-capability"
                && finding.message.contains("permission defaults")
        }));
        assert!(diff_workflows(&head, &base).is_empty());
    }
}

#[test]
fn literal_context_mentions_do_not_hide_new_expression_risks() {
    let base = workflows("on: push\npermissions: {}\njobs:\n  release:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo github.event.pull_request.title secrets.DEPLOY_TOKEN\n");
    let head = workflows("on: push\npermissions: {}\njobs:\n  release:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo ${{ github.event.pull_request.title }} ${{ secrets.DEPLOY_TOKEN }}\n");
    let findings = diff_workflows(&base, &head);
    for category in ["secret-access", "untrusted-context"] {
        assert!(
            findings.iter().any(|finding| finding.category == category),
            "missing {category}"
        );
    }
}
