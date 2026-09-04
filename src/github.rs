//! The GitHub REST calls Guardener makes, and nothing else.
//!
//! Deliberately hand-rolled against the JSON API rather than run through a
//! client library: the whole surface is four endpoints, and the typed clients
//! rearrange themselves between releases far more often than the REST API does.
//!
//! The token is minted by `actions/create-github-app-token` in the workflow and
//! arrives here through the environment, so this module never sees the App's
//! private key and never mints anything itself.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

const API: &str = "https://api.github.com";
const ACCEPT: &str = "application/vnd.github+json";
const API_VERSION: &str = "2022-11-28";
const AGENT: &str = "guardener";

/// GitHub rejects a check run carrying more than 50 annotations in one request.
/// Guardener sends the first 50 and says so in the summary rather than paging:
/// a pull request with more than 50 findings has a problem that a complete list
/// would not help anyone solve.
pub const ANNOTATION_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy)]
enum Method {
    Post,
    Patch,
    Put,
    Delete,
}

pub struct Client {
    token: String,
    dry_run: bool,
}

#[derive(Debug, Deserialize)]
struct Comment {
    id: u64,
    body: Option<String>,
}

/// One completed check run. Grouped into a struct because these fields travel
/// together into a single request and mean nothing apart.
pub struct CheckRun<'a> {
    pub name: &'a str,
    pub head_sha: &'a str,
    pub conclusion: &'a str,
    pub title: &'a str,
    pub summary: &'a str,
    pub annotations: &'a [Annotation],
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub level: &'static str,
    pub title: String,
    pub message: String,
}

impl Client {
    pub fn new(token: String, dry_run: bool) -> Self {
        Self { token, dry_run }
    }

    /// The headers every call carries. Split out because ureq types a request
    /// by whether it may have a body, so the verbs cannot share a builder.
    fn decorate<B>(&self, builder: ureq::RequestBuilder<B>) -> ureq::RequestBuilder<B> {
        builder
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("Accept", ACCEPT)
            .header("X-GitHub-Api-Version", API_VERSION)
            .header("User-Agent", AGENT)
    }

    fn get(&self, url: &str) -> Result<Value> {
        self.decorate(ureq::get(url))
            .call()
            .with_context(|| format!("GET {url}"))?
            .body_mut()
            .read_json()
            .with_context(|| format!("decoding the response to GET {url}"))
    }

    fn send(&self, method: Method, url: &str, body: Option<Value>) -> Result<()> {
        if self.dry_run {
            let rendered = body
                .as_ref()
                .map(|value| serde_json::to_string_pretty(value).unwrap_or_default())
                .unwrap_or_default();
            println!("--- would {method:?} {url}\n{rendered}");
            return Ok(());
        }
        let body = body.unwrap_or(Value::Null);
        match method {
            Method::Post => self.decorate(ureq::post(url)).send_json(body).map(|_| ()),
            Method::Patch => self.decorate(ureq::patch(url)).send_json(body).map(|_| ()),
            Method::Put => self.decorate(ureq::put(url)).send_json(body).map(|_| ()),
            Method::Delete => self.decorate(ureq::delete(url)).call().map(|_| ()),
        }
        .with_context(|| format!("{method:?} {url}"))
    }

    /// Keeps exactly one Guardener comment on a pull request.
    ///
    /// Reruns are the normal case — every push re-runs the gate — so a fresh
    /// comment each time would bury the conversation. An empty body means the
    /// findings are gone, and the comment is removed rather than replaced with
    /// a notice that there is nothing to report: a resolved problem should
    /// leave no trace on the page.
    pub fn upsert_comment(
        &self,
        owner: &str,
        repo: &str,
        pull_request: u64,
        marker: &str,
        body: Option<String>,
    ) -> Result<()> {
        let existing = self.find_comment(owner, repo, pull_request, marker)?;
        match (existing, body) {
            (Some(id), Some(body)) => self.send(
                Method::Patch,
                &format!("{API}/repos/{owner}/{repo}/issues/comments/{id}"),
                Some(json!({ "body": format!("{marker}\n{body}") })),
            ),
            (Some(id), None) => self.send(
                Method::Delete,
                &format!("{API}/repos/{owner}/{repo}/issues/comments/{id}"),
                None,
            ),
            (None, Some(body)) => self.send(
                Method::Post,
                &format!("{API}/repos/{owner}/{repo}/issues/{pull_request}/comments"),
                Some(json!({ "body": format!("{marker}\n{body}") })),
            ),
            (None, None) => Ok(()),
        }
    }

    /// Paginates to the end on purpose. Stopping at the first page would let a
    /// busy pull request push the marker comment out of view, and Guardener
    /// would then post a second one on every push — the exact pile-up the
    /// marker exists to prevent.
    fn find_comment(
        &self,
        owner: &str,
        repo: &str,
        pull_request: u64,
        marker: &str,
    ) -> Result<Option<u64>> {
        for page in 1.. {
            let url = format!(
                "{API}/repos/{owner}/{repo}/issues/{pull_request}/comments?per_page=100&page={page}"
            );
            let comments: Vec<Comment> = serde_json::from_value(self.get(&url)?)
                .context("unexpected shape for the comment list")?;
            let count = comments.len();
            if let Some(found) = comments
                .iter()
                .find(|comment| comment.body.as_deref().is_some_and(|b| b.contains(marker)))
            {
                return Ok(Some(found.id));
            }
            if count < 100 {
                return Ok(None);
            }
        }
        unreachable!()
    }

    pub fn create_check_run(&self, owner: &str, repo: &str, run: &CheckRun<'_>) -> Result<()> {
        let annotations: Vec<Value> = run
            .annotations
            .iter()
            .take(ANNOTATION_LIMIT)
            .map(|annotation| {
                json!({
                    "path": annotation.path,
                    "start_line": annotation.start_line,
                    "end_line": annotation.end_line,
                    "annotation_level": annotation.level,
                    "title": annotation.title,
                    "message": annotation.message,
                })
            })
            .collect();

        self.send(
            Method::Post,
            &format!("{API}/repos/{owner}/{repo}/check-runs"),
            Some(json!({
                "name": run.name,
                "head_sha": run.head_sha,
                "status": "completed",
                "conclusion": run.conclusion,
                "output": {
                    "title": run.title,
                    "summary": run.summary,
                    "annotations": annotations,
                },
            })),
        )
    }

    /// A GET whose absence is an answer rather than a failure. Branch
    /// protection and an unread file both report 404, and in both cases that
    /// is the fact being looked for.
    fn get_optional(&self, url: &str) -> Result<Option<Value>> {
        match self.decorate(ureq::get(url)).call() {
            Ok(mut response) => {
                Ok(Some(response.body_mut().read_json().with_context(
                    || format!("decoding the response to GET {url}"),
                )?))
            }
            Err(ureq::Error::StatusCode(404)) => Ok(None),
            Err(error) => Err(error).with_context(|| format!("GET {url}")),
        }
    }

    fn get_text(&self, url: &str) -> Result<Option<String>> {
        match self
            .decorate(ureq::get(url))
            .header("Accept", "application/vnd.github.raw")
            .call()
        {
            Ok(mut response) => {
                Ok(Some(response.body_mut().read_to_string().with_context(
                    || format!("reading the response to GET {url}"),
                )?))
            }
            Err(ureq::Error::StatusCode(404)) => Ok(None),
            Err(error) => Err(error).with_context(|| format!("GET {url}")),
        }
    }

    fn post_json(&self, url: &str, body: Value) -> Result<Value> {
        if self.dry_run {
            println!(
                "--- would Post {url}\n{}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );
            return Ok(Value::Null);
        }
        self.decorate(ureq::post(url))
            .send_json(body)
            .with_context(|| format!("POST {url}"))?
            .body_mut()
            .read_json()
            .with_context(|| format!("decoding the response to POST {url}"))
    }

    /// The repository itself: its default branch, and whether it is archived.
    pub fn repository(&self, owner: &str, repo: &str) -> Result<Value> {
        self.get(&format!("{API}/repos/{owner}/{repo}"))
    }

    /// Every path on a branch in one call, or `None` when the repository has no
    /// commits yet. Cheaper and far less fiddly than asking about each expected
    /// file separately.
    ///
    /// A repository with no commits answers 409 here. That is not a failure and
    /// not an empty tree: a repository nobody has pushed to cannot be missing a
    /// licence yet, and reporting four findings against it would be asking for
    /// work that the first push does anyway.
    pub fn tree(&self, owner: &str, repo: &str, branch: &str) -> Result<Option<Vec<String>>> {
        let url = format!("{API}/repos/{owner}/{repo}/git/trees/{branch}?recursive=1");
        let value = match self.decorate(ureq::get(&url)).call() {
            Ok(mut response) => response
                .body_mut()
                .read_json::<Value>()
                .with_context(|| format!("decoding the response to GET {url}"))?,
            Err(ureq::Error::StatusCode(409)) => return Ok(None),
            Err(error) => return Err(error).with_context(|| format!("GET {url}")),
        };
        Ok(Some(
            value["tree"]
                .as_array()
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| entry["path"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        ))
    }

    pub fn file(&self, owner: &str, repo: &str, path: &str) -> Result<Option<String>> {
        self.get_text(&format!("{API}/repos/{owner}/{repo}/contents/{path}"))
    }

    pub fn labels(&self, owner: &str, repo: &str) -> Result<Vec<String>> {
        let value = self.get(&format!("{API}/repos/{owner}/{repo}/labels?per_page=100"))?;
        Ok(value
            .as_array()
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|label| label["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Whether a branch is protected, or `None` when that cannot be told.
    ///
    /// Three answers, not two. 404 means the branch has no protection, which is
    /// the finding. 403 means the question was refused — the endpoint needs
    /// administration rights, and a private repository on a plan without
    /// protected branches refuses it outright. Reporting a refusal as "not
    /// protected" would raise a complaint about a repository that cannot act on
    /// it, so the check stays silent instead.
    pub fn is_protected(&self, owner: &str, repo: &str, branch: &str) -> Result<Option<bool>> {
        let url = format!("{API}/repos/{owner}/{repo}/branches/{branch}/protection");
        match self.decorate(ureq::get(&url)).call() {
            Ok(_) => Ok(Some(true)),
            Err(ureq::Error::StatusCode(404)) => Ok(Some(false)),
            Err(ureq::Error::StatusCode(403)) => Ok(None),
            Err(error) => Err(error).with_context(|| format!("GET {url}")),
        }
    }

    /// Keeps exactly one open issue carrying `marker`, the same way
    /// [`Self::upsert_comment`] keeps one comment. `None` closes it: a report
    /// that has nothing left to say should stop asking for attention, but the
    /// history of what it once said is worth keeping.
    pub fn upsert_issue(
        &self,
        owner: &str,
        repo: &str,
        marker: &str,
        title: &str,
        body: Option<String>,
    ) -> Result<()> {
        let existing = self.find_issue(owner, repo, marker)?;
        match (existing, body) {
            (Some(number), Some(body)) => self.send(
                Method::Patch,
                &format!("{API}/repos/{owner}/{repo}/issues/{number}"),
                Some(json!({ "title": title, "body": format!("{marker}\n{body}") })),
            ),
            (Some(number), None) => self.send(
                Method::Patch,
                &format!("{API}/repos/{owner}/{repo}/issues/{number}"),
                Some(json!({ "state": "closed" })),
            ),
            (None, Some(body)) => self.send(
                Method::Post,
                &format!("{API}/repos/{owner}/{repo}/issues"),
                Some(json!({ "title": title, "body": format!("{marker}\n{body}") })),
            ),
            (None, None) => Ok(()),
        }
    }

    fn find_issue(&self, owner: &str, repo: &str, marker: &str) -> Result<Option<u64>> {
        for page in 1.. {
            let url =
                format!("{API}/repos/{owner}/{repo}/issues?state=open&per_page=100&page={page}");
            let issues: Vec<Value> = serde_json::from_value(self.get(&url)?)
                .context("unexpected shape for the issue list")?;
            let count = issues.len();
            for issue in &issues {
                // The issues endpoint also returns pull requests. A pull
                // request is not where a standing report belongs.
                if issue.get("pull_request").is_some() {
                    continue;
                }
                if issue["body"].as_str().is_some_and(|b| b.contains(marker)) {
                    return Ok(issue["number"].as_u64());
                }
            }
            if count < 100 {
                return Ok(None);
            }
        }
        unreachable!()
    }

    /// Points a branch at the tip of another one, reporting whether it had to
    /// be created. An existing branch means an earlier run already opened this
    /// pull request, and pushing over it would rewrite whatever a person may
    /// have done to it since.
    pub fn create_branch(&self, owner: &str, repo: &str, branch: &str, from: &str) -> Result<bool> {
        let head = self.get(&format!("{API}/repos/{owner}/{repo}/git/ref/heads/{from}"))?;
        let sha = head["object"]["sha"]
            .as_str()
            .ok_or_else(|| anyhow!("{owner}/{repo} has no commit at the tip of {from}"))?;

        if self
            .get_optional(&format!(
                "{API}/repos/{owner}/{repo}/git/ref/heads/{branch}"
            ))?
            .is_some()
        {
            return Ok(false);
        }

        self.send(
            Method::Post,
            &format!("{API}/repos/{owner}/{repo}/git/refs"),
            Some(json!({ "ref": format!("refs/heads/{branch}"), "sha": sha })),
        )?;
        Ok(true)
    }

    /// Writes a file that does not exist yet. No `sha` is sent, so GitHub
    /// refuses the write if the path is already taken — the API enforces the
    /// rule that Guardener only ever adds.
    pub fn create_file(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        path: &str,
        message: &str,
        contents: &str,
    ) -> Result<()> {
        self.send(
            Method::Put,
            &format!("{API}/repos/{owner}/{repo}/contents/{path}"),
            Some(json!({
                "message": message,
                "branch": branch,
                "content": base64(contents.as_bytes()),
            })),
        )
    }

    pub fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<String> {
        let created = self.post_json(
            &format!("{API}/repos/{owner}/{repo}/pulls"),
            json!({ "head": head, "base": base, "title": title, "body": body }),
        )?;
        Ok(created["html_url"]
            .as_str()
            .unwrap_or("(dry run)")
            .to_string())
    }
}

/// Standard base64, written out rather than pulled in: the contents endpoint is
/// the only caller and this is the whole of what it needs.
fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let bits = chunk.iter().enumerate().fold(0u32, |acc, (index, byte)| {
            acc | (*byte as u32) << (16 - 8 * index)
        });
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(ALPHABET[((bits >> (18 - 6 * index)) & 0b11_1111) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Splits `owner/name` for the endpoints, which take the two halves separately.
pub fn split_repo(repo: &str) -> Result<(&str, &str)> {
    repo.split_once('/')
        .ok_or_else(|| anyhow!("expected a repository as owner/name, got {repo:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_pads_every_remainder() {
        // The three cases that exist, from RFC 4648's own examples.
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn base64_survives_bytes_that_are_not_text() {
        assert_eq!(base64(&[0x00, 0xff, 0x80]), "AP+A");
    }

    #[test]
    fn a_repository_must_be_named_owner_and_name() {
        assert_eq!(split_repo("suiflex/rdb").unwrap(), ("suiflex", "rdb"));
        assert!(split_repo("rdb").is_err());
    }
}
