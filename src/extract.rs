use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_yaml::{Mapping, Value};

use crate::model::{JobCapability, WorkflowCapability};

/// Extracts the supported GitHub Actions capability fields without evaluating
/// expressions or reading any secret values.
pub fn extract_workflow(path: PathBuf, source: &str) -> WorkflowCapability {
    let mut capability = WorkflowCapability {
        path,
        ..WorkflowCapability::default()
    };

    let document = match serde_yaml::from_str::<Value>(source) {
        Ok(document) => document,
        Err(error) => {
            capability
                .warnings
                .push(format!("could not parse workflow YAML: {error}"));
            return capability;
        }
    };
    let Some(workflow) = document.as_mapping() else {
        capability
            .warnings
            .push("workflow root must be a mapping".to_owned());
        return capability;
    };

    if let Some(trigger_value) = value_at(workflow, "on") {
        collect_triggers(
            trigger_value,
            &mut capability.triggers,
            &mut capability.warnings,
        );
    }
    if let Some(permission_value) = value_at(workflow, "permissions") {
        capability.permissions =
            collect_permissions(permission_value, "workflow", &mut capability.warnings);
    }
    if let Some(jobs) = value_at(workflow, "jobs") {
        let inherited_permissions = capability.permissions.clone();
        collect_jobs(jobs, &inherited_permissions, &mut capability);
    }

    capability
}

fn collect_triggers(value: &Value, triggers: &mut BTreeSet<String>, warnings: &mut Vec<String>) {
    match value {
        Value::String(trigger) => {
            triggers.insert(trigger.clone());
        }
        Value::Sequence(values) => {
            for trigger in values {
                if let Some(trigger) = trigger.as_str() {
                    triggers.insert(trigger.to_owned());
                } else {
                    warnings.push("workflow trigger list contains a non-string value".to_owned());
                }
            }
        }
        Value::Mapping(events) => {
            for (trigger, configuration) in events {
                if let Some(trigger) = trigger.as_str() {
                    triggers.insert(trigger.to_owned());
                    if !matches!(configuration, Value::Null | Value::Mapping(_)) {
                        warnings.push(format!(
                            "workflow trigger {trigger} configuration must be a mapping or null"
                        ));
                    }
                } else {
                    warnings
                        .push("workflow trigger map contains a non-string event name".to_owned());
                }
            }
        }
        _ => warnings.push("workflow triggers must be a string, list, or mapping".to_owned()),
    }
}

fn collect_permissions(
    value: &Value,
    context: &str,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, String> {
    let Some(permissions) = value.as_mapping() else {
        warnings.push(format!("{context} permissions must be a mapping"));
        return BTreeMap::new();
    };

    let mut scopes = BTreeMap::new();
    for (scope, access) in permissions {
        match (scope.as_str(), access.as_str()) {
            (Some(scope), Some(access @ ("read" | "write" | "none"))) => {
                scopes.insert(scope.to_owned(), access.to_owned());
            }
            (Some(scope), Some(access)) => {
                warnings.push(format!(
                    "{context} permission {scope} has unsupported access level {access}"
                ));
                scopes.insert(scope.to_owned(), access.to_owned());
            }
            _ => warnings.push(format!(
                "{context} permissions contain a non-string scope or access level"
            )),
        }
    }
    scopes
}

fn collect_jobs(
    value: &Value,
    inherited_permissions: &BTreeMap<String, String>,
    capability: &mut WorkflowCapability,
) {
    let Some(jobs) = value.as_mapping() else {
        capability
            .warnings
            .push("workflow jobs must be a mapping".to_owned());
        return;
    };

    for (id, value) in jobs {
        let Some(id) = id.as_str() else {
            capability
                .warnings
                .push("workflow jobs contain a non-string job id".to_owned());
            continue;
        };
        let Some(job) = value.as_mapping() else {
            capability
                .warnings
                .push(format!("job {id} must be a mapping"));
            continue;
        };

        let mut job_capability = JobCapability {
            id: id.to_owned(),
            permissions: inherited_permissions.clone(),
            ..JobCapability::default()
        };
        if let Some(permissions) = value_at(job, "permissions") {
            // A job permission map replaces the workflow map; omitted scopes are none.
            job_capability.permissions =
                collect_permissions(permissions, &format!("job {id}"), &mut capability.warnings);
        }
        job_capability.oidc = job_capability
            .permissions
            .get("id-token")
            .is_some_and(|access| access == "write");

        if let Some(runner) = value_at(job, "runs-on") {
            collect_runners(
                runner,
                id,
                &mut job_capability.runners,
                &mut capability.warnings,
            );
        }
        collect_job_values(job, id, &mut job_capability, &mut capability.warnings);
        capability.jobs.insert(id.to_owned(), job_capability);
    }
}

fn collect_runners(
    value: &Value,
    job_id: &str,
    runners: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
) {
    match value {
        Value::String(runner) => {
            collect_runner_label(runner, job_id, runners, warnings);
        }
        Value::Sequence(values) => {
            for runner in values {
                if let Some(runner) = runner.as_str() {
                    collect_runner_label(runner, job_id, runners, warnings);
                } else {
                    warnings.push(format!(
                        "job {job_id} runner list contains a non-string label"
                    ));
                }
            }
        }
        Value::Mapping(runner) => {
            for (key, value) in runner {
                let Some(key) = key.as_str() else {
                    warnings.push(format!("job {job_id} runs-on contains a non-string key"));
                    continue;
                };
                match key {
                    "group" => match value.as_str() {
                        Some(group) if !is_dynamic(group) => warnings.push(format!(
                            "job {job_id} runner group is an unsupported runner-group capability"
                        )),
                        Some(_) => warnings.push(format!("job {job_id} runner group is dynamic")),
                        None => {
                            warnings.push(format!("job {job_id} runner group must be a string"))
                        }
                    },
                    "labels" => collect_runners(value, job_id, runners, warnings),
                    _ => warnings.push(format!(
                        "job {job_id} runs-on contains unsupported key {key}"
                    )),
                }
            }
        }
        _ => warnings.push(format!("job {job_id} runs-on must be a string or list")),
    }
}

fn collect_runner_label(
    runner: &str,
    job_id: &str,
    runners: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
) {
    if is_dynamic(runner) {
        warnings.push(format!("job {job_id} runner label is dynamic"));
    } else {
        runners.insert(runner.to_owned());
    }
}

fn is_dynamic(value: &str) -> bool {
    value.contains("${{")
}

fn collect_job_values(
    job: &Mapping,
    job_id: &str,
    capability: &mut JobCapability,
    warnings: &mut Vec<String>,
) {
    collect_secret_references(&Value::Mapping(job.clone()), &mut capability.secrets);
    if value_at(job, "secrets").and_then(Value::as_str) == Some("inherit") {
        warnings.push(format!(
            "job {job_id} inherits secrets with unknown secret access"
        ));
    }
    if has_non_symbolic_secret_access(&Value::Mapping(job.clone())) {
        warnings.push(format!(
            "job {job_id} uses non-symbolic secrets with unknown secret access"
        ));
    }

    if let Some(uses) = value_at(job, "uses") {
        collect_action_reference(uses, job_id, &mut capability.action_refs, warnings);
    }
    if let Some(steps) = value_at(job, "steps") {
        let Some(steps) = steps.as_sequence() else {
            warnings.push(format!("job {job_id} steps must be a list"));
            return;
        };
        for step in steps {
            let Some(step) = step.as_mapping() else {
                warnings.push(format!("job {job_id} steps contain a non-mapping value"));
                continue;
            };
            if let Some(uses) = value_at(step, "uses") {
                collect_action_reference(uses, job_id, &mut capability.action_refs, warnings);
            }
            if let Some(run) = value_at(step, "run") {
                match run.as_str() {
                    Some(script) => {
                        capability.untrusted_script_context |= has_untrusted_context(script)
                    }
                    None => warnings.push(format!("job {job_id} step run must be a string")),
                }
            }
        }
    }
}

fn collect_action_reference(
    value: &Value,
    job_id: &str,
    action_refs: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    match value.as_str() {
        Some(action) => action_refs.push(action.to_owned()),
        None => warnings.push(format!("job {job_id} uses must be a string")),
    }
}

fn collect_secret_references(value: &Value, secrets: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => collect_secret_names(text, secrets),
        Value::Sequence(values) => {
            for value in values {
                collect_secret_references(value, secrets);
            }
        }
        Value::Mapping(values) => {
            for (key, value) in values {
                collect_secret_references(key, secrets);
                collect_secret_references(value, secrets);
            }
        }
        _ => {}
    }
}

fn has_non_symbolic_secret_access(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains("secrets[") || text.contains("secrets ["),
        Value::Sequence(values) => values.iter().any(has_non_symbolic_secret_access),
        Value::Mapping(values) => values.iter().any(|(key, value)| {
            has_non_symbolic_secret_access(key) || has_non_symbolic_secret_access(value)
        }),
        _ => false,
    }
}

fn collect_secret_names(text: &str, secrets: &mut BTreeSet<String>) {
    let mut remaining = text;
    while let Some(index) = remaining.find("secrets.") {
        let candidate = &remaining[index + "secrets.".len()..];
        let name_len = candidate
            .bytes()
            .take_while(|byte| {
                byte.is_ascii_uppercase()
                    || byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || *byte == b'_'
            })
            .count();
        if name_len > 0 {
            let name = &candidate[..name_len];
            if name.as_bytes()[0].is_ascii_alphabetic() || name.starts_with('_') {
                secrets.insert(name.to_owned());
            }
        }
        remaining = candidate;
    }
}

fn has_untrusted_context(script: &str) -> bool {
    [
        "github.event",
        "github.head_ref",
        "github.base_ref",
        "github.actor",
        "github.triggering_actor",
    ]
    .iter()
    .any(|context| script.contains(context))
}

fn value_at<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_owned()))
}
