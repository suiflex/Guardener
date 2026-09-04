//! A model reads the pull request and says what a scanner cannot.
//!
//! Kept deliberately unequal to the gate. ForgeGuard's findings are
//! deterministic and block a merge; this is a second opinion that never does.
//! It posts under its own marker so the two never blur together on the page,
//! and it is told what the scanner already covers so it stops repeating work
//! that was already done properly.
//!
//! It reads the diff from the API rather than a checkout, so unlike the gate it
//! needs no clone and could run anywhere a token reaches.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::{Registry, RepositoryEntry};
use crate::github::{split_repo, Client};

pub const MARKER: &str = "<!-- guardener:review -->";

/// What the deterministic scanner already reports. Naming it in the prompt is
/// the cheapest way to stop a model spending its attention — and the budget —
/// re-deriving findings that arrive on the same pull request with line numbers
/// and a rule id attached.
const ALREADY_COVERED: &str = "database queries and network calls inside loops, unbounded \
parallelism, sorting or linear lookups inside iteration, function complexity, hardcoded \
credentials, weak crypto, unsafe deserialization, XSS and other taint-to-sink flows, swallowed \
exceptions, duplicated blocks, large inline literals, and changed-line test coverage";

/// Only the parts worth arguing about in a pull request. Which model, which
/// endpoint and which key all arrive from the environment: they name a private
/// service, and this file is public.
#[derive(Debug, Deserialize)]
pub struct Settings {
    #[serde(default = "default_max_changed_lines")]
    pub max_changed_lines: usize,
    #[serde(default)]
    pub skip: Vec<String>,
}

fn default_max_changed_lines() -> usize {
    1500
}

/// One thing the model wants to say. `line` is advisory — it is rendered as
/// text, never used to place an inline comment, because a wrong position would
/// put a remark on code it is not about.
#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Note {
    path: String,
    #[serde(default)]
    line: Option<u64>,
    #[serde(default)]
    severity: Option<String>,
    comment: String,
}

pub struct Request<'a> {
    pub registry: &'a Path,
    pub settings: &'a Path,
    /// The repository to review in, or `None` to sweep the whole registry.
    pub repo: Option<&'a str>,
    /// The pull request to review, or `None` to sweep.
    pub pull_request: Option<u64>,
    /// How long a pull request must have sat untouched before the sweep will
    /// look at it.
    pub stale_days: u64,
    /// The most pull requests one sweep will review. See `sweep`.
    pub max: usize,
    /// Endpoint, key and model, all from the environment: together they name a
    /// private service, and none of them belong in a public repository.
    pub endpoint: &'a str,
    pub key: &'a str,
    pub model: &'a str,
}

pub fn run(client: &Client, request: &Request<'_>) -> Result<()> {
    let settings = load_settings(request.settings)?;
    let registry = Registry::load(request.registry)?;

    match (request.repo, request.pull_request) {
        (Some(repo), Some(number)) => {
            if registry.find(repo).is_none() {
                bail!("{repo} is not in the registry; add it before reviewing pull requests there");
            }
            let (owner, name) = split_repo(repo)?;
            review_one(client, request, &settings, owner, name, number)
        }
        // `--repo` without `--pr` narrows the sweep to one repository, the same
        // way `hygiene --repo` does. Sweeping the whole organization while
        // quietly ignoring a repository the caller named would be worse than
        // either behaviour on its own.
        (repo, None) => sweep(client, request, &settings, &registry, repo),
        (None, Some(_)) => bail!("--pr names a pull request but --repo does not say where"),
    }
}

/// Reviews every open pull request that has sat untouched for `stale_days` and
/// carries no review comment, across every repository in the registry.
///
/// The second condition is the one that matters. Every pull request opened
/// since a repository joined already got a review the moment it opened, so
/// without it a sweep would pay to review the whole organization again nightly.
/// With it, what is left is the genuine gap: a review that failed quietly under
/// `continue-on-error`, a repository whose model secrets arrived later, a pull
/// request older than the gate itself.
fn sweep(
    client: &Client,
    request: &Request<'_>,
    settings: &Settings,
    registry: &Registry,
    only: Option<&str>,
) -> Result<()> {
    if let Some(repo) = only {
        if registry.find(repo).is_none() {
            bail!("{repo} is not in the registry; add it before reviewing pull requests there");
        }
    }
    let cutoff = now_epoch().saturating_sub((request.stale_days * 86_400) as i64);
    let mut budget = request.max;

    for entry in &registry.repositories {
        if only.is_some_and(|repo| !entry.name.eq_ignore_ascii_case(repo)) {
            continue;
        }
        if budget == 0 {
            println!("stopping at --max {}", request.max);
            break;
        }
        budget -= sweep_repository(client, request, settings, entry, cutoff, budget)?;
    }

    let reviewed = request.max - budget;
    if client.is_dry_run() {
        println!("{reviewed} pull request(s) would be reviewed");
    } else {
        println!("{reviewed} pull request(s) reviewed");
    }
    Ok(())
}

/// One repository's share of the sweep. Returns how much of `budget` it spent.
///
/// Split from `sweep` rather than nested inside it because the two answer
/// different questions: which repositories are worth walking and how much of the
/// run's budget is left, against which of one repository's pull requests are
/// stale. The walk is linear either way — every open pull request is visited
/// once — but reading it as one function meant holding both questions at once.
fn sweep_repository(
    client: &Client,
    request: &Request<'_>,
    settings: &Settings,
    entry: &RepositoryEntry,
    cutoff: i64,
    budget: usize,
) -> Result<usize> {
    let (owner, name) = split_repo(&entry.name)?;

    // A sweep is worth more than any single repository in it. One that cannot
    // be read is reported and the walk continues, the same bargain the hygiene
    // sweep strikes.
    let open = match client.open_pull_requests(owner, name) {
        Ok(open) => open,
        Err(error) => {
            println!("{}: skipped ({error:#})", entry.name);
            return Ok(0);
        }
    };

    let mut spent = 0usize;
    for pull_request in open {
        if spent == budget {
            break;
        }
        let Some(number) = pull_request["number"].as_u64() else {
            continue;
        };
        let fresh = pull_request["updated_at"]
            .as_str()
            .and_then(epoch)
            .is_none_or(|updated| updated > cutoff);
        if fresh {
            continue;
        }
        if client.has_comment(owner, name, number, MARKER)? {
            continue;
        }

        // A dry run of a sweep lists and stops. This is a deliberate break from
        // `--pr --dry-run`, where the model is still asked because its answer is
        // the whole point of the preview: here the preview is *which* pull
        // requests would be read, and asking would be the full cost of the run
        // for a rehearsal of it.
        println!("{}#{number}: stale and never reviewed", entry.name);
        if !client.is_dry_run() {
            review_one(client, request, settings, owner, name, number)?;
        }
        spent += 1;
    }
    Ok(spent)
}

fn load_settings(path: &Path) -> Result<Settings> {
    toml::from_str(
        &std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))
}

fn review_one(
    client: &Client,
    request: &Request<'_>,
    settings: &Settings,
    owner: &str,
    name: &str,
    number: u64,
) -> Result<()> {
    let pull_request = client.pull_request(owner, name, number)?;

    // A bot's pull request was not written by anyone who can act on a review.
    if pull_request["user"]["type"].as_str() == Some("Bot") {
        println!("skipped: opened by a bot");
        return client.upsert_comment(owner, name, number, MARKER, None);
    }

    let diff = client.pull_request_diff(owner, name, number)?;
    let reviewable = filter(&diff, &settings.skip);
    let changed = changed_lines(&reviewable);

    if changed == 0 {
        println!("skipped: nothing left after filtering");
        return client.upsert_comment(owner, name, number, MARKER, None);
    }
    if changed > settings.max_changed_lines {
        println!(
            "skipped: {changed} changed lines exceeds {}",
            settings.max_changed_lines
        );
        return client.upsert_comment(
            owner,
            name,
            number,
            MARKER,
            Some(format!(
                "Not reviewed: {changed} changed lines is past the {} this is configured to read. \
                 A review of a change this size would be too shallow to trust.",
                settings.max_changed_lines
            )),
        );
    }

    let title = pull_request["title"].as_str().unwrap_or("");
    let answer = ask(
        request.endpoint,
        request.key,
        request.model,
        title,
        &reviewable,
    )?;
    let notes = parse(&answer)?;
    println!("{} note(s) over {changed} changed lines", notes.len());

    client.upsert_comment(owner, name, number, MARKER, render(&notes))
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or(0)
}

/// Seconds since the epoch for one of GitHub's timestamps, or `None` if it is
/// not shaped like one.
///
/// ponytail: hand-rolled rather than pulled from a date crate. The whole need is
/// "is this timestamp older than N days", against a field GitHub documents as
/// RFC 3339 in UTC and always renders at fixed width — a dependency for one
/// subtraction is a dependency to keep patched forever. The ceiling is real
/// though: this understands exactly `YYYY-MM-DDTHH:MM:SSZ` and nothing else, no
/// offsets and no fractional seconds. Reach for `time` or `chrono` the moment a
/// second caller needs anything more than this.
fn epoch(timestamp: &str) -> Option<i64> {
    let bytes = timestamp.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let number = |range: std::ops::Range<usize>| timestamp.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Days between 1970-01-01 and a civil date, by Howard Hinnant's algorithm.
///
/// Shifts the year to start in March so that the leap day lands at the end of
/// the cycle and needs no special case — which is the entire reason to use this
/// rather than count months by hand.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Drops whole files from the diff rather than individual hunks: a file worth
/// skipping is worth skipping entirely, and half a file is worse context than
/// none.
fn filter(diff: &str, skip: &[String]) -> String {
    let mut kept = String::new();
    for block in diff.split("\ndiff --git ") {
        let block = block.strip_prefix("diff --git ").unwrap_or(block);
        if block.trim().is_empty() {
            continue;
        }
        let header = block.lines().next().unwrap_or("");
        if skip.iter().any(|pattern| matches(header, pattern)) {
            continue;
        }
        let _ = write!(kept, "diff --git {block}");
        if !kept.ends_with('\n') {
            kept.push('\n');
        }
    }
    kept
}

/// `*.ext` matches a suffix; anything else matches anywhere in the path. No
/// glob engine, because a list of extensions and directory names is all this
/// has ever needed.
fn matches(header: &str, pattern: &str) -> bool {
    match pattern.strip_prefix('*') {
        Some(suffix) => header.split_whitespace().any(|path| path.ends_with(suffix)),
        None => header.contains(pattern),
    }
}

fn changed_lines(diff: &str) -> usize {
    diff.lines()
        .filter(|line| {
            (line.starts_with('+') && !line.starts_with("+++"))
                || (line.starts_with('-') && !line.starts_with("---"))
        })
        .count()
}

fn ask(endpoint: &str, key: &str, model: &str, title: &str, diff: &str) -> Result<String> {
    let system = format!(
        "You review a pull request diff for the suiflex organization. A deterministic scanner \
         already reports {ALREADY_COVERED}; never repeat any of those, they arrive on the same \
         pull request with line numbers already attached.\n\n\
         Report only what a careful human reviewer would raise and a scanner cannot: logic that \
         is wrong, an invariant the change breaks, an edge case it misses, an error path that \
         loses data silently, a name that misleads about what the code does, a change to a \
         contract that its callers will not survive.\n\n\
         Say nothing about formatting or style. Do not praise. Do not summarise the change. If \
         nothing meets that bar, return an empty array — that is the expected answer for most \
         pull requests, and a made-up remark costs more than silence.\n\n\
         Answer with JSON only: an array of objects with keys \"path\", \"line\", \"severity\" \
         (one of \"high\", \"medium\", \"low\") and \"comment\". No prose around it."
    );

    let response: Value = ureq::post(&format!(
        "{}/chat/completions",
        endpoint.trim_end_matches('/')
    ))
    .header("Authorization", &format!("Bearer {key}"))
    .header("Content-Type", "application/json")
    .send_json(json!({
        "model": model,
        // Zero because the same diff should not draw a different review on
        // a rerun; a comment that changes when nothing changed teaches
        // people to ignore it.
        "temperature": 0,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": format!("Pull request: {title}\n\n{diff}") },
        ],
    }))
    .context("the model endpoint rejected the request")?
    .body_mut()
    .read_json()
    .context("the model endpoint returned something that is not JSON")?;

    if let Some(usage) = response.get("usage") {
        println!("tokens: {usage}");
    }
    response["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .context("the model endpoint returned no message content")
}

/// Models wrap JSON in a fenced block often enough that refusing to read one
/// would mean failing on a correct answer.
fn parse(answer: &str) -> Result<Vec<Note>> {
    let trimmed = answer.trim();
    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|rest| rest.rsplit_once("```"))
        .map(|(body, _)| body)
        .unwrap_or(trimmed)
        .trim();
    if unfenced.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(unfenced)
        .with_context(|| format!("the model did not answer with the agreed JSON: {unfenced:.400}"))
}

fn render(notes: &[Note]) -> Option<String> {
    if notes.is_empty() {
        return None;
    }
    let mut out = String::from("### Review\n\n");
    for note in notes {
        let where_ = match note.line {
            Some(line) => format!("`{}:{}`", note.path, line),
            None => format!("`{}`", note.path),
        };
        let severity = note.severity.as_deref().unwrap_or("note");
        let _ = writeln!(out, "- **{severity}** · {where_} — {}", note.comment);
    }
    let _ = writeln!(
        out,
        "\nWritten by a model, and wrong more often than the checks above it. \
         Nothing here blocks a merge."
    );
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "\
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
 fn main() {}
+fn extra() {}
-fn gone() {}
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -1 +1 @@
-version = 3
+version = 4
";

    #[test]
    fn a_skipped_file_is_dropped_whole() {
        let skip = vec!["Cargo.lock".to_string()];
        let kept = filter(DIFF, &skip);

        assert!(kept.contains("src/main.rs"));
        assert!(!kept.contains("Cargo.lock"));
        // The kept file keeps all of its hunk, not just its header.
        assert!(kept.contains("+fn extra() {}"));
    }

    #[test]
    fn suffix_patterns_match_the_extension_and_nothing_else() {
        assert!(matches("a/x.lock b/x.lock", "*.lock"));
        assert!(!matches("a/lockfile.rs b/lockfile.rs", "*.lock"));
        assert!(matches("a/vendor/x.go b/vendor/x.go", "/vendor/"));
        assert!(!matches("a/src/x.go b/src/x.go", "/vendor/"));
    }

    #[test]
    fn file_headers_are_not_counted_as_changed_lines() {
        // Two real changes; the ---/+++ headers must not inflate that.
        assert_eq!(changed_lines(&filter(DIFF, &["Cargo.lock".to_string()])), 2);
    }

    #[test]
    fn a_fenced_answer_reads_the_same_as_a_bare_one() {
        let bare = r#"[{"path":"a.rs","line":3,"severity":"high","comment":"off by one"}]"#;
        let fenced = format!("```json\n{bare}\n```");

        let expected = vec![Note {
            path: "a.rs".to_string(),
            line: Some(3),
            severity: Some("high".to_string()),
            comment: "off by one".to_string(),
        }];
        assert_eq!(parse(bare).unwrap(), expected);
        assert_eq!(parse(&fenced).unwrap(), expected);
    }

    #[test]
    fn silence_is_a_valid_answer_and_leaves_no_comment() {
        assert!(parse("[]").unwrap().is_empty());
        assert!(parse("").unwrap().is_empty());
        assert!(render(&[]).is_none());
    }

    #[test]
    fn the_comment_says_it_does_not_block() {
        let body = render(&[Note {
            path: "a.rs".to_string(),
            line: None,
            severity: None,
            comment: "the retry loses the error".to_string(),
        }])
        .expect("a comment");

        assert!(body.contains("`a.rs`"));
        assert!(body.contains("Nothing here blocks a merge"));
    }

    #[test]
    fn epoch_reads_githubs_timestamps() {
        assert_eq!(epoch("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch("2024-01-01T00:00:00Z"), Some(1_704_067_200));
        assert_eq!(epoch("2026-09-05T12:34:56Z"), Some(1_788_611_696));
    }

    /// The leap day is the whole reason `days_from_civil` shifts the year to
    /// start in March. A hand-rolled month table gets this wrong.
    #[test]
    fn epoch_counts_the_leap_day() {
        let feb29 = epoch("2024-02-29T00:00:00Z").expect("a leap day");
        let mar01 = epoch("2024-03-01T00:00:00Z").expect("the day after");
        assert_eq!(mar01 - feb29, 86_400);

        // 2100 is divisible by 4 but not a leap year: the century rule.
        let feb28 = epoch("2100-02-28T00:00:00Z").expect("february");
        let mar01 = epoch("2100-03-01T00:00:00Z").expect("march");
        assert_eq!(mar01 - feb28, 86_400);
    }

    #[test]
    fn epoch_refuses_what_it_cannot_read() {
        assert_eq!(epoch(""), None);
        assert_eq!(epoch("2024-01-01"), None);
        assert_eq!(epoch("2024-01-01 00:00:00Z"), None);
        assert_eq!(epoch("2024-13-01T00:00:00Z"), None);
        assert_eq!(epoch("not a timestamp at all"), None);
    }

    /// The sweep's filter, stated as the comparison it actually makes. A pull
    /// request is a candidate only when it is *both* old enough and unreviewed;
    /// dropping the second half would re-review the whole organization nightly.
    #[test]
    fn stale_means_old_and_never_reviewed() {
        let cutoff = epoch("2026-09-01T00:00:00Z").expect("a cutoff");
        let candidate = |updated: &str, reviewed: bool| {
            epoch(updated).is_some_and(|at| at <= cutoff) && !reviewed
        };

        assert!(candidate("2026-08-01T00:00:00Z", false), "old, unreviewed");
        assert!(!candidate("2026-08-01T00:00:00Z", true), "old but reviewed");
        assert!(!candidate("2026-09-30T00:00:00Z", false), "fresh");
        assert!(!candidate("2026-09-30T00:00:00Z", true), "fresh, reviewed");
    }
}
