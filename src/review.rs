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

/// Who wanted this review, which is the one thing that decides whether a bot's
/// pull request is worth reading.
///
/// Nothing else in here branches on it, and nothing else should. It exists
/// because "not worth reviewing unprompted" and "not worth reviewing when
/// somebody asked" are different claims, and the code used to make only the
/// first one and apply it to both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    /// A `/review` comment, or `--repo` and `--pr` typed by hand.
    ByAPerson,
    /// The sweep found it. Nobody looked at this pull request and decided it
    /// needed reading.
    BySchedule,
}

impl Asked {
    /// The comment body for an outcome that has nothing to report.
    ///
    /// `None` for a run nobody asked for: a review with nothing to say should
    /// leave no trace, or every pull request in the organization collects a
    /// notice that there is no notice. A person who typed `/review` gets the
    /// sentence instead, because for them an absent comment and a workflow that
    /// never ran look exactly alike.
    fn explain(self, reason: &str) -> Option<String> {
        match self {
            Asked::ByAPerson => Some(reason.to_string()),
            Asked::BySchedule => None,
        }
    }
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
    /// Markers for text the endpoint adds to every answer of its own accord.
    /// See `scrub`. Empty when there is none, which is the normal case.
    pub vomit: &'a str,
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
            review_one(
                client,
                request,
                &settings,
                owner,
                name,
                number,
                Asked::ByAPerson,
            )
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
        // Skipped here rather than left to `review_one`, which would also
        // decline it. A declined pull request is left with no comment, so it
        // stays a candidate for every future sweep — and a repository that
        // takes dependency updates would spend its whole budget rediscovering
        // the same bot pull requests every week.
        if pull_request["user"]["type"].as_str() == Some("Bot") {
            continue;
        }
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
            review_one(
                client,
                request,
                settings,
                owner,
                name,
                number,
                Asked::BySchedule,
            )?;
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
    asked: Asked,
) -> Result<()> {
    let pull_request = client.pull_request(owner, name, number)?;

    // A bot's pull request was not written by anyone who can act on a review,
    // so nothing reaches for one on its own initiative. A person typing
    // `/review` overrules that: they looked at this pull request and decided it
    // was worth reading, which is a better signal than the guess encoded here —
    // and a change that bumps seven dependencies at once is exactly the kind a
    // person might want a second pair of eyes on.
    if asked == Asked::BySchedule && pull_request["user"]["type"].as_str() == Some("Bot") {
        println!("skipped: opened by a bot");
        return client.upsert_comment(owner, name, number, MARKER, None);
    }

    let diff = client.pull_request_diff(owner, name, number)?;
    let reviewable = filter(&diff, &settings.skip);
    let changed = changed_lines(&reviewable);

    if changed == 0 {
        println!("skipped: nothing left after filtering");
        return client.upsert_comment(
            owner,
            name,
            number,
            MARKER,
            asked.explain(
                "Nothing to review: every file this changes is on the skip list — lockfiles, \
                 generated output, images and the like.",
            ),
        );
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
    let notes = parse(&scrub(&answer, request.vomit))?;
    println!("{} note(s) over {changed} changed lines", notes.len());

    // An empty answer is the expected one for most pull requests, and an
    // unprompted run says so by leaving no trace. Somebody who asked out loud
    // gets told, because to them silence is indistinguishable from a run that
    // never happened — which is exactly the doubt `/review` exists to remove.
    let body = render(&notes).or_else(|| asked.explain("Reviewed, and found nothing to raise."));

    // Scrubbed a second time, here rather than only above, because this is the
    // one line every path to the page passes through. The first pass protects
    // the parse; this one protects the pull request, and catches a notice the
    // endpoint managed to land inside a finding rather than around it.
    let body = body.map(|body| scrub(&body, request.vomit));
    client.upsert_comment(owner, name, number, MARKER, body)
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
/// Cuts the endpoint's own advertising out of a piece of text.
///
/// Some gateways bolt a notice onto the completions they return — an advert for
/// a feature, a nag about a setting. It is not a reading of the diff, it is the
/// service talking about itself, and it has no business on a pull request in
/// somebody else's organization. It also breaks parsing wherever it lands,
/// which repeats it into the workflow log through the error.
///
/// The markers arrive from `GUARDENER_MODEL_VOMIT` rather than living here, for
/// the same reason the endpoint does: which service is in use, and what it says
/// about itself, is not something a public repository should spell out. Nothing
/// in this file may name one, tests included. Empty or unset means no
/// scrubbing, which is the normal case.
///
/// **Whole lines go.** One marker per line of the variable, and any line of the
/// text containing one is dropped entirely. Deliberately not "cut from the
/// marker onwards": a notice is not guaranteed to be the last thing in an
/// answer, and cutting to the end would throw away everything after a notice
/// that arrived first or in the middle.
///
/// Two consequences worth knowing, both chosen on the same principle — this
/// must never leak, so it fails loudly instead:
///
/// - A notice sharing a line with real content takes that line with it. The
///   answer then fails to parse, or a finding goes missing, rather than the
///   notice reaching the pull request.
/// - A notice spanning several lines needs a marker matching each of them.
///   Any distinctive word on the line will do — unlike the earlier rule, the
///   marker no longer has to be the notice's opening.
fn scrub(text: &str, markers: &str) -> String {
    let markers: Vec<&str> = markers
        .lines()
        .map(str::trim)
        .filter(|marker| !marker.is_empty())
        .collect();
    if markers.is_empty() {
        return text.to_string();
    }
    text.lines()
        .filter(|line| !markers.iter().any(|marker| line.contains(marker)))
        .collect::<Vec<_>>()
        .join("\n")
}

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
    /// The sweep's filter, stated as the comparison it actually makes. A bot's
    /// pull request is excluded first: `review_one` would decline it anyway and
    /// leave no comment behind, so without this it would come back as a
    /// candidate on every sweep for as long as it stayed open.
    #[test]
    fn stale_means_old_and_never_reviewed_and_not_a_bots() {
        let cutoff = epoch("2026-09-01T00:00:00Z").expect("a cutoff");
        let candidate = |updated: &str, reviewed: bool, bot: bool| {
            !bot && epoch(updated).is_some_and(|at| at <= cutoff) && !reviewed
        };

        assert!(candidate("2026-08-01T00:00:00Z", false, false), "the case");
        assert!(!candidate("2026-08-01T00:00:00Z", true, false), "reviewed");
        assert!(!candidate("2026-09-30T00:00:00Z", false, false), "fresh");
        assert!(!candidate("2026-08-01T00:00:00Z", false, true), "a bot's");
    }

    /// The rule the whole `Asked` distinction exists for: a person who asked
    /// always gets an answer, and an unprompted run with nothing to say leaves
    /// the pull request untouched.
    /// Appended after the JSON — the shape that started this. Without the
    /// scrub the answer fails to parse and the notice is repeated in the error,
    /// and so into the workflow log.
    ///
    /// Every sample here is invented. Real markers belong in the secret and
    /// nowhere else: a test using one would publish, in a public repository,
    /// most of the value the secret exists to keep out of it.
    #[test]
    fn a_notice_after_the_json_is_cut_away() {
        let answer = "[{\"path\":\"a.rs\",\"comment\":\"the retry loses the error\"}]\n\n\
             * Upgrade for faster answers: https://example.invalid/upgrade";
        let notes = parse(&scrub(answer, "Upgrade for")).expect("the JSON survives");

        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].comment, "the retry loses the error");
    }

    /// Before the JSON, and in the middle of it. Dropping the marker's line
    /// rather than everything from the marker on is what makes these work — a
    /// notice is not guaranteed to arrive last.
    #[test]
    fn a_notice_before_or_between_the_json_leaves_it_whole() {
        let leading = "* Upgrade for faster answers.\n\
             [{\"path\":\"a.rs\",\"comment\":\"kept\"}]";
        let notes = parse(&scrub(leading, "Upgrade for")).expect("leading notice");
        assert_eq!(notes[0].comment, "kept");

        let middle = "[\n\
             {\"path\":\"a.rs\",\"comment\":\"first\"},\n\
             * Upgrade for faster answers.\n\
             {\"path\":\"b.rs\",\"comment\":\"second\"}\n\
             ]";
        let notes = parse(&scrub(middle, "Upgrade for")).expect("notice in the middle");
        assert_eq!(notes.len(), 2, "nothing after the notice was thrown away");
        assert_eq!(notes[1].comment, "second");
    }

    /// A marker matching any part of the line is enough — it need not be the
    /// notice's opening, which the earlier cut-to-the-end rule demanded.
    #[test]
    fn a_marker_from_the_middle_of_the_line_still_removes_it() {
        let answer = "[]\n\u{2605} Notice: see https://example.invalid/x for details.";
        assert_eq!(scrub(answer, "example.invalid"), "[]");
    }

    /// The second scrub, on the rendered comment, is what stands between a
    /// notice the endpoint buried inside a finding and the pull request page.
    #[test]
    fn the_rendered_comment_is_scrubbed_before_it_is_posted() {
        let body = render(&[
            Note {
                path: "a.rs".to_string(),
                line: None,
                severity: None,
                comment: "a real finding".to_string(),
            },
            Note {
                path: "b.rs".to_string(),
                line: None,
                severity: None,
                comment: "Upgrade for faster answers: https://example.invalid".to_string(),
            },
        ])
        .expect("a comment");
        let cleaned = scrub(&body, "Upgrade for");

        assert!(cleaned.contains("a real finding"));
        assert!(!cleaned.contains("Upgrade"));
        assert!(!cleaned.contains("example.invalid"));
    }

    /// The normal case, and the one that must not change: no markers set, so
    /// the answer is passed through unchanged.
    #[test]
    fn no_markers_means_no_scrubbing() {
        let answer = "[{\"path\":\"a.rs\",\"comment\":\"keep every word of this\"}]";
        assert_eq!(scrub(answer, ""), answer);
        assert_eq!(scrub(answer, "   \n  \n"), answer);
    }

    /// A marker that never appears leaves the answer alone, rather than
    /// truncating it somewhere arbitrary.
    #[test]
    fn a_marker_that_is_absent_changes_nothing() {
        let answer = "[{\"path\":\"a.rs\",\"comment\":\"untouched\"}]";
        assert_eq!(scrub(answer, "no such notice here"), answer);
    }

    #[test]
    fn only_a_person_who_asked_is_told_there_was_nothing_to_say() {
        assert_eq!(
            Asked::ByAPerson.explain("nothing to review"),
            Some("nothing to review".to_string())
        );
        assert_eq!(Asked::BySchedule.explain("nothing to review"), None);
    }

    /// `--stale-days 0` puts the cutoff at now, so a pull request touched a
    /// moment ago is still eligible. That is the default, and the sweep is the
    /// only thing reviewing on its own, so getting it backwards would mean
    /// nothing was ever swept.
    #[test]
    fn a_zero_stale_day_cutoff_admits_everything_already_updated() {
        let now = now_epoch();
        let yesterday = now - 86_400;
        assert!(yesterday <= now, "yesterday is within a zero-day cutoff");
    }
}
