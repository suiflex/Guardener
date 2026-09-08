# Guardener website experience

Status: product and UX specification
Audience: product design, frontend, product engineering, and customer success

## Experience goal

The website has two jobs:

1. Explain why deterministic analysis and advisory LLM review belong together.
2. Let an engineering organization connect providers, understand current code
   quality, and diagnose review delivery without using the CLI for routine
   operation.
3. Give users an obvious choice between free self-deployment with their own LLM
   and a Guardener-operated service, without forcing contact with sales.

The marketing site and application share one visual system but have different
information density. Marketing earns trust; the dashboard makes state and
evidence inspectable.

## Information architecture

### Public website

| Route | Purpose | Primary action |
|---|---|---|
| `/` | Product promise, workflow, trust model, and GitHub/GitLab coverage | Self-host free / Start Cloud |
| `/product` | Deterministic scan, advisory review, dashboard, and deployment choices | View workflow |
| `/security` | Data flow, ephemeral processing, permissions, retention, and incident contact | Review security model |
| `/docs` | Community installation, BYO LLM, concepts, policies, provider behavior, runner, migration, and troubleshooting | Install Community |
| `/self-host` | Community license, requirements, deployment topology, model compatibility, upgrades, backups, telemetry, and Cloud migration | Get Community image |
| `/pricing` | Community, Cloud, Hybrid, and Enterprise boundaries; numeric prices appear only after validation | Compare options |
| `/status` | Service and incident state | Subscribe to updates |

Public claims must follow [positioning](01-positioning.md): no unsupported
accuracy, time-saving, availability, language-count, or compliance numbers.
The site says “free proprietary self-hosting,” never “open source,” and links to
the no-cost Community license before download. Existing open-source component
notices remain separately visible.

### Authenticated application

| Route | Core content |
|---|---|
| `/app` | Organization overview: activation, current jobs, new findings, quality movement, usage, and integration warnings |
| `/app/reviews` | Cross-repository PR/MR inbox with explainable priority, ownership, risk, and activity signals |
| `/app/repositories` | Provider, activation, default branch, policy, runner route, last scan, and health |
| `/app/repositories/:id` | Baseline, changed-code trend, open findings, PR/MR history, policy, and setup diagnostics |
| `/app/changes/:id` | One PR/MR head, job timeline, deterministic status, advisory summary, findings, publication state, and retry |
| `/app/findings` | Filterable normalized finding list with origin and disposition |
| `/app/policies` | Organization defaults and explicit repository overrides |
| `/app/learnings` | Proposed, approved, rejected, and retired repository guidance with source thread and audit history |
| `/app/integrations` | GitHub/GitLab installations, repositories, permissions, webhook health, and revoke flow |
| `/app/runners` | Runner groups, version, compatibility, capacity, last contact, and registration/revocation |
| `/app/usage` | Analysis jobs, worker time, model tokens, skips, and limits without source content |
| `/app/audit` | Actor, action, target, provider, timestamp, request identity, and outcome |
| `/app/settings` | Organization identity, members, retention, deletion, and notifications |

## Navigation model

The organization selector and repository search remain available throughout the
application. Primary navigation is Overview, Reviews, Repositories, Findings,
Policies, Integrations, Runners, Usage, and Audit. Learnings is part of Policies;
Settings is administrative and visually separate.

Deep links from GitHub and GitLab must land on the exact change and head SHA,
not a generic repository dashboard. A stale head remains viewable as history
and is labelled stale.

## Onboarding

### Shared path

1. **Choose deployment.** Select free Community, managed Cloud, or Cloud with a
   private runner. Explain who operates each component and where source/model
   traffic travels.
2. **Sign in.** Authenticate through GitHub or GitLab; do not infer organization
   access from email domain.
3. **Create or join an organization.** Show the Guardener role granted to the
   current user.
4. **Choose provider.** GitHub and GitLab are equally visible.
5. **Grant access.** Explain each requested permission immediately before the
   provider handoff.
6. **Select repositories.** Default to explicit selection, with select-all as a
   deliberate administrator action.
7. **Choose execution.** Managed ephemeral workers are the Cloud default; private
   runner setup is available without blocking cloud onboarding.
8. **Connect the model.** Community requires a compatible customer model route;
   Cloud offers only routes the tenant has approved. Show endpoint reachability,
   schema compatibility, and budget status without exposing the secret.
9. **Verify connection.** Test credentials, webhook receipt, repository read,
   provider write, and runner availability.
10. **Run baseline.** Explain that it is initially non-blocking and does not
   publish historical debt as PR/MR comments.
11. **Review first result.** Link to the exact provider change and show where
   deterministic policy differs from advisory model feedback.

Community onboarding happens on the local instance and must not redirect to a
Guardener account. Its completion screen may offer documentation, community
support, or an optional Cloud migration; declining leaves the installation
fully functional.

### GitHub-specific handoff

Use a dedicated future Guardener Cloud GitHub App with webhooks enabled. Do not
silently repurpose today's webhook-disabled `guardener-bot`, whose permission
model includes organization hygiene and add-only fixes. GitHub App permissions
determine both API access and available webhook subscriptions, and installation
owners must approve later permission increases; the UI must surface that state.

Official reference: [Choosing permissions for a GitHub App](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/choosing-permissions-for-a-github-app).

### GitLab-specific handoff

OAuth supplies user sign-in and delegated API access. Repository analysis also
requires project/group selection, webhook installation, and a credential able
to read repository content and create MR discussions/status. The setup screen
must show token expiry and refresh failures rather than converting them into a
generic analysis error.

Official reference: [GitLab as an OAuth 2.0 identity provider](https://docs.gitlab.com/integration/oauth_provider/).

## Core page requirements

### Organization overview

Lead with exceptions that need action:

- disconnected or permission-changed integrations;
- repositories with no successful baseline;
- growing queue age or unavailable private runner;
- current-head analysis failures; and
- policy changes awaiting explicit activation.

Trend charts are secondary. Every aggregate links to the repositories and
findings that compose it.

### Repository page

Show four clearly separated panels:

1. **Current health:** last completed baseline, open deterministic findings,
   and baseline movement.
2. **Changed-code activity:** recent PR/MR results and new/fixed findings.
3. **Policy:** inherited defaults and explicit overrides, with provenance.
4. **Delivery health:** provider permissions, last webhook, runner route, and
   last publication error.

Never combine deterministic and LLM counts without an origin filter or visual
distinction.

### Change detail

The canonical layout is:

- provider, repository, change number, branch, author, and exact head SHA;
- job stage timeline with retries, cancellation, and stale state;
- deterministic result and policy effect;
- advisory LLM result with model/prompt version and cost metadata;
- findings grouped by file, each with origin, severity, evidence, position,
  provider publication state, and disposition; and
- audit trail for retry, suppression, feedback, and publication.

The advisory area starts with a compact purpose, change walkthrough, and risk
summary before individual findings. This follows the successful review shape
documented by [CodeRabbit](https://docs.coderabbit.ai/) while preserving
Guardener's explicit deterministic/advisory split.

If a finding could not be posted inline, show the reason. Do not call it posted
until the provider returns success.

### Review inbox and triage

The beta review inbox helps teams manage PR/MR volume across providers. It may
rank current work using observable fields such as age, review request, activity,
deterministic status, open finding severity, change size, and repository policy.
Every priority label exposes the signals behind it.

LLM output may add advisory risk context but cannot label a change “safe to
merge,” auto-assign a reviewer, or override provider approvals. Saved views and
filters are preferred over opaque personalization.

### Learnings

A finding conversation may propose a repository convention. The learnings page
shows the exact proposed instruction, source thread, scope, proposer, and impact
preview. An administrator must approve it before it enters review context.
Editing creates a new version; retirement stops future use without erasing the
audit history. There is no hidden auto-memory.

### Policy editor

The editor exposes only product decisions already supported by the backend:

- automatic review on/off;
- managed or private runner route;
- path exclusions;
- deterministic mode and rule exceptions;
- full-scan schedule;
- changed-line/model budget limits; and
- notification preferences.

Community additionally exposes local deployment version, backup status, and
BYO model route health. It never displays the model secret after save. Cloud
conversion prompts appear only on operational pages where managed service
solves the visible problem—for example repeated upgrade or availability work—
not inside review results.

There is no LLM blocking control. Organization defaults and repository
overrides are shown side by side, and saving produces a before/after review plus
an audit event.

## State language

| System state | User-facing treatment |
|---|---|
| Debouncing | “Waiting briefly for more commits” |
| Queued | Position and queue-age context, without a false completion estimate |
| Running | Current stage and runner group |
| Stale | “A newer commit replaced this analysis; results were not published” |
| Partial publication | Identify exactly which provider writes succeeded or failed |
| Skipped | State the bound or policy that caused the skip |
| Model unavailable | Deterministic result remains visible; advisory review says unavailable |
| Permission revoked | Name the missing capability and link to reconnect |

Empty states teach the next action. Error states retain a support-safe request
ID but never display tokens, raw prompts, source excerpts beyond the finding's
authorized context, or provider response bodies containing secrets.

## Roles and authorization

| Action | Member | Administrator |
|---|---:|---:|
| View authorized repositories/results | Yes | Yes |
| Rate an LLM finding | Yes | Yes |
| Retry current-head analysis | Yes | Yes |
| Change organization/repository policy | No | Yes |
| Connect or revoke provider | No | Yes |
| Register or revoke runner | No | Yes |
| Manage members and retention | No | Yes |

Provider access is also enforced. A Guardener role never grants visibility to a
repository the current user is not authorized to view unless the organization
has explicitly adopted an organization-wide visibility policy that has passed
security review; that policy is outside MVP.

## Accessibility and responsive behavior

- Meet WCAG 2.2 AA for contrast, keyboard operation, focus order, labels, error
  association, and status announcements.
- Do not encode provider, finding origin, severity, or job state by color alone.
- Tables have accessible names, sortable-header state, and a usable narrow-screen
  alternative.
- Charts expose the same values as text or a data table.
- Long file paths and commit identifiers remain selectable and do not obscure
  primary actions.
- Destructive actions name their target and require confirmation.

## Product analytics

Cloud collects events for onboarding completion, first repository activation,
first completed review, finding disposition, retry, policy change, runner
connection, and integration revocation. Events contain stable internal IDs and
categories, not source bodies, diff text, prompt text, branch names, or comment
bodies.

Guardener Community telemetry is off by default. An operator may explicitly
enable anonymous release, deployment-shape, job-outcome, and error-code events;
the preview names every field, and disabling it stops delivery immediately.
Adoption must also be measurable through privacy-preserving signals that do not
depend on telemetry, such as release downloads, documentation journeys, support
requests, and optional instance-generated diagnostics.

The dashboard metrics and event taxonomy align with
[product direction](00-product-direction.md) and the launch gates in the
[roadmap](07-roadmap.md).
