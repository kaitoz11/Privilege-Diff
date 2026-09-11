use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::model::{Finding, JobCapability, Severity, WorkflowCapability};

/// Compares extracted workflow capabilities and reports only newly introduced
/// privileges or analysis warnings. This function is pure and performs no I/O.
pub fn diff_workflows(
    base: &BTreeMap<PathBuf, WorkflowCapability>,
    head: &BTreeMap<PathBuf, WorkflowCapability>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (path, head_workflow) in head {
        let base_workflow = base.get(path);

        if head_workflow.triggers.contains("pull_request_target")
            && !base_workflow
                .is_some_and(|workflow| workflow.triggers.contains("pull_request_target"))
        {
            findings.push(workflow_finding(
                path,
                Severity::High,
                "trigger-trust",
                "Added pull_request_target, which can run for pull requests from forks.",
                "Prefer pull_request; if pull_request_target is required, restrict permissions, secrets, and checkout to trusted base-repository code.",
            ));
        }

        for warning in BTreeSet::from_iter(head_workflow.warnings.iter()) {
            findings.push(workflow_finding(
                path,
                Severity::Warning,
                "unknown-capability",
                format!("Workflow analysis warning: {warning}"),
                "Resolve the unsupported or dynamic construct before relying on this comparison.",
            ));
        }

        for (job_id, head_job) in &head_workflow.jobs {
            let base_job = base_workflow.and_then(|workflow| workflow.jobs.get(job_id));
            diff_job(path, job_id, base_job, head_job, &mut findings);
        }
    }

    findings.sort_by(|left, right| {
        (&left.path, &left.job, &left.category, &left.message).cmp(&(
            &right.path,
            &right.job,
            &right.category,
            &right.message,
        ))
    });
    findings
}

fn diff_job(
    path: &Path,
    job_id: &str,
    base: Option<&JobCapability>,
    head: &JobCapability,
    findings: &mut Vec<Finding>,
) {
    let base_permissions = base.map(|job| &job.permissions);
    let base_grants_write_all =
        base.is_some_and(|job| job.permissions_all.as_deref() == Some("write-all"));
    if head.permissions_all.as_deref() == Some("write-all") && !base_grants_write_all {
        findings.push(job_finding(
            path,
            job_id,
            Severity::High,
            "token-permission",
            "Added write-all token permissions.",
            "Grant only the specific write scopes required by this narrowly scoped, reviewed job.",
        ));
    }
    for (scope, access) in &head.permissions {
        if scope != "id-token"
            && access == "write"
            && !base_grants_write_all
            && base_permissions
                .and_then(|permissions| permissions.get(scope))
                .is_none_or(|base_access| base_access != "write")
        {
            findings.push(job_finding(
                path,
                job_id,
                Severity::High,
                "token-permission",
                format!("Added {scope}: write token permission."),
                "Set the scope to read or none and grant write only in a narrowly scoped, reviewed job.",
            ));
        }
    }

    let base_secrets = base
        .map(|job| job.secrets.iter().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    for secret in &head.secrets {
        if !base_secrets.contains(secret) {
            findings.push(job_finding(
                path,
                job_id,
                Severity::High,
                "secret-access",
                format!("Added access to secret {secret}."),
                "Avoid exposing the secret to this job; use a narrowly scoped environment or a separate trusted workflow.",
            ));
        }
    }

    if head.oidc && !base.is_some_and(|job| job.oidc) {
        findings.push(job_finding(
            path,
            job_id,
            Severity::High,
            "oidc",
            "Added id-token: write, allowing this job to request OIDC tokens.",
            "Remove id-token: write unless federation is required, and constrain the cloud trust policy to this job.",
        ));
    }

    if head.runners.contains("self-hosted")
        && !base.is_some_and(|job| job.runners.contains("self-hosted"))
    {
        findings.push(job_finding(
            path,
            job_id,
            Severity::High,
            "self-hosted-runner",
            "Added a self-hosted runner.",
            "Use a GitHub-hosted runner; if self-hosted is required, isolate and ephemerally provision it.",
        ));
    }

    let base_actions = base
        .map(|job| job.action_refs.iter().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    for action in BTreeSet::from_iter(head.action_refs.iter()) {
        if is_mutable_action_ref(action) && !base_actions.contains(action) {
            findings.push(job_finding(
                path,
                job_id,
                Severity::High,
                "mutable-action-reference",
                format!("Added mutable action reference {action}."),
                mutable_action_remediation(action),
            ));
        }
    }

    if head.untrusted_script_context && !base.is_some_and(|job| job.untrusted_script_context) {
        findings.push(job_finding(
            path,
            job_id,
            Severity::High,
            "untrusted-context",
            "Added untrusted GitHub context interpolation in a run script.",
            "Pass untrusted context through an environment variable and validate it instead of interpolating it directly into run.",
        ));
    }
}

fn is_mutable_action_ref(action: &str) -> bool {
    if let Some((_, reference)) = action.rsplit_once('@') {
        return !is_full_commit_sha(reference) && !is_sha256_digest(reference);
    }

    action.starts_with("docker://")
}

fn mutable_action_remediation(action: &str) -> &'static str {
    if action.starts_with("docker://") {
        "Pin the container image to an immutable sha256 digest."
    } else {
        "Pin the action to a full-length commit SHA."
    }
}

fn is_full_commit_sha(reference: &str) -> bool {
    reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_sha256_digest(reference: &str) -> bool {
    reference.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn workflow_finding(
    path: &Path,
    severity: Severity,
    category: &str,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> Finding {
    Finding {
        severity,
        path: path.to_path_buf(),
        job: None,
        category: category.to_owned(),
        message: message.into(),
        remediation: remediation.into(),
    }
}

fn job_finding(
    path: &Path,
    job_id: &str,
    severity: Severity,
    category: &str,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> Finding {
    Finding {
        severity,
        path: path.to_path_buf(),
        job: Some(job_id.to_owned()),
        category: category.to_owned(),
        message: message.into(),
        remediation: remediation.into(),
    }
}
