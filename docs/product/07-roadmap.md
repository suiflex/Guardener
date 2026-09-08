# Guardener 12-month roadmap

Status: outcome roadmap
Audience: product, engineering, design, security, operations, and leadership

## Roadmap rules

- GitHub and GitLab reach each MVP exit criterion together.
- Deterministic scanning may enforce declared policy; LLM review stays advisory.
- Managed and private execution share contracts and analyzer behavior.
- Guardener Community ships early enough to create adoption evidence, remains
  usable without Cloud, and uses customer-funded infrastructure and LLM access.
- Community, Cloud, Hybrid, and Enterprise Self-Hosted differ by operating and
  governance responsibility, not by LLM merge authority.
- Numeric product and service targets are set only after a measured baseline.
- A phase does not exit because features exist; its named journeys and controls
  must be demonstrated with durable evidence.
- The current workflow-based Guardener remains supported until a repository
  explicitly migrates.

The roadmap adopts the strongest relevant product concepts visible in
[CodeRabbit's official product and documentation](https://docs.coderabbit.ai/):
automatic contextual reviews, summaries, inline conversation, controlled
learnings, queue triage, and change understanding. It does not copy autonomous
fixes, agent actions, or broad lifecycle scope into the first year.

## Phase 1 — Foundation and evidence (months 1–2)

### Outcome

The team can prove the integration, data, review, and safety contracts before
building the production control plane around assumptions.

### Deliverables

- Recruit design partners representing GitHub, GitLab, managed execution, and
  private execution needs.
- Map current Guardener/ForgeGuard output into versioned `ReviewJob`, `Finding`,
  `AnalysisResult`, and `RepositoryPolicy` contracts.
- Prototype GitHub App and GitLab OAuth/webhook installation in isolated test
  organizations/projects.
- Prove provider position mapping for additions, deletions, ranges, renames,
  and stale heads.
- Build a synthetic prompt-evaluation corpus with logic bugs, clean diffs,
  prompt injection, secret leakage, and invalid locations.
- Define source-data inventory, retention classes, tenant boundary, threat
  model, and provider/model subprocessors.
- Instrument webhook receipt, queue/job stages, publication outcome, model
  tokens, finding dispositions, and cleanup.
- Produce website information architecture and test onboarding prototypes with
  target users.
- Freeze the minimum Community topology, BYO LLM compatibility contract,
  no-cost proprietary Community license, and component/dependency license
  inventory for existing open-source parts.
- Prove the full local stack can start and complete a synthetic review while
  every Guardener-operated service endpoint is blocked.

### Evidence and baseline work

- Measure install-to-first-review time separately for GitHub and GitLab.
- Measure diff sizes, deterministic scan duration, prompt size, model latency,
  schema validity, position validity, and estimated cost on representative test
  repositories.
- Record user comprehension of deterministic versus advisory findings.
- Record which permissions/scopes are actually required by exercised endpoints.
- Measure installation failures and operator effort for the Community package;
  no unobserved “easy self-hosting” claim enters marketing.

### Exit criteria

- Both providers complete signed-webhook receipt, code read, safe inline
  publication, deterministic status publication, and revocation tests.
- A stale job cannot publish after a head update in either provider fixture.
- The contract and threat model have product, platform, and security approval.
- Evaluation and operational baselines are recorded with provenance.
- No source body appears in persistent test storage, logs, traces, or backups.
- Community preflight, BYO model compatibility, backup/restore, and uninstall
  drills pass on the one declared topology.

## Phase 2 — Design-partner MVP (months 3–5)

### Outcome

Selected organizations receive one reliable automatic Guardener review for the
current head of every eligible GitHub PR and GitLab MR.

### Deliverables

- Multi-tenant organization, role, integration, repository, policy, job,
  finding, usage, and audit control plane.
- GitHub and GitLab adapters with authenticated webhook ingestion, idempotency,
  debounce, cancellation, current-head checks, and reconciliation.
- Managed ephemeral worker running full ForgeGuard analysis without repository
  scripts.
- Diff-focused LLM review with bounded context, versioned schema, output
  validation, quiet no-finding result, and kill switch.
- Deterministic provider status plus separate advisory summary and safe inline
  comments/discussions.
- Conversational replies limited to Guardener-created finding threads.
- Marketing foundation, provider onboarding, organization overview,
  repositories, change details, findings, integrations, usage, and audit pages.
- Shadow-mode comparison for current workflow-based Guardener repositories.
- Guardener Community public preview with a version-pinned single-node package,
  basic dashboard, provider setup, BYO LLM preflight, upgrades, backups,
  export/import, local diagnostics, and telemetry off by default.
- Dual event ingestion for Community: authenticated webhook where reachable and
  bounded outbound polling/reconciliation where inbound traffic is blocked.

### Exit criteria

- All acceptance scenarios in [MVP requirements](02-mvp-prd.md) pass for both
  providers on managed workers.
- Exactly one current result is visible after duplicate, out-of-order, and rapid
  update events.
- A negative-path test proves an LLM high-severity finding cannot create or
  alter a blocking status.
- Oversized/model-failed reviews retain an honest deterministic result and a
  visible advisory failure/skip state.
- Source cleanup and tenant authorization checks pass under failure injection.
- Design partners can onboard and diagnose a failed review without database or
  operator access.
- A Community operator completes a real GitHub or GitLab review without a Cloud
  account, Guardener network dependency, or credential/source leakage.

## Phase 3 — Private beta and signal quality (months 6–8)

### Outcome

Guardener works in normal team operation, including sensitive-code execution,
policy tuning, and review-volume prioritization.

### Deliverables

- Outbound-only private runner registration, lease, compatibility, health,
  rotation, isolation, and cleanup.
- Managed/private analyzer parity suite and runner operating runbooks.
- Organization defaults, repository overrides, suppressions, path guidance,
  full-scan scheduling, and policy audit history.
- Explicit administrator-approved repository learnings derived from review
  conversations; no invisible auto-memory.
- Finding feedback and offline prompt/model evaluation loop with tenant-safe
  data controls.
- Quality trends separating baseline debt, changed-code findings,
  deterministic origin, and LLM origin.
- Review inbox/triage view using observable signals: current state, age, risk
  evidence, finding severity, reviewer state, and repository policy. It does
  not claim “safe to merge” from model judgment.
- Usage limits, fair scheduling, model/worker cost reporting, and support-safe
  diagnostics.
- Community-to-Cloud migration using versioned policy and normalized metadata
  export/import, with credentials re-entered at the destination.
- Supported-version qualification for the first self-managed GitLab and/or
  GitHub Enterprise Server demand proven by design partners.

### Exit criteria

- Private runners pass the same provider journeys and result contract as
  managed workers.
- Offline, revoked, incompatible, and compromised-runner drills behave as
  documented with no silent cloud fallback.
- Prompt/model candidate releases beat or match the deployed baseline on the
  approved evaluation gates with no security regression.
- Triage ordering can explain every signal used and never substitutes for human
  assignment or merge policy.
- Product, security, and operations approve the beta source-handling and
  incident runbooks.
- Migration preserves supported policy/findings metadata and never exports
  provider or model credentials.

## Phase 4 — General availability (months 9–12)

### Outcome

Guardener is operable, supportable, and commercially ready for the target
10–200 engineer segment without relying on design-partner exceptions.

### Deliverables

- Load-tested tenant isolation, queue fairness, autoscaling, provider rate-limit
  handling, and bounded recovery.
- Published product, API/runner, installation, security, retention, deletion,
  troubleshooting, status, and support documentation.
- Canary release, migration, rollback, backup/restore, incident, and tenant
  offboarding automation.
- Runner/analyzer compatibility policy and signed release provenance.
- Organization administration, member roles, usage metering, plan entitlements,
  and billing readiness; actual pricing follows validated packaging research.
- Support tooling that exposes metadata and audit evidence without default
  access to source content.
- Legacy Guardener migration guide with stay, shadow, migrate, and rollback
  paths per repository.
- Marketing launch only with measured, source-backed claims.
- Stable Community release and documentation, plus clear Cloud, Hybrid, and
  Enterprise Self-Hosted packaging that does not impose vendor-enforced
  repository, seat, or review limits on Community.

### Exit criteria

- Service objectives and alert thresholds are approved from observed beta and
  load-test baselines, with owners and runbooks.
- Restore, provider outage, queue exhaustion, model kill switch, credential
  rotation, and source-exposure drills complete successfully.
- GitHub and GitLab GA matrices have no unresolved critical parity gap.
- Tenant deletion and integration revocation complete under the published
  retention contract.
- Support can resolve the documented top failure modes from dashboard/audit
  evidence.
- Leadership approves measured unit economics and packaging; no public claim
  relies on unverified projections.

## Requirement traceability

| Product requirement | Architecture/experience owner | Delivery phase | Proof |
|---|---|---|---|
| FR-01 organization and roles | Dashboard API and website | 2 | Cross-tenant and role acceptance tests |
| FR-02 GitHub connection | GitHub adapter | 1–2 | Install, event, read, write, revoke journey |
| FR-03 GitLab connection | GitLab adapter | 1–2 | OAuth/webhook, read, discussion/status, revoke journey |
| FR-04 repository activation | Integration service and onboarding | 2 | Permission/webhook/runner preflight tests |
| FR-05–07 normalized automatic jobs | Ingress and orchestrator | 1–2 | Duplicate, ordering, debounce, cancellation, stale-head tests |
| FR-08 deterministic full scan | ForgeGuard and execution plane | 2 | Full scan and no-repository-script negative test |
| FR-09 LLM changed-code review | LLM gateway and worker | 1–2 | Bound, injection, schema, no-finding, timeout evaluations |
| FR-10 provider feedback | Provider publisher | 1–2 | Status, summary, inline mapping, partial-write tests |
| FR-11 finding lifecycle | Result validator and findings store | 2–3 | Fingerprint/disposition/provider-reference tests |
| FR-12 policy | Policy compiler and editor | 2–3 | Inheritance, override, audit, no-LLM-blocking tests |
| FR-13 dashboard | Website and dashboard API | 2–3 | Authorized user journeys and accessibility audit |
| FR-14 feedback | Finding detail and evaluation pipeline | 2–3 | Authorization, audit, evaluation export tests |
| FR-15 execution routing | Orchestrator and runner | 2–3 | Managed/private parity and no-fallback tests |
| FR-16 audit | Control plane | 2 | Actor/action/target/outcome coverage check |
| FR-17 uninstall | Integration and data lifecycle | 2–4 | Revoke, cancel, write-stop, deletion tests |
| FR-18 retry | Change detail and orchestrator | 2 | Current-head idempotent retry tests |
| FR-19 finding conversation | Provider adapters and LLM gateway | 2 | Authorized/unauthorized reply, bound, and stale-head tests |
| FR-20 change summary | LLM gateway and change detail | 2 | Clean, risky, large, and model-failed summary fixtures |
| FR-21 Community deployment | Community bundle and local application | 1–2 | Offline-from-Guardener install, first-review, upgrade, backup/restore, uninstall tests |
| FR-22 BYO LLM | LLM gateway and Community preflight | 1–2 | Compatible/incompatible endpoint, schema, timeout, budget, and no-fallback tests |
| FR-23 portable configuration | Export/import boundary | 2–3 | Round-trip compatibility plus credential/source absence inspection |

## Deliberately deferred

The roadmap stops before autonomous fixes, post-merge actions, IDE extensions,
issue-planning agents, Slack/Discord agents, cross-repository semantic graphs,
or multi-node/HA Community orchestration. The initial free deployment stays one
supported topology; advanced HA, air-gap lifecycle, identity, audit, and support
belong to Enterprise Self-Hosted. Broader agents remain later discovery areas
until the smaller review loop proves trustworthy.

## Ownership model

Each phase has one product outcome owner and named engineering owners for
provider integration, control plane, execution/runner, AI quality, website, and
operations. Security is an approver for provider permissions, source handling,
runner isolation, model/prompt releases, and launch. Design-partner feedback is
evidence, not acceptance authority for security controls.
