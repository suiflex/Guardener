# Guardener MVP product requirements

Status: implementation-driving product requirements
Audience: product, engineering, design, QA, security, and operations

## Objective

Deliver a single product in which a 10–200 person engineering organization can
connect GitHub and GitLab repositories, receive deterministic full-repository
analysis and automatic advisory LLM review on each PR/MR head, inspect results
in a web dashboard, and choose free self-deployment with a customer-supplied
LLM, managed Cloud, or Cloud with private execution.

“MVP” means both providers complete the same acceptance journeys. It does not
mean every provider API or every code-analysis category is identical.

## Success condition

The MVP is ready for design partners when:

- one GitHub organization and one GitLab group can complete onboarding without
  Guardener engineers changing customer source repositories by hand;
- opening or updating an eligible PR/MR produces one deterministic result and
  one advisory review result for the current head;
- valid findings can be placed inline, while uncertain positions fall back to
  the summary rather than the wrong line;
- cloud and private runners satisfy the same job/result contract;
- a Guardener Community deployment can complete the same core review journey
  without a Guardener Cloud account and without sending its model credential to
  Guardener;
- the dashboard explains job state, policy, finding origin, and failure; and
- tests demonstrate that LLM output cannot set a blocking provider status.

No current baseline supports numeric adoption, latency, or accuracy claims.
Those measures are instrumented during the MVP and governed by the
[roadmap](07-roadmap.md).

## Personas and jobs

| Persona | Job to be done |
|---|---|
| Developer | When I update a PR/MR, show new actionable risk on the changed lines without repeating known debt. |
| Tech lead | Apply a consistent standard while preserving explicit repository exceptions. |
| Platform engineer | Connect repositories once, observe failures, and roll back integration safely. |
| Engineering manager | See adoption, current risk, and trends across repositories and providers. |
| Security reviewer | Verify least privilege, source handling, audit events, and trust-boundary controls. |
| Independent maintainer | Run useful automatic review without paying Guardener or exposing private code to Guardener infrastructure. |

## Core journeys

### J1 — Connect and review

1. An organization administrator signs in and creates or selects a Guardener
   organization.
2. They install the GitHub App or authorize the GitLab integration and select
   repositories.
3. Guardener validates permissions and webhook delivery before declaring the
   repository active.
4. Guardener runs a non-blocking deterministic baseline scan.
5. A new PR/MR head triggers a debounced review job.
6. The provider receives deterministic status/annotations and a separate
   advisory review summary with safe inline comments.
7. The dashboard links the result to the exact head SHA and policy version.

### J2 — Update rapidly

1. Several commits arrive during the debounce window.
2. Guardener retains one queued job for the newest head.
3. An older running job is cancelled when safe or allowed to finish without
   publishing.
4. Only the newest head can update current provider feedback.
5. Matching findings update existing Guardener-authored comments or threads;
   resolved findings become visibly resolved or disappear according to provider
   capability.

### J3 — Use a private runner

1. An administrator creates a runner registration and receives a short-lived,
   one-time registration credential.
2. The runner establishes an outbound authenticated connection.
3. Eligible jobs are leased, executed in an isolated temporary workspace, and
   returned through the shared result contract.
4. The workspace and code-bearing logs are destroyed after submission.
5. The dashboard shows runner health without exposing source or credentials.

### J4 — Self-deploy Community with BYO LLM

1. An operator installs the supported Guardener Community package inside a
   network that can reach its GitHub or GitLab host and chosen model endpoint.
2. They configure provider credentials and an OpenAI-compatible model endpoint,
   model identifier, and secret through environment or secret-store input—not a
   repository file.
3. A connection preflight verifies provider read/write capability and model
   schema compatibility without retaining the supplied secret.
4. An eligible PR/MR completes deterministic analysis, advisory summary, and
   safe inline publication through the local stack.
5. The operator can export policy and normalized metadata or uninstall without
   creating a Guardener Cloud account.

## Functional requirements

| ID | Requirement | MVP acceptance |
|---|---|---|
| FR-01 | Organization and membership | An administrator can create an organization, invite members, and assign administrator or member roles. |
| FR-02 | GitHub connection | A GitHub App installation can select repositories, receive PR events, read code, write review comments, and write deterministic checks with least-required permissions. |
| FR-03 | GitLab connection | A GitLab OAuth/integration flow can select projects, receive MR events, read code, write discussions, and publish deterministic commit status. Setup discloses that GitLab's `api` scope is broader than repository read and is required for the MVP's write API operations. |
| FR-04 | Repository activation | Guardener verifies credentials, webhook health, default branch, and runner route before marking a repository active. |
| FR-05 | Event normalization | GitHub and GitLab events become one `ReviewJob` keyed by tenant, provider, repository, change number, and head SHA. |
| FR-06 | Automatic triggers | PR/MR open, reopen, and source-head update trigger analysis; metadata-only updates do not unless policy-relevant. |
| FR-07 | Debounce and cancellation | Bursty commits collapse into one newest-head job; stale work cannot publish current feedback. |
| FR-08 | Deterministic full scan | ForgeGuard scans the repository without repository-supplied commands on managed workers and records baseline plus changed-code findings. |
| FR-09 | LLM changed-code review | The reviewer receives the diff, PR/MR metadata, applicable policy, and bounded relevant context—not an unrestricted repository dump. |
| FR-10 | Provider feedback | Deterministic results use provider status/annotations; LLM findings use a separate advisory summary and inline comments only when position validation succeeds. |
| FR-11 | Finding lifecycle | Stable fingerprints support open, fixed, accepted, suppressed, invalid, and stale dispositions without duplicating feedback. |
| FR-12 | Policy | Organization defaults can be overridden explicitly per repository; LLM blocking is not an available setting. |
| FR-13 | Dashboard | Users can inspect repositories, PR/MR jobs, findings, quality history, policy, usage, integrations, runners, and audit events. |
| FR-14 | Feedback | Authorized users can mark an LLM finding useful or invalid and supply an optional reason without changing merge status. |
| FR-15 | Execution routing | Each repository uses managed ephemeral workers or a named private-runner group. |
| FR-16 | Audit | Installation, policy, suppression, credential, runner, model/prompt, and provider-write events are attributable and timestamped. |
| FR-17 | Uninstall | Revocation stops new reads/writes, invalidates credentials, rejects queued jobs, and begins metadata deletion according to policy. |
| FR-18 | Manual retry | Authorized users can retry a failed current-head job; retry reuses idempotency rules and cannot revive a stale result. |
| FR-19 | Finding conversation | Authorized PR/MR participants can reply to a Guardener-created finding and receive a bounded, current-head-aware explanation. |
| FR-20 | Change summary | Every completed advisory review returns a concise purpose/risk/walkthrough summary even when it has no inline finding. |
| FR-21 | Community deployment | A signed proprietary single-organization package under a no-cost Community license runs integration, orchestration, analysis, publication, and the basic dashboard without a Guardener Cloud dependency or vendor-enforced repository/seat/review limit. |
| FR-22 | BYO LLM | Community accepts a declared compatible model endpoint, identifier, and secret from the operator; preflight, budgets, schema validation, and kill controls apply exactly as they do to managed model routes. |
| FR-23 | Portable configuration | Community can export and import repository policy plus normalized non-source metadata using a documented versioned format; raw provider and model credentials are never exported. |

## Provider parity contract

| User outcome | GitHub | GitLab |
|---|---|---|
| Detect change lifecycle | `pull_request` webhook | Merge request webhook |
| Deterministic result | Check run with annotations | Commit pipeline status; use external status checks only where the customer tier supports them |
| Advisory summary | PR review/timeline comment | MR overview note or discussion |
| Inline advisory finding | Pull-request review comment with current commit/path/line/side | MR diff discussion with current base/start/head SHA and old/new path/line |
| Retry identity | GitHub delivery ID plus normalized event identity | `webhook-id`/`Idempotency-Key` plus normalized event identity |
| Stale protection | Verify latest PR head before write | Verify latest MR head before write; provider status APIs also reject mismatched heads in supported flows |

Provider documentation supporting these contracts:

- GitHub documents [pull-request webhook events](https://docs.github.com/en/webhooks/webhook-events-and-payloads#pull_request), [line-aware review comments](https://docs.github.com/en/rest/pulls/comments#create-a-review-comment-for-a-pull-request), and [check runs](https://docs.github.com/en/rest/checks/runs).
- GitLab documents [merge-request webhook events](https://docs.gitlab.com/user/project/integrations/webhook_events/#merge-request-events), [diff discussions](https://docs.gitlab.com/api/discussions/#create-a-new-thread-in-the-merge-request-diff), and [commit pipeline status](https://docs.gitlab.com/api/commits/#set-commit-pipeline-status).

## Job and finding behavior

### Job states

`received → debounced → queued → leased → running → publishing → completed`

Terminal alternatives are `failed`, `cancelled`, and `stale`. A job may retry a
transient stage, but its provider, repository, change number, and head SHA never
change.

### Severity and authority

| Origin | Severity | May affect Guardener deterministic status? |
|---|---|---|
| ForgeGuard rule | info, warning, error | Yes, according to repository policy |
| LLM reviewer | low, medium, high | No |

LLM severity is prioritization metadata, not proof. The UI and provider comment
must use “advisory” language and must not imitate an approval, rejection, or
required-check conclusion.

### Inline eligibility

An inline comment is published only if all of these are true:

- the job head still matches the provider's current head;
- the path is present in the analyzed diff;
- the line and side can be resolved to that provider's current diff position;
- the finding is not a duplicate of an active Guardener finding; and
- the body passes output and secret-leak validation.

Otherwise the finding remains in the summary and dashboard with an explanation
such as “inline position unavailable.” It is never attached to a best-guess
line.

## Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-01 | Webhook handlers authenticate the raw request, persist an idempotent receipt, enqueue work, and acknowledge within the provider deadline. |
| NFR-02 | Tenant identity is included in every authorization and storage lookup; cross-tenant identifiers are rejected. |
| NFR-03 | Raw source, diffs, and code-bearing prompts are ephemeral by default and absent from application logs, traces, error trackers, and backups. |
| NFR-04 | Secrets are encrypted at rest, redacted from logs, scoped by tenant/provider, and rotatable without redeploying application code. |
| NFR-05 | Provider writes are idempotent and audited; retries cannot create duplicate current findings. |
| NFR-06 | Analysis is bounded by repository size, changed lines, files, runtime, model tokens, and tenant concurrency. A bounded skip is visible, not silent. |
| NFR-07 | Managed workers use isolated ephemeral workspaces, no inbound access, and no execution of repository scripts. |
| NFR-08 | The control plane exposes health, queue age, job duration, provider/API failures, model usage, and runner availability without logging source. |
| NFR-09 | Dashboard flows meet WCAG 2.2 AA for keyboard access, focus, labels, contrast, and status communication. |
| NFR-10 | Every persisted record has a documented retention class and tenant-deletion path. |
| NFR-11 | Community starts without contacting Guardener services; update checks and anonymous usage telemetry are separate, explicit opt-ins and never contain repository identity or source. |
| NFR-12 | The supported Community topology is reproducible, version-pinned, upgradeable, backed up, and removable through documented operator procedures. |

## MVP exclusions

- Automatic code edits, commits, or suggestion application.
- IDE plugins and a general repository chat interface.
- Merge blocking based on LLM output.
- Arbitrary execution of customer CI commands on managed workers.
- Enterprise SSO, SCIM, custom data residency, or certification commitments.
- High availability, horizontal scale, and air-gapped enterprise lifecycle
  automation in the initial Community package.
- Billing enforcement beyond metering and internal usage limits.
- Automatic replacement of existing Guardener workflow files.

## Acceptance scenarios

The release test matrix must cover both providers for:

1. first installation and selected-repository activation;
2. PR/MR open, reopen, and a new source commit;
3. rapid consecutive pushes inside the debounce window;
4. duplicate and out-of-order webhook delivery;
5. stale result attempting to publish after a new head exists;
6. valid addition, deletion, range, renamed-file, and unavailable inline position;
7. empty, oversized, binary-only, generated-only, and draft changes;
8. deterministic finding with blocking and non-blocking repository policy;
9. LLM high-severity finding proving no blocking provider status is written;
10. model timeout, malformed output, rate limit, and unavailable endpoint;
11. provider API failure before and after partial publication;
12. untrusted fork/source project with repository commands disabled;
13. cloud worker and private runner result parity;
14. disabled, offline, revoked, and version-incompatible private runner;
15. integration uninstall with queued and running work; and
16. tenant authorization failures for every dashboard object type;
17. authorized and unauthorized replies to a Guardener finding thread; and
18. a clean diff producing a useful summary with no invented finding;
19. Community startup and first review with every Guardener service endpoint
   blocked;
20. compatible, incompatible, unreachable, rate-limited, and malformed BYO
   model routes; and
21. Community policy/metadata export-import with credentials and source proven
   absent from the archive.

## Dependencies

- The normalized contracts and service boundaries in
  [platform architecture](04-platform-architecture.md).
- The trust, retention, and model controls in
  [AI review, security, and governance](05-ai-review-security-governance.md).
- The onboarding and operational surfaces in
  [website experience](03-website-experience.md).
- The release and runner model in
  [deployment, CI/CD, and operations](06-deployment-cicd-operations.md).
