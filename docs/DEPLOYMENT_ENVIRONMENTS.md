# Deployment Environments

> **Audience:** karis-ky protocol operators, release engineers, governance multisig signers.
>
> **Companion docs:** [`OPERATOR_RUNBOOK.md`](OPERATOR_RUNBOOK.md) (post-deploy operations),
> [`UPGRADE_CHECKLIST.md`](UPGRADE_CHECKLIST.md) (upgrade procedure), [`docs/adr/ADR-009`](adr/ADR-009-per-investor-persistent-storage.md) (v5 → v6 redeploy),
> [`scripts/deploy.sh`](../scripts/deploy.sh) + [`scripts/verify_deployment.sh`](../scripts/verify_deployment.sh).

This document describes how to deploy the karis-ky escrow contract to the three
target environments — **testnet**, **staging**, and **mainnet** — using the
config files in the repository root. It enumerates the differences between
environments, the pre-flight checklist per environment, and the rollback
procedure per environment.

---

## Quick reference

| Environment | Config file | Network | RPC URL | Was it the real money? |
|-------------|------------|---------|---------|------------------------|
| Testnet | [`.env.testnet`](../.env.testnet) | `testnet` | `https://soroban-testnet.stellar.org` | No |
| Staging | `.env.staging` | `staging` | internal staging RPC | No (mirror topology) |
| Mainnet | `.env.mainnet` | `mainnet` | `https://soroban-mainnet.stellar.org` | **Yes** |

Schema version: **7** — must match `pub const SCHEMA_VERSION` in `escrow/src/lib.rs`. Interface version: **2**.

---

## Environment configuration files

Each `.env.<network>` file in the repository root is loaded by `scripts/deploy.sh` via the `--env` flag. All three files share the same shape; only the network-specific values differ.

```text
STELLAR_NETWORK        # Network name passed to the Stellar CLI
SOROBAN_RPC_URL        # Soroban JSON-RPC endpoint
NETWORK_PASSPHRASE     # Stellar network passphrase (used in transaction signing)
SOURCE_SECRET          # Stellar secret key (S...) of the funded deployer
DEPLOYER_ADDRESS       # Stellar public key (G...) matching SOURCE_SECRET
WASM_TARGET            # Cargo build target (wasm32v1-none for Soroban)
WASM_PATH              # Path to the built WASM artifact
EXPECTED_SCHEMA_VERSION# SCHEMA_VERSION the verifier expects to read back
EXPECTED_*             # Optional: initial-state fields the verifier cross-checks
```

The deployer **must** update `SOURCE_SECRET` and `DEPLOYER_ADDRESS` before
running `scripts/deploy.sh --env .env.<network>`. Testnet keys can be funded
via `stellar keys generate <name> --network testnet --fund`. **Never** reuse a
mainnet secret in any other environment.

---

## Environment differences

### `testnet`

- **Network passphrase:** `Test SDF Network ; September 2015`
- **RPC URL:** `https://soroban-testnet.stellar.org`
- **Fund source:** Friendbot (`curl https://friendbot.stellar.org?addr=<G...>`) or the Stellar CLI's `--fund` flag.
- **Token contract:** Any SEP-41 token deployed on testnet (commonly a `stellar contract asset deploy --asset native` wrapper).
- **Admin role:** A **single** test signer key is acceptable for development. Do **not** use a real key.
- **Failure cost:** Zero. Testnet XLM has no monetary value.
- **Lifecycle:** Reset weekly by the SDF. Plan deployments and tests within a single week; do not assume long-lived storage across resets.

### `staging`

- **Network passphrase:** `Staging Network ; March 2026` (or whatever the internal staging passphrase is for your deployment).
- **RPC URL:** Internal staging RPC (typically `https://soroban-staging.internal.example.com`).
- **Fund source:** Treasury hot wallet (separate from mainnet treasury).
- **Token contract:** A staging token contract whose metadata mirrors the production token. Funding tokens are **not** real money but should behave like production tokens (decimals, transfer semantics).
- **Admin role:** The **production admin multisig** but with staging-specific signers. The signing set is the same shape as production but the keys are different.
- **Failure cost:** Operational only (engineer hours). No real money at risk.
- **Lifecycle:** Persistent. State survives operator decisions; no automatic reset.

### `mainnet`

- **Network passphrase:** `Public Global Stellar Network ; September 2015`
- **RPC URL:** `https://soroban-mainnet.stellar.org`
- **Fund source:** Mainnet treasury multisig. **Never** use a single-key wallet.
- **Token contract:** The production funding token (e.g. USDC on Stellar).
- **Admin role:** Protocol governance multisig. Threshold **must** be ≥ 2-of-3 in production.
- **Failure cost:** **Real money**. Every action must be reviewed, signed by the required threshold, and recorded.
- **Lifecycle:** Permanent. State on mainnet is irreversible.

### What changes between environments (summary)

| Setting | testnet | staging | mainnet |
|---------|---------|---------|---------|
| Network passphrase | `Test SDF Network ; September 2015` | staging passphrase | `Public Global Stellar Network ; September 2015` |
| RPC URL | public Soroban testnet | internal staging RPC | public Soroban mainnet |
| Funding source | Friendbot | staging treasury | mainnet treasury multisig |
| Token contract | testnet asset wrapper | staging token | production token |
| Admin role | single test key | staging multisig (separate keys) | production multisig (same shape, different keys) |
| Failure cost | none | operational | real money |
| State persistence | ~1 week | indefinite | indefinite |

What **does not** change between environments:

- The WASM artifact (the same compiled binary is deployed to all three).
- `SCHEMA_VERSION` (8) and `CONTRACT_INTERFACE_VERSION` (1) — both are source-code constants.
- The escrow init parameter shape.
- The deploy script (`scripts/deploy.sh`) and the verifier (`scripts/verify_deployment.sh`).

---

## Pre-flight checklist per environment

### Testnet

- [ ] `WASM_PATH` points at a fresh build (`cargo build --target wasm32v1-none --release -p karis_ky_escrow`).
- [ ] `SOURCE_SECRET` / `DEPLOYER_ADDRESS` are funded testnet keys (`stellar keys address <name>` shows a non-empty balance).
- [ ] `EXPECTED_SCHEMA_VERSION` matches the source constant.
- [ ] Optional: `EXPECTED_ADMIN`, `EXPECTED_INVOICE_ID`, `EXPECTED_FUNDING_TOKEN` are filled in for state verification.
- [ ] `scripts/check_wasm_size.py` passes against the artifact.
- [ ] `EXPECTED_INVOICE_ID` is unique to this test (avoid colliding with existing test escrows).

### Staging

- [ ] All testnet checks pass.
- [ ] The staging RPC URL is reachable (`curl <RPC_URL>` returns a non-empty response).
- [ ] `DEPLOYER_ADDRESS` is on the staging multisig's allowlist (the staging deployer is a separate identity from the mainnet deployer).
- [ ] The staging funding token contract address is verified (call `get_funding_token` on a known prior deployment or look it up in the staging inventory).
- [ ] The WASM hash is known in advance — capture it before deploying so the verifier's comparison is meaningful.
- [ ] `EXPECTED_INVOICE_ID` is unique to this staging deploy.

### Mainnet

- [ ] All staging checks pass.
- [ ] **Governance approval** is recorded for the deploy (link to the governance proposal or RFC).
- [ ] **All required signers** of the deployer multisig are confirmed available and willing to sign.
- [ ] The deployer multisig's threshold and signer set have **not** changed since the last successful mainnet deploy. If they have, re-confirm with governance before deploying.
- [ ] The funding-token contract address is verified **independently** (e.g. via Stellar Expert, not just from this config file).
- [ ] The admin multisig's address is verified **independently** (the address in this file should be the address that governance has on file).
- [ ] `EXPECTED_INVOICE_ID` is unique to this mainnet deploy and follows the protocol's invoice ID conventions (ASCII alphanumeric + underscore, ≤ 32 chars).
- [ ] A rollback plan is on file (see [Rollback procedure](#rollback-procedure-per-environment) below).
- [ ] An off-chain `backend/backup_escrow_state.py` snapshot of any existing related escrow state is taken before the deploy.
- [ ] The deployer runs the deploy and verifier from a **hardware-secured** session (no copy-paste of secrets to a clipboard shared with other apps).

---

## Deployment procedure

The same procedure applies to all three environments; only the env file differs.

```bash
# 1. Build the WASM (run from the repository root)
cargo build --target wasm32v1-none --release -p karis_ky_escrow

# 2. Check size (fails the build if the artifact exceeds 1 MB or grew >10% vs baseline)
python3 scripts/check_wasm_size.py \
  --wasm target/wasm32v1-none/release/karis_ky_escrow.wasm \
  --baseline scripts/wasm_size_baseline.json

# 3. Deploy (loads the env file)
bash scripts/deploy.sh --env .env.<network>
# Capture the CONTRACT_ID printed by this step.

# 4. Verify (uses the same env file and the new CONTRACT_ID)
CONTRACT_ID=<captured-id> \
EXPECTED_ADMIN=<expected-admin> \
EXPECTED_INVOICE_ID=<expected-invoice-id> \
EXPECTED_FUNDING_TOKEN=<expected-funding-token> \
bash scripts/verify_deployment.sh --env .env.<network>
# Exit code 0 = PASS, 1 = FAIL. CI gates on this.

# 5. Initialize the escrow
# (Use the init CLI recipe from docs/escrow-sim-stellar-cli.md)
```

The deployer **must not** move to step 5 if step 4 reports any failure. A
failed verification is a **release blocker**; do not initialize the escrow
until the verifier passes.

---

## Rollback procedure per environment

### Testnet

Rollback on testnet is trivial because nothing of value is at stake.

1. **Confirm the failure.** Reproduce the failure on the deployed contract (`scripts/verify_deployment.sh` will exit non-zero on the same checks).
2. **Redeploy.** Run `bash scripts/deploy.sh --env .env.testnet` again with a fixed WASM artifact.
3. **Verify.** Run `bash scripts/verify_deployment.sh --env .env.testnet --contract-id <new-id>`.
4. **If the failure persists**, file an issue with the failing command output and the on-chain `get_version` / `get_build_metadata` values.

No further action is needed. Testnet state is reset weekly; the broken instance will disappear on its own.

### Staging

Staging rollback follows the same shape as testnet but with stricter validation.

1. **Confirm the failure.** Document the failing verification step, the on-chain state (call `get_escrow`, `get_build_metadata`, `get_version`), and the expected state from the staging config.
2. **Freeze the broken instance** by calling `set_legal_hold(true)` from the staging admin multisig. This prevents any further state transitions on the broken instance until the issue is understood.
3. **Redeploy** with a fixed WASM artifact: `bash scripts/deploy.sh --env .env.staging`. The new instance gets a new `CONTRACT_ID`.
4. **Verify** the new instance: `bash scripts/verify_deployment.sh --env .env.staging --contract-id <new-id>`.
5. **Decide on the broken instance:**
   - If it was never initialized (no investors, no escrow state), call `set_legal_hold(false)` and leave it archived.
   - If it was initialized and held staging principal, follow the v5 → v6 redeploy pattern from [ADR-009](adr/ADR-009-per-investor-persistent-storage.md): parallel-run with the new instance, migrate investors via standard `fund` calls on the new instance, archive the old instance when terminal.
6. **Post-mortem.** Write a short report covering root cause, time-to-detection, time-to-recovery, and any process changes.

### Mainnet

Mainnet rollback is the highest-stakes procedure. It **must** be coordinated by governance and executed with the multisig threshold required for the deployer key.

1. **Confirm the failure.** Cross-verify the failing check on the deployed contract from **at least two independent RPC endpoints** (e.g. `https://soroban-mainnet.stellar.org` and a self-hosted mirror). Record the on-chain state: `get_version`, `get_build_metadata`, `get_escrow_summary`, `verify_asset_custody`, the current ledger sequence, and the deployed WASM hash.
2. **Open a governance incident.** Notify all multisig signers and the governance council. Pause any pending investor actions (deposits, claims, settlements) on the affected escrow(s) by calling `set_legal_hold(true)` from the admin multisig.
3. **Take an off-chain snapshot** with `backend/backup_escrow_state.py` to preserve the current state for forensic analysis.
4. **Decide on the recovery strategy.** Three options, in order of preference:

   **Option A — Redeploy a fixed WASM (preferred for additive-only bugs).** If the broken WASM is reachable via `upgrade` (the contract exposes `LiquifactEscrow::upgrade(new_wasm_hash)`), governance calls `upgrade` with the fixed WASM hash. The instance retains its `CONTRACT_ID`, state, and event history.
   - Prepare the upgrade transaction, sign with the deployer multisig threshold.
   - Submit the upgrade.
   - Run `scripts/verify_deployment.sh --env .env.mainnet --contract-id <existing-id>`.
   - Clear the legal hold once verification passes.

   **Option B — Redeploy a fresh instance (required for breaking-storage bugs).** If the bug is in storage layout or in a state path that cannot be fixed by upgrading code alone, deploy a fresh instance following the procedure above. Migrate investors per the [v5 → v6 migration pattern](adr/ADR-009-per-investor-persistent-storage.md).

   **Option C — Halt and reassess.** If neither A nor B is safe (e.g. the bug is in token custody math and any code path may have already misbehaved), keep the legal hold in place, notify affected investors, and **do not redeploy** until a full audit has been completed.

5. **For Options A and B**, after recovery, run `scripts/verify_deployment.sh` against the recovered instance and confirm `EXPECTED_*` fields match the staging / mainnet inventory.
6. **Post-mortem.** Publish an incident report including:
   - Timeline (detection → freeze → recovery → resume)
   - Root cause
   - Blast radius (which escrows / investors were affected)
   - Process changes (what guard rail would have caught this earlier)
   - Sign-offs from governance

7. **Update `docs/MAINNET_INVENTORY.md`** (or equivalent) with the new `CONTRACT_ID`, WASM hash, and the deploy timestamp.

---

## Cross-references

- [`scripts/deploy.sh`](../scripts/deploy.sh) — environment-aware deployer
- [`scripts/verify_deployment.sh`](../scripts/verify_deployment.sh) — post-deployment verifier (CI-friendly exit codes)
- [`scripts/check_wasm_size.py`](../scripts/check_wasm_size.py) — artifact size gate
- [`scripts/local-env.sh`](../scripts/local-env.sh) — local Soroban validator setup for development
- [`backend/backup_escrow_state.py`](../backend/backup_escrow_state.py) — off-chain state snapshot for forensic / rollback use
- [`docs/OPERATOR_RUNBOOK.md`](OPERATOR_RUNBOOK.md) — post-deploy operational guide
- [`docs/UPGRADE_CHECKLIST.md`](UPGRADE_CHECKLIST.md) — additive upgrade procedure (Option A above)
- [`docs/adr/ADR-009`](adr/ADR-009-per-investor-persistent-storage.md) — v5 → v6 redeploy pattern (Option B above)
- [`docs/adr/ADR-008`](adr/ADR-008-backup-restore-rejection.md) — why on-chain restore is rejected
- [Stellar Soroban CLI docs](https://developers.stellar.org/docs/tools/soroban-cli/stellar-cli) — CLI reference