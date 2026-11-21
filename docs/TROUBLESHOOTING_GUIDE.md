# Escrow Contract Troubleshooting Guide

This guide covers common issues encountered by operators, investors, and SMEs when interacting with the karis-ky escrow contract, including symptoms, causes, and solutions.

---

## Table of Contents

- [Operator Issues](#operator-issues)
- [Investor Issues](#investor-issues)
- [SME (Beneficiary) Issues](#sme-beneficiary-issues)
- [Token Integration Issues](#token-integration-issues)
- [Storage and Performance Issues](#storage-and-performance-issues)
- [Diagnostic Commands](#diagnostic-commands)

---

## Operator Issues

### 1. Cannot initialize escrow

**Symptom:**  
`init()` call fails with error code

**Possible Causes:**

- **AmountMustBePositive (1)**: Escrow amount is zero or negative
  - **Solution**: Provide a positive integer for the invoice amount
  
- **YieldBpsOutOfRange (2)**: Yield basis points outside 0–10,000
  - **Solution**: Ensure yield_bps is between 0 and 10,000 (0% to 100%)
  
- **InvoiceIdInvalidLength (4)**: Invoice ID is empty or >32 characters
  - **Solution**: Provide 1–32 character invoice ID
  
- **InvoiceIdInvalidCharset (5)**: Invoice ID contains non-alphanumeric characters (only `[A-Za-z0-9_]` allowed)
  - **Solution**: Replace spaces, hyphens, special chars with underscores
  
- **EscrowAlreadyInitialized (3)**: Escrow instance already exists
  - **Solution**: Create a new contract instance; `init()` is one-time only
  
- **MinContributionNotPositive (6)**: min_contribution is set to zero
  - **Solution**: Either omit min_contribution or set to >0
  
- **MinContributionExceedsAmount (7)**: min_contribution > invoice amount
  - **Solution**: Set min_contribution ≤ invoice amount
  
- **MaxUniqueInvestorsNotPositive (8)** or **MaxPerInvestorNotPositive (9)**: Investor cap is zero
  - **Solution**: Set cap to >0 or omit to allow unlimited

**Diagnostic Commands:**

```bash
# Verify escrow state before init
stellar contract invoke \
  --network testnet \
  --id <CONTRACT_ID> \
  -- get_escrow

# If get_escrow returns "not initialized", safe to init
# If it returns escrow state, already initialized
```

---

### 2. Escrow is stuck in "open" status

**Symptom:**  
Escrow is open but investors cannot deposit; funding is blocked

**Possible Causes:**

- **EscrowNotOpenForFunding (103)**: Escrow status is not 0 (open)
  - **Solution**: Check current status with `get_escrow`. If funded/settled/cancelled, deposits are no longer accepted
  
- **LegalHoldBlocksFunding (102)**: Legal hold is active
  - **Solution**: Call `clear_legal_hold()` if you have admin authority and the two-phase clear delay has passed (see [Legal Hold](#legal-hold-issues))
  
- **FundingDeadlinePassed (164)**: Funding deadline has passed
  - **Solution**: Update funding deadline with `update_funding_deadline()` if still open

**Diagnostic Commands:**

```bash
# Check escrow status
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow

# Check legal hold status
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_legal_hold

# Check funding deadline
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_funding_deadline

# Expected output:
# status: 0 (open), status: 1 (funded), status: 2 (settled), status: 3 (withdrawn/cancelled)
```

---

### 3. Legal Hold Issues

**Symptom:**  
Operations fail with `LegalHoldBlocks*` errors (30–125, 140, 160)

**Causes and Solutions:**

#### Active Legal Hold (LegalHoldBlocksFunding, LegalHoldBlocksSettlement, etc.)

**Cause**: Admin has activated legal hold to prevent risky transitions

**Solution:**
1. Verify current hold status: `get_legal_hold()`
2. If hold is active and should be cleared:
   - If you are admin, call `request_legal_hold_clear()` to start two-phase delay
   - Wait for delay period (configured at init)
   - Call `clear_legal_hold()` to finalize
3. If hold is necessary (compliance), await resolution before clearing

**Two-Phase Clear Process:**

```bash
# Step 1: Request clear (start delay)
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  --source <ADMIN_KEY> \
  -- request_legal_hold_clear

# Step 2: Wait for delay (check LegalHoldClearDelayOverflow constant)

# Step 3: Clear hold
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  --source <ADMIN_KEY> \
  -- clear_legal_hold
```

---

### 4. Escrow fails to settle

**Symptom:**  
`settle()` returns error; escrow remains in "funded" status

**Possible Causes:**

- **SettlementNotFunded (121)**: Escrow is not funded yet
  - **Solution**: Wait for funding target to be reached; use `get_escrow()` to check funded_amount vs. funding_target
  
- **MaturityNotReached (122)**: Current time is before configured maturity timestamp
  - **Solution**: Wait until maturity timestamp; error message includes seconds remaining
  - **Diagnostic**: Call to get time remaining:
    ```bash
    # Check escrow maturity
    stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow
    # Compare escrow.maturity with current ledger timestamp (check Horizon API)
    curl https://horizon-testnet.stellar.org/ledgers/latest | jq '.closed_at'
    ```
  
- **LegalHoldBlocksSettlement (120)**: Legal hold is active
  - **Solution**: Follow [Legal Hold Issues](#legal-hold-issues) to clear hold
  
- **SME not authorized** (auth error code)
  - **Solution**: Call with `--source <SME_KEY>` (the beneficiary address)

---

### 5. Cannot withdraw funds as SME

**Symptom:**  
`withdraw()` fails; SME cannot pull liquidity

**Possible Causes:**

- **WithdrawalNotFunded (124)**: Escrow status is not 1 (funded)
  - **Solution**: Wait for funding target to be reached
  
- **LegalHoldBlocksWithdrawal (123)**: Legal hold is active
  - **Solution**: Clear legal hold (see [Legal Hold Issues](#legal-hold-issues))
  
- **InsufficientContractBalance (164)**: Contract balance < funded_amount
  - **Cause**: Tokens were transferred out or not transferred in; custody mismatch
  - **Solution**: 
    1. Verify contract token balance: `soroban contract read-state --id <CONTRACT_ID> --network testnet`
    2. Transfer missing funds to contract
    3. Retry withdraw
  
- **Token transfer failed** (error codes 36–41)
  - **Cause**: Token is non-standard (fee-on-transfer, rebasing, hooks)
  - **Solution**: Verify token integration with [Token Integration Checklist](ESCROW_TOKEN_INTEGRATION_CHECKLIST.md)

**Diagnostic Commands:**

```bash
# Check contract balance
soroban contract read-state --id <CONTRACT_ID> --network testnet

# Check escrow funded_amount
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow

# Manually verify token balance
stellar contract invoke --network testnet \
  --id <FUNDING_TOKEN_ADDRESS> \
  -- balance --account <CONTRACT_ADDRESS>
```

---

### 6. Migration fails

**Symptom:**  
`migrate()` returns typed error (90–92)

**Possible Causes:**

- **MigrationVersionMismatch (90)**: `from_version` does not match stored schema version
  - **Solution**: Call `get_version()` to check current version, pass correct version
  
- **AlreadyCurrentSchemaVersion (91)**: Already at latest schema
  - **Solution**: No migration needed; deployed version is current
  
- **NoMigrationPath (92)**: No migration path from provided version
  - **Cause**: Operator attempted to migrate to a version with no implemented path
  - **Solution**: See [OPERATOR_RUNBOOK.md](OPERATOR_RUNBOOK.md) for upgrade procedures; may require redeploy

---

### 7. Insufficient contract balance during sweep

**Symptom:**  
`sweep_terminal_dust()` fails; dust cannot be swept to treasury

**Possible Causes:**

- **DustSweepNotTerminal (33)**: Escrow is not in terminal status (0–2 only; 3 = terminal)
  - **Solution**: Wait for escrow to reach settled/withdrawn/cancelled status
  
- **NoFundingTokenBalanceToSweep (34)**: Contract has zero token balance
  - **Solution**: Only sweep if residue exists; check balance first
  
- **EffectiveSweepAmountZero (35)**: Sweep amount computed to zero
  - **Solution**: Increase sweep amount or check that balance exists
  
- **SweepExceedsLiabilityFloor (42)**: Sweep would reduce balance below outstanding liabilities
  - **Solution**: Reduce sweep amount to preserve investor claims

---

## Investor Issues

### 8. Cannot fund (rejected by allowlist)

**Symptom:**  
`fund()` returns `InvestorNotAllowlisted (104)`

**Cause**: Allowlist gate is active and investor address is not allowlisted

**Solution:**

1. Contact escrow operator to add your address to allowlist
2. Operator calls `set_investors_allowlisted()` with your address:
   ```bash
   stellar contract invoke --network testnet --id <CONTRACT_ID> \
     --source <ADMIN_KEY> \
     -- set_investors_allowlisted \
       --entries '[{"investor": "<YOUR_ADDRESS>", "allowed": 1}]'
   ```

**Diagnostic Commands:**

```bash
# Check if allowlist is active
stellar contract invoke --network testnet --id <CONTRACT_ID> -- is_allowlist_active

# If active, ask operator to verify you're allowlisted
```

---

### 9. Funding fails: below minimum contribution

**Symptom:**  
`fund()` returns `FundingBelowMinContribution (101)`

**Cause**: Your deposit amount is less than configured minimum

**Solution**: 
1. Check minimum: `get_escrow()` and review docs / off-chain parameters
2. Increase deposit amount to meet minimum
3. Or contact operator to lower/remove minimum

**Diagnostic Commands:**

```bash
# Check minimum contribution floor
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow

# min_contribution_floor should be visible in escrow state
```

---

### 10. Funding rejected: exceeds per-investor cap

**Symptom:**  
`fund()` returns `InvestorContributionExceedsCap (106)`

**Cause**: Your total contribution would exceed per-investor limit

**Solution**:
1. Check your current contribution: `get_investor_contribution(<YOUR_ADDRESS>)`
2. Calculate remaining capacity
3. Either fund less, or ask operator to raise cap

**Diagnostic Commands:**

```bash
# Check your current contribution
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_investor_contribution --investor <YOUR_ADDRESS>

# Check per-investor cap
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow
# (check max_per_investor_cap field)
```

---

### 11. Funding rejected: max unique investors reached

**Symptom:**  
`fund()` returns `UniqueInvestorCapReached (107)`

**Cause**: Escrow has reached maximum number of unique investors; no new investors allowed

**Solution**:
1. If you previously invested, use the same address to add more
2. Otherwise, this escrow cannot accept more investors; find another escrow

**Diagnostic Commands:**

```bash
# Check current investor count and cap
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow
# Compare unique_funder_count vs max_unique_investors_cap
```

---

### 12. Cannot claim payout: commitment lock active

**Symptom:**  
`claim_investor_payout()` returns `InvestorCommitmentLockNotExpired (128)` with context showing remaining time

**Cause**: You made a tiered deposit with a lock commitment; waiting period has not elapsed

**Solution**: Wait until the unlock timestamp shown in the error context

**Diagnostic Commands:**

```bash
# Check your claim lock timestamp
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_investor_claim_not_before --investor <YOUR_ADDRESS>

# Compare with current ledger timestamp
# (if current_time < claim_not_before, still locked)
```

---

### 13. Cannot claim payout: escrow not settled

**Symptom:**  
`claim_investor_payout()` returns `InvestorClaimNotSettled (127)`

**Cause**: Escrow has not been settled yet; payouts are only claimable after settlement

**Solution**: Wait for escrow to be settled (SME must call `settle()` after maturity)

**Diagnostic Commands:**

```bash
# Check escrow status
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow
# status should be 2 (settled); if <2, not yet settled
```

---

### 14. Payout calculation differs from expectation

**Symptom:**  
`compute_investor_payout()` returns unexpected amount

**Cause**: Pro-rata calculation based on funding snapshot and yield

**Solution**: Verify calculation with the authoritative on-chain formula:

1. Get funding snapshot: `get_funding_close_snapshot()`
2. Get your contribution: `get_investor_contribution(<YOUR_ADDRESS>)`
3. Get your effective yield: `get_investor_effective_yield(<YOUR_ADDRESS>)`
4. Apply formula from [escrow-pro-rata.md](escrow-pro-rata.md)

**Diagnostic Commands:**

```bash
# Check funding snapshot
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_funding_close_snapshot

# Check your contribution
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_investor_contribution --investor <YOUR_ADDRESS>

# Check your effective yield
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_investor_effective_yield --investor <YOUR_ADDRESS>

# Compute payout on-chain to verify
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- compute_investor_payout --investor <YOUR_ADDRESS>
```

---

## SME (Beneficiary) Issues

### 15. Cannot rotate beneficiary

**Symptom:**  
`propose_admin()` or `rotate_beneficiary()` fails

**Possible Causes:**

- **LegalHoldBlocksBeneficiaryRotation (160)**: Legal hold is active
  - **Solution**: Clear legal hold (see [Legal Hold Issues](#legal-hold-issues))
  
- **RotationNotOpen (161)**: Escrow status is not 0 or 1 (open/funded)
  - **Solution**: Cannot rotate beneficiary after settlement
  
- **NewSmeSameAsCurrent (162)**: Proposed SME is already the current SME
  - **Solution**: Provide a different address

---

### 16. Collateral record not persisting

**Symptom:**  
`record_sme_collateral_commitment()` succeeds but collateral is not visible

**Important Note**: Collateral records are **metadata only** — they do **not** lock, reserve, or move tokens. Records are advisory for off-chain risk review.

**Diagnostic Commands:**

```bash
# Check collateral record
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_sme_collateral_commitment

# If empty, no record has been made yet
# Record is stored; check off-chain indexer for CollateralRecordedEvt
```

---

## Token Integration Issues

### 17. Token transfer fails after funding

**Symptom:**  
`withdraw()` or `claim_investor_payout()` fails with error codes 36–41

**Cause**: Token behaves unexpectedly; SEP-41 non-compliant

**Solutions**:

1. Verify token is on compliance checklist: [ESCROW_TOKEN_INTEGRATION_CHECKLIST.md](ESCROW_TOKEN_INTEGRATION_CHECKLIST.md)
2. Check for:
   - **Fee-on-transfer tokens**: Not supported; balance check will fail
   - **Rebasing tokens**: Not supported; balance mismatch
   - **Hook tokens**: Not supported; unexpected balance delta
3. If token is non-standard, contact token issuer or use alternate

**Diagnostic Commands:**

```bash
# Before depositing, verify token is SEP-41 compliant:
# 1. Transfer a small test amount to contract
# 2. Check pre- and post-transfer balances match exactly
# 3. If they don't, token is non-compliant

stellar contract invoke --network testnet \
  --id <FUNDING_TOKEN_ADDRESS> \
  -- transfer \
    --from <INVESTOR> \
    --to <CONTRACT> \
    --amount 100

# Then verify balance delta is exactly 100
```

---

## Storage and Performance Issues

### 18. Cannot fund: contract storage/CPU limits exceeded

**Symptom:**  
`fund()` fails with CPU/memory limit error

**Cause**: Too many unique investors; per-address storage overhead

**Solution**:
1. New investor? Wait for an investor to refund/claim to free slots
2. Existing investor? Use same address to add more funding
3. Operator can increase CPU limits or deploy new escrow

**Diagnostic Commands:**

```bash
# Check current unique investor count
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow
# Review unique_funder_count vs contract max

# Check storage footprint
soroban contract read-state --id <CONTRACT_ID> --network testnet | wc -l
```

---

### 19. Ledger time is unexpected

**Symptom:**  
Maturity checks or claim locks use unexpected timestamps

**Cause**: Ledger time skew; contract uses validator-observed time, not wall-clock

**Solution**: Use Soroban's ledger-based timestamps for all time-sensitive logic; integrators must treat ledger boundaries as soft:

```bash
# Get current ledger timestamp
curl https://horizon-testnet.stellar.org/ledgers/latest | jq '.closed_at'

# This is the authoritative time for contract operations
```

**Reference**: [escrow-ledger-time.md](escrow-ledger-time.md)

### 20. Attestation log disappeared

**Symptom:**
`get_attestation_append_log()` returns no entries or the attestation data is no longer
  available for an escrow that previously had records.

**Cause:**
The attestation append log is stored in instance storage. If the contract instance TTL was
not extended before expiry, instance entries may have been archived or evicted.

**Solution:**
1. Check the contract instance's TTL and archival status using the network's RPC tools.
2. If the entry is archived, follow the network's supported restoration procedure before
   querying the contract again. Calling `bump_ttl` regularly before expiry prevents this
   risk; it does not guarantee recovery of already-evicted data.
3. Rebuild the audit history from `AttestationDigestAppended` events and durable off-chain
   records if the log cannot be restored. See [Escrow Attestations](escrow-attestations.md)
   and [TTL semantics and operational `bump_ttl`](escrow-gas-storage-notes.md).

---

## Diagnostic Commands

### Summary of Common Diagnostic Queries

```bash
# === Escrow State ===
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_escrow

# === Versions ===
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_version       # Schema version
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_interface_version  # Interface version

# === Investor Info ===
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_investor_contribution --investor <ADDRESS>

stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_investor_effective_yield --investor <ADDRESS>

stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_investor_claim_not_before --investor <ADDRESS>

# === Administrative State ===
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_legal_hold
stellar contract invoke --network testnet --id <CONTRACT_ID> -- is_allowlist_active
stellar contract invoke --network testnet --id <CONTRACT_ID> -- get_funding_deadline

# === Pro-Rata Calculation ===
stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- compute_investor_payout --investor <ADDRESS>

stellar contract invoke --network testnet --id <CONTRACT_ID> \
  -- get_funding_close_snapshot

# === Event Diagnostics ===
# Listen for ErrorDiagnosticEmitted events in Soroban event stream
# Parse ErrorDiagnostic struct for user-friendly error recovery
```

---

## Escalation

If issues persist after following this guide:

1. **Check ADRs and detailed docs**: Links in each section provide architecture context
2. **Review operator runbook**: [OPERATOR_RUNBOOK.md](OPERATOR_RUNBOOK.md)
3. **Contact operator** with diagnostic outputs (see above)
4. **Open issue** with error code, version info, and diagnostic data

---

## References

- [ADR-001: State Model](adr/ADR-001-state-model.md)
- [ADR-002: Auth Boundaries](adr/ADR-002-auth-boundaries.md)
- [ADR-003: Settlement Flow](adr/ADR-003-settlement-flow.md)
- [ADR-004: Legal Hold](adr/ADR-004-legal-hold.md)
- [ADR-005: Tiered Yield](adr/ADR-005-tiered-yield.md)
- [escrow-error-messages.md](escrow-error-messages.md) — Full error code reference
- [escrow-pro-rata.md](escrow-pro-rata.md) — Payout math
- [OPERATOR_RUNBOOK.md](OPERATOR_RUNBOOK.md) — Deployment and operations
- [ESCROW_TOKEN_INTEGRATION_CHECKLIST.md](ESCROW_TOKEN_INTEGRATION_CHECKLIST.md) — Token compatibility
