use crate::model::{Finding, Severity};

/// Formats findings in their deterministic diff order.
pub fn render_text(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "No privilege-increasing changes detected.\n".to_owned();
    }

    let mut report = String::new();
    for finding in findings {
        let severity = match finding.severity {
            Severity::High => "HIGH",
            Severity::Warning => "WARNING",
        };
        report.push_str(&format!("{severity}  {}", finding.path.display()));
        if let Some(job) = &finding.job {
            report.push_str(&format!(" (job: {job})"));
        }
        report.push_str(&format!(
            " [{}]\n  {}\n  Suggested control: {}\n",
            finding.category, finding.message, finding.remediation
        ));
    }
    report
}

/// Emits a compact JSON array of findings, terminated by a newline.
pub fn render_json(findings: &[Finding]) -> Result<String, serde_json::Error> {
    let mut report = serde_json::to_string(findings)?;
    report.push('\n');
    Ok(report)
}
