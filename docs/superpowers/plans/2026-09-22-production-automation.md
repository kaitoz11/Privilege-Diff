# Production Automation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Privilege Diff continuously verified, dependency-audited, and capable of publishing provenance-attested release binaries.

**Architecture:** Keep pull-request validation separate from privileged release publishing. The CI workflow uses read-only permissions and pinned actions; the release workflow runs only for version tags, builds a fixed Linux binary, publishes checksums and a GitHub artifact attestation with narrowly scoped OIDC and attestation permissions.

**Tech Stack:** GitHub Actions, Rust stable, Cargo, `cargo audit`, `cargo deny`, GitHub artifact attestations.

**Spec:** `docs/project-brief.md`

## Global Constraints

- All third-party and GitHub Actions references use a full immutable commit SHA with an inline version comment.
- CI workflows use explicit least-privilege `permissions`; pull-request jobs never receive write, OIDC, attestation, or secrets permissions.
- Release publishes only on `v*` tags and must derive artifacts and checksums in the same job before attesting them.
- Build provenance is documented with a consumer verification command.
- Production automation remains local-first for the CLI: no runtime network or GitHub token requirement is introduced.

---

### Task 1: Establish repository automation policy and dependency checks

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `deny.toml`
- Create: `.github/dependabot.yml`
- Modify: `README.md`

**Interfaces:**
- Consumes: pull requests, pushes to `main`, Cargo lockfile.
- Produces: required CI validation for format, tests, Clippy, `cargo audit`, and `cargo deny check`.

- [ ] **Step 1: Add failing static policy checks**

Create `tests/automation.rs` that loads the workflow/config files and asserts: a top-level empty permission map, SHA-pinned action references, a Rust format/test/Clippy job, audit and deny commands, and Dependabot Cargo updates.

- [ ] **Step 2: Run the static policy test**

Run: `cargo test --test automation`

Expected: FAIL because automation files do not exist.

- [ ] **Step 3: Implement minimal automation**

Create CI with checkout, Rust toolchain, Cargo cache, and separately named quality/dependency jobs. Add a restrictive `deny.toml` for advisories, licenses, sources, and duplicate bans. Configure weekly Dependabot Cargo updates.

- [ ] **Step 4: Verify static policy and Rust suite**

Run: `cargo test --test automation && cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings`

Expected: all exit 0.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml .github/dependabot.yml deny.toml tests/automation.rs README.md
git commit -m "ci: add pinned verification and dependency checks"
```

### Task 2: Add release artifact and provenance workflow

**Files:**
- Create: `.github/workflows/release.yml`
- Modify: `tests/automation.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: pushed tag matching `v*`.
- Produces: `privilege-diff-<target>.tar.gz`, SHA-256 checksum, GitHub Release assets, and artifact attestations.

- [ ] **Step 1: Add failing release-policy tests**

Extend `tests/automation.rs` to assert tag-only trigger, explicit release permissions, pinned checkout/toolchain/attest/upload/release actions, checksum generation, and attestation of the archive and checksum.

- [ ] **Step 2: Run the release policy test**

Run: `cargo test --test automation release_workflow`

Expected: FAIL because release workflow does not exist.

- [ ] **Step 3: Implement the release workflow**

Build `x86_64-unknown-linux-gnu` in a clean Ubuntu runner, package the binary and LICENSE, write a SHA-256 checksum, upload release assets, and use `actions/attest` with `contents: write`, `attestations: write`, `id-token: write`, and only the permissions required by its job.

- [ ] **Step 4: Verify tests and configuration**

Run: `cargo test --test automation && cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings`

Expected: all exit 0.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml tests/automation.rs README.md
git commit -m "ci: add attested release workflow"
```

### Task 3: Verify the production automation baseline

**Files:**
- Modify: `README.md`

**Interfaces:**
- Consumes: released artifact and GitHub repository identity.
- Produces: documented `gh attestation verify` and SHA-256 verification commands.

- [ ] **Step 1: Document artifact verification**

Add exact commands for `sha256sum --check` and `gh attestation verify <archive> -R kaitoz11/Privilege-Diff`.

- [ ] **Step 2: Run full local verification**

Run: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings`

Expected: all exit 0.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document release verification"
```
