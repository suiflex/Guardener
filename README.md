# Guardener

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
      app_id: ${{ secrets.SUIFLEX_BOT_APP_ID }}
      private_key: ${{ secrets.SUIFLEX_BOT_PRIVATE_KEY }}
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

## Not built yet

`review` — a model reading the diff — is next. It will post under its own
marker, kept apart from the findings above: ForgeGuard's output can be trusted
and a model's cannot, and the two should not look alike on a pull request page.
