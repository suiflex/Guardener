//! Runs the ForgeGuard gate over a pull request's changed lines and reports it.
//!
//! Two surfaces, because they answer different questions. The check run puts
//! each finding on the line that caused it, which is where someone fixing it is
//! looking, and its conclusion is what a branch protection rule can require.
//! The comment answers "what is wrong with this pull request" in one place for
//! someone reading the conversation rather than the diff.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};
use forgeguard_core::model::{GateReport, GateStatus, Severity};
use forgeguard_core::run_changed_gate;

use crate::config::{self, Registry};
use crate::github::{split_repo, Annotation, CheckRun, Client, ANNOTATION_LIMIT};

pub const MARKER: &str = "<!-- guardener:check -->";
const CHECK_NAME: &str = "guardener / forgeguard";

pub struct Request<'a> {
    pub root: &'a Path,
    pub registry: &'a Path,
    pub organization_default: &'a Path,
    pub repo: &'a str,
    pub pull_request: u64,
    pub head_sha: &'a str,
    pub base: &'a str,
    /// Set for pull requests from outside the organization. ForgeGuard's
    /// `[[commands]]` are shell commands read from the checkout's own
    /// configuration, so running them on a fork's branch would execute code the
    /// author of that branch controls. The tree-sitter scan reads and never
    /// executes, so it stays on.
    pub untrusted: bool,
}

pub fn run(client: &Client, request: &Request<'_>) -> Result<()> {
    let (owner, name) = split_repo(request.repo)?;
    let registry = Registry::load(request.registry)?;
    let config = config::resolve(
        request.root,
        request.organization_default,
        request.repo,
        registry.find(request.repo),
    )?;

    let report = run_changed_gate(request.root, &config, request.untrusted, Some(request.base))
        .context("the ForgeGuard gate failed to run")?;

    client.create_check_run(
        owner,
        name,
        &CheckRun {
            name: CHECK_NAME,
            head_sha: request.head_sha,
            conclusion: conclusion(report.status),
            title: &title(&report),
            summary: &summary(&report, request.untrusted),
            annotations: &annotations(&report),
        },
    )?;

    client.upsert_comment(owner, name, request.pull_request, MARKER, comment(&report))
}

/// A blocked gate fails the check; a gate that only found warnings is reported
/// as neutral rather than failing, so that a required check never turns advice
/// into a merge block. Which findings block is ForgeGuard's decision, made from
/// the repository's mode and rule policy — not something to second-guess here.
fn conclusion(status: GateStatus) -> &'static str {
    match status {
        GateStatus::Blocked => "failure",
        GateStatus::Warning => "neutral",
        GateStatus::Passed => "success",
    }
}

fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "failure",
        Severity::Warning => "warning",
        Severity::Info => "notice",
    }
}

fn title(report: &GateReport) -> String {
    let summary = &report.summary;
    match report.status {
        GateStatus::Passed => "No findings on the changed lines".to_string(),
        GateStatus::Warning | GateStatus::Blocked => format!(
            "{} error, {} warning, {} info",
            summary.errors, summary.warnings, summary.info
        ),
    }
}

fn summary(report: &GateReport, untrusted: bool) -> String {
    let mut out = String::new();
    let failed: Vec<_> = report
        .checks
        .iter()
        .filter(|check| !check.success)
        .collect();

    let _ = writeln!(
        out,
        "{} finding(s) on lines this pull request changed, {} of them blocking.",
        report.findings.len(),
        report.summary.blocking_findings
    );
    if untrusted {
        let _ = writeln!(
            out,
            "\nThe configured quality commands were skipped: this branch comes from outside the organization."
        );
    }
    if !failed.is_empty() {
        let _ = writeln!(out, "\nFailed checks:");
        for check in failed {
            let _ = writeln!(out, "- `{}` (`{}`)", check.name, check.command);
        }
    }
    if report.findings.len() > ANNOTATION_LIMIT {
        let _ = writeln!(
            out,
            "\nOnly the first {ANNOTATION_LIMIT} findings are annotated inline."
        );
    }
    out
}

fn annotations(report: &GateReport) -> Vec<Annotation> {
    report
        .findings
        .iter()
        .map(|finding| Annotation {
            path: finding.path.to_string_lossy().into_owned(),
            start_line: finding.line.max(1),
            // GitHub rejects an annotation whose end precedes its start, which
            // is what a stale or zeroed end line would produce.
            end_line: finding
                .end_line
                .unwrap_or(finding.line)
                .max(finding.line)
                .max(1),
            level: level(finding.severity),
            title: format!("{} {}", finding.rule_id, finding.title),
            message: format!("{}\n\n{}", finding.evidence, finding.recommendation),
        })
        .collect()
}

/// `None` when there is nothing to say, which removes any comment left by an
/// earlier run.
fn comment(report: &GateReport) -> Option<String> {
    let failed: Vec<_> = report
        .checks
        .iter()
        .filter(|check| !check.success)
        .collect();
    if report.findings.is_empty() && failed.is_empty() {
        return None;
    }

    let mut out = String::from("### ForgeGuard\n\n");
    if !report.findings.is_empty() {
        let _ = writeln!(out, "| | Rule | Where | What |");
        let _ = writeln!(out, "|---|---|---|---|");
        for finding in &report.findings {
            let mark = if finding.blocking { "🔴" } else { "🟡" };
            let _ = writeln!(
                out,
                "| {mark} | `{}` | `{}:{}` | {} |",
                finding.rule_id,
                finding.path.display(),
                finding.line,
                finding.title,
            );
        }
        let _ = writeln!(out, "\n🔴 blocks the merge · 🟡 advisory");
    }
    if !failed.is_empty() {
        let _ = writeln!(out, "\n**Failed checks**\n");
        for check in failed {
            let _ = writeln!(out, "- `{}` — `{}`", check.name, check.command);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgeguard_core::model::{CheckResult, EvidenceConfidence, Finding, GateSummary};
    use std::path::PathBuf;

    fn report(status: GateStatus, findings: Vec<Finding>, checks: Vec<CheckResult>) -> GateReport {
        let summary = GateSummary {
            errors: 0,
            warnings: 0,
            info: 0,
            blocking_findings: findings.iter().filter(|f| f.blocking).count(),
            findings_baselined: 0,
            checks_passed: checks.iter().filter(|c| c.success).count(),
            checks_failed: checks.iter().filter(|c| !c.success).count(),
        };
        GateReport {
            status,
            findings,
            checks,
            summary,
        }
    }

    fn finding(rule_id: &str, severity: Severity, line: usize, end_line: Option<usize>) -> Finding {
        Finding {
            rule_id: rule_id.to_string(),
            title: "Something worth a look".to_string(),
            severity,
            confidence: EvidenceConfidence::Structural,
            blocking: severity == Severity::Error,
            path: PathBuf::from("src/main.rs"),
            line,
            end_line,
            evidence: "evidence".to_string(),
            recommendation: "recommendation".to_string(),
        }
    }

    #[test]
    fn warnings_are_neutral_so_advice_never_blocks_a_merge() {
        assert_eq!(conclusion(GateStatus::Passed), "success");
        assert_eq!(conclusion(GateStatus::Warning), "neutral");
        assert_eq!(conclusion(GateStatus::Blocked), "failure");
    }

    #[test]
    fn a_clean_report_removes_the_comment_rather_than_replacing_it() {
        let clean = report(GateStatus::Passed, vec![], vec![]);
        assert!(comment(&clean).is_none());
    }

    #[test]
    fn a_failed_check_alone_still_earns_a_comment() {
        let checks = vec![CheckResult {
            name: "test".to_string(),
            command: "cargo test".to_string(),
            required: true,
            success: false,
            exit_code: Some(101),
            duration_ms: 1,
            output: String::new(),
            cached: false,
        }];
        let body = comment(&report(GateStatus::Blocked, vec![], checks)).expect("a comment");
        assert!(body.contains("cargo test"));
    }

    #[test]
    fn an_annotation_never_ends_before_it_starts() {
        let findings = vec![
            finding("FG-ALG-001", Severity::Warning, 12, Some(4)),
            finding("FG-SEC-001", Severity::Error, 3, None),
        ];
        let annotated = annotations(&report(GateStatus::Blocked, findings, vec![]));

        assert_eq!(annotated[0].start_line, 12);
        assert_eq!(annotated[0].end_line, 12);
        assert_eq!(annotated[0].level, "warning");
        assert_eq!(annotated[1].end_line, 3);
        assert_eq!(annotated[1].level, "failure");
    }

    #[test]
    fn the_summary_says_why_commands_were_skipped_for_a_fork() {
        let clean = report(GateStatus::Passed, vec![], vec![]);
        assert!(summary(&clean, true).contains("outside the organization"));
        assert!(!summary(&clean, false).contains("outside the organization"));
    }
}
