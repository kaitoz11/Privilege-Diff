use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use privilege_diff::render::{render_json, render_text};
use privilege_diff::{
    diff_workflows, extract_workflow, workflows_at, Severity, WorkflowCapability,
};

#[derive(Parser)]
#[command(
    version,
    about = "Compare GitHub Actions privileges between local Git revisions"
)]
struct Args {
    /// Local Git repository to read
    #[arg(long)]
    repo: PathBuf,
    /// Base Git revision
    #[arg(long)]
    base: String,
    /// Head Git revision
    #[arg(long)]
    head: String,
    /// Report format
    #[arg(long, value_enum, default_value = "text")]
    format: Format,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}

fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let failed = error.use_stderr();
            if error.print().is_err() || failed {
                return ExitCode::FAILURE;
            }
            return ExitCode::SUCCESS;
        }
    };

    match run(args) {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<ExitCode, String> {
    let base = collect(&args.repo, &args.base, "base")?;
    let head = collect(&args.repo, &args.head, "head")?;
    let findings = diff_workflows(&base, &head);
    let report = match args.format {
        Format::Text => render_text(&findings),
        Format::Json => render_json(&findings)
            .map_err(|error| format!("could not serialize JSON report: {error}"))?,
    };
    io::stdout()
        .lock()
        .write_all(report.as_bytes())
        .map_err(|error| format!("could not write report: {error}"))?;

    Ok(
        if findings
            .iter()
            .any(|finding| finding.severity == Severity::High)
        {
            ExitCode::from(2)
        } else {
            ExitCode::SUCCESS
        },
    )
}

fn collect(
    repo: &Path,
    revision: &str,
    side: &str,
) -> Result<BTreeMap<PathBuf, WorkflowCapability>, String> {
    workflows_at(repo, revision)
        .map_err(|error| format!("{side} revision: {error}"))?
        .into_iter()
        .map(|(path, source)| {
            // The library conservatively warns on parse failures. The CLI must
            // instead return an input error, without exposing YAML source values.
            serde_yaml::from_str::<serde_yaml::Value>(&source).map_err(|error| {
                let location = error.location().map_or_else(String::new, |location| {
                    format!(" at line {}, column {}", location.line(), location.column())
                });
                format!(
                    "{side} revision, {}: could not parse workflow YAML{location}",
                    path.display()
                )
            })?;
            let capability = extract_workflow(path.clone(), &source);
            Ok((path, capability))
        })
        .collect()
}
