use crate::model::{Finding, Severity};

fn escape_terminal_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\0'..='\u{1f}' | '\u{7f}' => {
                use std::fmt::Write;

                write!(escaped, "\\x{:02x}", character as u32)
                    .expect("writing to a String cannot fail");
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

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
        report.push_str(&format!(
            "{severity}  {}",
            escape_terminal_text(&finding.path.display().to_string())
        ));
        if let Some(job) = &finding.job {
            report.push_str(&format!(" (job: {})", escape_terminal_text(job)));
        }
        report.push_str(&format!(
            " [{}]\n  {}\n  Suggested control: {}\n",
            escape_terminal_text(&finding.category),
            escape_terminal_text(&finding.message),
            escape_terminal_text(&finding.remediation)
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::render_text;
    use crate::model::{Finding, Severity};

    #[test]
    fn text_rendering_escapes_control_characters_from_finding_fields() {
        let findings = [Finding {
            severity: Severity::High,
            path: PathBuf::from(".github/workflows/check\nname\u{1b}[31m.yml"),
            job: Some("release\rjob".to_owned()),
            category: "token\u{7f}permission".to_owned(),
            message: "new\tpermission\u{1b}[0m".to_owned(),
            remediation: "use\u{0001}least privilege".to_owned(),
        }];

        assert_eq!(
            render_text(&findings),
            "HIGH  .github/workflows/check\\nname\\x1b[31m.yml (job: release\\rjob) [token\\x7fpermission]\n  new\\tpermission\\x1b[0m\n  Suggested control: use\\x01least privilege\n"
        );
    }
}
