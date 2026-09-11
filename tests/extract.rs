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
