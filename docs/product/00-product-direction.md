# Guardener product direction

Status: product direction for validation
Audience: product, engineering, security, operations, and design
Horizon: 12 months

## Decision

Guardener will evolve from the suiflex GitHub Actions driver into a code-quality
product for engineering teams of 10–200 people. The product combines two
different kinds of judgment without pretending they have equal authority:

- ForgeGuard performs deterministic repository analysis and may enforce a
  repository's declared merge policy.
- The Guardener reviewer uses an LLM to inspect a PR or MR diff with bounded
  supporting context. Its output is always advisory.

GitHub and GitLab are both MVP platforms. Guardener is distributed through two
entry points that share the same review contracts:

- **Guardener Community** is free to self-deploy, runs the control and execution
  planes inside the user's network, and uses a customer-supplied compatible LLM
  endpoint and credentials.
- **Guardener Cloud** provides the managed web experience, orchestration,
  policy, history, audit, model routing, and ephemeral workers. A customer may
  instead attach an outbound-only private runner.

Community is the adoption hook, not a time-limited trial. A team can prove the
complete deterministic-plus-LLM review loop without sending source or model
credentials to Guardener or paying Guardener. Paid products sell operation,
governance, scale, and support rather than restoring an intentionally crippled
core workflow.

This document is the index for the product package:

- [Positioning](01-positioning.md)
- [MVP product requirements](02-mvp-prd.md)
- [Website experience](03-website-experience.md)
- [Platform architecture](04-platform-architecture.md)
- [AI review, security, and governance](05-ai-review-security-governance.md)
- [Deployment, CI/CD, and operations](06-deployment-cicd-operations.md)
- [12-month roadmap](07-roadmap.md)

## Starting point

Guardener today is a Rust CLI, not a service. A command reads the world, makes
one decision, writes once, and exits. Its current product behavior is:

| Capability | Current behavior |
|---|---|
| Source control | GitHub only, through `src/github.rs` |
| Deterministic gate | ForgeGuard over changed lines, reported as a check and one edited comment |
| LLM review | Pull-request diff from the API, rendered into one ordinary comment |
| Review trigger | Authorized `/review` comment or bounded weekly sweep |
| Inline LLM comments | Not built; the model's path and line are text only |
| Organization governance | Registry-driven hygiene report; `--fix` only adds missing files |
| Execution | GitHub Actions invokes the CLI; no webhook listener or database |

The future platform is a new service boundary. This product direction does not
change the behavior above, activate webhooks on the existing app, or move
automatic model review into the existing gate workflow. Those changes require
separate implementation and migration decisions.

## User problem

Engineering teams typically assemble code quality from separate linters,
security tools, coverage systems, repository settings, and review bots. The
result has three recurring failures:

1. Developers receive duplicated or contradictory feedback across several
   interfaces.
2. Existing repository debt makes global quality gates too noisy, while
   changed-code problems still reach human review.
3. LLM reviewers can find logic risks that static rules miss, but their
   probabilistic output is unsafe as an unexplained merge authority.

Guardener's job is to produce one prioritized view of changed-code risk and
repository health while keeping deterministic enforcement separate from model
opinion.

## Target users

The first market is product engineering organizations with 10–200 engineers,
multiple repositories, a working pull-request or merge-request practice, and no
dedicated team operating an enterprise code-quality platform.

Primary users:

- Developers want useful feedback on the code they just changed, in the PR or
  MR where they are already working.
- Tech leads want consistent policy without forcing every repository into one
  language-specific pipeline.
- Engineering managers want trends, adoption, and unresolved risk across
  repositories.
- Security and platform owners want least-privilege integration, data handling
  guarantees, and an auditable explanation of every automated write.

## Product promise

> Guardener finds deterministic code-quality violations across the repository,
> reviews each proposed change for higher-order logic risk, and returns the
> result where engineers work—without giving a language model control of the
> merge button.

## Concept reference: CodeRabbit plus ForgeGuard discipline

[CodeRabbit's official product](https://www.coderabbit.ai/) demonstrates a
useful interaction model: automatically review each PR, provide focused inline
feedback, let authors discuss findings with the bot, learn team conventions,
prioritize the review queue, and expose repository health. Guardener adopts
that product shape where it supports the promise above.

Guardener's distinctive boundary is ForgeGuard. Repository-wide deterministic
analysis remains a first-class product surface and the only Guardener result
eligible to enforce merge policy. Model findings remain visibly advisory,
learnings require administrator approval, and source handling supports private
outbound-only execution. Automatic fixes, post-merge actions, and broad agents
are deferred until the smaller review loop is proven safe and useful.

## Product family

| Product surface | Responsibility |
|---|---|
| Guardener Community | Free-to-use proprietary single-organization self-deployment with GitHub/GitLab integration, local control plane, ForgeGuard analysis, advisory review, inline comments, and a basic dashboard; the operator supplies infrastructure and an LLM |
| Guardener Cloud | Managed multi-tenant control plane, provider integrations, upgrades, backups, policy, history, usage, audit, workers, and approved model routing |
| Guardener Hybrid | Guardener Cloud control plane with an outbound-only customer runner and customer-approved model route for private-network repositories |
| Guardener Enterprise Self-Hosted | Paid hardened self-hosting for SSO/SCIM, advanced RBAC and audit, high availability, air-gapped operation, signed upgrades, and support |
| Guardener Runner | Ephemeral analyzer that checks out code, runs approved analysis, submits results, and discards the workspace |
| Guardener CLI | Local and CI entry point; remains a one-shot process rather than becoming the control plane |
| ForgeGuard | Deterministic analysis and policy engine used by the CLI and runner |

The minimum product reuses the CLI and ForgeGuard engine. It does not rewrite
deterministic rules in the web service or create separate GitHub and GitLab
analysis engines. Community and paid deployments consume the same public job,
finding, policy, and result contracts so adoption does not create a migration
dead end.

## Principles

1. **Changed code is the interaction boundary.** The developer sees findings
   that are relevant to the current PR or MR. Full scans establish repository
   health and baseline debt; they do not dump historical debt into every
   change.
2. **Deterministic facts and model opinions stay distinct.** Origin, rule or
   prompt version, evidence, severity, and enforcement eligibility are visible
   on every finding.
3. **The LLM never blocks merge.** A repository may make deterministic policy
   status required; model output is a comment and dashboard signal only.
4. **No source retention by default.** Managed workers delete checkouts,
   patches, and code-bearing prompts after the job. Persistent data contains
   findings, locations, hashes, metadata, usage, and audit events—not source
   bodies.
5. **Untrusted code is data.** Managed workers do not execute repository scripts
   or repository-supplied quality commands. Private runners may opt in only
   through an explicit trusted-repository policy.
6. **One event, one current result.** Delivery identity, head SHA, cancellation,
   and finding fingerprints prevent stale or duplicated feedback.
7. **Installation is reversible.** Uninstalling the provider integration or
   revoking a private runner stops access without requiring source changes.
8. **Free means useful.** Community includes the complete core review loop and
   has no vendor-enforced repository, seat, or review count limit. Its natural
   costs are customer infrastructure and model usage.
9. **Deployment changes operations, not trust.** Self-deployment and BYO LLM do
   not weaken output validation, current-head checks, least privilege, audit,
   or the rule that model output cannot block a merge.

## Outcomes and measures

Product progress is measured through outcomes, not the volume of generated
comments:

| Outcome | Measure | Required interpretation |
|---|---|---|
| Fast activation | Time from integration install to first completed review | Split by provider and worker type |
| Useful findings | Findings accepted, fixed, or positively rated | Never use model self-confidence as evidence |
| Low noise | Findings dismissed as invalid or repeatedly suppressed | Split deterministic and LLM origins |
| Reliable feedback | Eligible head SHAs with one terminal analysis result | Exclude user-cancelled jobs |
| Controlled cost | Model tokens and compute per reviewed change | Track by tenant, repository, and model version |
| Safe handling | Source-retention and unauthorized-write incidents | Target is zero; every event is investigated |
| Community activation | Self-deployed instances completing a first real PR/MR review | Do not require telemetry; support an operator-generated anonymous diagnostic report |
| Sustainable conversion | Community organizations choosing Cloud, Hybrid, support, or enterprise governance | Attribute the reason; do not manufacture conversion through core-feature removal |

Baselines do not exist in this repository. The first roadmap phase instruments
them before numeric service or product targets are committed. Any numbers in a
future launch review are targets, not claims about current performance.

## Non-goals for the first 12 months

- Replacing GitHub Actions, GitLab CI, or general-purpose CI orchestration.
- Automatically modifying application code or applying model suggestions.
- Giving model findings merge-blocking authority.
- Hosting a general chat assistant over a customer's entire source tree.
- Replacing language-native compilers, linters, dependency scanners, or test
  frameworks when their native result can be ingested.
- Promising certification, data residency, or regulated-industry compliance
  before the controls have been independently assessed.
- Quietly rewriting repository workflows, labels, branch protection, or policy.
- Providing free hosted model inference without a measured abuse and unit-
  economics policy; Community users bring their own model capacity.
- High-availability or air-gapped Community orchestration in the first release;
  the free path starts with one supported production topology.

## Product decisions still validated by delivery

The direction is fixed; implementation assumptions must still be proven in the
roadmap:

- GitHub and GitLab must reach behavioral parity, not identical APIs.
- Full deterministic scans must remain affordable without executing untrusted
  repository instructions.
- Diff-focused model context must produce useful findings without repository-
  wide prompt ingestion.
- The private runner protocol must work outbound-only and reveal no long-lived
  provider credential to the control plane when customer policy forbids it.
- The Community package must install, upgrade, back up, and uninstall without a
  Guardener Cloud account, and its LLM adapter must pass the same schema and
  safety evaluations across declared compatible endpoints.
- Community is distributed as a signed proprietary image/binary under a no-cost
  Community license; its platform source is not published. Existing MIT-
  licensed components keep their licenses, and release requires a component-
  by-component license and dependency inventory.
