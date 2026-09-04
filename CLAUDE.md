# Guardener

Rust CLI (`guardener`) that applies the suiflex engineering standard across the organization's
GitHub repositories. Not a service: every subcommand reads the world, decides, writes once,
exits. Runs from GitHub Actions.

`AGENTS.md` is a symlink to this file — keep it readable by any agent, not just Claude.

For code changes, use `/forgeguard-engineering`.

## Layout

| Path | What |
|---|---|
| `src/main.rs` | clap CLI, three subcommands, env-var reading (`GUARDENER_TOKEN`, `GUARDENER_MODEL*`) |
| `src/check.rs` | `check` — ForgeGuard gate over a PR's changed lines → check run + one edited comment |
| `src/hygiene.rs` | `hygiene` — org drift sweep → one standing issue; `--fix` opens add-only PRs |
| `src/review.rs` | `review` — model reads the diff, posts a second-opinion comment; sweeps stale PRs when `--pr` is omitted |
| `src/github.rs` | the only place that talks to the GitHub API (`ureq`); `Client::new(token, dry_run)` |
| `src/config.rs` | `config/repositories.toml` registry + per-repo policy resolution |
| `config/*.toml` | org policy: watched repos, default ForgeGuard config, label set, review limits |
| `templates/` | files `hygiene --fix` adds to a repository |
| `.github/workflows/` | `check.yml` and `review.yml` are called by other repos; the rest run this repo |

Analysis itself lives upstream in `forgeguard-core` (git dependency, pinned by tag in
`Cargo.toml`). This repo is the org-wide driver around it.

Setup — the app, its permissions, the secrets — is in [INSTALL.md](INSTALL.md); what each
subcommand does and why is in [README.md](README.md). Do not restate either here.

## Commands

```sh
make            # list targets
make verify     # fmt + lint + test — exactly what CI runs, in CI's order
make fix        # cargo fmt --all
make hygiene    # preview the org sweep, writes nothing
make gate ROOT=../ForgeGuard REPO=suiflex/ForgeGuard PR=66 BASE=origin/main
```

`GUARDENER_TOKEN` is required even for `--dry-run`: a dry run withholds the *writes*, not the
reads. `make` fills it from `gh auth token`.

Tests are `#[cfg(test)]` modules at the bottom of each `src/*.rs`. No integration test dir.

## Invariants — do not break these

- **`.github/workflows/check.yml` runs on `pull_request_target`.** It must never execute
  anything from the branch it analyses. Do not add a step that runs the checkout's scripts, and
  do not drop `--untrusted` (which is derived from the *event*, not the checkout, and disables
  ForgeGuard's configured quality commands for out-of-org branches).
- **`hygiene --fix` only ever adds.** It never edits or replaces an existing file, and never
  changes a setting. Labels and branch protection are reported for a person to act on. A bot
  that quietly revises decisions org-wide is worse than the drift it catches. This also
  dictates design: a new capability ships as a *new* stub file, because one bolted onto an
  already-installed `guardener.yml` could never be rolled out without hand-written PRs.
- **The daily hygiene schedule never passes `--fix`.** That stays a `workflow_dispatch` a
  person triggers.
- **`review` can never block a merge.** No check run, own comment marker, and the workflow step
  carries `continue-on-error`. The gate decides mergeability; the model does not. `review.yml`
  drops the `continue-on-error` because a person asked out loud and should see a failure — it
  is still safe, because a comment-triggered run is nobody's required status.
- **`/review` must clear all four gates in the caller's `if:`** (org owner, comment is on a PR,
  body starts with `/review`, `author_association` is OWNER/MEMBER/COLLABORATOR). `issue_comment`
  is the same risk class as `pull_request_target` — base-repo context, writable token, text a
  stranger can write. Without the association check, anyone can spend the model budget.
- **The review sweep is always bounded.** `--max` (default 10) caps one run, and `--dry-run`
  lists candidates without asking the model — deliberately unlike `review --pr --dry-run`,
  which does ask. A first sweep meets every never-reviewed PR at once.
- **No endpoints, keys or model names in `config/`.** They arrive as `GUARDENER_MODEL_URL`,
  `GUARDENER_MODEL_KEY`, `GUARDENER_MODEL` secrets, so changing the reviewing model is a secret
  change, not a PR against a public repo.
- **`config/forgeguard.toml` stays `mode = "strict"` with no `[[commands]]`.** Strict is
  bearable only because the gate reads changed lines only. Never soften it with
  `warnings_block = false` — that overrides the mode instead of adjusting it; a repo needing
  less says so via `mode` in `config/repositories.toml`, where the exception is visible.
- **Third-party actions are pinned by commit SHA** (hygiene reports unpinned ones). The
  `dtolnay/rust-toolchain` SHA must stay the head of `stable` — that action hardcodes a version
  per branch, and a SHA from the wrong branch installs the wrong toolchain regardless of the
  comment beside it.
- **A repo listed in `config/repositories.toml` is in the hygiene sweep.** The gate is separate
  and opt-in: it runs only where `.github/workflows/guardener.yml` exists.

## Conventions

- Comments explain *why*, at length, above the thing. Match that density — this codebase argues
  its decisions in prose and the existing files are the style guide.
- All GitHub API calls go through `src/github.rs`. The only other HTTP caller is
  `src/review.rs`, which talks to the model endpoint — nothing else should reach for `ureq`.
- Every write path must honour `dry_run` (`Client` holds the flag).
- Conventional Commits. No AI-attribution trailers or footers.
