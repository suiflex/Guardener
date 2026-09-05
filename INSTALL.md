# Installing Guardener

Setting this up is mostly work in GitHub's settings pages, and about ten
minutes of it. The parts that go wrong are few and specific, so they each have
a check you can run and a symptom written next to them.

Nothing here needs Rust unless you intend to run Guardener somewhere other than
GitHub Actions.

---

## 1. The app

Guardener acts as its own GitHub App rather than borrowing the one that labels
pull requests. The two want very different things — triage reads team
membership and writes labels; this writes check runs, opens pull requests and
reads branch protection — and sharing one would mean granting the label bot the
right to commit to every repository in the organization.

Create it at `github.com/organizations/<org>/settings/apps/new`.

| Field | Value |
|---|---|
| Name | `Guardener Bot` — the slug becomes the identity on every comment, and cannot be changed later |
| Homepage URL | your Guardener repository |
| Webhook | **uncheck Active** |
| Where can this be installed | Only on this account |

The webhook stays off because nothing listens. Guardener is a binary that
Actions runs, not a service. Subscribe to no events.

### Permissions

These are derived from the fourteen REST endpoints `src/github.rs` actually
calls, and nothing is here for future use.

| Repository permission | Why |
|---|---|
| Metadata: read | Required of every app |
| Contents: read and write | Read a repository's tree and workflows; write the branch and files a `--fix` pull request carries |
| Checks: read and write | Report the gate result on a pull request |
| Issues: read and write | Read the label set; keep the standing hygiene issue |
| Pull requests: read and write | The gate's comment, the review's comment, and opening a `--fix` pull request |
| Administration: **read** | Read branch protection, and nothing else |

Two traps live in this table.

**"Administration" appears twice.** The one you want is under **Repository
permissions**. The one under **Organization permissions** has a similar name
and grants read access to organization settings, which Guardener never touches.
Granting the wrong one does not fail loudly: `hygiene.yml` asks the token for
`permission-administration: read`, and `actions/create-github-app-token`
refuses to mint it, so the whole hygiene job dies at its first step.

**Adding a permission later is not enough.** GitHub does not apply it to an
existing installation until an organization owner accepts the change, at
`github.com/organizations/<org>/settings/installations/<installation id>`.
Until then the app declares the permission and the installation does not have
it. Section 5 shows how to see the difference.

If `Administration: read` is more than you want to grant — it is the most
sensitive line in the table — leave it out, delete `permission-administration:
read` from `.github/workflows/hygiene.yml`, and add `exempt =
["branch-protection"]` to each entry in `config/repositories.toml`. Everything
else keeps working; you lose only the detection of a branch anyone can push to.

### Install it

Install the app on the organization, for **All repositories**, or for a
selection that covers every repository in `config/repositories.toml` plus
Guardener itself.

Then **Generate a private key**. A `.pem` file downloads. Two things about it:

- It already *is* the `-----BEGIN`/`-----END` block. Do not reformat it, do not
  join the lines. `actions/create-github-app-token` reads PKCS#1 as it comes.
- Anyone holding it can act as the bot on every repository the app is installed
  on. Copy it into the secret, then delete the file.

If you clicked "Generate a private key" more than once you now have more than
one working key. Delete the ones you are not using, on the app's settings page.

---

## 2. Secrets

Organization secrets, at
`github.com/organizations/<org>/settings/secrets/actions`. Every repository
that calls the gate needs to see them, so set Repository access accordingly.

| Secret | Value | Needed for |
|---|---|---|
| `GUARDENER_BOT_APP_ID` | the app's App ID | everything |
| `GUARDENER_BOT_PRIVATE_KEY` | the whole `.pem` | everything |
| `GUARDENER_MODEL_URL` | base URL, without `/chat/completions` | the review only |
| `GUARDENER_MODEL_KEY` | the endpoint's key | the review only |
| `GUARDENER_MODEL` | the model's name at that endpoint | the review only |
| `GUARDENER_MODEL_VOMIT` | a distinctive phrase from any notice the endpoint bolts on | only if it does |

The model secrets are optional. A repository that passes no `model_url` gets the
gate and no second opinion, and says so in its log.

`GUARDENER_MODEL_VOMIT` is for the endpoint that appends its own advertising to
every completion — a nag about a setting, a link to a feature. That text is not
a reading of the diff and must not reach a pull request, and wherever it lands it
also breaks parsing, which repeats it into the workflow log through the error.
One marker per line, and **any line containing one is dropped whole** — from the
model's answer before it is parsed, and again from the finished comment before it
is posted. Whole lines rather than "everything after the marker", because a
notice is not guaranteed to arrive last; cutting to the end would throw away real
findings that came after one arriving first or in the middle.

Any distinctive phrase from the notice will do — it need not be the opening. A
notice spanning several lines needs a marker matching each of them. And a notice
sharing a line with real content takes that line with it, so the answer fails to
parse or a finding goes missing rather than the notice reaching a pull request:
this is meant to fail loudly rather than leak.

They are secrets rather than configuration because together they name a private
service. Keeping them out of the repository also means changing which model
reviews pull requests is an edit to a secret, not a pull request.

```sh
pbcopy < ~/Downloads/"<your app> private key.pem"   # macOS
```

---

## 3. Labels

The organization's triage workflow cannot apply a label that does not exist, so
a repository missing one silently loses part of its triage. Guardener reports
this but does not fix it — creating labels is not something a bot should do
behind your back.

```sh
# from the organization's .github repository
./scripts/sync-labels.sh <org>/Guardener <org>/<repo> ...
```

---

## 4. Turning it on for a repository

Two separate switches.

**The hygiene sweep** covers whatever is listed in
`config/repositories.toml`. Add a line and it is covered on the next run.

**The gate and the review** run only where the repository asks for them. Add
`.github/workflows/guardener.yml` to that repository — or let `guardener
hygiene --fix` open the pull request that adds it:

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
      model_url: ${{ secrets.GUARDENER_MODEL_URL }}
      model_key: ${{ secrets.GUARDENER_MODEL_KEY }}
      model: ${{ secrets.GUARDENER_MODEL }}
```

The model secrets are listed for compatibility and are unused: `check.yml` runs
the gate and nothing else. Nothing reviews on a push — the model is asked by a
`/review` comment or by the weekly sweep, both below.

**Asking for a review by hand** takes a second file,
`.github/workflows/review.yml`, which `guardener hygiene --fix` adds alongside
the first. It answers `/review` on a pull request by running the review on its
own — not the gate a second time — and only for the owner, an organization
member, or an invited collaborator. That last part is not decoration:
`issue_comment` runs in the base repository's context with a writable token, so
without it anyone able to type in a comment box could spend the model budget.
The full file, with the reasoning beside each condition, is
`templates/workflows/review.yml`.

A second file rather than another trigger on the one above, because `--fix` may
only add and never edit: a capability bolted onto `guardener.yml` could not
reach the repositories that already have one without a hand-written pull request
to each.

### Why this file has to exist in every repository

There is no way to make a workflow run on another repository's pull requests
from somewhere else. A workflow runs only from `.github/workflows/` **in the
repository where the event happened**. That is a rule of GitHub Actions, not a
limitation of Guardener.

The organization's `.github` repository is a common guess and does not do this.
What it does provide organization-wide is default community health files —
issue and pull request templates, `CONTRIBUTING`, `CODE_OF_CONDUCT`,
`SECURITY`, `FUNDING`, and the profile README. Workflows are not on that list.

What `.github` *is* good for, and what it is already used for here, is holding
the reusable workflow so the logic lives once. But a reusable workflow still has
to be **called**, and the call is the file above. That is why it is two lines of
substance and no logic: the thinking lives upstream, and this only says "this
repository takes part".

Organizations on GitHub Enterprise Cloud can require a workflow across
repositories through a ruleset. On Free and Team plans that is not available, so
the stub is the mechanism.

The good news is that placing it is Guardener's own job. Six repositories
currently report `gate-workflow` missing, and:

```sh
gh workflow run hygiene.yml --repo <org>/Guardener -f fix=true
```

opens one pull request per repository adding exactly the files it is missing,
from the templates in `templates/workflows/`. Review and merge them and the
gate — and `/review` with it — is on everywhere.

One thing to check before trusting a run of it. `--fix` refuses to touch a
repository where its branch already exists, and says `guardener/hygiene already
exists; left as it is` rather than force anything. That is the right refusal,
but it also means a branch left behind by an earlier sweep — one whose pull
request was closed, or never opened — silently blocks every later addition to
that repository. A dry run says so plainly, per repository, which is the reason
to read one first:

```sh
gh api repos/<org>/<repo>/git/ref/heads/guardener/hygiene --jq .object.sha
gh api -X DELETE repos/<org>/<repo>/git/refs/heads/guardener/hygiene
```

Delete the abandoned ones, then run the sweep. Run it without `-f fix=true` first and read the issue it
writes — `--fix` opens a pull request against every repository that is missing
something, and that is a lot of notifications to send by accident.

---

## 5. Checking that it worked

Each of these answers one question, and each of them has caught a real failure.

**Do the secrets reach this repository?** This is the single most common
reason every workflow fails at once. It needs no organization admin rights.

```sh
gh api /repos/<org>/<repo>/actions/organization-secrets --jq '.secrets[].name'
```

An empty list means the organization secret is restricted to selected
repositories and this one is not among them.

**Does the installation actually have the permissions?** `declared` is what the
app asks for; the installation is what it was given.

```sh
gh api /apps/<app-slug> --jq '.permissions'
gh api /orgs/<org>/installations \
  --jq '.installations[] | select(.app_slug=="<app-slug>") | .permissions'
```

If `administration` appears in the first and not the second, the permission
change is still waiting to be accepted.

**Does the sweep work end to end?** Run it once by hand:

```sh
gh workflow run hygiene.yml --repo <org>/Guardener
```

It should finish green and leave one issue titled *Repository hygiene*, authored
by `app/<app-slug>` — not by `github-actions`. The author is the proof that the
app token was used rather than the workflow's own.

---

## 6. The model

Three ways to give Guardener a model, in the order of how much they ask of you.

### A. An endpoint you already have

Set the three model secrets and stop. Guardener posts to
`<GUARDENER_MODEL_URL>/chat/completions` in the shape OpenAI made common, which
is what most providers and gateways speak. Nothing is deployed and nothing is
exposed.

### B. A model on your own VPS, reached from GitHub

Run Ollama, vLLM or similar on the VPS, put it behind TLS and a bearer key, and
point `GUARDENER_MODEL_URL` at it.

Understand what this means before doing it: GitHub-hosted runners come from
GitHub's address ranges, which are wide and change, so an IP allowlist is not
practical. In effect the endpoint is open to the internet, and an unauthenticated
inference endpoint is found by scanners within days. So:

- a long random bearer key, checked before any inference happens;
- TLS, via a reverse proxy — not the model server's own listener;
- a request rate and size limit, because the cost of an abused endpoint is a bill;
- bind the model itself to `127.0.0.1` and let only the proxy reach it.

If that reads as more exposure than you want, option C avoids all of it.

### C. A self-hosted runner on the VPS, model on localhost

Register a GitHub Actions self-hosted runner on the VPS and change
`runs-on: ubuntu-latest` to your runner's label. Now the workflow itself
executes on the VPS, so `GUARDENER_MODEL_URL` can be `http://127.0.0.1:11434/v1`
and the model is never exposed at all. No new Guardener code, and the gate,
the sweep and the review all keep working exactly as they do now.

The cost is a real one. GitHub advises against self-hosted runners on public
repositories, because a pull request from a fork can otherwise run its author's
code on your machine. Guardener's gate never executes anything from the branch
it analyses — `--untrusted` turns off ForgeGuard's configured commands for any
branch outside the repository, and the scan is a parse — but the runner is still
a machine that outside pull requests cause to do work. If you take this path,
make the runner ephemeral and containerised, and consider restricting the
workflow to pull requests that are not from forks.

---

## 7. Running Guardener itself on the VPS

Be clear about what exists. **There is no `guardener serve`.** Guardener is a
binary that runs once and exits; webhook-driven operation is designed for but
not written. So a VPS today can do one of these usefully:

**The hygiene sweep on a timer.** It reads the API and needs no checkout, so it
runs anywhere. It replaces the scheduled workflow, nothing else.

**Everything, via a self-hosted runner** — option C above. This is the better
answer for "run it all on my VPS", because it needs no code that does not
exist, and because the gate needs a git working tree that Actions provides for
free and a service would have to clone for itself on every pull request.

If you want the sweep on a timer anyway, the missing piece is a token: on a
runner there is no `create-github-app-token`, and Guardener does not mint one —
by design, so that the private key never enters the process. Either use a
personal access token, or mint an installation token first:

```sh
#!/usr/bin/env bash
# Prints an installation token, valid one hour.
set -euo pipefail
APP_ID=<app id>
INSTALLATION_ID=<installation id>
KEY=/etc/guardener/private-key.pem

b64() { openssl base64 -A | tr '+/' '-_' | tr -d '='; }
now=$(date +%s)
header=$(printf '{"alg":"RS256","typ":"JWT"}' | b64)
payload=$(printf '{"iat":%d,"exp":%d,"iss":"%s"}' "$((now - 60))" "$((now + 540))" "$APP_ID" | b64)
signature=$(printf '%s.%s' "$header" "$payload" \
  | openssl dgst -sha256 -sign "$KEY" -binary | b64)

curl -sS -X POST \
  -H "Authorization: Bearer ${header}.${payload}.${signature}" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/app/installations/${INSTALLATION_ID}/access_tokens" \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["token"])'
```

Then a systemd timer around it:

```ini
# /etc/systemd/system/guardener-hygiene.service
[Unit]
Description=Guardener hygiene sweep

[Service]
Type=oneshot
WorkingDirectory=/opt/guardener
ExecStart=/bin/bash -lc '\
  GUARDENER_TOKEN=$(/opt/guardener/mint-token.sh) \
  /opt/guardener/guardener hygiene'
```

```ini
# /etc/systemd/system/guardener-hygiene.timer
[Unit]
Description=Guardener hygiene sweep, daily

[Timer]
OnCalendar=*-*-* 06:17:00
Persistent=true

[Install]
WantedBy=timers.target
```

`/opt/guardener` needs the binary and the `config/` directory beside it. Keep
the private key readable only by the service user; it is the whole of the bot's
identity.

---

## 8. When it does not work

Every entry here is something that actually happened during setup.

**Every workflow fails at the token step.** The organization secret does not
reach that repository. Check with the `organization-secrets` command in section
5 and add the repository to the secret's access list.

**Only the hygiene job fails, at its first step.** The app has
`organization_administration` where it needs repository `administration`, or the
permission change has not been accepted yet. Section 1 covers both.

**CI fails with `feature edition2024 is required`.** The Rust toolchain is older
than the code needs. `dtolnay/rust-toolchain` keeps one branch per toolchain and
each branch hardcodes its own version, so pinning that action to a commit taken
from the `1.78.0` branch installs 1.78.0 however the comment beside it reads.
Pin to the head of `stable` and pass `toolchain: stable` explicitly.

**A dry run fails with a 401.** `--dry-run` withholds the writes, not the reads
— it has to know what is already there for the preview to mean anything. Give
it a token; a read-only personal one will do.

**The gate blocks on warnings.** That is `mode = "strict"` in
`config/forgeguard.toml` working as intended: on version 2 it blocks at warning
severity and above. It is bearable because the gate reads only the lines a pull
request changed. A repository that needs a softer gate says so with its own
`mode` in the registry — and do not reach for `[policies] warnings_block =
false`, which overrides the mode rather than adjusting it and quietly turns
strict back into default.

**The review says nothing.** That is the expected answer for most pull requests;
it is asked to return an empty array rather than invent a remark. Check the step
log: it prints why it skipped — a bot author, a diff past
`max_changed_lines`, or no endpoint configured.
