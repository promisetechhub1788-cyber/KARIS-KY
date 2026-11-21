# Issue: Create CHANGELOG.md for User-Visible Schema-Version Changes

**Type:** Documentation / release process
**Status:** Backlog, ready for review and assignment
**Priority:** Medium
**Severity:** Medium
**Related:** [README schema version changelog](../../README.md), [Schema versioning ADR](../adr/ADR-007-storage-key-evolution.md), [Interface versioning](../escrow-interface-versioning.md), [Upgrade decision trees](../UPGRADE_DECISION_TREES.md), [Operator runbook](../OPERATOR_RUNBOOK.md)

## Description

Create a repository-root `CHANGELOG.md` that records user-visible changes for
each `SCHEMA_VERSION` of the escrow contract. The changelog should let
investors, integrators, operators, auditors, and release reviewers answer:
what changed, which behavior or data is affected, whether existing instances
need migration or redeployment, and what action is required from consumers.

The changelog must be organized by schema version and must distinguish:

- contract storage/schema changes that affect deployed instances;
- public entrypoint, read API, event, error, and SDK behavior changes;
- operational or security changes visible to deployers and operators; and
- internal changes that do not alter user-visible behavior.

`SCHEMA_VERSION` is the on-chain value stored under `DataKey::Version` and is
not interchangeable with the contract interface version, WASM version, or
release tag. The document must preserve that distinction and explain whether a
version change is additive, migration-compatible, or redeploy-required.

## Current Behavior

The repository has partial version history in the README and many feature,
ADR, deployment, and completion documents. The README currently describes
schema versions 1 through 7, but there is no canonical root changelog with a
stable release-note format, dates, user impact, upgrade action, or links to the
implementation and verification evidence.

As a result, version information is distributed across documents and can drift.
A reader cannot reliably determine from one place whether a feature changed
on-chain storage, altered public behavior, required a new deployment, or was
only an internal implementation change.

## Steps to Reproduce

1. Start from a clean checkout and look for a root-level `CHANGELOG.md`.
2. Search the repository for `SCHEMA_VERSION` and compare the version history
   in `README.md`, `escrow/src/lib.rs`, deployment scripts, SDK metadata, ADRs,
   and feature completion reports.
3. Choose a version transition, such as v6 to v7, and try to answer from one
   canonical document:
   - what changed for investors and integrators;
   - whether stored data is compatible;
   - whether a migration or redeployment is required;
   - which public entrypoints, events, errors, or read APIs changed; and
   - which deployment and verification steps an operator must perform.
4. Observe that the information is spread across multiple documents and that
   several historical documents contain stale version references.
5. Prepare a release or pull request that changes `SCHEMA_VERSION` without
   updating a required changelog entry. No repository check currently prevents
   that omission.

## Expected Behavior

- A root `CHANGELOG.md` is the canonical user-facing history for schema-version
  changes and links to deeper technical documents where appropriate.
- Every documented schema version has a consistent entry containing the
  version, release/date or status, summary, user-visible impact, storage and
  compatibility classification, upgrade action, public API/event/error impact,
  operational notes, and verification references.
- The history covers all currently documented versions, v1 through the current
  `SCHEMA_VERSION`, without inventing release claims that cannot be verified.
- Readers can distinguish schema version from interface version and WASM/release
  version, including additive changes that do not require migration.
- Entries clearly state whether existing deployments need no action, a WASM
  upgrade, an explicit migration, or a new deployment/redeployment.
- Unreleased, planned, or superseded work is labeled as such rather than being
  presented as a deployed user-visible change.
- Future schema-version changes update the changelog in the same change set as
  the source, generated contract metadata, relevant API/SDK documentation, and
  deployment guidance.

## Actual Behavior

There is no canonical `CHANGELOG.md`. The README provides a compact schema
version table, while detailed user-visible changes are described inconsistently
across ADRs, feature summaries, deployment docs, and implementation reports.
Some documents still reference earlier current versions, and there is no
checklist or CI validation requiring a changelog entry when `SCHEMA_VERSION`
changes.

Operators and integrators must manually reconcile these sources, which creates
risk of deploying a contract with misunderstood compatibility requirements or
failing to communicate changed entrypoints, events, errors, storage behavior,
or investor workflows.

## Proposed Solution

1. Add a root-level `CHANGELOG.md` using a consistent versioned format. Include
   an introduction explaining that the file tracks user-visible changes and
   that `SCHEMA_VERSION` is read from `DataKey::Version`.
2. Backfill verified entries for v1 through the current v7. At minimum, capture:
   - v1 initial escrow schema and funding/settlement flow;
   - v2 per-investor yield and claim timing keys;
   - v3 funding-close snapshot, contribution floor/cap, and unique-funder data;
   - v4 attestation API and append log;
   - v5 yield tiers, registry/treasury data, and layout compatibility warning;
   - v6 persistent per-investor storage and redeployment requirement; and
   - v7 dispute-pause state and its additive compatibility classification.
3. For every entry, use fields for status/date, affected users, public surface,
   storage/schema impact, compatibility and upgrade path, operational action,
   security or accounting impact, and links to source/ADR/tests/docs.
4. Reconcile the backfilled history against `escrow/src/lib.rs`, generated
   `sdk-ts/spec.json`, build metadata, README, deployment scripts, ADRs, and
   release evidence. Mark uncertain historical dates or unreleased features
   explicitly instead of guessing.
5. Define the change policy: a pull request that changes `SCHEMA_VERSION` must
   update `CHANGELOG.md`, the relevant compatibility/read API/operator docs,
   and generated metadata in the same change set.
6. Add a lightweight CI or review check that detects a changed
   `SCHEMA_VERSION` and fails when the changelog and required documentation are
   not updated. The check must not require a version bump for additive internal
   changes that are explicitly documented as no schema change.
7. Link the README's compact table and deployment/runbook guidance to the
   canonical changelog, while retaining concise quick-reference material where
   it helps operators.
8. Add a contributor/release checklist describing how to classify changes and
   how to write entries for schema, interface, security, operational, and
   internal changes.

## Environment Context

- **Repository:** `KARIS-KY`
- **Contract:** `escrow` Soroban smart contract
- **Platform:** Stellar Soroban
- **Language/toolchain:** Rust, Cargo, Soroban SDK, TypeScript SDK, shell/CI
- **Canonical source:** `escrow/src/lib.rs` (`SCHEMA_VERSION` and
  `DataKey::Version`)
- **Current documented schema:** v7
- **Related metadata:** `sdk-ts/spec.json`, `escrow/target/build_metadata.json`,
  deployment scripts, README, ADRs, and operator/deployment documentation
- **Audience:** investors, integrators, SDK users, indexers, operators,
  auditors, release managers, and maintainers
- **Compatibility concerns:** additive storage keys, persistent-storage moves,
  stored XDR/layout changes, explicit migrations, redeployment, API/event/error
  changes, and backward-compatible defaults
- **Verification:** documentation link checks, metadata/version consistency,
  changelog policy check, and existing Cargo/SDK test suites where applicable

## Acceptance Criteria

- [ ] A root-level `CHANGELOG.md` exists and explains its scope, audience,
      schema-version source, and relationship to interface/WASM versions.
- [ ] Verified entries cover schema versions v1 through the current v7, with
      no unsupported claims presented as released behavior.
- [ ] Each entry includes date/status, summary, user-visible impact, affected
      public surfaces, storage impact, compatibility classification, upgrade or
      deployment action, operational/security notes, and evidence links.
- [ ] Each version explicitly states whether existing instances require no
      action, a WASM upgrade, migration, or redeployment.
- [ ] Additive-only changes and breaking stored-layout changes are clearly
      distinguished, including the v5/v6 compatibility implications.
- [ ] The v7 dispute-pause behavior is documented separately from the legal-hold
      overlay and interface-version changes.
- [ ] Unreleased, planned, superseded, and internal-only changes have explicit
      labels and are not mixed into the released version history.
- [ ] README and operator/deployment documentation link to the canonical
      changelog and do not contradict its current-version or upgrade guidance.
- [ ] A documented release checklist requires changelog updates whenever
      `SCHEMA_VERSION`, public APIs, events, errors, SDK behavior, or deployment
      compatibility changes.
- [ ] CI or an equivalent automated review check detects a `SCHEMA_VERSION`
      change without a corresponding changelog/documentation update.
- [ ] Links, formatting, version references, and generated metadata pass the
      repository's available validation checks.
- [ ] A maintainer can use the changelog alone to determine the user and
      operator action for every supported schema transition.

## Assignment Notes

Before assignment, confirm the authoritative release dates and deployed status
for each historical version, especially documents that still describe v6 as
current. Decide whether the changelog should track only schema-version entries
or also include unreleased interface/security changes in a separate section.
Assign one owner for version classification and one reviewer familiar with
Soroban storage compatibility and deployment procedures. Do not close the issue
until the changelog, README, runbook, generated metadata, and CI/review policy
agree on the current schema version.
