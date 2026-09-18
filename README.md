# Privilege Diff

**Review GitHub Actions changes by their effective privilege, not just their YAML diff.**

Privilege Diff is an open-source, local-first CLI that compares
two revisions of a repository's GitHub Actions configuration. It explains whether
a pull request expands who can trigger a workflow, what untrusted input can reach
execution, which credentials or secrets are reachable, and what a compromised
third-party action could do.

## Install and run

The CLI requires Rust/Cargo and Git 2.45 or newer. Install it from this checkout:

```sh
cargo install --path .
privilege-diff --repo /path/to/repository --base HEAD~1 --head HEAD
```

To compare the two most recent commits in the current repository, run:

```sh
privilege-diff --repo . --base HEAD~1 --head HEAD
```

For development without installing the binary, prepend `cargo run --`:

```sh
cargo run -- --repo . --base HEAD~1 --head HEAD
```

`--repo`, `--base`, and `--head` are required. Revisions can be any local Git
revisions Git can resolve, such as `origin/main` and `HEAD`. The repository must
contain both requested revisions. The comparison reads committed `.yml` and
`.yaml` files below `.github/workflows` from those revisions; uncommitted edits
are not included. It does not need a GitHub token or network access.
Git lazy fetching is disabled with Git's `--no-lazy-fetch` global option:
missing objects in a partial clone produce an input error instead of triggering
a fetch. Git versions older than 2.45 reject that option before object access,
so they are not supported. Make the requested revisions and workflow blobs
available locally before running a comparison.

## Output and exit status

`--format text` is the default. It prints each finding's severity, workflow path,
optional job, category, explanation, and suggested control. With no findings it
prints `No privilege-increasing changes detected.`

Use `--format json` for a compact JSON array, terminated by a newline:

```sh
privilege-diff --repo . --base HEAD~1 --head HEAD --format json
```

Each JSON finding has `severity`, `path`, `job`, `category`, `message`, and
`remediation` fields. `job` is `null` for workflow-level findings, and an empty
report is `[]`. Output order is deterministic. Literal environment values and
script bodies are not printed. Findings can include capability identifiers and
references, such as symbolic secret names, action references, and unsupported
permission values.

| Status | Meaning |
| --- | --- |
| `0` | A warning-only report, or no findings. |
| `2` | At least one high-severity finding was reported, including when warnings are also present. The report is still written to stdout. |
| `1` | Invalid arguments, an unreadable/non-repository path, an unresolved revision, or malformed workflow YAML. Errors are written to stderr and no report is emitted. |

`--help` and `--version` exit `0`. In automation, treat `2` as a review result
rather than a command failure that prevents reading stdout.

## Current detection and limitations

This first milestone reports newly introduced high-severity findings for:

- `pull_request_target` triggers;
- `write-all` or individual `*: write` token permissions (except `id-token`, which is reported separately);
- symbolic `secrets.NAME` references inside expressions, including workflow
  environment variables inherited by jobs;
- `id-token: write` / OIDC access;
- self-hosted runners;
- newly added mutable action references, including Docker images not pinned to a `sha256` digest; and
- direct interpolation of selected untrusted `github.*` contexts in `run` scripts.

It also emits warning findings when it encounters supported-field structures it
cannot model confidently, including dynamic runner labels, dynamic/non-symbolic
secret access (such as `toJSON(secrets)`), missing required workflow keys, and
unsupported YAML shapes. Jobs without explicit workflow or job permissions warn
because repository and organization permission defaults are unavailable locally;
an explicit `permissions: {}` remains a known empty restriction.
Existing uncertainty warnings from
the head revision remain visible even when they were also present in the base
revision. A warning-only report deliberately exits `0`, but warnings must be
reviewed; a zero exit code is not a safety guarantee.

Job environment variables override workflow variables with the same name.
Step environment overrides are conservatively warned about, and inherited
secret exposure may be overstated in those jobs. Caller workflow environment
variables are not propagated to reusable workflow jobs. Literal mentions of
`secrets.NAME` or `github.event` outside expressions do not count as access.

The tool is a conservative, static comparison—not a complete GitHub Actions
interpreter. It does not execute workflows, evaluate expressions or shell code,
trace data flow, inspect reusable workflow or composite-action definitions,
resolve action contents, or apply GitHub/organization policy. It only compares
workflow files present in the head revision and reports additions; it does not
report removals or risk reductions. SARIF,
policy-as-code checks, and a GitHub Action wrapper are not included in this
milestone.

## Why this project

CI workflows are executable supply-chain policy. A harmless-looking workflow
change can introduce `pull_request_target`, grant `contents: write`, expose a
secret to a new job, or replace an immutable action SHA with a mutable tag. The
usual code review view makes these changes difficult to reason about together.

Privilege Diff turns a workflow change into a concise security review. The
following is illustrative rather than literal CLI output; actual text output
renders one finding at a time with a category and suggested control:

```text
HIGH  .github/workflows/release.yml
  New reachable capability: repository contents write
  New trust boundary: pull_request_target can run for fork-originated PRs
  New supply-chain dependency: acme/publish-action@v4 (mutable tag)
  Suggested control: use pull_request or keep checkout on the base ref; pin action
```

It complements, rather than replaces, workflow linters such as zizmor and
organization-level GitHub Actions policies.

## Status

The first local CLI milestone supports text and JSON reports. See [the project
brief](docs/project-brief.md) and [research notes](docs/research.md).

## License

Apache-2.0. See [LICENSE](LICENSE).
