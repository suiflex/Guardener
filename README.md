# Guardener — the suiflex engineering standard, applied to every repository

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/logo-dark.svg">
    <img src="assets/brand/logo-light.svg" alt="Guardener" width="360">
  </picture>
</p>

Runs the suiflex engineering standard across the organization's repositories.

[ForgeGuard](https://github.com/suiflex/ForgeGuard) is excellent locally and
runs only when someone remembers to run it. Guardener is the same analysis,
applied to every pull request in the organization whether anyone remembers or
not, from one policy kept in one place.

## What it does today

**`guardener check`** runs ForgeGuard's gate over the lines a pull request
changed and reports the result twice: as a check run whose annotations land on
the offending lines, and as a single comment that is edited in place on every
push and removed once the findings are gone.

**`guardener hygiene`** walks every watched repository and reports what has
drifted from the organization standard — a missing licence, owners file, triage
or gate workflow, a label the triage workflow needs and cannot find, an
unprotected default branch, a third-party action named by a tag rather than a
commit. It writes one issue on this repository, edited in place and closed once
there is nothing left to say.

`--fix` opens a pull request adding the files a repository is missing. It only
ever *adds*: it never edits or replaces a file that already exists, and it never
changes a setting. Labels and branch protection are reported and left to a
person. That boundary is the point — a bot that quietly revises decisions across
an organization is worse than the drift it was meant to catch.

The daily sweep never passes `--fix`; opening pull requests across the
organization is something a person triggers from `workflow_dispatch`.

## The app

Guardener acts as its own GitHub App, `guardener-bot`, rather than borrowing the
one that labels pull requests. The two need very different things: triage reads
team membership and writes labels, while this writes check runs, opens pull
requests and reads branch protection. Sharing one app would mean granting the
label bot the right to commit to every repository in the organization.

Its credentials reach the workflows as the organization secrets
`GUARDENER_BOT_APP_ID` and `GUARDENER_BOT_PRIVATE_KEY`. Every repository that
calls the gate needs them, so they have to be visible to it — an organization
secret restricted to selected repositories that omits one is the failure this
is most often traced back to. Check with:

```sh
gh api /repos/suiflex/<repo>/actions/organization-secrets --jq '.secrets[].name'
```

The app needs, and needs no more than:

| Repository permission | Why |
|---|---|
| Metadata: read | Required of every app |
| Contents: read and write | Read a repository's tree and workflows; write the branch and files a `--fix` pull request carries |
| Checks: read and write | Report the gate result on a pull request |
| Issues: read and write | Read the label set; keep the standing hygiene issue |
| Pull requests: read and write | The gate's comment, and opening a `--fix` pull request |
| Administration: read | Read branch protection, and nothing else — drop it and `exempt = ["branch-protection"]` if that is too much to grant |

No organization permissions, and no webhook: nothing here listens, so the app
should have its webhook switched off. Each workflow narrows further than this
through `permission-*` on the token it mints, so a mistake in one workflow
cannot reach what the others use.

## Policy

`config/forgeguard.toml` is the ForgeGuard configuration used by any repository
that does not ship its own. A repository that keeps a `.forgeguard/config.toml`
overrides it completely — the organization default is a floor, not a ceiling.

`config/repositories.toml` lists the watched repositories: the public ones, and
not `homebrew-tap` or `scoop-bucket`, whose contents a release workflow writes
rather than a person. One line each is the normal case; `mode`, `rules` and
`exempt` are there for the repository that genuinely needs an exception, so the
exception is visible centrally instead of buried.

`config/labels.toml` is the label set the organization's triage workflow
applies. `suiflex/.github/scripts/sync-labels.sh` creates those labels; this
file is how Guardener audits that it was run.

Being listed in the registry puts a repository in the hygiene sweep. The gate is
separate and opt-in: it runs only where the repository has added the stub below.

The policy is `strict`, which on ForgeGuard's version 2 blocks every finding at
warning severity or above rather than errors alone. That is bearable only
because the gate reads the lines a pull request changed and nothing else: it
blocks on work already in front of the author, never on the history behind it.
A repository that needs a softer gate says so with its own `mode` in the
registry.

Setting all of this up — the app, its permissions, the secrets, the model — is
in [INSTALL.md](INSTALL.md).

## Turning the gate on for a repository

Add this to the repository as `.github/workflows/guardener.yml` — or let
`guardener hygiene --fix` open the pull request that adds it:

```yaml
name: Guardener

on:
  pull_request_target:
    types: [opened, reopened, synchronize]

jobs:
  forgeguard:
    if: github.repository_owner == 'suiflex'
    uses: suiflex/Guardener/.github/workflows/check.yml@main
    secrets:
      app_id: ${{ secrets.GUARDENER_BOT_APP_ID }}
      private_key: ${{ secrets.GUARDENER_BOT_PRIVATE_KEY }}
```

## Running it by hand

`make` lists what there is. `make verify` is what CI runs, in CI's order.
`make hygiene` and `make hygiene-fix` preview the sweep and the pull requests it
would open, writing nothing.


`--dry-run` prints the requests instead of sending them. It still reads — it
has to know what is already there for the preview to be worth anything — so it
needs a token, and a read-only personal one will do:

```sh
GUARDENER_TOKEN=$(gh auth token) cargo run -- check \
  --root ../ForgeGuard \
  --repo suiflex/ForgeGuard \
  --pr 123 --head-sha "$(git -C ../ForgeGuard rev-parse HEAD)" \
  --base origin/main \
  --dry-run
```

The hygiene sweep reads the whole organization, so a dry run is the way to see
what it would say and what it would open, without writing anything:

```sh
GUARDENER_TOKEN=$(gh auth token) cargo run -- hygiene --dry-run
```

A real run reads its token from `GUARDENER_TOKEN`.

## Brand

`assets/brand/` holds the mark, the wordmark in both themes, and
`app-icon.png` — the 512×512 square uploaded as the GitHub App's avatar.

The shield is ForgeGuard's, because this enforces ForgeGuard's standard. It is
drawn as an outline rather than filled so the two are told apart at the twenty
pixels a bot avatar actually gets beside a comment, and the eye at its centre
says what this one does that the other does not: it watches, without being
asked.

## The review

`guardener review` has a model read the pull request and say what a scanner
cannot: logic that is wrong, an invariant the change breaks, an error path that
loses data quietly, a contract its callers will not survive. It is told what
ForgeGuard already reports so it does not spend the budget re-deriving findings
that arrive with line numbers attached, and it is asked to return an empty
answer when nothing meets that bar — which is the expected answer for most pull
requests.

It is deliberately unequal to the gate. It posts under its own marker, it never
creates a check run, and it runs in a workflow of its own, so a model that is
slow, wrong or unreachable can never be the reason a pull request cannot merge.

It is also never asked on its own initiative. Two things can start it: a
`/review` comment, and the weekly sweep below. It deliberately does not run on
every push — a reading nobody asked for, repeated on every commit of every pull
request in the organization, paid for far more of them than anyone read.

It reads the diff from the API rather than a checkout, so unlike the gate it
needs no clone.

Three environment variables carry the endpoint, and none of them are in this
repository — together they name a private service, so they arrive as the
secrets `GUARDENER_MODEL_URL`, `GUARDENER_MODEL_KEY` and `GUARDENER_MODEL`.
Changing which model reviews pull requests is a change to a secret, not a pull
request against this repository. The endpoint is expected to answer at
`<url>/chat/completions` in the shape OpenAI made common, which is also what
Ollama, vLLM and most gateways speak.

`config/review.toml` holds what is worth arguing about instead: how large a
change is still worth reading, and which files carry no judgement worth paying
for.

A repository that passes no `model_url` to the workflow still gets the gate;
it just gets no second opinion.

### Asking for one

Comment `/review` on a pull request and the model reads it again. That runs
`.github/workflows/review.yml` — the review on its own, not the gate a second
time: the review reads the diff from the API, so it needs neither the checkout
nor the merge base the gate cannot do without.

Only the owner, an organization member, or an invited collaborator can ask.
`issue_comment` runs in the base repository's context with a writable token, so
without that check anyone able to type in a comment box could spend the model
budget. `guardener hygiene --fix` opens the pull request that adds the stub to a
repository missing it.

### The sweep

`guardener review` with no `--pr` walks every watched repository for open pull
requests that carry no review comment at all, and reviews them. Since nothing
reviews on every push any more, this is what catches a pull request whose author
never thought to type `/review` — and the "no comment at all" condition is what
stops it re-reading the ones already answered.

`--stale-days` defaults to 0, so age is no bar: a pull request opened this
morning is as eligible as one from last month. Raise it to leave recent work
alone. `--max` (default 30) bounds a run, because the first one meets every
never-reviewed pull request at once. `--repo` narrows the walk to one
repository, the way `hygiene --repo` does.

A dry run lists the candidates and asks the model nothing, unlike a dry run of a
single review, where the model's answer is the thing worth previewing:

```sh
GUARDENER_TOKEN=$(gh auth token) cargo run -- review --dry-run
```

`.github/workflows/review-sweep.yml` runs it weekly, on a Monday morning, and
takes `stale_days`, `max` and `dry_run` from `workflow_dispatch`.

## Not built yet

Inline review comments. The model already reports a file and a line, but they
are rendered as text: placing a remark on the wrong line is worse than placing
it in a list, and the diff-position arithmetic that avoids that has not been
written.
