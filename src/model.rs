use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct WorkflowCapability {
    pub path: PathBuf,
    pub triggers: BTreeSet<String>,
    pub permissions: BTreeMap<String, String>,
    pub jobs: BTreeMap<String, JobCapability>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Eq, PartialEq, Serialize)]
pub struct JobCapability {
    pub id: String,
    pub permissions: BTreeMap<String, String>,
    pub secrets: BTreeSet<String>,
    pub oidc: bool,
    pub runners: BTreeSet<String>,
    pub action_refs: Vec<String>,
    pub untrusted_script_context: bool,
}
