# Privilege Diff

**Review GitHub Actions changes by their effective privilege, not just their YAML diff.**

Privilege Diff is an open-source, local-first CLI and GitHub Action that compares
two revisions of a repository's GitHub Actions configuration. It explains whether
a pull request expands who can trigger a workflow, what untrusted input can reach
execution, which credentials or secrets are reachable, and what a compromised
third-party action could do.

## Why this project

CI workflows are executable supply-chain policy. A harmless-looking workflow
change can introduce `pull_request_target`, grant `contents: write`, expose a
secret to a new job, or replace an immutable action SHA with a mutable tag. The
usual code review view makes these changes difficult to reason about together.

Privilege Diff turns a workflow change into a concise security review:

```text
HIGH  .github/workflows/release.yml
  New reachable capability: repository contents write
  New trust boundary: pull_request_target can run for fork-originated PRs
  New supply-chain dependency: acme/publish-action@v4 (mutable tag)
  Suggested control: use pull_request or keep checkout on the base ref; pin action
```

It complements, rather than replaces, workflow linters such as zizmor and
organization-level GitHub Actions policies.

## Planned first release

1. Parse GitHub Actions workflow and composite-action YAML from a base and head revision.
2. Build a conservative per-job capability model: triggers, token permissions,
   secret references, OIDC access, runner class, `uses:` dependencies, and
   untrusted-context execution.
3. Report additions, removals, and risk-increasing changes in terminal text and
   SARIF.
4. Provide policy-as-code checks for baseline rules: no mutable action refs,
   no new privileged fork-triggered execution, and no new secret access without
   an explicit review annotation.
5. Ship as a standalone CLI first; the GitHub Action is a thin wrapper.

## Non-goals for v0.1

- Runtime network monitoring or secret rotation.
- Replacing GitHub organization policy, branch protection, CodeQL, or zizmor.
- Proving that arbitrary shell code is safe.
- Connecting to GitHub by default; local Git input is the primary interface.

## Initial technical direction

Rust, because the project benefits from a single static binary, reliable YAML
handling, and easy SARIF output. The CLI will accept local Git revisions, e.g.
`privilege-diff --base origin/main --head HEAD`, so it can run in any CI system
without a GitHub token. GitHub API enrichment will remain opt-in.

## Status

Planning. See [the project brief](docs/project-brief.md) and
[research notes](docs/research.md). Contributions and early design discussion
are welcome once the repository is published to GitHub.

## License

Apache-2.0. See [LICENSE](LICENSE).
