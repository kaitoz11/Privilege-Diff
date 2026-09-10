# Research notes — 2026-09-11

## Evidence

1. GitHub’s security documentation warns that untrusted `github` context values
   can become script-injection inputs, and that `pull_request_target` requires
   least privilege, restricted secrets, and isolated ephemeral runners.
2. GitHub added organization policy controls to block actions and require full
   commit-SHA pinning in 2025. That protects policy-governed repositories, but
   it does not explain the total security impact of a proposed workflow change.
3. The 2025 compromise of `tj-actions/changed-files` demonstrated the impact of
   a compromised third-party action; analyses reported that tags were redirected
   to malicious commits and secrets could be exposed.
4. zizmor already provides capable snapshot-oriented static analysis for GitHub
   Actions, including injection, credential, permission, and ref issues.
5. OpenSSF Scorecard includes project-level token-permission and workflow-health
   checks, rather than a PR-specific semantic privilege review.

## Project implication

The differentiator must not be “another YAML linter.” Privilege Diff’s narrow
scope is to compare **base vs. proposed workflow capability** and explain the
review consequence. It treats existing scanners and GitHub policy as inputs and
complements, not competitors.

## Sources

- [GitHub: script-injection risks](https://docs.github.com/en/actions/concepts/security/script-injections)
- [GitHub: secure use of `pull_request_target`](https://docs.github.com/en/actions/reference/security/securely-using-pull_request_target)
- [GitHub: Actions SHA-pinning and blocking policy](https://github.blog/changelog/2025-08-15-github-actions-policy-now-supports-blocking-and-sha-pinning-actions/)
- [GitHub: security reference](https://docs.github.com/en/actions/reference/security)
- [Palo Alto Networks Unit 42: tj-actions incident](https://unit42.paloaltonetworks.com/github-actions-supply-chain-attack/)
- [zizmor](https://github.com/zizmorcore/zizmor)
- [OpenSSF Scorecard](https://github.com/ossf/scorecard)
