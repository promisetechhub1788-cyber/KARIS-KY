# Requirements Document

## Introduction

The escrow contract exposes `record_sme_collateral_commitment` for SMEs to write collateral metadata, but provides no dedicated, directly named public read entrypoint. The existing read access is provided by `get_sme_collateral_commitment`; however, risk teams and compliance auditors who discover the write entrypoint cannot easily find a symmetrically named getter in the contract ABI, the TypeScript SDK, or the REPL CLI. They fall back to `get_escrow`, `export_state`, or scanning the full `EscrowSummary`.

This feature adds a `get_collateral_commitment(env: Env) -> Option<SmeCollateralCommitment>` entrypoint (aliasing the existing `get_sme_collateral_commitment` storage read), exposes it via `getCollateralCommitment(): Promise<SmeCollateralCommitment | null>` in the TypeScript SDK, adds a `get_collateral` command to the REPL CLI, adds tests for the `None` and `Some` paths, and documents the entrypoint in `docs/escrow-sme-collateral.md` and `docs/escrow-read-api.md`.

## Glossary

- **Escrow_Contract**: The `LiquifactEscrow` Soroban smart contract in `escrow/src/lib.rs`.
- **SmeCollateralCommitment**: The `#[contracttype]` struct stored under `DataKey::SmeCollateralPledge`; fields are `asset: Symbol`, `amount: i128`, `recorded_at: u64`, `updated_at: u64`.
- **Read_Entrypoint**: A Soroban contract function that reads instance storage without mutating state, emitting events, or requiring authorization.
- **SDK**: The TypeScript `EscrowClient` in `sdk-ts/src/client.ts`.
- **REPL_CLI**: The Rust interactive CLI in `repl-cli/src/main.rs`.
- **Commitment_Key**: `DataKey::SmeCollateralPledge` — the instance-storage key holding the single `SmeCollateralCommitment` record.
- **None_State**: The state before any `record_sme_collateral_commitment` call, in which `Commitment_Key` is absent from instance storage.

---

## Requirements

### Requirement 1: Contract Read Entrypoint

**User Story:** As a risk analyst or compliance auditor, I want a dedicated `get_collateral_commitment` entrypoint on the escrow contract, so that I can retrieve collateral metadata directly from the ABI without searching through `get_escrow` or `export_state`.

#### Acceptance Criteria

1. THE `Escrow_Contract` SHALL expose a public function named `get_collateral_commitment` that accepts `env: Env` and returns `Option<SmeCollateralCommitment>`.
2. WHEN `get_collateral_commitment` is called before any `record_sme_collateral_commitment` call, THE `Escrow_Contract` SHALL return `None`.
3. WHEN `get_collateral_commitment` is called after a successful `record_sme_collateral_commitment` call, THE `Escrow_Contract` SHALL return `Some(SmeCollateralCommitment)` with `asset`, `amount`, `recorded_at`, and `updated_at` equal to the values written by that call.
4. THE `get_collateral_commitment` function SHALL NOT mutate instance storage, emit events, or require any authorization.
5. WHEN `get_collateral_commitment` is called after multiple sequential `record_sme_collateral_commitment` calls, THE `Escrow_Contract` SHALL return the commitment written by the most recent call.

---

### Requirement 2: SDK Method

**User Story:** As a TypeScript application developer, I want a `getCollateralCommitment()` method on `EscrowClient`, so that I can retrieve collateral metadata with the same naming convention as the contract entrypoint.

#### Acceptance Criteria

1. THE `SDK` SHALL expose an async method `getCollateralCommitment(): Promise<SmeCollateralCommitment | null>` on the `EscrowClient` class.
2. WHEN `getCollateralCommitment` is called, THE `SDK` SHALL invoke the `get_collateral_commitment` contract entrypoint via `simulate` and return the decoded result.
3. WHEN the contract returns `None`, THE `SDK` SHALL resolve the promise with `null`.
4. WHEN the contract returns `Some(SmeCollateralCommitment)`, THE `SDK` SHALL resolve the promise with the decoded `SmeCollateralCommitment` object.

---

### Requirement 3: REPL CLI Command

**User Story:** As an operator using the REPL CLI, I want a `get_collateral` command, so that I can inspect collateral metadata interactively alongside other read commands.

#### Acceptance Criteria

1. THE `REPL_CLI` SHALL recognize the command input `get_collateral` (and the hyphenated alias `get-collateral`) and route it to a handler.
2. WHEN `get_collateral` is executed in mock mode (no contract specified), THE `REPL_CLI` SHALL return a JSON object representing a sample `SmeCollateralCommitment`.
3. WHEN `get_collateral` is executed and no commitment has been recorded, THE `REPL_CLI` SHALL return a JSON object with a `collateral_commitment` field set to `null`.
4. THE `REPL_CLI` `help` command SHALL list `get_collateral` alongside existing commands.
5. WHEN `help get_collateral` is invoked, THE `REPL_CLI` SHALL return a description of the command including its return fields and an example invocation.

---

### Requirement 4: Test Coverage

**User Story:** As a contract developer, I want tests for `get_collateral_commitment`, so that regressions in the read path are caught before deployment.

#### Acceptance Criteria

1. THE test suite SHALL include a test that calls `get_collateral_commitment` on a freshly initialized escrow and asserts the return value is `None`.
2. THE test suite SHALL include a test that calls `record_sme_collateral_commitment` followed by `get_collateral_commitment` and asserts the returned `SmeCollateralCommitment` matches the recorded values.
3. WHEN `record_sme_collateral_commitment` is called twice, THE test suite SHALL verify that `get_collateral_commitment` returns the values from the second call.
4. THE test suite SHALL verify that `get_collateral_commitment` returns the same value as `get_sme_collateral_commitment` for all tested states (round-trip equivalence between the two read functions).

---

### Requirement 5: Documentation

**User Story:** As a risk team member or integration engineer, I want `docs/escrow-sme-collateral.md` and `docs/escrow-read-api.md` to document `get_collateral_commitment`, so that I can find the entrypoint without reading the source code.

#### Acceptance Criteria

1. THE `docs/escrow-sme-collateral.md` file SHALL include a section describing `get_collateral_commitment`, its return type, and its relationship to `record_sme_collateral_commitment`.
2. THE `docs/escrow-read-api.md` file SHALL include an entry for `get_collateral_commitment` in the read-only entrypoints table, with its storage key, return type, and `None`-before-record behavior documented.
3. THE documentation SHALL note that `get_collateral_commitment` and `get_sme_collateral_commitment` read the same `DataKey::SmeCollateralPledge` storage key and produce identical results.
4. THE documentation SHALL include the disclaimer that the returned record is metadata-only and is not proof of on-chain asset custody or an enforced lien.
