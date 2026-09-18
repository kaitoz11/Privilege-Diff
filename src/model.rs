use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub path: PathBuf,
    pub job: Option<String>,
    pub category: String,
    pub message: String,
    pub remediation: String,
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct WorkflowCapability {
    pub path: PathBuf,
    pub triggers: BTreeSet<String>,
    pub permissions: BTreeMap<String, String>,
    pub permissions_all: Option<String>,
    /// False means repository/organization defaults are unavailable locally.
    pub permissions_explicit: bool,
    pub jobs: BTreeMap<String, JobCapability>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct JobCapability {
    pub id: String,
    pub permissions: BTreeMap<String, String>,
    pub permissions_all: Option<String>,
    /// True when permissions are declared on this job or inherited from the workflow.
    pub permissions_explicit: bool,
    pub secrets: BTreeSet<String>,
    pub oidc: bool,
    pub runners: BTreeSet<String>,
    pub action_refs: Vec<String>,
    pub untrusted_script_context: bool,
}
