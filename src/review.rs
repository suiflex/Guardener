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

use crate::config::Registry;
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
    pub repo: &'a str,
    pub pull_request: u64,
    /// Endpoint, key and model, all from the environment: together they name a
    /// private service, and none of them belong in a public repository.
    pub endpoint: &'a str,
    pub key: &'a str,
    pub model: &'a str,
}

pub fn run(client: &Client, request: &Request<'_>) -> Result<()> {
    let (owner, name) = split_repo(request.repo)?;
    let registry = Registry::load(request.registry)?;
    if registry.find(request.repo).is_none() {
        bail!(
            "{} is not in the registry; add it before reviewing pull requests there",
            request.repo
        );
    }
    let settings: Settings = toml::from_str(
        &std::fs::read_to_string(request.settings)
            .with_context(|| format!("failed to read {}", request.settings.display()))?,
    )
    .with_context(|| format!("failed to parse {}", request.settings.display()))?;

    let pull_request = client.pull_request(owner, name, request.pull_request)?;

    // A bot's pull request was not written by anyone who can act on a review.
    if pull_request["user"]["type"].as_str() == Some("Bot") {
        println!("skipped: opened by a bot");
        return client.upsert_comment(owner, name, request.pull_request, MARKER, None);
    }

    let diff = client.pull_request_diff(owner, name, request.pull_request)?;
    let reviewable = filter(&diff, &settings.skip);
    let changed = changed_lines(&reviewable);

    if changed == 0 {
        println!("skipped: nothing left after filtering");
        return client.upsert_comment(owner, name, request.pull_request, MARKER, None);
    }
    if changed > settings.max_changed_lines {
        println!(
            "skipped: {changed} changed lines exceeds {}",
            settings.max_changed_lines
        );
        return client.upsert_comment(
            owner,
            name,
            request.pull_request,
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

    client.upsert_comment(owner, name, request.pull_request, MARKER, render(&notes))
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
}
