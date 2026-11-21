# Escrow Upgrade Checklist — Quick Reference

Use this checklist for every upgrade. Cross off items as you complete them. Keep a copy in your ops runbook.

---

## Pre-Upgrade Phase (Day 0)

### Determine upgrade type

- [ ] Run: `git diff HEAD~1 escrow/src/lib.rs | grep -A20 '#\[contracttype\]'`
- [ ] Result: No struct layout changes detected?
  - **YES** → Proceed with **ADDITIVE** path
  - **NO** → Switch to **REDEPLOY** path

### Build and verify

- [ ] `rustup target add wasm32v1-none`
- [ ] `cargo fmt --all -- --check` ✓ passes
- [ ] `cargo clippy -p karis-ky_escrow -- -D warnings` ✓ passes (zero warnings)
- [ ] `cargo test -p karis-ky_escrow` ✓ all pass
- [ ] `cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow` ✓ >= 95%
- [ ] `ls -lh target/wasm32v1-none/release/karis-ky_escrow.wasm` ✓ exists

### Security and governance approval

- [ ] Security team reviewed WASM diff
- [ ] Governance multisig approved upgrade type (additive / redeploy)
- [ ] Legal confirmed no investor agreement violations
- [ ] All live instances documented in inventory spreadsheet
- [ ] Admin key is multisig or governance contract (never single EOA)
- [ ] Funding token is SEP-41 compliant (no fee-on-transfer)

### Verify instance state

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- get_version
done
```

- [ ] All instances return schema version matching inventory
- [ ] No instances in "unknown" or corrupted state

### Testnet staging

- [ ] Upload WASM to testnet: `stellar contract upload ... --network testnet`
- [ ] Call `get_version` on testnet instance ✓ returns expected schema
- [ ] Call `get_escrow` on testnet instance ✓ no errors
- [ ] Spot-check investor queries on testnet ✓ reads succeed

---

## Additive WASM Upgrade Path (Day 1)

### Upload to mainnet

```bash
stellar contract upload \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source $SOURCE_SECRET \
  --network mainnet
```

- [ ] Upload succeeds
- [ ] Record WASM hash: `_________________`

### Activate legal hold (funded instances only)

```bash
for CONTRACT_ID in $FUNDED_INSTANCES; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- set_legal_hold --active true
done
```

- [ ] Legal hold set on all funded instances
- [ ] Record timestamp: `_________________`

### Propose upgrade (two-step flow, step 1 of 2)

> **Note:** `upgrade` is now gated behind a two-step propose/execute flow with a
> minimum delay. The legacy direct `upgrade --new_wasm_hash` entrypoint must not
> be used; it is deprecated and will be removed at the next interface version bump.

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- propose_upgrade --new_wasm_hash <WASM_HASH>
done
```

- [ ] Proposal invocation succeeds for all instances
- [ ] No timeouts or RPC errors
- [ ] Record proposal timestamp per instance: `_________________`

### Confirm proposal event (before execution)

```bash
# Inspect the Soroban event stream for the propose_upgrade event on each instance.
# Expected event topic: propose_upgrade (with the proposed WASM hash and executable-at ledger/time).
stellar events \
  --id $CONTRACT_ID \
  --network mainnet \
  --start-ledger <PROPOSE_LEDGER>
```

- [ ] `propose_upgrade` event observed for every instance
- [ ] Event payload WASM hash matches the uploaded hash
- [ ] Event payload executable-at time matches the recorded proposal timestamp + minimum delay
- [ ] No unexpected or duplicate proposal events

### Wait out the minimum delay

- [ ] Confirm the minimum delay has elapsed since the proposal timestamp
- [ ] Do **not** attempt `execute_upgrade` before the delay expires (it will fail)
- [ ] Record earliest executable time per instance: `_________________`

### Execute upgrade (two-step flow, step 2 of 2)

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- execute_upgrade
done
```

- [ ] Execution invocation succeeds for all instances
- [ ] No timeouts or RPC errors
- [ ] `execute_upgrade` event observed for every instance
- [ ] Event payload WASM hash matches the proposed hash
=======

### Verify upgrades

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- get_version
done
```

- [ ] All return expected schema version
- [ ] All return same version (no mismatches)

### Spot-check state reads

```bash
# For 2–3 random instances:
stellar contract invoke \
  --id <RANDOM_CONTRACT> \
  --source $SOURCE_SECRET \
  --network mainnet \
  -- get_escrow
```

- [ ] All state reads succeed without errors
- [ ] Escrow status matches pre-upgrade snapshot

### Clear legal hold

```bash
for CONTRACT_ID in $FUNDED_INSTANCES; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- set_legal_hold --active false
done
```

- [ ] Legal hold cleared on all funded instances
- [ ] Record timestamp: `_________________`

### Enable investor funding

- [ ] Notify integrators: funding is live
- [ ] Monitor integrator logs for new deposits

---

## Redeploy Path (Day 1–2)

### Pre-redeploy snapshot

```bash
for CONTRACT_ID in $INSTANCE_IDS; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- get_escrow > snapshot_${CONTRACT_ID}.json
done
```

- [ ] All snapshots captured
- [ ] Verify files contain complete escrow state: `jq '.funded_amount' snapshot_*.json`

### Upload to mainnet

- [ ] (Same as additive path)
- [ ] Record WASM hash: `_________________`

### Deploy new instances (per old instance)

```bash
for OLD_CID in $OLD_INSTANCE_IDS; do
  NEW_CID=$(stellar contract deploy \
    --wasm-hash $WASM_HASH \
    --source $SOURCE_SECRET \
    --network mainnet)
  
  # Init with same parameters from snapshot
  stellar contract invoke --id $NEW_CID ... -- init ...
done
```

- [ ] All new instances deployed
- [ ] All new instances initialized without errors
- [ ] Record mapping: OLD_CID → NEW_CID in migration_log.txt

### Verify new instances

```bash
for NEW_CID in $NEW_INSTANCE_IDS; do
  stellar contract invoke \
    --id $NEW_CID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- get_version
done
```

- [ ] All return new schema version
- [ ] `get_escrow` returns initialized state (no errors)

### Restore investor data (if already funded)

- [ ] Enumerate pre-redeploy contributions from indexer
- [ ] For each investor, call `fund()` or `fund_batch()` on new instance
- [ ] Verify `funded_amount` matches pre-redeploy snapshot

```bash
# Example: fund_batch restore
stellar contract invoke \
  --id $NEW_CID \
  --source $SOURCE_SECRET \
  --network mainnet \
  -- fund_batch --entries '[{"investor":"G...","amount":"100000000"}]'
```

- [ ] All investor contributions restored
- [ ] New instance `funded_amount` == old instance `funded_amount`

### Update integrations

- [ ] Update API: contract ID pointers
- [ ] Update indexer: new contract IDs
- [ ] Update UI: clear caches
- [ ] Notify investors (if required by agreements)
- [ ] Record notification timestamp: `_________________`

### Retire old instances

- [ ] Activate legal hold on old instances (preserve state, block operations)
- [ ] Archive old contract IDs in compliance log
- [ ] Update discovery (API/UI) to point to new IDs only

```bash
for OLD_CID in $OLD_INSTANCE_IDS; do
  stellar contract invoke \
    --id $OLD_CID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- set_legal_hold --active true
done
```

- [ ] Legal hold active on all old instances
- [ ] Confirmed: new instances receive all future funding

---

## Post-Upgrade Monitoring (Day 1–3)

### Immediate (1 hour post-upgrade)

- [ ] Run health check script: `bash health_check.sh` for all instances
- [ ] Confirm all instances return version without errors
- [ ] Spot-check 2–3 investor contribution queries
- [ ] Monitor RPC error logs: **should be 0**
- [ ] Check Soroban event stream: no unexpected contract errors

### 24-hour check

- [ ] Settlement transactions: do any exist? (Call `get_escrow` status)
- [ ] If settled: Investor claim payouts succeed?
- [ ] Attestation digests (if used): still readable and intact?
- [ ] Compare current `get_escrow` output to pre-upgrade snapshot: any deltas?

### 72-hour final audit

- [ ] Run batch health check script on all instances
- [ ] Dump state hashes and diff against day-0 snapshot
- [ ] Confirm no investor principal loss or duplication
- [ ] Confirm legal holds are cleared (additive) or active (redeploy)
- [ ] **All clear?** → Close upgrade log and move to post-incident review

---

## Rollback Decision Tree

**Trigger:** Unexpected behavior post-upgrade (settlement fails, state corruption, RPC errors)

### Step 1: Activate emergency legal hold

```bash
stellar contract invoke \
  --id $AFFECTED_INSTANCES \
  --source $SOURCE_SECRET \
  --network mainnet \
  -- set_legal_hold --active true
```

- [ ] Legal hold activated immediately on affected instances
- [ ] Record timestamp: `_________________`
- [ ] Notify investors (template: see ops playbook)

### Step 2: Investigate root cause (15–30 min)

- [ ] Dump `get_escrow` state: does it match pre-upgrade?
- [ ] Check RPC logs: network errors or contract panics?
- [ ] Review typed error codes emitted (see escrow-error-messages.md)
- [ ] Is root cause identified?

### Step 3: Decide rollback vs. fix

| Condition | Action |
|-----------|--------|
| **State corruption detected** | Rollback immediately (Step 4) |
| **RPC network errors only** | Wait 5 min, retry once; escalate if persistent |
| **Behavioral bug in new logic** | Deploy fix and upgrade again; OR rollback if fix is risky |

### Step 4: Rollback (additive upgrade only)

Rollback uses the same guarded two-step flow: propose the old WASM hash, wait out
the minimum delay, then execute.

```bash
for CONTRACT_ID in $AFFECTED_INSTANCES; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- propose_upgrade --new_wasm_hash <OLD_WASM_HASH>
done
```

- [ ] `propose_upgrade` event observed for all affected instances
- [ ] Minimum delay elapsed since proposal

```bash
for CONTRACT_ID in $AFFECTED_INSTANCES; do
  stellar contract invoke \
    --id $CONTRACT_ID \
    --source $SOURCE_SECRET \
    --network mainnet \
    -- execute_upgrade
done
```

- [ ] Rollback invocation succeeds
- [ ] `execute_upgrade` event observed for all affected instances
- [ ] `get_version` and `get_escrow` return data correctly
- [ ] State matches pre-upgrade snapshot (verified)
- [ ] Legal hold remains active pending investigation

### Step 5: Investigation and post-incident

- [ ] Root cause documented in ticket
- [ ] Fix implemented and re-tested on testnet
- [ ] Security review of fix
- [ ] Governance re-approval
- [ ] Redeploy fixed WASM after Day 2 hold period

---

## Emergency Contacts

| Role | Contact | Response time |
|------|---------|----------------|
| Ops on-call | `ops@karis-ky.dev` / Slack #ops-emergency | 15 min |
| Governance multisig | `gov@karis-ky.dev` | 1 hour |
| Security team | `security@karis-ky.dev` | 30 min |

---

## Sign-off

```
Upgrade type:      [ ] Additive  [ ] Redeploy
WASM hash:         _________________________
Instances affected: _________________________
Started:           _________________________
Completed:         _________________________
Approved by:       _________________________
Verified by:       _________________________
Date:              _________________________
```

---

## Post-Upgrade Review (Day 7)

Schedule a post-incident review meeting to discuss:

- [ ] What went well
- [ ] What could be smoother next time
- [ ] Any surprises or undocumented behavior
- [ ] Update runbook with lessons learned
