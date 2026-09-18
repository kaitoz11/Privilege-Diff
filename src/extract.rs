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
            let location = error.location().map_or_else(String::new, |location| {
                format!(" at line {}, column {}", location.line(), location.column())
            });
            capability
                .warnings
                .push(format!("could not parse workflow YAML{location}"));
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
    } else {
        capability
            .warnings
            .push("workflow is missing required on triggers".to_owned());
    }
    if let Some(permission_value) = value_at(workflow, "permissions") {
        capability.permissions_explicit = true;
        (capability.permissions, capability.permissions_all) =
            collect_permissions(permission_value, "workflow", &mut capability.warnings);
    }
    if let Some(jobs) = value_at(workflow, "jobs") {
        let inherited_permissions = capability.permissions.clone();
        let inherited_permissions_all = capability.permissions_all.clone();
        collect_jobs(
            jobs,
            &inherited_permissions,
            inherited_permissions_all,
            value_at(workflow, "env"),
            &mut capability,
        );
    } else {
        capability
            .warnings
            .push("workflow is missing required jobs".to_owned());
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
) -> (BTreeMap<String, String>, Option<String>) {
    if let Some(access @ ("read-all" | "write-all")) = value.as_str() {
        return (BTreeMap::new(), Some(access.to_owned()));
    }
    let Some(permissions) = value.as_mapping() else {
        warnings.push(format!("{context} permissions must be a mapping"));
        return (BTreeMap::new(), None);
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
    (scopes, None)
}

fn collect_jobs(
    value: &Value,
    inherited_permissions: &BTreeMap<String, String>,
    inherited_permissions_all: Option<String>,
    workflow_env: Option<&Value>,
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
            permissions_all: inherited_permissions_all.clone(),
            permissions_explicit: capability.permissions_explicit,
            ..JobCapability::default()
        };
        if let Some(permissions) = value_at(job, "permissions") {
            job_capability.permissions_explicit = true;
            // A job permission map replaces the workflow map; omitted scopes are none.
            (job_capability.permissions, job_capability.permissions_all) =
                collect_permissions(permissions, &format!("job {id}"), &mut capability.warnings);
        }
        if !job_capability.permissions_explicit {
            capability.warnings.push(format!(
                "job {id} permission defaults from the repository or organization are unknown"
            ));
        }
        job_capability.oidc = job_capability
            .permissions
            .get("id-token")
            .is_some_and(|access| access == "write")
            || job_capability.permissions_all.as_deref() == Some("write-all");

        if let Some(runner) = value_at(job, "runs-on") {
            collect_runners(
                runner,
                id,
                &mut job_capability.runners,
                &mut capability.warnings,
            );
        }
        collect_job_values(job, id, &mut job_capability, &mut capability.warnings);
        collect_inherited_env(
            workflow_env,
            job,
            id,
            &mut job_capability,
            &mut capability.warnings,
        );
        capability.jobs.insert(id.to_owned(), job_capability);
    }
}

fn collect_inherited_env(
    workflow_env: Option<&Value>,
    job: &Mapping,
    job_id: &str,
    capability: &mut JobCapability,
    warnings: &mut Vec<String>,
) {
    // Caller workflow env is not passed into a reusable workflow.
    if value_at(job, "uses").is_some() {
        return;
    }
    let Some(workflow_env) = workflow_env else {
        return;
    };
    let Some(workflow_env) = workflow_env.as_mapping() else {
        warnings.push("workflow env must be a mapping; secret exposure is unknown".to_owned());
        return;
    };
    let mut effective_env = workflow_env.clone();
    if let Some(job_env) = value_at(job, "env") {
        if let Some(job_env) = job_env.as_mapping() {
            effective_env.extend(job_env.clone());
        } else {
            warnings.push(format!(
                "job {job_id} env override is unsupported; secret exposure is unknown"
            ));
        }
    }
    let has_step_override = value_at(job, "steps")
        .and_then(Value::as_sequence)
        .is_some_and(|steps| {
            steps.iter().any(|step| {
                step.as_mapping()
                    .and_then(|step| value_at(step, "env"))
                    .is_some_and(|env| {
                        env.as_mapping()
                            .is_none_or(|env| env.keys().any(|key| effective_env.contains_key(key)))
                    })
            })
        });
    if has_step_override {
        warnings.push(format!("job {job_id} step env override is not modeled; inherited secret exposure is conservative"));
    }
    let effective_env = Value::Mapping(effective_env);
    collect_secret_references(&effective_env, &mut capability.secrets);
    if has_non_symbolic_secret_access(&effective_env) {
        warnings.push(format!(
            "job {job_id} env uses non-symbolic secrets with unknown secret access"
        ));
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
        Value::String(text) => context_accesses(text, "secrets")
            .iter()
            .any(|name| name.is_none_or(|name| !is_secret_name(name))),
        Value::Sequence(values) => values.iter().any(has_non_symbolic_secret_access),
        Value::Mapping(values) => values.iter().any(|(key, value)| {
            has_non_symbolic_secret_access(key) || has_non_symbolic_secret_access(value)
        }),
        _ => false,
    }
}

fn collect_secret_names(text: &str, secrets: &mut BTreeSet<String>) {
    for name in context_accesses(text, "secrets").into_iter().flatten() {
        if is_secret_name(name) {
            secrets.insert(name.to_owned());
        }
    }
}

fn is_secret_name(name: &str) -> bool {
    name.starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn has_untrusted_context(script: &str) -> bool {
    context_accesses(script, "github").iter().any(|property| {
        // Whole-context and dynamic access can include attacker-controlled fields.
        property.is_none_or(|property| {
            ["event", "head_ref", "base_ref", "actor", "triggering_actor"]
                .iter()
                .any(|untrusted| property.eq_ignore_ascii_case(untrusted))
        })
    })
}

/// Static root-context property accesses within expressions only. None means
/// a whole-context or dynamic access that cannot name a single property.
fn context_accesses<'a>(text: &'a str, context: &str) -> Vec<Option<&'a str>> {
    let mut accesses = Vec::new();
    for expression in expression_regions(text) {
        let tokens = expression_tokens(expression);
        for (index, token) in tokens.iter().enumerate() {
            if !token.eq_ignore_ascii_case(context) || (index > 0 && tokens[index - 1] == ".") {
                continue;
            }
            let property = if tokens.get(index + 1) == Some(&".") {
                tokens.get(index + 2).copied().filter(|property| {
                    property.starts_with(|character: char| {
                        character.is_ascii_alphabetic() || character == '_'
                    })
                })
            } else {
                None
            };
            accesses.push(property);
        }
    }
    accesses
}

fn expression_regions(text: &str) -> Vec<&str> {
    let mut expressions = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("${{") {
        remaining = &remaining[start + 3..];
        let bytes = remaining.as_bytes();
        let mut quoted = false;
        let mut end = 0;
        while end < bytes.len() {
            if bytes[end] == b'\'' {
                // A doubled quote toggles twice, preserving the quoted region.
                quoted = !quoted;
            } else if !quoted && bytes[end..].starts_with(b"}}") {
                break;
            }
            end += 1;
        }
        if end == bytes.len() {
            break;
        }
        expressions.push(&remaining[..end]);
        remaining = &remaining[end + 2..];
    }
    expressions
}

fn expression_tokens(expression: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut remaining = expression;
    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        let Some(first) = remaining.chars().next() else {
            break;
        };
        let length = if first == '\'' {
            let bytes = remaining.as_bytes();
            let mut end = 1;
            while end < bytes.len() {
                if bytes[end] == b'\'' {
                    end += 1;
                    if bytes.get(end) != Some(&b'\'') {
                        break;
                    }
                }
                end += 1;
            }
            end
        } else if first.is_ascii_alphanumeric() || first == '_' {
            remaining
                .bytes()
                .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
                .count()
        } else {
            first.len_utf8()
        };
        tokens.push(&remaining[..length]);
        remaining = &remaining[length..];
    }
    tokens
}

fn value_at<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.to_owned()))
}
