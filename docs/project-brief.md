# Privilege Diff: project brief

## Problem

GitHub Actions configuration changes can alter a repository's effective
privilege without making the impact obvious in a line-oriented YAML diff. This
is especially important when a PR adds a privileged trigger, expands a
`GITHUB_TOKEN` scope, exposes a secret, enables OIDC, adopts a self-hosted
runner, or changes an action reference from a commit SHA to a mutable tag.

Existing tools are valuable but solve adjacent problems:

- **zizmor** finds insecure workflow patterns in a snapshot.
- **OpenSSF Scorecard** measures project security posture.
- **GitHub Actions policies** centrally restrict allowed actions and can enforce
  SHA pinning.

None makes the *semantic privilege delta between two workflow revisions* the
primary review artifact. That is the narrow wedge for this project.

## Chosen direction

Build **Privilege Diff**, a local-first GitHub Actions security-diff engine.
It analyzes the base and head revisions, creates a conservative capability
record for every job, and produces findings only when a capability is added or
risk materially increases.

### User journey

1. A contributor changes `.github/workflows/release.yml`.
2. CI runs `privilege-diff --base <merge-base> --head HEAD`.
3. The tool maps both revisions into capability records.
4. It emits a focused report: added privilege, changed trust boundary, affected
   file/job, evidence, severity, and a suggested safer pattern.
5. Repository policy can fail the check for high-risk changes unless a
   narrowly scoped, reviewed acknowledgement is present.

## Alternatives considered

### 1. General workflow linter

Broader reach, but a mature ecosystem already covers this. It creates recurring
noise from pre-existing findings and does not focus reviewer attention on what
a specific PR changed.

### 2. Action pinning/updater

Useful after attacks involving mutable tags, but GitHub organization policies
and Dependabot already cover much of the control. It would be too narrow for a
new standalone project.

### 3. Privilege-aware diff engine — selected

The output is directly usable in code review, works locally without credentials,
and combines several individually understandable changes into the security
meaning reviewers care about. It can call or coexist with other scanners.

## Architecture

```text
Git revisions
    │
    ├── workflow collector ──► YAML normalizer ──► capability extractor
    │                                                   │
    │                                                   ▼
    └── policy config ───────────────────────────► semantic diff engine
                                                        │
                                ┌───────────────────────┼──────────────────────┐
                                ▼                       ▼                      ▼
                           terminal report            SARIF              CI exit code
```

Components have strict roles:

- **Collector:** loads workflow and composite-action files from Git revisions.
- **Normalizer:** preserves source locations and resolves GitHub Actions YAML
  quirks without executing expressions.
- **Capability extractor:** produces a per-job model for trigger trust, token
  scopes, secret/OIDC usage, runner exposure, action references, and risky
  data-flow signals.
- **Semantic diff engine:** classifies additions, removals, and risk changes;
  it never treats an unknown construct as safe.
- **Policy layer:** lets teams tighten severity and require acknowledgements.
- **Renderers:** produce human-readable review output and SARIF.

## Initial capability model

| Dimension | Diff considers risky when |
|---|---|
| Trigger trust | `pull_request_target`, issue/comment events, or a new attacker-controlled path is introduced |
| Token | a scope is added or changes from read to write |
| Secrets | a secret is newly referenced, particularly in a job reachable from untrusted input |
| OIDC | `id-token: write` becomes available |
| Runner | a job moves to self-hosted or a less isolated runner class |
| Action dependency | a mutable ref is introduced, a SHA becomes a tag, or an owner/repository changes |
| Execution | attacker-controlled GitHub context can be interpolated into a script or privileged action input |

## Error handling and safety

- Parse failures are findings with source locations, not silent omissions.
- Unsupported expressions produce an `unknown` capability that policies can
  treat as warn or fail.
- The default mode reads local Git objects only and does not transmit workflow
  contents, secrets, or repository metadata.
- The tool never expands secret values and only records symbolic references.

## Testing strategy

- Golden fixtures for safe and unsafe base/head pairs.
- Unit tests per extractor and diff rule.
- Parser fuzzing for YAML and expression-heavy workflows.
- Integration tests that assert stable terminal, JSON, and SARIF output.
- Regression fixtures for public GitHub Actions security advisories.

## Delivery sequence

1. Capability schema, revision collector, and text diff for triggers,
   permissions, action pins, and secrets.
2. Policy file, JSON/SARIF output, and GitHub Action wrapper.
3. OIDC, runner, composite-action, reusable-workflow, and data-flow coverage.

## Open-source operating model

- Apache-2.0 license.
- A `SECURITY.md` with private reporting guidance.
- Reproducible releases with provenance and checksums once a release pipeline
  exists.
- Conservative versioning: a new rule starts as informational or warning until
  it is well tested and documented.
