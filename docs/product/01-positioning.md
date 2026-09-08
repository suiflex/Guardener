# Guardener positioning

Status: product and go-to-market hypothesis
Audience: product, founders, sales, marketing, and customer-facing engineering

## Category

Guardener is a pull-request and merge-request code-quality platform. It brings
together deterministic repository analysis, changed-code governance, and an
advisory LLM reviewer in one workflow.

It should not be sold as “an AI that replaces code review.” Human ownership is
part of the product. The model is a second opinion; the deterministic engine is
the only Guardener component eligible to enforce merge policy.

## Positioning statement

For engineering teams managing many repositories without a dedicated code-
quality platform team, Guardener provides one GitHub and GitLab workflow for
repository-wide deterministic analysis and focused review of every proposed
change. Unlike a traditional full-repository dashboard alone or a generic AI
review bot, Guardener keeps legacy debt out of the developer's current change,
separates enforceable facts from model judgment, and supports ephemeral managed
analysis, a private runner, or a free self-deployed Community instance using the
customer's own LLM.

## Ideal customer profile

Guardener is initially suited to organizations that:

- have 10–200 engineers and more repositories than one maintainer can audit;
- use GitHub, GitLab, or both;
- already rely on PR or MR review before merging;
- want consistent engineering standards but cannot standardize every language
  pipeline immediately;
- need model-assisted review but will not allow a model to approve or reject a
  merge; and
- may prefer SaaS operations, but also include teams whose repositories, model
  endpoints, or dependencies are reachable only through a private network.

The first buyer is likely an engineering or platform lead. Developers are the
daily users, and security is an approval stakeholder because the integration
reads private source code.

## Competitive frame

This is a product-positioning comparison, not an exhaustive feature inventory.

| Alternative | What it does well | Gap Guardener addresses |
|---|---|---|
| Language-native linters and scanners | Precise, fast, ecosystem-specific rules | Fragmented policy, reporting, and repository coverage |
| Traditional code-quality platform | Repository-wide debt, dashboards, established gates | Changed-code experience can be separated from higher-order review context |
| Generic AI review bot | Natural-language reasoning about a diff | Probabilistic findings can be noisy, duplicate scanners, or blur merge authority |
| CI scripts maintained per repository | Complete local control | Repeated setup, drift, and limited organization-wide visibility |
| Human review alone | Context, accountability, product judgment | Reviewer availability and inconsistent coverage of recurring engineering risks |

Current official packaging shows a useful market gap. CodeRabbit offers free
use for open-source repositories while listing self-hosting as an Enterprise
capability ([CodeRabbit plans](https://docs.coderabbit.ai/management/plans)).
SonarQube distributes a free, open-source, self-managed Community Build
([SonarQube downloads](https://www.sonarsource.com/products/sonarqube/downloads/)).
Guardener's hypothesis combines those adoption mechanics: a useful free
self-deployed review product, including private repositories, where the user
pays their own infrastructure and LLM bill.

Guardener wins only if it makes these layers cooperate. Adding another comment
stream without reducing duplication is not differentiation.

## CodeRabbit concept, Guardener focus

CodeRabbit is the closest product-shape reference. Its official
[product page](https://www.coderabbit.ai/) and
[documentation](https://docs.coderabbit.ai/) describe automatic contextual code
review, summaries, inline suggestions and conversations, continuous learnings,
PR triage, change understanding, security scans, and integrations beyond the
source-control review surface.

Guardener does not need to reproduce that entire portfolio. The first-year
combination is:

| CodeRabbit-shaped concept | Guardener decision |
|---|---|
| Automatic contextual PR review | Include for every eligible GitHub PR and GitLab MR head |
| Summary and inline conversation | Include, with strict position validation and bounded replies |
| Continuous learnings | Include only as administrator-approved, versioned repository guidance |
| PR triage | Include in private beta using explainable workflow and finding signals |
| Codebase health | Include through deterministic ForgeGuard full scans and trend history |
| Committable/autonomous fixes | Defer; suggestions are text and never automatically applied |
| Pre/post-merge agent actions | Defer; Guardener is not a general workflow agent |
| Broad chat/IDE/Slack surfaces | Defer until the provider review loop proves demand |

This focus produces a clearer promise than “CodeRabbit clone”: CodeRabbit-like
review ergonomics combined with a deterministic engineering-standard engine
whose authority and evidence remain separate from the model.

## Product pillars

### 1. Two kinds of evidence, clearly separated

Every finding says whether it came from a deterministic rule or an LLM prompt,
which version produced it, and whether it can affect policy status. Severity
does not silently turn a model opinion into a gate.

### 2. Full health, changed-code action

A full deterministic scan establishes a baseline and dashboard trend. PR/MR
feedback highlights new, changed, or resurfaced findings so existing debt does
not make incremental adoption impossible.

### 3. Native provider workflow

GitHub and GitLab users receive a summary, inline discussions when a position
can be proven, and a link to the same normalized result in Guardener. Provider
parity is defined by user outcome rather than identical visual components.

### 4. One contract across deployment choices

Community, Cloud workers, and customer-hosted runners execute the same analyzer
and result contract. The operating boundary changes, but provider behavior,
finding meaning, and the deterministic/advisory authority split do not.

### 5. Governance that explains itself

Repository policy, suppressions, model/prompt version, integration writes, and
administrative changes are auditable. Guardener reports drift; it does not
quietly revise protected repository decisions.

## Messaging hierarchy

### Primary message

**One code-quality review for every PR and MR—deterministic where it must be,
advisory where an LLM helps.**

### Supporting messages

- See new risk without making developers clean up the whole repository first.
- Self-host free with your own LLM, or let Guardener operate it for you.
- Keep source ephemeral or keep the entire review path inside your own network.
- Use one policy and dashboard across GitHub and GitLab.
- Trace every finding to a rule, prompt, commit, and disposition.

### Proof required before public use

The following claims must not appear on the marketing website until measured:

- percentage reduction in review time;
- false-positive or finding-acceptance rates;
- supported language count;
- service availability or review-latency guarantees;
- compliance, residency, or “zero access” claims; and
- cost savings relative to another product.

Until then, demonstrations should use observed workflows and clearly labelled
design-partner results.

## Packaging direction

Prices remain undecided, but product boundaries are explicit:

| Offer | Customer gets | Customer operates | Commercial role |
|---|---|---|---|
| Community | Free-to-use proprietary package with core GitHub/GitLab review, ForgeGuard full scans, advisory summaries and inline findings, basic policy/history/dashboard, and BYO compatible LLM | Entire single-organization stack, upgrades, backup, availability, provider credentials, and model cost | Free adoption and trust-building |
| Cloud | Community workflow plus hosted integrations, managed workers/model routes, automatic upgrades/backups, team administration, and service operations | Repository selection and policy | Subscription based on measured active usage/cost drivers |
| Hybrid | Cloud control/governance with outbound-only private execution and a customer-approved model route | Runner capacity, private-network access, and optional model endpoint | Premium privacy/network deployment |
| Enterprise Self-Hosted | Hardened full deployment plus SSO/SCIM, advanced RBAC/audit export, HA/air-gap options, signed upgrade channel, and support | Chosen infrastructure under supported topology | Annual license and support |

Community has no vendor-enforced repository, seat, or review-count limit. It is
not free compute: operators pay infrastructure and model usage. Cloud packaging
should follow measured active repositories, analysis compute, and model usage;
it should not charge by comment count, which would reward noise.

Community is a signed proprietary image/binary under a no-cost Community
license, not an open-source platform release. Existing open-source components
retain their licenses and required notices. Cloud convenience and enterprise
operations remain valuable without removing the core review loop from the free
self-hosted package.

## Adoption motion

1. Choose **Self-host free** or **Start Cloud** without a sales call.
2. Connect selected repositories and a compatible LLM route; Community keeps
   both credentials inside the customer's deployment.
3. Run a non-blocking baseline scan and show repository health privately.
4. Enable automatic advisory review on PRs/MRs.
5. Tune exclusions and suppressions using observed noise.
6. Optionally make deterministic changed-code policy required.
7. Move to Cloud for managed operation, Hybrid for private execution with
   hosted governance, or Enterprise Self-Hosted for regulated-scale controls.

Migration preserves repository policy and normalized metadata through an
explicit export/import contract. Cloud signup is never required to keep a
Community installation running, and Community telemetry is off by default.

The default rollout is non-blocking. A deterministic gate becomes required only
through an explicit repository or organization policy change. LLM review never
crosses that boundary.

## Demonstration narrative

A credible product demo should show one change moving through both forms of
analysis:

1. A new commit triggers exactly one current job after debounce.
2. A deterministic finding appears with a rule ID and policy consequence.
3. A separate logic-risk observation appears as an advisory inline comment.
4. A rapid follow-up commit makes the old job stale and updates, rather than
   duplicates, the current feedback.
5. The dashboard shows the repository baseline, changed-code result, finding
   origins, and audit record.
6. The worker workspace is destroyed and no source body appears in stored job
   data.

## Related decisions

- [Product direction](00-product-direction.md)
- [MVP requirements](02-mvp-prd.md)
- [Website experience](03-website-experience.md)
- [Roadmap](07-roadmap.md)
