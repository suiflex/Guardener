# Guardener AI review, security, and governance

Status: mandatory product controls for MVP
Audience: AI engineering, security, platform, legal/privacy, and product

## Governing rule

The LLM is an untrusted analysis component. It may propose a finding, severity,
location, explanation, or suggestion. It cannot call source-control tools,
publish a comment, change policy, expose credentials, approve a change, request
changes, or set a blocking status.

Every external effect flows through deterministic authorization, validation,
current-head verification, idempotency, and audit controls in the Guardener
control plane.

## Trust boundaries

| Input or actor | Trust treatment |
|---|---|
| PR/MR title, description, diff, source, comments | Untrusted customer-controlled data and possible prompt injection |
| Repository configuration | Untrusted on managed workers; parsed under schema and bounds, never executed |
| Provider webhook | Untrusted until raw-body signature/token and replay checks pass |
| LLM response | Untrusted data until schema, content, location, and secret validation pass |
| Private runner result | Authenticated but still schema-validated, tenant-bound, and current-job-bound |
| Community operator | Privileged inside its own instance; cannot weaken hard safety invariants through repository content or normal UI policy |
| Customer-supplied model endpoint | External processor chosen by the operator; untrusted response and independently governed retention/network boundary |
| Dashboard user | Authenticated but authorized for every tenant/repository/action |
| Provider publisher | Privileged narrow component; accepts only validated operations |

## Review scope

The automatic reviewer receives:

- PR/MR title and description;
- base/head identifiers and normalized textual diff;
- repository and path-specific engineering guidance;
- deterministic finding summaries so it does not repeat ForgeGuard;
- bounded surrounding code for changed symbols/files; and
- explicit instructions to return no findings when evidence is insufficient.

It does not receive an unrestricted repository dump, provider credentials,
secrets, unrelated tenant content, private user data, build logs, arbitrary web
results, or write-capable tools.

The initial changed-line ceiling inherits the current Guardener review default
of 1,500 lines. Larger changes receive deterministic analysis and an explicit
“advisory review skipped: change exceeds configured bound” result. Design-
partner evidence may justify changing the default later.

## Prompt and output contract

Production prompts are immutable versioned artifacts. A prompt release records:

- prompt ID and semantic version;
- supported output schema version;
- intended finding classes and explicit exclusions;
- deterministic categories already covered by ForgeGuard;
- model/provider compatibility and decoding parameters;
- file, line, byte, token, time, retry, and finding-count limits;
- injection-resistance tests and evaluation result; and
- author, reviewer, approval, rollout, and rollback metadata.

The model must return structured findings with only these meanings:

| Field | Validation |
|---|---|
| `path` | Must be a normalized path in the analyzed diff |
| `start_line`, `end_line`, `side` | Optional; must resolve against the exact provider diff before inline use |
| `severity` | `low`, `medium`, or `high`; always advisory |
| `title` | Short plain text with length bound |
| `explanation` | Evidence-based explanation with length bound; no hidden HTML |
| `suggestion` | Optional; displayed as text in MVP, never automatically committed |
| `evidence` | References changed behavior or code location; model confidence is not evidence |

Unknown fields are rejected. Malformed responses are not repaired with a second
unbounded conversational loop; one schema-constrained retry is allowed when the
remaining job budget permits, then the advisory result fails visibly.

## Automatic, conversational review

Guardener adopts the useful interaction model shown by
[CodeRabbit](https://www.coderabbit.ai/): review every eligible change
automatically, summarize it, place focused inline findings, and let engineers
reply in the provider thread.

The MVP supports bounded conversation:

1. An authorized participant replies to a Guardener-created finding thread.
2. A provider webhook authenticates the event and links it to the exact finding.
3. The model receives the finding, the new reply, the original bounded evidence,
   and current-head context.
4. Guardener posts a reply only after the same validation and current-head
   checks used for initial review.

Conversation does not grant broader repository context or tools. The model may
explain, withdraw the finding, or acknowledge a repository convention. It may
not claim a fix is verified unless a later deterministic scan or changed diff
provides evidence.

## Learnings and repository guidance

Continuous learning is useful but unsafe when every conversational correction
becomes permanent instruction. Guardener therefore uses an explicit learning
workflow:

- A user can propose a learning from a thread, such as a repository convention.
- The proposal records source thread, author, repository scope, and proposed
  instruction.
- A repository administrator approves, edits, or rejects it.
- Approved guidance is versioned, auditable, and removable.
- Guidance is treated as untrusted context and cannot override security,
  retention, authorization, or non-blocking controls.

There is no invisible model memory in MVP. User feedback also feeds offline
evaluation only after tenant-safe redaction and documented consent.

## Finding quality controls

Before publication, Guardener enforces:

1. **Schema:** exact types, enum values, counts, and length limits.
2. **Relevance:** path belongs to the diff and cited lines exist.
3. **Freshness:** head SHA still matches the current PR/MR.
4. **Deduplication:** fingerprint does not match an active deterministic or LLM
   finding with the same issue identity.
5. **Secret and unsafe-content scan:** reject suspected credentials, unsafe links,
   control characters, hidden markup, and prompt/system text leakage.
6. **Authority language:** remove any status/approval instruction that implies
   the model controls mergeability.
7. **Publication budget:** cap summary size, inline count, and replies per change.

When confidence is insufficient, omission is correct. A quiet review is a valid
successful result.

## Prompt injection controls

Repository content may contain instructions addressed to the model. The prompt
and gateway must:

- delimit provider metadata, instructions, diff, and supporting context as
  separate data sections;
- state that repository content cannot alter policy, reveal prompt text, request
  tools, change output schema, or authorize external action;
- provide no credentials or write tools to the model;
- allowlist context fetches before the model call rather than letting model text
  choose arbitrary files or URLs;
- cap iterations at one review call plus one schema retry;
- scan output for prompt leakage and secret-like material; and
- retain injection fixtures in the evaluation suite.

These controls reduce risk; they do not make model output trusted. The
deterministic publisher boundary remains mandatory.

## Data handling

Default source policy is ephemeral/no retention:

- checkouts, diffs, code context, and code-bearing prompts/responses exist only
  for the active job;
- application logs, traces, metrics, analytics, support tools, and backups must
  exclude source bodies and raw prompts;
- only structured findings, normalized locations, hashes, dispositions, job
  metadata, model usage, and audit events persist;
- model providers must be configured not to train on or retain submitted code
  beyond the contracted processing need; and
- a private runner is available when a customer cannot send code to a managed
  worker. No silent managed-worker fallback is allowed.

Product and legal must publish the actual subprocessors and retention behavior
before external launch. This document is a required design, not a certification
claim.

Deployment changes who operates the boundary:

- Guardener Cloud documents its model subprocessors and sends code only through
  a tenant-approved managed route.
- Hybrid executes source access on the private runner and calls only the model
  route approved for that runner group; no silent managed fallback is allowed.
- Guardener Community keeps provider and model credentials in the local
  instance. Guardener receives no source or credential unless the operator
  separately opts into a support workflow and reviews the exact payload.

“Self-hosted” does not guarantee that code stays local when the operator points
Community at an external LLM. The product must show the endpoint host and make
that data-flow consequence explicit. A local model route keeps the complete
review data path inside the customer's network.

## Secrets and credentials

- Provider credentials and model keys live in a dedicated encrypted secret
  store, separated by environment and tenant.
- Jobs receive short-lived, least-privilege credentials or opaque brokered
  access; long-lived installation and refresh credentials do not enter job logs.
- Rotation does not require source commits or application rebuilds.
- Secret reads, rotations, revocations, and failed decryptions are audited.
- Model endpoint, key, and model name remain outside public repository config,
  preserving the current Guardener invariant.
- Community accepts these values through environment variables, mounted secret
  files, or a supported local secret store. They are never embedded in export
  archives, diagnostic bundles, browser storage, or provider comments.

## Authorization

Every read and write checks tenant, organization role, provider integration,
repository selection, and object ownership. A webhook actor is not automatically
a Guardener administrator. Replies from unauthorized users may remain visible
in GitHub/GitLab but do not trigger paid model work.

The provider publisher uses a narrow operation allowlist:

- fetch current PR/MR head;
- create/update deterministic status;
- create/update Guardener summary;
- create/update/resolve Guardener-owned inline thread where supported; and
- read Guardener-owned publication references for idempotency.

It cannot merge, approve, request changes as a formal review decision, push a
commit, modify settings, or write outside the current repository/change.

## Model operations

Each model route defines:

- endpoint and model identifier held in secret/configuration service;
- prompt/schema compatibility;
- request timeout and at most one transient transport retry;
- tenant concurrency, token, and spend limits;
- fail-open behavior for mergeability—the deterministic result remains
  authoritative when the model fails;
- redacted request/response accounting; and
- circuit breaker and operator disable switch.

The initial BYO contract targets the OpenAI-compatible request shape already
used by Guardener. Compatibility means passing Guardener's declared request,
structured-output, timeout, and evaluation suite; an endpoint is not supported
merely because it accepts a similarly named path. The operator owns endpoint
availability, capacity, usage charges, and provider terms. Guardener owns safe
request construction, bounds, validation, observable failure, and the promise
that model failure cannot block a merge.

No automatic model fallback may send code to a provider the tenant has not
approved. Changing model/provider or retention terms is an auditable
administrative action and may require renewed customer consent.

## Evaluation and release gates

Maintain a versioned evaluation corpus made from licensed synthetic examples,
public code with compatible rights, and customer examples only with explicit
consent and tenant-safe handling. It covers logic bugs, broken invariants,
error-path data loss, contract breaks, false-positive traps, prompt injection,
secrets, large diffs, generated files, renamed paths, and no-finding changes.

For every prompt/model release, measure:

- schema validity and unsafe-output rejection;
- relevant/actionable ratings and invalid dispositions;
- overlap with deterministic findings;
- inline-position validity;
- no-finding precision on clean fixtures;
- latency, tokens, and cost by change size; and
- regression against the currently deployed prompt/model.

Release requires no regression in security fixtures and explicit approval by
AI engineering and security. Product-quality thresholds are set after Phase 1
baselines; they must not be invented from model confidence.

## Incident and kill controls

Operators can disable model calls globally, per provider, tenant, repository, or
model route without disabling deterministic analysis. They can also disable all
provider writes while continuing safe ingestion.

An incident response records affected tenants/jobs, provider writes, prompt and
model versions, exposure window, containment, deletion, notification decision,
and prevention. Raw customer source is not copied into the incident system.

## Explicit exclusions

The first 12 months do not include automatic commits, one-click model fixes,
autonomous code agents, web search from review prompts, issue-tracker context,
post-merge actions, silent auto-learning, or a Guardener-funded free inference
pool. Community remains free because users supply compute and an LLM; this does
not relax any review safety control. Broader capabilities are considered only
after the core loop has measured safety and usefulness.
