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
