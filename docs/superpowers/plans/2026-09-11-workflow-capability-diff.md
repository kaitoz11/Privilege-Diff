# Workflow Capability Diff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a local Rust CLI that compares GitHub Actions workflow capability between two Git revisions and reports risk-increasing changes.

**Architecture:** The CLI obtains workflow files from local Git revisions, normalizes only the GitHub Actions fields required by the first milestone, and extracts a `WorkflowCapability` for each file. A deterministic diff engine compares capabilities per workflow path and emits terminal findings; it defaults to conservative warnings for unsupported values.

**Tech Stack:** Rust stable, `clap` for CLI parsing, `serde_yaml` for YAML parsing, `serde_json` for machine-readable output, `thiserror` for typed errors, Cargo test.

**Spec:** `docs/project-brief.md`

## Global Constraints

- Read local Git objects only; never require a GitHub token or transmit workflow content.
- Never read or render secret values; record symbolic `secrets.NAME` references only.
- Report unsupported or malformed workflow structures as warnings, never as safe.
- v0.1 supports workflow-level triggers and permissions plus job-level permissions, secret references, OIDC, runner labels, and `uses:` references.
- Produce stable, human-readable text and JSON output.

---

## File structure

- `Cargo.toml`: binary package metadata and narrow dependency list.
- `src/main.rs`: CLI argument parsing, exit behavior, and renderer selection.
- `src/lib.rs`: public library surface used by unit and integration tests.
- `src/git.rs`: reads only `.github/workflows/*.yml` and `.yaml` paths from a revision using the `git` executable.
- `src/model.rs`: serializable capability and finding types.
- `src/extract.rs`: YAML traversal and capability extraction without shell execution.
- `src/diff.rs`: stable comparison and severity classification.
- `src/render.rs`: text and JSON formatting.
- `tests/fixtures/`: base/head workflow pairs used by integration tests.
- `tests/cli.rs`: invokes the compiled binary against a temporary Git repository.

### Task 1: Establish the executable crate and revision collector

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/git.rs`
- Create: `tests/git.rs`

**Interfaces:**
- Produces: `pub fn workflows_at(repo: &Path, revision: &str) -> Result<BTreeMap<PathBuf, String>, GitError>`.
- Consumes: a local Git repository path and a revision accepted by `git rev-parse`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn reads_only_workflow_yaml_files_at_a_revision() {
    let files = workflows_at(repo.path(), "HEAD").unwrap();
    assert_eq!(files.keys().collect::<Vec<_>>(), vec![&PathBuf::from(".github/workflows/check.yml")]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test git reads_only_workflow_yaml_files_at_a_revision`

Expected: FAIL because the crate and `workflows_at` do not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn workflows_at(repo: &Path, revision: &str) -> Result<BTreeMap<PathBuf, String>, GitError> {
    // use `git ls-tree -r --name-only <revision> -- .github/workflows`
    // then `git show <revision>:<path>` for *.yml and *.yaml files
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test git reads_only_workflow_yaml_files_at_a_revision`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/lib.rs src/git.rs tests/git.rs
git commit -m "feat: read workflows from git revisions"
```

### Task 2: Extract conservative workflow capabilities

**Files:**
- Create: `src/model.rs`
- Create: `src/extract.rs`
- Create: `tests/extract.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `pub fn extract_workflow(path: PathBuf, source: &str) -> WorkflowCapability`.
- Produces: `WorkflowCapability { path, triggers, permissions, jobs, warnings }`.
- Produces: `JobCapability { id, permissions, secrets, oidc, runners, action_refs, untrusted_script_context }`.

- [ ] **Step 1: Write the failing test**

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test extract extracts_job_privileges_from_a_workflow`

Expected: FAIL because `extract_workflow` does not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn extract_workflow(path: PathBuf, source: &str) -> WorkflowCapability {
    // Deserialize to serde_yaml::Value; collect string/map trigger forms,
    // workflow and job permissions, runner labels, `uses`, and `secrets.NAME`.
    // Add a warning instead of panicking on unexpected shapes.
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test extract extracts_job_privileges_from_a_workflow`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/model.rs src/extract.rs src/lib.rs tests/extract.rs
git commit -m "feat: extract workflow capabilities"
```

### Task 3: Compare capabilities and classify risk additions

**Files:**
- Create: `src/diff.rs`
- Create: `tests/diff.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `pub fn diff_workflows(base: &BTreeMap<PathBuf, WorkflowCapability>, head: &BTreeMap<PathBuf, WorkflowCapability>) -> Vec<Finding>`.
- Produces: `Finding { severity, path, job: Option<String>, category, message, remediation }`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn reports_new_write_permission_on_an_existing_job() {
    let findings = diff_workflows(&base, &head);
    assert!(findings.iter().any(|finding| {
        finding.severity == Severity::High && finding.category == "token-permission"
    }));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test diff reports_new_write_permission_on_an_existing_job`

Expected: FAIL because `diff_workflows` does not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn diff_workflows(base: &WorkflowSet, head: &WorkflowSet) -> Vec<Finding> {
    // Report new pull_request_target, write permissions, secret access,
    // id-token: write, self-hosted runner, mutable action reference, and
    // untrusted context used in run. Sort by path, job, category, message.
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test diff reports_new_write_permission_on_an_existing_job`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/diff.rs src/lib.rs tests/diff.rs
git commit -m "feat: detect privilege-increasing workflow changes"
```

### Task 4: Deliver CLI text and JSON reports

**Files:**
- Create: `src/main.rs`
- Create: `src/render.rs`
- Create: `tests/cli.rs`
- Modify: `README.md`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `privilege-diff --repo <path> --base <revision> --head <revision> [--format text|json]`.
- Produces: exit `0` with no high findings, exit `2` when high findings exist, exit `1` for input or parse errors.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn emits_json_and_fails_for_a_high_risk_delta() {
    Command::cargo_bin("privilege-diff").unwrap()
        .args(["--repo", repo.path().to_str().unwrap(), "--base", "HEAD~1", "--head", "HEAD", "--format", "json"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains("\"severity\":\"high\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli emits_json_and_fails_for_a_high_risk_delta`

Expected: FAIL because the binary does not exist.

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(clap::Parser)]
struct Args { repo: PathBuf, base: String, head: String, format: Format }

fn main() -> ExitCode {
    // collect, extract, diff, render; return 2 only when a High finding exists
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli emits_json_and_fails_for_a_high_risk_delta`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/render.rs src/lib.rs tests/cli.rs README.md
git commit -m "feat: add workflow privilege diff CLI"
```

### Task 5: Verify the first milestone

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: a local repository with at least two revisions.
- Produces: documented install, usage, exit-code, and limitation guidance.

- [ ] **Step 1: Add a full runnable usage example**

```markdown
cargo install --path .
privilege-diff --repo . --base HEAD~1 --head HEAD
```

- [ ] **Step 2: Run formatting and the full suite**

Run: `cargo fmt --check && cargo test && cargo clippy -- -D warnings`

Expected: all commands exit `0`.

- [ ] **Step 3: Run an end-to-end manual smoke check**

Run: `cargo run -- --repo . --base HEAD~1 --head HEAD --format json`

Expected: valid JSON output and a documented exit status.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: document first workflow diff release"
```
