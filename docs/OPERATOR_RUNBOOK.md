# karis-ky Operator Runbook: Redeploy vs. On-Chain Upgrade

> **Scope:** Stellar / Soroban only. This runbook does not apply to EVM or
> Solidity deployments. CLI examples use the `stellar` CLI; verify flag syntax
> against your installed version via `stellar --version`.

---

## 1. Decision tree — redeploy vs. on-chain WASM upgrade

On Stellar/Soroban, "upgrading" a contract means preserving the contract ID and
stored ledger entries while switching the instance to new WASM bytecode. The
standard contract pattern is an admin-gated entrypoint that calls
`env.deployer().update_current_contract_wasm(new_wasm_hash)`. If the deployed
contract does not expose that upgrade entrypoint, operators must redeploy a new
contract instance instead.

```
Does the existing instance expose an admin-gated WASM upgrade entrypoint?
│
├─ NO  → REDEPLOY (new contract address, new init)
│
└─ YES → Did InvoiceEscrow or any stored #[contracttype] XDR shape change?
          │
          ├─ YES → REDEPLOY (new contract address, new init)
          │         └─ Reason: stored XDR must decode against the new WASM's
          │                    contract types; layout changes are breaking.
          │
          └─ NO  → Is the change only new DataKey variants read with defaults?
                    │
                    ├─ YES → WASM UPGRADE IN PLACE. Do not call migrate().
                    │
                    └─ NO  → Does the change require rewriting existing storage?
                              │
                              ├─ YES → Extend migrate() first, test it, then upgrade
                              │         and call migrate() with the stored version.
                              │
                              └─ NO  → WASM UPGRADE IN PLACE, after code review.
```

> **Key Soroban difference from EVM:** there is no `delegatecall`-style proxy
> pattern in this contract. A same-address upgrade preserves stored data and
> runs the new WASM against that data. A stored type layout change is therefore
> a breaking storage change unless an explicit, tested migration can decode the
> old data shape and rewrite it safely.

---

## 2. `SCHEMA_VERSION` lifecycle

`SCHEMA_VERSION` (defined in `escrow/src/lib.rs`) and `DataKey::Version` track
the storage schema independently of the WASM binary version.

| Action | `DataKey::Version` | `SCHEMA_VERSION` in WASM |
|--------|--------------------|--------------------------|
| Fresh `init` | Written to `SCHEMA_VERSION` | Same |
| Additive-only WASM upgrade | Unchanged (old value stays) | New WASM constant |
| Layout-breaking change + redeploy + new `init` | Written to new `SCHEMA_VERSION` | Same |
| Operator calls `migrate()` after extending it | Updated by `migrate` to new version | Same |

### When to bump `SCHEMA_VERSION`

Bump `SCHEMA_VERSION` when **any** of the following is true:

- You change the XDR shape of `InvoiceEscrow`, `SmeCollateralCommitment`,
  `FundingCloseSnapshot`, `YieldTier`, or any other `#[contracttype]` struct
  stored at an existing key.
- You remove or rename an existing `DataKey` variant that live instances use.
- You change the semantic meaning of an existing stored value in a backward-
  incompatible way.

Do **not** bump `SCHEMA_VERSION` for:

- Adding a new `DataKey` variant read with `.get(...).unwrap_or(default)`.
- Adding a new `#[contracttype]` stored at a new key.
- Behavioral changes that do not touch stored state.

### Changelog-based transition classification

This table is derived from the `SCHEMA_VERSION` changelog in
`escrow/src/lib.rs`. It is the operator classification for historical
transitions through version 5.

| Transition | Changelog source | Operator action |
|------------|------------------|-----------------|
| Fresh deploy → 1 | Initial schema (`InvoiceEscrow` v1, basic fund / settle) | Fresh `init`; no migration exists before v1. |
| 1 → 2 | Added `InvestorEffectiveYield`, `InvestorClaimNotBefore` | Additive. No `migrate()` call required when readers default missing values. |
| 2 → 3 | Added `FundingCloseSnapshot`, `MinContributionFloor`, `MaxUniqueInvestorsCap`, `UniqueFunderCount` | Additive. No `migrate()` call required when readers default missing values. |
| 3 → 4 | Added `PrimaryAttestationHash`, `AttestationAppendLog` | Additive. No `migrate()` call required. |
| 4 → 5 | Added `YieldTierTable`, `RegistryRef`, `Treasury`; tightened `InvoiceEscrow` layout | Conditional. Additive keys alone need no `migrate()`, but any `InvoiceEscrow` XDR/layout change is breaking and requires redeploy. |
| 5 → 6 | Per-investor keys moved to persistent storage | Breaking for existing instances. Redeploy required because prior per-investor instance keys are address-keyed and not enumerable by the contract. |

Operational rule: additive keys are safe only when old instances can read them
with explicit defaults. Changing, renaming, or removing an existing key or
changing an existing stored type's XDR shape is not additive.

### Implementing a real migration in `migrate()`

```rust
pub fn migrate(env: Env, from_version: u32) -> u32 {
    // Keep this guard first. Current code already requires admin auth before
    // version checks so every future storage rewrite is admin-gated.
    Self::get_escrow(env.clone()).admin.require_auth();

    let stored: u32 = env.storage().instance().get(&DataKey::Version).unwrap_or(0);
    ensure(
        &env,
        stored == from_version,
        EscrowError::MigrationVersionMismatch,
    );

    if from_version >= SCHEMA_VERSION {
        fail(&env, EscrowError::AlreadyCurrentSchemaVersion)
    }

    // Example pattern for a future same-instance migration:
    if from_version == 6 && SCHEMA_VERSION == 7 {
        // 1. Read only old-version data that the new WASM can still decode.
        // 2. Validate arithmetic with checked_* operations.
        // 3. Write new keys or rewritten values exactly once.
        // 4. Write DataKey::Version last.
        env.storage().instance().set(&DataKey::Version, &7u32);
        return 7;
    }

    fail(&env, EscrowError::NoMigrationPath)
}
```

Step-by-step implementation requirements for a real migration:

1. Bump `SCHEMA_VERSION` in `escrow/src/lib.rs`.
2. Add the migration branch above the terminal `EscrowError::NoMigrationPath`.
3. Keep `Self::get_escrow(env.clone()).admin.require_auth()` before all version
   checks and before every storage write.
4. Read `DataKey::Version` and require `stored == from_version`.
5. For each migrated value, prove the new WASM can decode the old stored value;
   otherwise redeploy instead of migrating in place.
6. Use checked arithmetic for transformed numeric values and keep writes
   bounded. The migration should be O(number of explicitly supplied or
   enumerable keys); do not design a migration that assumes contract storage can
   enumerate all investors.
7. Write all transformed state before writing `DataKey::Version`.
8. Write `DataKey::Version` last and return the new version.
9. Add unit tests for version mismatch, already-current version, unauthorized
   caller, the successful migration path, and repeated calls after success.
10. Update this runbook, the rustdoc changelog, and any affected read/API docs.

**Current state (v6):** `migrate()` fails with typed contract errors on **all**
paths. No migration work is implemented. The entrypoint is admin-gated before
version checks so any future storage-mutating migration path is authenticated by
construction. See [ADR-007](adr/ADR-007-storage-key-evolution.md) for the
storage-key evolution policy. Operators must redeploy if `InvoiceEscrow` layout
changes.

### Current `migrate()` panic policy

This table must match the `migrate` rustdoc in `escrow/src/lib.rs`.

| Condition | Typed error |
|-----------|-------------|
| `stored_version != from_version` | `EscrowError::MigrationVersionMismatch` |
| Migration already applied; replay detected | `EscrowError::MigrationAlreadyApplied` |
| `from_version >= SCHEMA_VERSION` | `EscrowError::AlreadyCurrentSchemaVersion` |
| Any `from_version < SCHEMA_VERSION` without an implemented migration branch | `EscrowError::NoMigrationPath` |

Because Soroban aborts the transaction on contract panic, these errors perform
no storage writes in the current release. Operators must not call `migrate()` as
a bookkeeping step after additive upgrades.

### Migration idempotency and replay protection

**Starting from schema version 7**, the `migrate()` entrypoint enforces idempotency via a
stored nonce [`DataKey::MigrationExecutionLog`]:

- After a successful migration, a nonce is written to prevent re-application of the same
  version transition to the same instance.
- If you call `migrate(from_version)` twice, the second call fails with 
  [`EscrowError::MigrationAlreadyApplied`] (code 93).
- This is a safety measure to prevent double-application of state transformations
  (e.g., yield recalculations, per-investor claim locks, collateral audits).

**Operator action:** Do not retry a `migrate()` call if you receive error code 93. Instead:

1. Verify the on-chain version with `get_version()`.
2. If it matches your target, the migration succeeded.
3. If it does not, investigate the first call's failure reason and contact governance.

**Developer action:** When implementing a migration path (v6 → v7, etc.):

```rust
if from_version == 6 && SCHEMA_VERSION == 7 {
    // 1. Perform state transformation
    // ... your migration logic ...
    
    // 2. Write new version (must be last state write)
    env.storage().instance().set(&DataKey::Version, &7u32);
    
    // 3. Write idempotency nonce in same transaction (prevents replay)
    env.storage().instance().set(
        &DataKey::MigrationExecutionLog(6, 7),
        &env.ledger().sequence(),
    );
    
    // 4. Optional: Record completion timestamp for audit trails
    env.storage().instance().set(
        &DataKey::MigrationCompletedAt(6, 7),
        &env.ledger().timestamp(),
    );
    
    // 5. Return new version
    return 7;
}
```

---

## 3. Pre-flight checklist (testnet → mainnet)

Complete all items before promoting to Mainnet.

### Read-only RPC pre-flight

Use `scripts/pre-flight-checklist.sh` for the read-only contract checks. The
independent checks run concurrently and their results are printed in a stable
order after every started request has completed. A failed, timed-out, malformed,
or non-successful RPC response makes the script exit non-zero.

Prerequisites: Bash, `stellar`, `curl` as required by the installed CLI,
`jq` for downstream inspection, and the GNU `timeout` utility. Configure the
Stellar CLI network first, then run:

```bash
export STELLAR_NETWORK=testnet
export CONTRACT_ID=C...
bash scripts/pre-flight-checklist.sh
```

The default network is `local`. Set `CHECK_TIMEOUT_SECS` to override the
30-second timeout per query. The script uses the configured named network and
does not print or accept signing secrets; these checks are read-only. Keep
dependent checks outside the parallel batch if future checks require output
from an earlier query.

### Build & verify

```bash
# 1. Add WASM target
rustup target add wasm32v1-none

# 2. Build release WASM
cargo build --target wasm32v1-none --release -p karis-ky_escrow

# 3. Format check
cargo fmt --all -- --check

# 4. Lint (zero warnings)
cargo clippy -p karis-ky_escrow -- -D warnings

# 5. Full test suite
cargo test -p karis-ky_escrow

# 6. Coverage gate (≥ 95% lines)
cargo llvm-cov \
  --features testutils \
  --fail-under-lines 95 \
  --summary-only \
  -p karis-ky_escrow

# 7. Confirm WASM artifact exists
ls target/wasm32v1-none/release/karis-ky_escrow.wasm
```

### Contract security checklist

- [ ] `admin` is a multisig or governed contract (not an EOA alone).
- [ ] `funding_token` is a standard SEP-41 token (no fee-on-transfer).
- [ ] `treasury` address is controlled by karis-ky governance.
- [ ] `invoice_id` matches off-chain invoice slug (ASCII alphanumeric + `_`,
      max 32 chars).
- [ ] `maturity` is set in ledger timestamp seconds (not wall-clock oracle).
- [ ] `min_contribution` and `max_unique_investors` match legal offering
      documents.
- [ ] Legal hold (`set_legal_hold`) procedure is documented in ops playbook.
- [ ] Attestation digests and their canonical off-chain encoding are
      documented.
- [ ] Instance TTL is monitored for every active escrow, and `bump_ttl` is
  scheduled before expiry to preserve instance state, including attestation
  records. See [TTL semantics and operational `bump_ttl`](escrow-gas-storage-notes.md).
- [ ] CI passes: format, clippy, tests, coverage ≥ 95%.

### Testnet smoke test

```bash
export STELLAR_NETWORK=testnet
export SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
export SOURCE_SECRET=S...          # deployer secret key
export LIQUIFACT_ADMIN_ADDRESS=G...

# Upload WASM
stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK

# Deploy instance
stellar contract deploy \
  --wasm-hash <WASM_HASH_FROM_UPLOAD> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK

# Call init (example — adjust params to your invoice)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- init \
  --admin $LIQUIFACT_ADMIN_ADDRESS \
  --invoice_id INV001 \
  --sme_address G... \
  --amount 10000000000 \
  --yield_bps 800 \
  --maturity 0 \
  --funding_token C... \
  --registry null \
  --treasury G... \
  --yield_tiers null \
  --min_contribution null \
  --max_unique_investors null

# Verify stored version matches SCHEMA_VERSION (should return 6)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- get_version
```

---

## 4. WASM upgrade in place (additive-only changes)

Use this path only when no `#[contracttype]` stored struct layout has changed
and the currently deployed contract exposes an admin-gated upgrade entrypoint
that calls `env.deployer().update_current_contract_wasm(new_wasm_hash)`.

```bash
# Step 1: Upload new WASM (get new hash)
stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK

# Step 2: Invoke the deployed contract's upgrade entrypoint, if present.
# The current karis-ky escrow contract does not expose this entrypoint.
stellar contract invoke \
  --id <EXISTING_CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- upgrade --new_wasm_hash <NEW_WASM_HASH>

# Step 3 (optional): Call migrate() only if you implemented a migration path.
# In the current release, migrate() panics — do NOT call it unless extended.
# stellar contract invoke --id <CONTRACT_ID> ... -- migrate --from_version 4
```

> **Soroban note:** `env.deployer().update_current_contract_wasm` replaces the
> WASM for the contract at the current ID. The stored instance data is
> preserved. Old XDR is decoded against the **new** WASM types on the next read.

---

## 5. Redeploy (layout-breaking changes)

When `InvoiceEscrow` struct or any stored `#[contracttype]` changes XDR shape,
the only safe path is a fresh deploy.

```bash
# 1. Build and upload new WASM (as above).

# 2. Deploy new contract instance — this gets a new contract ID.
stellar contract deploy \
  --wasm-hash <NEW_WASM_HASH> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK
# → prints NEW_CONTRACT_ID

# 3. Call init on the new instance.
stellar contract invoke \
  --id <NEW_CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- init ...

# 4. Migrate off-chain state (investor records, indexer pointers) to new contract ID.
# 5. Retire old contract: set legal hold, then archive off-chain reference.
```

**The old contract instance is NOT deleted on-chain** — Soroban does not
support contract destruction. Operators must:

- Communicate the new contract ID to all integrators and indexers.
- Ensure no new funding flows reach the old contract (update integrator configs
  before announcing the migration).
- Keep legal hold active on the old contract if it has live principal.

---

## 6. Rollback protocol

There is **no automatic on-chain downgrade** path on Soroban. If a WASM upgrade
introduces a bug, recovery still requires an available admin-gated upgrade
entrypoint on the deployed contract:

```
Option A (safest): Re-upload previous WASM, invoke the contract's upgrade
                   entrypoint back to the old hash. Works only if stored data
                   is still compatible with old WASM types.

Option B (layout-broken): Redeploy from old WASM hash (already uploaded).
                           Migrate investor positions off-chain.

Option C (emergency): Activate legal hold on the broken contract to block
                      payouts and settlement. Communicate status to investors.
                      Proceed with Option A or B after root cause is confirmed.
```

```bash
# Option A — revert WASM through the contract's upgrade entrypoint
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- upgrade --new_wasm_hash <PREVIOUS_WASM_HASH>

# Emergency hold (before investigating)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source $SOURCE_SECRET \
  --network $STELLAR_NETWORK \
  -- set_legal_hold --active true
```

---

## 7. Legal hold coordination during upgrade windows

1. **Before** uploading new WASM: activate legal hold on any live escrow
   instance that will be upgraded, to block in-flight settlement or claims.

   ```bash
   stellar contract invoke --id <ID> ... -- set_legal_hold --active true
   ```

2. **Perform** the WASM upload and, if the contract supports it, invoke the
   admin-gated upgrade entrypoint.

3. **Verify** the upgraded contract: call `get_version`, `get_escrow`, and
   run smoke tests on Testnet mirror.

4. **Clear** legal hold once you are satisfied the upgrade is correct.

   ```bash
   stellar contract invoke --id <ID> ... -- clear_legal_hold
   ```

> **Important:** `clear_legal_hold` requires the **same `admin`** that set it.
> If admin was rotated during the upgrade, the new admin must call it. There is
> no bypass or timelock in the current contract — operational playbooks must
> ensure admin continuity.

---

## 8. Interaction Matrix: Legal Hold vs. Dispute Pause

The escrow contract implements two independent risk-control overlays:

- **Legal hold** (`DataKey::LegalHold`): a persistent boolean compliance gate
  controlled by the admin, with no automatic expiry.
- **Dispute pause** (`DataKey::DisputePause`): a time-bounded administrative
  freeze with an auto-expiry based on ledger time.

Both controls block overlapping risk-bearing operations. This section documents
the precedence rules and expected behavior when one or both controls are active.

### Control precedence

**Legal hold is evaluated first.** When both controls are active on the same
operation, the operation returns the legal-hold error without falling through to
the dispute-pause check. This is a stable API contract: future changes to this
precedence require explicit decision-making and must be documented before
deployment.

### Operation matrix: all four combinations

| State | `fund` / `fund_with_commitment` | `settle` | `withdraw` | `claim_investor_payout` | `sweep_terminal_dust` |
|---|---|---|---|---|---|
| **Both hold ✓ + pause ✓** | ❌ `LegalHoldBlocksFunding` (102) | ❌ `LegalHoldBlocksSettlement` (120) | ❌ `LegalHoldBlocksWithdrawal` (123) | ❌ `LegalHoldBlocksInvestorClaims` (125) | ❌ `LegalHoldBlocksTreasuryDustSweep` (30) |
| **Hold ✓ + pause ✗** | ❌ `LegalHoldBlocksFunding` (102) | ❌ `LegalHoldBlocksSettlement` (120) | ❌ `LegalHoldBlocksWithdrawal` (123) | ❌ `LegalHoldBlocksInvestorClaims` (125) | ❌ `LegalHoldBlocksTreasuryDustSweep` (30) |
| **Hold ✗ + pause ✓** | ❌ `DisputePausedBlocksFunding` (165) | ❌ `DisputePausedBlocksSettlement` (166) | ❌ `DisputePausedBlocksWithdrawal` (167) | ❌ `DisputePausedBlocksInvestorClaims` (168) | ✅ Allowed (dispute pause does not gate treasury operations) |
| **Hold ✗ + pause ✗** | ✅ (if other preconditions met) | ✅ (if other preconditions met) | ✅ (if other preconditions met) | ✅ (if other preconditions met) | ✅ (if other preconditions met) |

**Key observations:**

- **Dispute pause does not block dust sweep.** This design allows treasury
  operations to proceed even during dispute resolution. If you need to prevent
  treasury operations during a dispute, activate a legal hold instead.
- **Once one control is cleared, the other remains effective.** Clearing legal
  hold while a dispute pause is active does not unblock operations; they remain
  blocked by the `DisputePausedBlocksX` error. Conversely, resuming or expiring
  the dispute pause does not unblock operations if legal hold is still active.
- **Independent activation and clearing.** Activating hold or pause in either
  order produces the same final state: both are active. Similarly, clearing them
  in either order produces the same state. Clearing one never clears the other.

### Clearing mechanism and edge cases

**Legal hold clearing:**
- Admin calls `set_legal_hold(false, reason)` directly.
- If a clear-delay is configured (set at init via
  `legal_hold_clear_delay_secs`), the admin must first call
  `request_clear_legal_hold()` and wait until the ledger timestamp reaches the
  returned `clearable_at` boundary before calling `set_legal_hold(false, ...)`.
- Clearing legal hold does **not** affect an active dispute pause.
- If the admin key is compromised while legal hold is active, governance must
  execute `propose_admin` and `accept_admin` to rotate the admin, then the new
  admin can call `set_legal_hold(false, ...)`.

**Dispute pause clearing:**
- Admin calls `resume_dispute(reason)` to manually clear the pause before expiry.
- **Or** the pause expires automatically when `ledger.timestamp() >= expires_at`.
- Resuming or expiring the dispute pause does **not** affect an active legal
  hold.
- The expiry boundary is checked every time an operation is attempted; a pause
  is considered active if `now < expires_at`, and inactive if `now >= expires_at`.

**Example scenario:**

```
1. Escrow is open and funded.
2. Admin activates legal hold for compliance review:
   set_legal_hold(true, "compliance review INV-002")
3. Admin also activates dispute pause due to a dispute ticket:
   pause_dispute("TICKET-456", 86400)  // 24 hours
4. Attempt settle → blocked by LegalHoldBlocksSettlement (120)
5. After dispute is resolved, admin calls resume_dispute:
   → Pause is now inactive
6. Attempt settle → still blocked by LegalHoldBlocksSettlement (120)
7. Governance approves and admin clears legal hold:
   set_legal_hold(false, "compliance review complete")
8. Attempt settle → succeeds (assuming maturity and other preconditions met)
```

### Monitoring and alerting

Operators should set up alerts for:

- **Simultaneous activation:** if both `LegalHoldChanged` and `DisputePausedEvt`
  events occur within a short time window (e.g., < 5 minutes), escalate to the
  compliance and engineering teams to ensure the activation was intentional.
- **Prolonged hold + pause combo:** if both controls are active for > 7 days,
  trigger an automated review of the support ticket and hold reason.
- **Orphaned pause after hold clear:** if a dispute pause remains active after
  legal hold is cleared, ensure the pause is either manually resolved or allowed
  to expire; do not leave a paused escrow in production without active
  governance review.

See [`docs/DEPLOYER_SECURITY.md`](DEPLOYER_SECURITY.md) §3.1 for event
monitoring setup.

---

## 9. Security notes for operators

### Token economics (out of scope)

`escrow/src/external_calls.rs` explicitly documents that **fee-on-transfer,
rebasing, and hook tokens are out of scope**. The post-transfer balance-equality
assertions will `panic!` (safe failure) if the token does not conform to
standard SEP-41 behavior. Governance must vet any token contract before it is
used as `funding_token` in an escrow instance.

### No EVM proxy patterns

This contract does not implement a proxy pattern (no `delegatecall` equivalent
on Soroban). Same-address upgrade authority should flow through a contract
entrypoint that requires admin authorization before calling
`env.deployer().update_current_contract_wasm`. The current release does not
expose that entrypoint, so operators should use redeploy for new WASM.

### Admin key hygiene

- Use a multisig wallet or a governed contract as `admin` at all times.
- Never use a single-signer hot wallet as `admin` in production.
- Admin rotation is two-step: `propose_admin` requires the current admin's
  authorization, and `accept_admin` requires the proposed successor's
  authorization. Test both steps on Testnet before executing on Mainnet.

### `migrate()` is not a no-op

Calling `migrate()` with a mismatched `from_version` **panics and aborts the
transaction**. This is intentional — it prevents operators from accidentally
skipping version validation. Do not script automated `migrate()` calls without
first implementing the migration path.

---

## 10. Version compatibility matrix

| WASM version (SCHEMA_VERSION) | Can read data from | Notes |
|-------------------------------|-------------------|-------|
| 6 | 6 | Same version — fully compatible |
| 6 | 5 | Requires redeploy for per-investor key relocation; no in-place migration path |
| 6 | ≤4 | Only with an explicit migration path or redeploy; new optional keys absent → defaults when compatible |
| ≤5 reading 6 data | ❌ | Older WASM reads per-investor accounting from instance storage |

---

## 11. Glossary

| Term | Meaning in this context |
|------|------------------------|
| WASM upload | `stellar contract upload` — publishes bytecode to network; returns a hash |
| WASM upgrade | Admin-gated contract entrypoint calling `env.deployer().update_current_contract_wasm` for an existing contract ID |
| Redeploy | Deploy a **new** contract instance; old instance is not migrated automatically |
| `DataKey::Version` | On-chain stored schema version set by `init` and updated by `migrate` |
| `SCHEMA_VERSION` | Compile-time constant in WASM; the target version for `init` and migration |
| Legal hold | Admin-set flag that blocks settlement, withdrawal, and investor claims |
| SEP-41 | [Stellar token interface standard](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md) |
