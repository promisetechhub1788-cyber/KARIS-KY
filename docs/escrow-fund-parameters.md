# Escrow Fund Parameters — Reference Guide

> **Contract version:** `SCHEMA_VERSION = 8` | `INTERFACE_VERSION = 1`
> **Target audience:** SDK integrators and operators calling `fund` and `fund_with_commitment`

This document describes parameters and behavior for both funding entrypoints:
[`fund`](#fund) (simple, base-yield deposits) and
[`fund_with_commitment`](#fund_with_commitment) (first-deposit tiered yield). Use this
alongside [Escrow Init Parameters](escrow-init-parameters.md),
[Escrow Lifecycle](escrow-lifecycle.md), and [ADR-005: Tiered Yield](adr/ADR-005-tiered-yield.md).

---

## Quick Comparison: `fund` vs. `fund_with_commitment`

| Aspect | `fund` | `fund_with_commitment` |
|--------|--------|----------------------|
| **Callable by** | Any investor (multiple times) | **First deposit only** per investor |
| **Yield** | Base yield (`escrow.yield_bps`) | Tier-selected yield (if configured) or base |
| **Lock period** | None; claim immediately at settlement | Optional; `committed_lock_secs` enforced |
| **Parameters** | `investor`, `amount` | `investor`, `amount`, `committed_lock_secs` |
| **Use case** | Standard deposits, retail, simple yield | Commitment-based incentives, lock-ups |
| **Second call** | Allowed (stacks contributions) | **Rejected** with Code 36 (`TieredSecondDeposit`) |

---

## `fund` — Simple Base-Yield Deposit

### Signature

```rust
pub fn fund(env: Env, investor: Address, amount: i128) -> InvoiceEscrow
```

### Parameters

| Parameter | Type | Constraints | Description |
|-----------|------|-------------|-------------|
| `investor` | `Address` | Valid Stellar address | The funding account (must call this function for auth). |
| `amount` | `i128` | `> 0`, `≤ funding_token` balance | Principal amount in base units. Rejects on overflow. |

### Behavior

1. **Auth required:** `investor.require_auth()` — caller must sign the transaction.
2. **Preconditions checked (in order):**
   - Amount must be positive (Code 100: `FundingAmountNotPositive`). Zero amounts are **always** rejected, independent of floor configuration.
   - Meets minimum contribution floor if configured; floor defaults to 0 when absent (Code 101: `FundingBelowMinContribution`). **Absence-equals-zero semantics:** on older schema versions where `MinContributionFloor` is not set, the floor is treated as 0, allowing any positive amount.
   - Escrow must be Open (status = 0) (Code 103: `EscrowNotOpenForFunding`)
   - Legal hold must not be active (Code 102: `LegalHoldBlocksFunding`)
   - Dispute pause must not be active (Code 165: `DisputePausedBlocksFunding`)
   - Funding deadline must not have passed (Code 153: `FundingDeadlinePassed`)
   - Investor must be allowlisted if allowlist is active (Code 104: `InvestorNotAllowlisted`)
   - Investor must pass sanctions screening (Code 205: `SanctionsScreeningFailed`)
   - Investor must pass KYC check if registry configured (Code 173: `InvestorNotVerified`)

3. **Contribution tracking:**
   - New total for investor = `previous_contribution + amount`
   - Must not exceed per-investor cap if configured (Code 106: `InvestorContributionExceedsCap`)

4. **Unique investor cap:**
   - If previous contribution was 0, investor is "new" — count increments
   - If unique investor cap is configured, reject if count would exceed (Code 107: `UniqueInvestorCapReached`)

5. **Yield assignment (first deposit only):**
   - On first call (`prev == 0`): investor's effective yield = escrow's base `yield_bps`
   - Stored persistently; used for all future claims
   - Subsequent calls reuse the same effective yield

6. **Concentration limit:**
   - If configured, check investor concentration = `(investor_total * 100) / total_funded`
   - Reject if exceeds configured cap (but no explicit error code — check logs for `ConcentrationLimitExceeded`)

7. **Funding target check:**
   - New total (`funded_amount + amount`) must not exceed `funding_target`
   - Overflow in `funded_amount` would cause Code 110: `FundedAmountOverflow`

8. **Status transition:**
   - If `funded_amount >= funding_target` after deposit, transition to Funded (status = 1)
   - Single-write immutable snapshot created (`FundingCloseSnapshot`)

9. **Event emission:**
   - `InvestorFunded` event with amount, contribution total, effective yield, tier lock (0 for simple fund)
   - If funded → Funded transition, `FundingClosedEvent` emitted

### Return Value

Updated `InvoiceEscrow` struct reflecting new `funded_amount` and possibly new `status`.

### Gas Cost

Typical: 5,000–12,000 CPU units. Higher with tier lookup, caps, and allowlist checks.

### Example (TypeScript SDK)

```typescript
const escrow = await client.fund(investorAddress, "10000000000"); // 10,000 tokens (7 decimals)
console.log(`Escrow status: ${escrow.status}`); // 0 = Open, 1 = Funded
console.log(`Total funded: ${escrow.funded_amount}`);
```

---

## `fund_with_commitment` — First-Deposit Tiered Yield

### Signature

```rust
pub fn fund_with_commitment(
    env: Env,
    investor: Address,
    amount: i128,
    committed_lock_secs: u64,
) -> InvoiceEscrow
```

### Parameters

| Parameter | Type | Constraints | Description |
|-----------|------|-------------|-------------|
| `investor` | `Address` | Valid Stellar address | The funding account (must call this function for auth). |
| `amount` | `i128` | `> 0`, `≤ funding_token` balance | Principal amount in base units. Rejects on overflow. |
| `committed_lock_secs` | `u64` | Any non-negative value | Lock duration in seconds. `0` means immediate claim eligibility (no lock). |

### Behavior (First-Deposit-Only Discipline)

1. **Auth required:** `investor.require_auth()` — caller must sign the transaction.

2. **First-deposit gate (CRITICAL):**
   - If investor's `previous_contribution > 0`, **immediately reject** with Code 36 (`TieredSecondDeposit`)
   - This is enforced before any other checks — non-negotiable constraint
   - Prevents changing an investor's tier after initial leg

3. **Preconditions (same as `fund`):**
   - Amount must be positive (Code 100: `FundingAmountNotPositive`). Zero amounts are **always** rejected, independent of floor configuration.
   - Meets minimum contribution floor if configured; floor defaults to 0 when absent (Code 101: `FundingBelowMinContribution`). **Absence-equals-zero semantics:** on older schema versions where `MinContributionFloor` is not set, the floor is treated as 0, allowing any positive amount.
   - Escrow must be Open (status = 0) (Code 103: `EscrowNotOpenForFunding`)
   - Legal hold, dispute pause, deadline, allowlist, sanctions, KYC checks
   - Per-investor contribution cap (Code 106: `InvestorContributionExceedsCap`)
   - Unique investor cap (Code 107: `UniqueInvestorCapReached`)
   - Funding target not exceeded (Code 110: `FundedAmountOverflow` on overflow)

### Tier Selection Algorithm

Called internally as `effective_yield_for_commitment(base_yield, committed_lock_secs)`:

1. **If `committed_lock_secs == 0`:**
   - Return `(base_yield, 0)` — no lock, use base yield

2. **If no tier table configured:**
   - Return `(base_yield, 0)` — no tiers, use base yield

3. **If tier table is empty:**
   - Return `(base_yield, 0)` — empty table, use base yield

4. **Tier matching (greedy, highest yield wins):**
   - Iterate over `yield_tiers` in order
   - For each tier: if `committed_lock_secs >= tier.min_lock_secs` and `tier.yield_bps > current_best`
   - Update `current_best = tier.yield_bps`
   - After all tiers scanned, return `(current_best, best_lock)` where `best_lock` is the `min_lock_secs` of the best-matching tier

5. **Example:**
   ```rust
   yield_tiers = [
     { min_lock_secs: 2_592_000,  yield_bps: 1000 },  // 30 days → 10%
     { min_lock_secs: 7_776_000,  yield_bps: 1200 },  // 90 days → 12%
     { min_lock_secs: 15_552_000, yield_bps: 1500 }   // 180 days → 15%
   ]
   ```

   | Committed lock | Matching tiers | Selected yield | Lock threshold |
   |---|---|---|---|
   | 0 seconds | None | base_yield | 0 |
   | 1_296_000 (15 days) | None | base_yield | 0 |
   | 2_592_000 (30 days) | [0] | 1000 bps | 2_592_000 |
   | 5_000_000 (58 days) | [0] | 1000 bps | 2_592_000 |
   | 7_776_000 (90 days) | [0, 1] | 1200 bps | 7_776_000 |
   | 15_552_000 (180 days) | [0, 1, 2] | 1500 bps | 15_552_000 |
   | 30_000_000 (347 days) | [0, 1, 2] | 1500 bps | 15_552_000 |

### Claim Lock Enforcement

After effective yield is selected, a **claim lock** is computed:

1. If `committed_lock_secs == 0`:
   - `claim_not_before = 0` (no lock)

2. If `committed_lock_secs > 0`:
   - `claim_not_before = now + committed_lock_secs` (Unix timestamp)
   - Validated: `claim_not_before <= escrow.maturity` (Code 111: `CommitmentLockExceedsMaturity`)
   - This ensures the lock expires before or at the maturity date

3. **Optional investor lock-in period:**
   - If `InvestorLockInSecs` is configured globally, a separate lock-in is applied
   - `lock_until = now + lock_in_secs`

4. **Claim gate (enforced by `claim_investor_payout`):**
   - SME cannot settle the escrow until both locks have passed:
     - `ledger.timestamp() >= claim_not_before` (commitment lock)
     - `ledger.timestamp() >= lock_in_until` (optional global lock-in)

### Yield and Concentration

Same logic as `fund`:

1. Effective yield is stored persistently for this investor
2. Concentration cap is checked if configured
3. If `funded_amount >= funding_target`, transition to Funded and snapshot

### Status Transition and Events

Same as `fund`:

1. If `funded_amount >= funding_target`, escrow transitions to Funded (status = 1)
2. Emit `InvestorFunded` event with:
   - Amount
   - Total investor contribution
   - Effective yield (selected via tier)
   - Tier lock (the `min_lock_secs` threshold, or 0 if no tier)

### Return Value

Updated `InvoiceEscrow` struct.

### Gas Cost

Typical: 7,000–15,000 CPU units. Higher with multiple tiers (O(n) scan).
- Tier lookup: ~500 CPU per tier
- Lock computation: ~1,000 CPU
- Keep tier count ≤ 5 for optimal gas

### Example (TypeScript SDK)

```typescript
// Investor commits to 90-day lock for elevated yield
const escrow = await client.fundWithCommitment(
  investorAddress,
  "10000000000",    // 10,000 tokens
  "7776000"         // 90 days in seconds
);

console.log(`Escrow status: ${escrow.status}`); // 0 or 1
console.log(`Effective yield: ${escrow.investorEffectiveYield} bps`); // e.g., 1200 (12%)
console.log(`Claim unlock time: ${escrow.investorClaimNotBefore}`); // Unix timestamp
```

---

## Yield Tier Configuration (Init-Time)

Yield tiers are set during [`init`](escrow-init-parameters.md#10-yield_tiers--tiered-yield-ladder-immutable)
and are **immutable** after initialization.

### Tier Struct

```rust
pub struct YieldTier {
    pub min_lock_secs: u64,
    pub yield_bps: i64,
}
```

### Validation Rules (enforced at init)

| Rule | Error Code | Recovery |
|------|-----------|----------|
| Tier `yield_bps` in `0..=10_000` | Code 10 | Correct yield BPS |
| Tier `yield_bps >= base_yield` | Code 11 | Raise tier yield or lower base |
| `min_lock_secs` strictly increasing | Code 12 | Sort tiers by `min_lock_secs` |
| `yield_bps` non-decreasing across tiers | Code 13 | Ensure higher tiers have ≥ yield |

### Example Configurations

**Conservative (3-tier, moderate incentives):**
```json
{
  "base_yield": 600,
  "yield_tiers": [
    { "min_lock_secs": 2592000,  "yield_bps": 700 },
    { "min_lock_secs": 7776000,  "yield_bps": 850 },
    { "min_lock_secs": 15552000, "yield_bps": 1000 }
  ]
}
```

**Aggressive (2-tier, steep incentive curve):**
```json
{
  "base_yield": 800,
  "yield_tiers": [
    { "min_lock_secs": 2592000,  "yield_bps": 1000 },
    { "min_lock_secs": 7776000,  "yield_bps": 1500 }
  ]
}
```

**Simple (no tiers, same yield for all):**
```json
{
  "base_yield": 900,
  "yield_tiers": null
}
```

---

## Minimum Contribution Floor — Absence-Equals-Zero Semantics

The `MinContributionFloor` parameter guards against dust contributions and is **immutable** after initialization.

### Key Properties

| Property | Behavior |
|----------|----------|
| **Stored as** | `DataKey::MinContributionFloor` (instance storage) |
| **Default** | When absent (not set at init or on older schema versions), the floor defaults to **0** |
| **Validation** | If set, must be `> 0`. Zero floors are forbidden at init to avoid semantic ambiguity. |
| **Mutability** | Not mutable after `init`. Floor is locked in for the escrow's lifetime. |
| **Enforcement** | Applied to **every** `fund` and `fund_with_commitment` call: `amount >= floor` always. |

### Absence-Equals-Zero Policy (Schema Compatibility)

When reading `MinContributionFloor` during `fund`:

```rust
let floor: i128 = env.storage()
    .instance()
    .get(&DataKey::MinContributionFloor)
    .unwrap_or(0);  // ← Absence (None) → 0
```

**Why this matters:**

1. **On schema v2 and earlier:** `MinContributionFloor` key does not exist; read returns `None`, treated as floor = 0.
2. **On schema v3+:** `MinContributionFloor` is always set (either to a positive value at init, or by default during migration).
3. **Backward compatibility:** Upgrading from v2 → v3 and beyond: no migration needed. Old instances simply read floor as 0.

### Zero-Amount Rejection (Independent of Floor)

Zero amounts are **always** rejected with Code 100 (`FundingAmountNotPositive`), independent of floor:

```rust
ensure(&env, amount > 0, EscrowError::FundingAmountNotPositive);  // Always first
ensure(&env, amount >= floor, EscrowError::FundingBelowMinContribution);  // Always second
```

This ensures:
- No race condition between floor updates and concurrent fund calls
- Zero amounts are never "sneaked through" via floor = 0
- Explicit semantics: positive amounts are the baseline, floor is an additional guard

---

## Error Codes Reference

### Fund-Specific Errors

| Code | Constant | Condition | Recovery |
|------|----------|-----------|----------|
| 100 | `FundingAmountNotPositive` | `amount <= 0` | Pass positive amount |
| 101 | `FundingBelowMinContribution` | `amount < min_contribution_floor` | Increase deposit or contact operator |
| 102 | `LegalHoldBlocksFunding` | Legal hold is active | Contact operator/governance |
| 103 | `EscrowNotOpenForFunding` | Escrow status != 0 (Open) | Wait or redeploy escrow |
| 104 | `InvestorNotAllowlisted` | Allowlist active, investor not on list | Contact operator |
| 105 | `InvestorContributionOverflow` | Contribution arithmetic overflow (rare) | Split deposit or contact operator |
| 106 | `InvestorContributionExceedsCap` | Total would exceed per-investor cap | Reduce amount or split over time |
| 107 | `UniqueInvestorCapReached` | New investor but cap full | Contact operator or fund as existing investor |
| 108 | **`TieredSecondDeposit`** | **`fund_with_commitment` called twice by same investor** | **Use `fund` for additional deposits, or deploy new escrow** |
| 109 | `InvestorClaimTimeOverflow` | Lock timestamp arithmetic overflow (rare) | Use shorter lock period |
| 110 | `FundedAmountOverflow` | Total funded amount overflow (rare) | Contact operator |
| 111 | `CommitmentLockExceedsMaturity` | `claim_not_before > escrow.maturity` | Use shorter lock period |
| 153 | `FundingDeadlinePassed` | Funding window closed | Redeploy escrow if still needed |
| 165 | `DisputePausedBlocksFunding` | Dispute pause is active | Wait for dispute resolution |
| 173 | `InvestorNotVerified` | KYC check failed | Complete KYC or contact operator |
| 205 | `SanctionsScreeningFailed` | Sanctions provider rejects investor | Contact compliance team |

---

## Storage Footprint & Limits

Each investor is stored under a persistent key:

```rust
// Per-investor records (persistent storage, bounded per investor)
DataKey::InvestorContribution(investor)               // i128
DataKey::InvestorEffectiveYield(investor)            // i64
DataKey::InvestorClaimNotBefore(investor)            // u64 (timestamp)
DataKey::InvestorLockInUntil(investor)              // u64 (timestamp)
DataKey::InvestorHistory(investor)                  // Vec of history records
```

### Practical Limits

- **Supported investor cardinality:** Configured at init via `max_unique_investors` (no hard-coded global max)
- **Per-investor history records:** Unbounded, but TTL and compaction policies may apply
- **Escrow instance storage footprint:** Fixed ~5–10 KB per escrow (stable across investor count)

---

## Workflow Examples

### Example 1: Simple Base-Yield Deposit

**Setup:**
- Invoice: 100,000 USDC, base yield 8%, no tiers

**Action:**
```typescript
// Investor A deposits 50,000 USDC
await client.fund(investorA, "50000000000");
```

**Result:**
- Investor A's contribution: 50,000
- Investor A's effective yield: 8% (base)
- Escrow status: Open (50% of target reached)
- Claim lock: None (eligible to claim immediately after settlement)

**Action:**
```typescript
// Investor A adds 30,000 more USDC
await client.fund(investorA, "30000000000");
```

**Result:**
- Investor A's contribution: 80,000 (stacked)
- Effective yield unchanged: 8% (set on first call, reused)
- Escrow status: Open (80% of target)

---

### Example 2: Tiered Yield with Commitment Lock

**Setup:**
- Invoice: 100,000 USDC, base yield 6%, 3-tier ladder:
  - 30 days @ 8%
  - 90 days @ 10%
  - 180 days @ 12%

**Action:**
```typescript
// Investor B commits 50,000 USDC for 90 days
await client.fundWithCommitment(investorB, "50000000000", "7776000");
```

**Result:**
- Investor B's contribution: 50,000
- Effective yield: 10% (matched 90-day tier)
- Claim lock: now + 90 days (UTC timestamp)
- Escrow status: Open (50% funded)

**Investor B tries a second deposit (FAILS):**
```typescript
await client.fund(investorB, "20000000000"); // Any call by same investor fails
// Error: Code 36 (TieredSecondDeposit)
```

**Action:**
```typescript
// Investor C deposits 50,000 USDC with no lock commitment
await client.fundWithCommitment(investorC, "50000000000", "0");
```

**Result:**
- Investor C's contribution: 50,000
- Effective yield: 6% (base, committed_lock_secs = 0 matches no tier)
- Claim lock: None (0)
- Escrow status: Funded (100% target met)
- `FundingCloseSnapshot` created (immutable)

**Settlement phase:**
```typescript
// SME settles after maturity window
await client.settle(smeAddress);
// Escrow transitions to Settled (status = 2)
```

**Investor B's claim (after 90-day lock expires):**
```typescript
// At settlement + 90 days, Investor B claims
const payout = await client.claimInvestorPayout(investorB);
// Receives: principal + 10% yield
```

**Investor C's claim (immediate):**
```typescript
// Investor C claims right away (no lock)
const payout = await client.claimInvestorPayout(investorC);
// Receives: principal + 6% yield
```

---

## Integration Checklist

- [ ] Understand **first-deposit-only discipline** for `fund_with_commitment` — second calls will reject
- [ ] If using tiers, review tier configuration at init; tiers are immutable
- [ ] Verify investor can call `fund` for additional deposits (not `fund_with_commitment`)
- [ ] Confirm tier thresholds match business incentives (e.g., 90-day commitment for 10% yield)
- [ ] Handle Code 36 (`TieredSecondDeposit`) gracefully in SDK/UI — guide user to `fund` instead
- [ ] Monitor `InvestorClaimNotBefore` timestamp; investor cannot claim until it passes
- [ ] Test lock-expire scenario: settle escrow, wait for locks, then claim
- [ ] Monitor `FundingClosedEvent` to confirm escrow transitions to Funded
- [ ] If concentration caps are configured, test boundary conditions
- [ ] Review [Escrow Error Codes](escrow-error-messages.md) for complete error handling

---

## Related Documents

- [Escrow Init Parameters](escrow-init-parameters.md) — `yield_tiers`, `min_contribution`, caps
- [Escrow Lifecycle & State Machine](escrow-lifecycle.md) — fund → settle → claim flow
- [Escrow Error Codes](escrow-error-messages.md) — all typed error references
- [ADR-005: Tiered Yield](adr/ADR-005-tiered-yield.md) — design rationale
- [ADR-003: Settlement Flow](adr/ADR-003-settlement-flow.md) — two-phase settlement
- [CLI Simulation Recipes](escrow-sim-stellar-cli.md) — test funding entrypoints locally
