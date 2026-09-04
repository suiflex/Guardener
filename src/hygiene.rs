//! Checks that every watched repository still carries what the organization
//! decided every repository carries.
//!
//! Drift here is quiet. A repository loses its triage workflow in a merge, or
//! is created without the label set, and nothing fails — it simply stops being
//! covered, and nobody finds out until they go looking. This walks the list and
//! says so out loud, in one place.
//!
//! Reporting is the default and fixing is not. `--fix` only ever *adds* a file
//! that is missing: it never edits or replaces one, and it never touches
//! settings. Everything a repository already has is the repository's own
//! decision, and a bot that silently revises those decisions across an
//! organization is worse than the drift it was meant to catch.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::Registry;
use crate::github::{split_repo, Client};

pub const MARKER: &str = "<!-- guardener:hygiene -->";
const ISSUE_TITLE: &str = "Repository hygiene";
const BRANCH: &str = "guardener/hygiene";

/// What the fix branch is named after, and how a fix pull request explains
/// itself. Kept next to the branch name so the two never drift apart.
const FIX_TITLE: &str = "chore: restore the organization standard files";

#[derive(Debug, Deserialize)]
struct Labels {
    names: Vec<String>,
}

/// A file the organization expects, and the copy used to restore it.
struct Expected {
    check: &'static str,
    /// Any of these satisfies the expectation; the first is what `--fix` writes.
    accepted: &'static [&'static str],
    contents: &'static str,
}

const EXPECTED: &[Expected] = &[
    Expected {
        check: "license",
        accepted: &["LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"],
        // Guardener's own licence, not a second copy of it. Handing out a file
        // that could drift from the one this tool lives under would be a poor
        // advertisement for a tool that exists to catch drift.
        contents: include_str!("../LICENSE"),
    },
    Expected {
        check: "codeowners",
        accepted: &[".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"],
        contents: include_str!("../templates/CODEOWNERS"),
    },
    Expected {
        check: "triage-workflow",
        accepted: &[".github/workflows/suiflex.yml"],
        contents: include_str!("../templates/workflows/suiflex.yml"),
    },
    Expected {
        check: "gate-workflow",
        accepted: &[".github/workflows/guardener.yml"],
        contents: include_str!("../templates/workflows/guardener.yml"),
    },
    // A separate file rather than another trigger on the gate's workflow, and
    // that is forced rather than chosen: `--fix` may only add, never edit, so a
    // capability bolted onto guardener.yml could not reach the repositories
    // that already have one without a hand-written pull request to each.
    Expected {
        check: "review-workflow",
        accepted: &[".github/workflows/review.yml"],
        contents: include_str!("../templates/workflows/review.yml"),
    },
];

#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub check: &'static str,
    pub detail: String,
    /// Set only when the finding is a missing file, which is the only kind
    /// `--fix` is allowed to act on.
    pub fix: Option<Fix>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Fix {
    pub path: &'static str,
    pub contents: &'static str,
}

pub struct Report {
    pub repo: String,
    pub findings: Vec<Finding>,
    /// Set when `--fix` opened a pull request, or explained why it did not.
    pub fixed: Option<String>,
    /// Why the repository was not checked, when it was not. A sweep is worth
    /// more than any single repository in it: one that cannot be read, or has
    /// nothing to read yet, is recorded and the walk continues rather than
    /// costing the report for every repository that could be checked.
    pub skipped: Option<String>,
}

pub struct Request<'a> {
    pub registry: &'a std::path::Path,
    pub labels: &'a std::path::Path,
    /// Where the standing report is kept.
    pub report_to: &'a str,
    /// Target a single repository instead of the whole registry.
    pub repo: Option<&'a str>,
    pub fix: bool,
}

pub fn run(client: &Client, request: &Request<'_>) -> Result<()> {
    let registry = Registry::load(request.registry)?;
    let expected_labels: Labels = toml::from_str(
        &std::fs::read_to_string(request.labels)
            .with_context(|| format!("failed to read {}", request.labels.display()))?,
    )
    .with_context(|| format!("failed to parse {}", request.labels.display()))?;

    let targets = targets(&registry, request.repo)?;

    let mut reports = Vec::new();
    for (name, exempt) in &targets {
        let mut report = match inspect(client, name, &expected_labels.names, exempt) {
            Ok(report) => report,
            Err(error) => Report {
                repo: name.clone(),
                findings: Vec::new(),
                fixed: None,
                skipped: Some(format!("could not be read — {error:#}")),
            },
        };
        if request.fix && !report.findings.is_empty() {
            report.fixed = Some(match apply(client, &report) {
                Ok(outcome) => outcome,
                Err(error) => format!("could not open a pull request: {error:#}"),
            });
        }
        reports.push(report);
    }

    if let Some(target) = request.repo {
        if let Some(rendered) = render(&reports) {
            println!("{rendered}");
        } else {
            println!("All standard files and settings in order for {target}.");
        }
        Ok(())
    } else {
        let (owner, repo) = split_repo(request.report_to)?;
        client.upsert_issue(owner, repo, MARKER, ISSUE_TITLE, render(&reports))
    }
}

fn targets(registry: &Registry, repo: Option<&str>) -> Result<Vec<(String, Vec<String>)>> {
    match repo {
        Some(target) => {
            let _ = split_repo(target)?;
            if let Some(entry) = registry.find(target) {
                Ok(vec![(entry.name.clone(), entry.exempt.clone())])
            } else {
                Ok(vec![(target.to_string(), Vec::new())])
            }
        }
        None => Ok(registry
            .repositories
            .iter()
            .map(|entry| (entry.name.clone(), entry.exempt.clone()))
            .collect()),
    }
}

fn inspect(
    client: &Client,
    repo: &str,
    expected_labels: &[String],
    exempt: &[String],
) -> Result<Report> {
    let (owner, name) = split_repo(repo)?;
    let metadata = client
        .repository(owner, name)
        .with_context(|| format!("failed to read {repo}"))?;

    // An archived repository is meant to have stopped changing. Asking it to
    // adopt a new standard is asking for a pull request nobody will merge.
    if metadata["archived"].as_bool().unwrap_or(false) {
        return Ok(Report {
            repo: repo.to_string(),
            findings: Vec::new(),
            fixed: None,
            skipped: Some("archived".to_string()),
        });
    }

    let default_branch = metadata["default_branch"].as_str().unwrap_or("main");
    let Some(paths) = client.tree(owner, name, default_branch)? else {
        return Ok(Report {
            repo: repo.to_string(),
            findings: Vec::new(),
            fixed: None,
            skipped: Some("no commits yet; nothing to check until the first push".to_string()),
        });
    };
    let mut findings = Vec::new();

    for expectation in EXPECTED {
        if exempt.iter().any(|check| check == expectation.check) {
            continue;
        }
        if expectation
            .accepted
            .iter()
            .any(|candidate| paths.iter().any(|path| path == candidate))
        {
            continue;
        }
        findings.push(Finding {
            check: expectation.check,
            detail: format!("`{}` is missing", expectation.accepted[0]),
            fix: Some(Fix {
                path: expectation.accepted[0],
                contents: expectation.contents,
            }),
        });
    }

    if !exempt.iter().any(|check| check == "labels") {
        let present = client.labels(owner, name)?;
        let missing: Vec<&str> = expected_labels
            .iter()
            .filter(|label| !present.contains(label))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            findings.push(Finding {
                check: "labels",
                detail: format!(
                    "triage cannot apply {}; run scripts/sync-labels.sh for this repository",
                    missing
                        .iter()
                        .map(|label| format!("`{label}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                fix: None,
            });
        }
    }

    if !exempt.iter().any(|check| check == "branch-protection")
        && client.is_protected(owner, name, default_branch)? == Some(false)
    {
        findings.push(Finding {
            check: "branch-protection",
            detail: format!("`{default_branch}` accepts a direct push"),
            fix: None,
        });
    }

    if !exempt.iter().any(|check| check == "pinned-actions") {
        // One finding per repository, not per reference. The same action is
        // named once per job, so listing every occurrence turned a fifteen-line
        // report into a three-hundred-line one and buried the checks that ask
        // someone to do something. The report says which repositories have
        // drifted; the diff says where.
        let mut references = BTreeSet::new();
        for path in paths.iter().filter(|path| is_workflow(path)) {
            let Some(source) = client.file(owner, name, path)? else {
                continue;
            };
            references.extend(unpinned(&source));
        }
        if !references.is_empty() {
            findings.push(Finding {
                check: "pinned-actions",
                detail: format!(
                    "{} third-party action(s) named by tag: {}",
                    references.len(),
                    references
                        .iter()
                        .map(|reference| format!("`{reference}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                fix: None,
            });
        }
    }

    Ok(Report {
        repo: repo.to_string(),
        findings,
        fixed: None,
        skipped: None,
    })
}

fn is_workflow(path: &str) -> bool {
    path.starts_with(".github/workflows/") && (path.ends_with(".yml") || path.ends_with(".yaml"))
}

/// Third-party actions named by a tag, which a tag owner can move under us.
///
/// Two exemptions. A path starting with `./` is this repository's own code, and
/// a `suiflex/` reference is the organization's own reusable workflow, which
/// exists precisely so a change lands once upstream — pinning it to a commit
/// would defeat the reason it was factored out.
fn unpinned(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("- uses:").or_else(|| {
            trimmed
                .strip_prefix("uses:")
                .filter(|_| !trimmed.starts_with("uses::"))
        }) else {
            continue;
        };
        let reference = rest
            .split('#')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(['"', '\''])
            .to_string();
        if reference.is_empty()
            || reference.starts_with("./")
            || reference.starts_with("docker://")
            || reference.starts_with("suiflex/")
        {
            continue;
        }
        let pinned = reference
            .rsplit_once('@')
            .is_some_and(|(_, git_ref)| is_commit(git_ref));
        if !pinned {
            found.push(reference);
        }
    }
    found
}

fn is_commit(git_ref: &str) -> bool {
    git_ref.len() == 40 && git_ref.chars().all(|c| c.is_ascii_hexdigit())
}

/// Opens one pull request carrying every missing file, on a branch named after
/// this tool. An existing branch means an earlier run already opened it; that
/// pull request is left alone rather than force-pushed over.
fn apply(client: &Client, report: &Report) -> Result<String> {
    let fixes: Vec<&Fix> = report
        .findings
        .iter()
        .filter_map(|finding| finding.fix.as_ref())
        .collect();
    if fixes.is_empty() {
        return Ok("nothing to add; the remaining findings are settings, not files".to_string());
    }

    let (owner, name) = split_repo(&report.repo)?;
    let metadata = client.repository(owner, name)?;
    let default_branch = metadata["default_branch"].as_str().unwrap_or("main");

    if !client.create_branch(owner, name, BRANCH, default_branch)? {
        return Ok(format!("`{BRANCH}` already exists; left as it is"));
    }

    for fix in &fixes {
        client.create_file(
            owner,
            name,
            BRANCH,
            fix.path,
            &format!("chore: add {}", fix.path),
            fix.contents,
        )?;
    }

    let body = format!(
        "Adds the organization standard files this repository is missing:\n\n{}\n\nOpened by Guardener. \
         Nothing here replaces an existing file.",
        fixes
            .iter()
            .map(|fix| format!("- `{}`", fix.path))
            .collect::<Vec<_>>()
            .join("\n")
    );
    client.create_pull_request(owner, name, BRANCH, default_branch, FIX_TITLE, &body)
}

/// `None` when every repository is in order, which closes the standing issue.
fn render(reports: &[Report]) -> Option<String> {
    if reports
        .iter()
        .all(|report| report.findings.is_empty() && report.skipped.is_none())
    {
        return None;
    }

    let mut out = String::new();
    let total: usize = reports.iter().map(|report| report.findings.len()).sum();
    let checked = reports
        .iter()
        .filter(|report| report.skipped.is_none())
        .count();
    let drifted = reports
        .iter()
        .filter(|report| !report.findings.is_empty())
        .count();

    let _ = writeln!(
        out,
        "{total} finding(s) across {drifted} of {checked} repositories checked."
    );
    let skipped = reports.len() - checked;
    if skipped > 0 {
        // Said out loud rather than left to arithmetic on the line above: a
        // repository quietly dropping out of the sweep is the failure this
        // whole report exists to prevent.
        let _ = writeln!(out, "{skipped} not checked, listed below.");
    }
    let _ = writeln!(out);

    for report in reports
        .iter()
        .filter(|r| !r.findings.is_empty() || r.skipped.is_some())
    {
        let _ = writeln!(out, "### {}\n", report.repo);
        if let Some(reason) = &report.skipped {
            let _ = writeln!(out, "- **not checked** — {reason}");
        }
        for finding in &report.findings {
            let _ = writeln!(out, "- **{}** — {}", finding.check, finding.detail);
        }
        if let Some(fixed) = &report.fixed {
            let _ = writeln!(out, "\n{fixed}");
        }
        let _ = writeln!(out);
    }

    // Only when it is true. A footer offering a fix on a report with nothing
    // fixable in it reads as boilerplate, and boilerplate is how a report stops
    // being read.
    if reports
        .iter()
        .any(|report| report.findings.iter().any(|finding| finding.fix.is_some()))
    {
        let _ = writeln!(
            out,
            "Missing files can be added with `guardener hygiene --fix`. Labels and branch \
             protection are reported only, and are changed by a person."
        );
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(repo: &str, findings: Vec<Finding>) -> Report {
        Report {
            repo: repo.to_string(),
            findings,
            fixed: None,
            skipped: None,
        }
    }

    #[test]
    fn a_tidy_organization_closes_the_issue() {
        assert!(render(&[report("suiflex/rdb", vec![])]).is_none());
    }

    #[test]
    fn the_report_names_the_repository_and_the_check() {
        let body = render(&[
            report("suiflex/rdb", vec![]),
            report(
                "suiflex/websift",
                vec![Finding {
                    check: "license",
                    detail: "`LICENSE` is missing".to_string(),
                    fix: None,
                }],
            ),
        ])
        .expect("a report");

        assert!(body.contains("1 finding(s) across 1 of 2 repositories checked."));
        assert!(!body.contains("not checked"));
        assert!(body.contains("### suiflex/websift"));
        assert!(body.contains("**license**"));
        assert!(!body.contains("### suiflex/rdb"));
    }

    #[test]
    fn the_fix_footer_appears_only_when_something_is_fixable() {
        let fixable = Finding {
            check: "license",
            detail: "`LICENSE` is missing".to_string(),
            fix: Some(Fix {
                path: "LICENSE",
                contents: "",
            }),
        };
        let settings_only = Finding {
            check: "branch-protection",
            detail: "`main` accepts a direct push".to_string(),
            fix: None,
        };

        assert!(render(&[report("suiflex/rdb", vec![fixable])])
            .unwrap()
            .contains("--fix"));
        assert!(!render(&[report("suiflex/rdb", vec![settings_only])])
            .unwrap()
            .contains("--fix"));
    }

    #[test]
    fn a_repository_that_was_not_checked_is_reported_rather_than_lost() {
        let body = render(&[
            report("suiflex/rdb", vec![]),
            Report {
                repo: "suiflex/arsy".to_string(),
                findings: Vec::new(),
                fixed: None,
                skipped: Some("could not be read — http status: 403".to_string()),
            },
        ])
        .expect("a report");

        assert!(body.contains("### suiflex/arsy"));
        assert!(body.contains("**not checked**"));
        // The counts describe what was actually looked at, not the list length.
        assert!(body.contains("0 finding(s) across 0 of 1 repositories checked."));
        assert!(body.contains("1 not checked"));
    }

    #[test]
    fn a_tag_is_unpinned_and_a_commit_is_not() {
        let source = "\
jobs:
  build:
    steps:
      - uses: actions/checkout@v5
      - uses: actions/create-github-app-token@bcd2ba49218906704ab6c1aa796996da409d3eb1 # v3.2.0
";
        assert_eq!(unpinned(source), vec!["actions/checkout@v5"]);
    }

    #[test]
    fn the_organizations_own_reusable_workflows_are_not_expected_to_be_pinned() {
        let source = "    uses: suiflex/.github/.github/workflows/pr-triage.yml@main\n";
        assert!(unpinned(source).is_empty());
    }

    #[test]
    fn local_and_docker_actions_are_left_alone() {
        let source = "      - uses: ./.github/actions/setup\n      - uses: docker://alpine:3\n";
        assert!(unpinned(source).is_empty());
    }

    #[test]
    fn only_workflow_files_are_read_for_pins() {
        assert!(is_workflow(".github/workflows/ci.yml"));
        assert!(is_workflow(".github/workflows/ci.yaml"));
        assert!(!is_workflow(".github/dependabot.yml"));
        assert!(!is_workflow("docs/workflows/ci.yml"));
    }

    #[test]
    fn targets_resolves_all_repositories_when_repo_is_none() {
        let registry = Registry {
            repositories: vec![
                crate::config::RepositoryEntry {
                    name: "suiflex/rdb".to_string(),
                    mode: None,
                    rules: Default::default(),
                    exempt: vec!["branch-protection".to_string()],
                },
                crate::config::RepositoryEntry {
                    name: "suiflex/websift".to_string(),
                    mode: None,
                    rules: Default::default(),
                    exempt: vec![],
                },
            ],
        };
        let result = targets(&registry, None).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            (
                "suiflex/rdb".to_string(),
                vec!["branch-protection".to_string()]
            )
        );
        assert_eq!(result[1], ("suiflex/websift".to_string(), vec![]));
    }

    #[test]
    fn targets_resolves_registered_repository_with_its_exemptions() {
        let registry = Registry {
            repositories: vec![crate::config::RepositoryEntry {
                name: "suiflex/rdb".to_string(),
                mode: None,
                rules: Default::default(),
                exempt: vec!["branch-protection".to_string()],
            }],
        };
        let result = targets(&registry, Some("suiflex/rdb")).unwrap();
        assert_eq!(
            result,
            vec![(
                "suiflex/rdb".to_string(),
                vec!["branch-protection".to_string()]
            )]
        );
    }

    #[test]
    fn targets_resolves_unregistered_valid_repository() {
        let registry = Registry {
            repositories: vec![],
        };
        let result = targets(&registry, Some("suiflex/custom-repo")).unwrap();
        assert_eq!(result, vec![("suiflex/custom-repo".to_string(), vec![])]);
    }

    #[test]
    fn targets_rejects_invalid_repository_name() {
        let registry = Registry {
            repositories: vec![],
        };
        assert!(targets(&registry, Some("invalid-name")).is_err());
    }
}
