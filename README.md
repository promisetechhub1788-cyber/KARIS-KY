# karis-ky Escrow Contracts

Soroban smart contracts for karis-ky, the invoice liquidity network on Stellar.
This repository contains the `escrow` contract that holds investor funds for
tokenized invoices until settlement and is maintained under the `karis-ky` project name.

> **Security:** To report a vulnerability, please follow our [security policy](SECURITY.md).
> Do not open public issues for security reports.

---

## Getting started (full stack)

This repository ships five cooperating pieces. The contract is the source of
truth; everything else is a client of it.

```
                    ┌──────────────────────────────┐
                    │  escrow/  (Soroban contract) │
                    │  funds, settle, claim, tiers │
                    └──────────────┬───────────────┘
                                   │  Soroban RPC
              ┌────────────────────┼────────────────────┐
              │                    │                    │
     ┌────────▼────────┐  ┌────────▼────────┐  ┌────────▼────────┐
     │  sdk-ts/        │  │  REPL / CLI     │  │  backend/       │
     │  typed client   │  │  (Stellar CLI)  │  │  indexer + API  │
     └────────┬────────┘  └─────────────────┘  └────────┬────────┘
              │                                         │  HTTP
              │                                         │
              └──────────────────┬──────────────────────┘
                                 │
                        ┌────────▼────────┐
                        │  frontend/      │
                        │  investor UI    │
                        └─────────────────┘
```

- **`escrow/`** — the Soroban contract. Build with `cargo build`, test with `cargo test`.
- **`sdk-ts/`** — TypeScript SDK wrapping the contract's typed client and contract types.
- **REPL / CLI** — the Stellar CLI pointed at the locally deployed contract for ad-hoc calls.
- **`backend/`** — indexer and HTTP API that reads contract events and serves the frontend.
- **`frontend/`** — investor-facing UI that talks to the backend API.

### Run everything locally

1. **Prerequisites** — the Rust toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml)
   (see [Supported Rust toolchain](#supported-rust-toolchain)), the `wasm32v1-none` target
   (`rustup target add wasm32v1-none`), Node.js 18+, and the Soroban / Stellar CLI.

2. **Start the local chain and deploy the contract** — one command sets up a
   local Soroban validator, identities, a test token, and a deployed contract:

   ```bash
   source scripts/local-env.sh
   ```

   This exports the environment variables (RPC URL, network passphrase,
   contract ID, identities) that the SDK, REPL, backend, and frontend all read.

3. **Build and exercise the contract directly (REPL / CLI)** — with the
   environment sourced, use the Stellar CLI to call the deployed contract, e.g.
   `stellar contract invoke --id $CONTRACT_ID -- <entrypoint> ...`.

## Escrow contract — public entrypoints

| Entrypoint | Description |
|------------|-------------|
| `init` | Create an invoice escrow; binds funding token, treasury, optional registry. See [parameter reference](docs/escrow-init-parameters.md). |
| `fund` | Record investor principal; marks escrow funded when target is met. See [fund parameters reference](docs/escrow-fund-parameters.md). |
| `fund_with_commitment` | First deposit with optional lock period; selects tiered yield. See [fund parameters reference](docs/escrow-fund-parameters.md). |
| `settle` | Mark a funded escrow as settled (SME auth required; maturity enforced). |
| `clone_settled_escrow` | Clone a settled escrow template to create a new independent escrow with the same parameters (admin auth required). |
| `withdraw` | SME pulls funded liquidity (accounting record). |
| `claim_investor_payout` | Investor records a payout claim after settlement. |
| `sweep_terminal_dust` | Treasury sweeps rounding residue from a terminal escrow. |
| `migrate` | Schema version gate — **typed errors on all paths** in the current release (codes 90–92). |
| `set_legal_hold` | Admin activates/clears compliance hold with reason (max 256 bytes; required when activating). |
| `pause_dispute` | Admin temporarily freezes escrow due to dispute (separate from legal hold). |
| `resume_dispute` | Admin manually resumes a paused escrow (or wait for auto-expiration). |
| `is_dispute_paused` | Check if dispute pause is currently active. |
| `get_dispute_pause` | Retrieve active dispute pause state (ticket, timestamps). |
| `bind_primary_attestation_hash` | Admin sets a single-write 32-byte digest. |
| `append_attestation_digest` | Admin appends to bounded audit log. |
| `record_sme_collateral_commitment` | SME records collateral pledge (metadata only). |
| `get_escrow` | Read current escrow state. |
| `get_version` | Read stored `DataKey::Version`. |
| `check_escrow_health` | Read-only health check: returns `(warning_type, funded_ratio_bps, time_to_maturity_secs)` for risk monitoring. |
| `get_escrow_health` | Formatted health summary with warning label, percentage, and recommendations. |
| `export_state` | Admin serializes all enumerable instance-storage state into an `EscrowSnapshot` (disaster recovery / migration). |
| `import_state` | Admin restores instance-storage state from an `EscrowSnapshot` onto a fresh, uninitialized instance. |

   ```bash
   cd sdk-ts
   npm install
   npm run build
   npm run example
   ```

5. **Run the backend** — start the indexer + API against the local chain
   (see [`backend/`](backend/) for its own setup and env vars).

6. **Run the frontend** — point it at the backend API and open it in a browser
   (see [`frontend/`](frontend/) for its own setup and env vars).

A new contributor can follow steps 1–6 in order to run the full stack locally.

---

## FAQ

**How do I run the tests?**
Run `cargo test` from the repository root to execute the full workspace test suite, or `cargo test -p karis-ky_escrow` to target the escrow crate only. CI mirrors these commands, so a green local run is a good pre-push signal.
See also: [Quick start](#quick-start) and [`escrow/`](escrow/).

**What is the current schema version?**
The authoritative version is the `SCHEMA_VERSION` constant in `escrow/src/lib.rs`, stored on-chain under `DataKey::Version` at `init`. The current value is `7`; production instances should match the deployed WASM.
See also: [Schema version changelog](#schema-version-changelog-datakeyversion).

**Which tokens are supported?**
The escrow binds a single funding token at `init` via the `FundingToken` key, so any SEP-41 / Soroban-compatible token contract can be used. The token is fixed for the lifetime of the escrow and cannot be changed after initialization.
See also: [Escrow init parameters](docs/escrow-init-parameters.md).

**How do I perform a legal hold?**
Without a configured guardian, the admin calls `set_legal_hold` to activate or clear a compliance hold. With a guardian, the admin calls `propose_legal_hold` and the guardian calls `confirm_legal_hold` to activate; the admin still clears the hold. Legal hold is distinct from a dispute pause and is coordinated per the operator runbook.
See also: [`docs/OPERATOR_RUNBOOK.md`](docs/OPERATOR_RUNBOOK.md).

**What happens if the admin key is lost?**
Admin-gated operations (legal hold, dispute pause, attestations, cloning) become unavailable, so admin keys should be held in governance multisig / custody. Recovery requires redeploying or re-initializing under a new admin per the runbook; there is no on-chain key-recovery path.
See also: [Release runbook: build, deploy, verify](#release-runbook-build-deploy-verify).

**What is a dispute pause and how does it differ from a legal hold?**
`pause_dispute` temporarily freezes an escrow due to a dispute, and `resume_dispute` (or auto-expiration) lifts it; `is_dispute_paused` / `get_dispute_pause` report the state. It is a separate mechanism from `set_legal_hold`, which is a compliance control.
See also: [Escrow contract — public entrypoints](#escrow-contract--public-entrypoints).

**How is the maturity date enforced?**
`settle` requires SME auth and enforces the invoice maturity date, so a funded escrow cannot be settled before maturity. Maturity is set at `init` and is part of the stored `InvoiceEscrow`.
See also: [Escrow init parameters](docs/escrow-init-parameters.md).

**How do yield tiers work?**
`fund_with_commitment` records the first deposit with an optional lock period and selects a tiered yield from the `YieldTierTable`. Per-investor effective yield is stored under `InvestorEffectiveYield` and used at claim time.
See also: [Escrow fund parameters](docs/escrow-fund-parameters.md).

**How do I migrate an existing escrow?**
Call `migrate(from_version)`, which currently emits typed errors on all paths (codes 90–92) and has no silent migration path to version 6. Additive-key upgrades need no `migrate` call, but struct-layout changes require a redeploy or an explicit migration implementation.
See also: [`migrate` entrypoint — typed error semantics](#migrate-entrypoint--typed-error-semantics).

**How do I set up the REPL / CLI?**
Source `scripts/local-env.sh` to spin up a local Soroban validator, identities, test token, and a deployed contract in one command. Then use the Stellar CLI (or the TypeScript SDK) to interact with the deployed contract.
See also: [Local development (one-command)](#local-development-one-command) and [TypeScript SDK](#typescript-sdk).

---

## Prerequisites

- The Rust toolchain pinned in [`rust-toolchain.toml`](rust-toolchain.toml) (see
  [Supported Rust toolchain](#supported-rust-toolchain))
- `wasm32v1-none` target for WASM builds: `rustup target add wasm32v1-none`
- Soroban / Stellar CLI (optional — for deployment and contract interaction)

For local development and CI, Rust alone is sufficient.

### Supported Rust toolchain

The supported range is the channel pinned in [`rust-toolchain.toml`](rust-toolchain.toml)
and `stable`:

- **Pinned channel** — the exact toolchain declared in `rust-toolchain.toml`. `rustup`
  selects it automatically when you work in this repository, so local builds match CI.
- **`stable`** — the latest stable release, which contributors on a freshly updated
  toolchain will typically have.

CI runs `cargo fmt --check`, `cargo clippy`, and `cargo test` on **both** channels via a
matrix build, so a regression on either channel surfaces as a failing job. Nightly-only
features are out of scope.

Bumping the minimum supported Rust version (MSRV) is a deliberate change: open an issue
and discuss it before raising the pinned channel or relying on newer language features.

### SDK Examples

Common integration patterns are demonstrated in [`examples/basic_workflow.rs`](examples/basic_workflow.rs):
init, fund, settle, claim, tiered yield, oracle settlement, and NFT minting workflows.

---

## Quick start

```bash
cargo build
cargo test
```

### Local development (one-command)

```bash
source scripts/local-env.sh
```

This sets up a complete local Soroban environment — validator, identities,
test token, and deployed contract — ready for development. See
[scripts/local-env.sh](scripts/local-env.sh) for details.

### TypeScript SDK

```bash
cd sdk-ts
npm install
npm run build
npm run example
```

See [`sdk-ts/`](sdk-ts/) for the typed client wrapper, contract types, and example usage.

---

## Schema version changelog (`DataKey::Version`)

The `SCHEMA_VERSION` constant in `escrow/src/lib.rs` is stored on-chain under
`DataKey::Version` at [`init`] and is the

/* … truncated 2889 chars — edit only what you need near the top … */
