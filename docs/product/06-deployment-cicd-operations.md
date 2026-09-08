# Guardener deployment, CI/CD, and operations

Status: delivery and operating model
Audience: infrastructure, platform, release, security, and customer engineering

## Deployment decision

Guardener has four deployment profiles:

1. **Community:** free-to-use proprietary single-organization self-deployment.
   The customer runs the signed image/binary, local control plane, worker, web
   UI, database, provider credentials, and a compatible BYO LLM route.
2. **Cloud:** Guardener operates the control plane, isolated ephemeral workers,
   approved model routes, upgrades, and backups.
3. **Hybrid:** Cloud control plane with an outbound-only customer runner and
   customer-approved model route for private-network execution.
4. **Enterprise Self-Hosted:** paid supported deployment for advanced identity,
   audit, high availability, air-gap, and commercial support needs.

All profiles run the same analyzer release and exchange the same versioned
`ReviewJob`/`AnalysisResult` contracts from
[platform architecture](04-platform-architecture.md). Community is not a trial
and requires no Guardener account, license-server heartbeat, or Guardener-
funded model usage.

The first Community release supports one production topology: a version-pinned
single-node container composition with the Guardener application, worker, and
PostgreSQL. Kubernetes, multi-node scheduling, and HA are deliberately deferred
until demand justifies the operating surface.

## Environment topology

| Environment | Purpose | Data rule |
|---|---|---|
| Development | Local component work and synthetic provider fixtures | No production credentials or customer source |
| Preview | Per-change UI/API verification with synthetic tenants | Automatically expires |
| Staging | Production-like integration, migration, load, and rollback rehearsal | Dedicated test GitHub/GitLab organizations only |
| Production | Customer traffic | Tenant isolation, audited access, approved models and regions only |
| Community operator deployment | Customer-owned production or evaluation | One organization; customer controls network, provider/model credentials, persistence, backup, and availability |

Each environment has separate provider applications, webhook secrets, OAuth
credentials, model routes, encryption keys, databases, queues, object stores,
and observability projects. Production credentials are never usable from lower
environments.

## Deployable units

| Unit | Release behavior |
|---|---|
| Web application | Immutable static/server artifact; API compatibility checked before promotion |
| Control-plane API | Backward-compatible deployment before dependent workers/UI |
| Webhook ingress | Independently scalable; minimal dependencies and fast rollback |
| Orchestrator/publisher | Drains or hands off leases before replacement |
| Managed worker image | Immutable, signed, vulnerability-scanned, digest-pinned |
| Guardener Runner | Versioned image/binary with compatibility window and upgrade instructions |
| Analyzer bundle | Pinned Guardener CLI and ForgeGuard version, signed and recorded per job |
| Prompt/schema bundle | Versioned configuration artifact promoted independently under AI release gates |
| Community bundle | Version-pinned application, worker, PostgreSQL composition, migration command, health check, backup/restore, and uninstall guide; no bundled LLM |

The first implementation should keep the number of separately deployed
services small: ingress/API, asynchronous worker/orchestrator, web application,
and the runner/analyzer image are sufficient until measured scaling or security
isolation requires another split.

## Source-to-artifact mapping

Repository visibility and artifact availability are separate decisions. The
current MIT-licensed `suiflex/Guardener` repository owns the CLI/analyzer. A
separate private proprietary `guardener-platform` monorepo owns every service
and deployment profile. Community, Cloud, Hybrid, and Enterprise are release
targets from that one platform codebase, not separate repositories.

| Source | Release artifact | Consumer |
|---|---|---|
| `suiflex/Guardener` plus pinned `forgeguard-core` | Versioned analyzer binary/library and license notices | Platform worker, private runner, current CI users |
| `guardener-platform` | Signed Community application/worker image, web assets, single-node deployment bundle, SBOM, checksums, and proprietary Community license | Free self-hosted operators; no source-repository access required |
| `guardener-platform` | Digest-pinned control-plane, web, and managed-worker images | Guardener Cloud production |
| `guardener-platform` | Versioned outbound-only Runner image/binary plus compatibility manifest | Hybrid customers |
| `guardener-platform` | Supported self-hosted bundle and offline entitlement material | Enterprise Self-Hosted customers |

Every release manifest records the platform commit, analyzer and ForgeGuard
versions, job/result schema, prompt/schema bundle, image digests, SBOM, and
applicable license notices. Hybrid is the Cloud release plus a compatible Runner
artifact; it is not a third application build. Community artifacts may be
downloaded from a registry or as an offline archive while the source repository
remains private.

Do not create separate Community, Cloud, or Hybrid repositories merely to hide
features. Entitlements and deployment configuration select paid operational
capabilities. Split a component later only when it has an independently proven
security boundary, owner/release cadence, toolchain, or scaling requirement.

## Application CI pipeline

Every change runs, in order:

1. format, lint, unit, contract, and authorization tests;
2. provider webhook and API fixture tests for GitHub and GitLab;
3. database migration compatibility and rollback checks where applicable;
4. prompt/schema evaluation when AI artifacts change;
5. dependency, secret, license, and container vulnerability scans;
6. reproducible build of immutable artifacts and software bill of materials;
7. artifact signing and provenance attestation;
8. a profile matrix proving the same tagged contracts on Community, Cloud, and
   Hybrid topologies; and
9. preview deployment plus smoke checks for changed web/API surfaces.

Promotion is build-once: staging and production use the same digests. A failed
optional model review test cannot hide a deterministic or security test failure.

## Release flow

1. Merge creates signed release candidates.
2. Deploy control-plane schema/API changes to staging using expand-compatible
   migrations.
3. Run provider installation, webhook, analysis, publication, stale-head, and
   uninstall journeys against dedicated GitHub and GitLab test projects.
4. Run managed/private worker parity and cleanup checks.
5. Promote to a production canary tenant set.
6. Compare errors, queue age, latency, schema validity, provider writes, model
   usage, and cleanup signals with the prior version.
7. Expand gradually or roll back the deployable unit.
8. Mark the release complete only after old worker leases drain and migration
   compatibility is confirmed.

Database migrations use expand/migrate/contract. Destructive contract steps are
separate later releases after all readers and writers are proven migrated.

Community releases use the same signed images and schema compatibility checks.
Operators pull a named version, run a preflight and backup, apply the documented
migration, and verify health before resuming provider writes. Automatic update
checks are optional; silent unattended upgrades are not allowed.

## Rollback

Every release records the last compatible API, worker, analyzer, prompt, and
schema versions. Rollback actions are:

- stop new leases to the faulty version;
- route new traffic/jobs to the prior known-good digest;
- allow compatible in-flight jobs to finish or mark them stale;
- disable provider writes if publication correctness is uncertain;
- roll back prompt/model route independently when only advisory output regresses;
- never reinterpret an already persisted result under a different policy; and
- use a forward migration when data has crossed an irreversible schema boundary.

The model kill switch leaves deterministic analysis operating. A control-plane
incident may instead pause ingestion after durable receipts or disable all
writes, depending on the failing boundary.

## GitHub repository rollout

Guardener Cloud uses a dedicated GitHub App with webhooks enabled. Community
uses an operator-created GitHub App so its private key and webhook secret remain
local. Installing either App for selected repositories is the repository
opt-in.

When the provider cannot reach a Community webhook endpoint, the instance may
use bounded outbound polling/reconciliation with a durable cursor and the same
head-SHA idempotency rules. Polling is a visible lower-immediacy mode, not an
excuse to expose a private endpoint. GitHub Enterprise Server requires a
declared supported-version matrix before being marketed as compatible.

The existing `guardener-bot` and workflow-based deployment continue unchanged
during migration. A repository must never receive both legacy and SaaS
deterministic publications for the same head. Activation therefore checks for
the legacy workflow and requires the administrator to choose:

- stay on legacy Guardener;
- run SaaS in shadow mode with no provider writes; or
- migrate provider writes to SaaS after a side-by-side result comparison.

The app requests only endpoint-derived permissions. GitHub notes that App
permissions control both REST access and available webhook subscriptions, and
installation owners must approve later permission increases:
[Choosing GitHub App permissions](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/choosing-permissions-for-a-github-app).

## GitLab project rollout

GitLab onboarding uses OAuth for user authorization plus project/group webhook
installation and project selection. Cloud launches with GitLab.com. Community
and Hybrid add self-managed GitLab only through a declared supported-version
matrix plus host validation, OAuth/token compatibility, egress, TLS, webhook or
outbound polling, and provider API contract tests.

The consent screen explains the permission trade-off: GitLab documents
`read_repository` as read-only source access and `api` as broad API read/write
access. MVP needs `api` to publish discussions and status, but Guardener's
service-side allowlist still limits operations to selected projects and the
documented review endpoints:
[GitLab OAuth scopes](https://docs.gitlab.com/integration/oauth_provider/#view-all-authorized-applications).

For new webhooks, prefer GitLab signing tokens with HMAC-SHA256 and timestamp
validation. Keep legacy secret-token compatibility only when a supported GitLab
deployment lacks signing tokens:
[GitLab webhook signing tokens](https://docs.gitlab.com/user/project/integrations/webhooks/#signing-tokens).

Deterministic status defaults to commit pipeline status across tiers. GitLab
external status checks remain an optional adapter capability because the
official documentation identifies them as Ultimate-tier:
[External status checks](https://docs.gitlab.com/user/project/merge_requests/status_checks/).

## Private runner

### Registration

An administrator creates a runner group and receives a one-time credential with
a short expiry. Registration exchanges it for a rotatable runner identity bound
to one tenant and runner group. The control plane never displays the credential
again.

### Connection and lease

- Runner initiates outbound TLS; no inbound customer firewall rule is required.
- Mutual authentication binds every poll/stream and result to the registered
  runner identity.
- Jobs are leased, not pushed; a lease has expiry, attempt number, analyzer
  version, policy snapshot, and resource bounds.
- Provider/source access is brokered with short-lived credentials or performed
  through a customer-controlled credential available only to the runner.
- A result for the wrong tenant, job, attempt, or head is rejected.
- Offline private runners do not silently spill work into managed execution.

### Isolation and cleanup

Each job receives a fresh workspace and process/container boundary, read-only
analyzer image, bounded CPU/memory/disk/time/network, and a restricted egress
policy. Repository scripts remain disabled unless a future explicit trusted-
repository feature is designed and separately reviewed.

After submission, the runner deletes checkout, diff, context, prompt/response,
temporary credentials, and transient logs. Cleanup failure prevents reuse of
the workspace and emits a high-priority operational event.

### Compatibility

The control plane publishes minimum and recommended runner versions. An old but
compatible runner may finish existing leases; an incompatible runner receives
no new jobs and the dashboard provides a concrete upgrade path. Forced upgrades
require a security reason and an announced deadline.

## Community operations

- Provider and model endpoints are configurable instance settings; credentials
  enter through environment, mounted secrets, or a supported local secret store
  and never through repository configuration.
- A preflight checks database persistence, provider permissions, webhook or
  polling reachability, model compatibility, disk capacity, and clock/TLS state.
- The instance exposes local health, queue, model usage, backup age, migration,
  and version status without requiring Guardener telemetry.
- Anonymous telemetry and update checks are separate opt-ins. A diagnostic
  bundle is generated locally, previews its manifest, and excludes source,
  diffs, prompts, tokens, provider/model secrets, and repository identity.
- Export/import includes versioned policy and normalized non-source metadata;
  destination operators re-enter every credential.
- Uninstall removes application components only after an explicit operator
  choice to retain or delete the named database and backup artifacts.

## Customer repository CI/CD

Guardener observes customer changes; it does not become their deployment
system. Integration rules are:

- installation is opt-in and repository selection is auditable;
- no direct commits, workflow rewrites, branch-protection changes, or label
  changes are made by the SaaS MVP;
- deterministic status can become required only through an explicit human
  repository setting outside Guardener's automatic rollout;
- LLM review never creates a required status;
- provider comments/statuses link to the exact Guardener job and head SHA; and
- uninstallation/revocation stops new work and removes credentials without
  requiring a source change.

This preserves Guardener's current principle that organization-wide automation
must not quietly revise repository decisions.

## Secrets and key operations

- Store provider app keys, webhook signing material, OAuth refresh tokens, model
  credentials, runner CAs, and encryption keys in the production secret/KMS
  boundary.
- Use workload identity instead of static infrastructure credentials.
- Rotate webhook, OAuth, model, runner, and encryption credentials through
  documented runbooks with dual-key overlap where protocols permit.
- Audit secret creation, read, rotation, failure, and revocation without logging
  values.
- Never pass model endpoints, keys, or model names through customer repository
  configuration.

For Community, “production secret/KMS boundary” means the operator-selected
local mechanism documented by the supported topology. Guardener does not copy
those secrets to a vendor service, update endpoint, telemetry event, export, or
diagnostic bundle.

## Backups and deletion

Back up metadata, findings, policy history, audit, and encrypted integration
state. Do not back up ephemeral checkout volumes, raw diffs, or code-bearing
model traffic. Restore tests run in an isolated environment and verify tenant
boundaries before the backup is considered usable.

Tenant deletion disables integrations and runner leases first, then removes
active metadata and secrets, records a tombstone without customer content, and
expires backup copies under the published retention schedule.

## Operating signals and alerts

| Signal | Page-worthy condition |
|---|---|
| Invalid webhook signatures | Sudden tenant/provider spike or attack pattern |
| Receipt latency/errors | Risk of provider delivery failure |
| Queue age | Sustained breach relative to measured baseline |
| Lease expiry/cleanup failure | Worker isolation or reliability issue |
| Current-head publication failure | Developers may see missing/misleading status |
| Cross-tenant authorization denial | Any unexpected code path or correlated pattern |
| Source-like data in logs | Any confirmed occurrence |
| Model schema/unsafe-output rejection | Sustained regression after prompt/model release |
| Runner version/capacity | Tenant has no compatible capacity for queued work |

Numeric alerts are calibrated from staging load tests and design-partner
baselines, then recorded in runbooks. This document does not invent production
thresholds before those observations exist.

## Required runbooks before beta

- GitHub and GitLab credential rotation/revocation.
- Webhook delivery failure and reconciliation.
- Queue backlog and worker exhaustion.
- Provider rate limiting and outage.
- Model provider outage or unsafe-output spike.
- Private runner offline, compromised, incompatible, or cleanup failure.
- Source-data exposure investigation and deletion.
- Cross-tenant authorization incident.
- Database restore and migration rollback.
- Tenant offboarding and deletion.
- Community install, upgrade, rollback, backup/restore, diagnostics, export,
  and uninstall.
- Webhook-unreachable Community operation through outbound polling and
  reconciliation.
