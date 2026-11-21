# Escrow Init Parameters — Reference Guide

> **Contract version:** `SCHEMA_VERSION = 8` | `INTERFACE_VERSION = 1`
> **Target audience:** Operators, integrators, and SDK consumers calling `LiquifactEscrow::init`

This document describes every parameter accepted by [`init`](../escrow/src/lib.rs), including
valid ranges, defaults, examples (conservative vs. aggressive), and gas cost implications.
Use this alongside the [lifecycle guide](escrow-lifecycle.md) and [CLI simulation recipes](escrow-sim-stellar-cli.md).

---

## Parameter Reference Table

| # | Parameter | Type | Required | Valid Range | Default | Mutable after init? |
|---|-----------|------|----------|-------------|---------|---------------------|
| 1 | `admin` | `Address` | **Yes** | Any valid Stellar address | — | Yes — via `propose_admin` / `accept_admin` |
| 2 | `invoice_id` | `String` | **Yes** | 1–32 bytes, `[A-Za-z0-9_]` only | — | No |
| 3 | `sme_address` | `Address` | **Yes** | Any valid Stellar address | — | Yes — via `rotate_beneficiary` (before settlement only) |
| 4 | `amount` | `i128` | **Yes** | `> 0` | — | Partially — `funding_target` is mutable while Open |
| 5 | `yield_bps` | `i64` | **Yes** | `0..=10_000` (0–100%) | — | No |
| 6 | `maturity` | `u64` | **Yes** | `0` (no gate) or future Unix timestamp | — | Yes — via `update_maturity` while Open |
| 7 | `funding_token` | `Address` | **Yes** | Any deployed SEP-41 token address | — | **No — immutable** |
| 8 | `registry` | `Option<Address>` | No | Any valid address or `null` | `null` | **No — immutable** |
| 9 | `treasury` | `Address` | **Yes** | Any valid Stellar address | — | **No — immutable** |
| 10 | `yield_tiers` | `Option<Vec<YieldTier>>` | No | Non-empty vec with valid tiers or `null` | `null` (no tiering) | **No — immutable** |
| 11 | `min_contribution` | `Option<i128>` | No | `> 0` and `≤ amount`, or `null` | `null` (0 = no floor) | No |
| 12 | `max_unique_investors` | `Option<u32>` | No | `> 0`, or `null` | `null` (unlimited) | Partially — can be **lowered** while Open |
| 13 | `max_per_investor` | `Option<i128>` | No | `> 0`, or `null` | `null` (unlimited) | No |
| 14 | `legal_hold_clear_delay` | `Option<u64>` | No | Any non-negative u64 seconds, or `null` | `null` (0 = immediate) | No |
| 15 | `funding_deadline` | `Option<u64>` | No | Future ledger timestamp `> ledger.timestamp()`, or `null` | `null` (no deadline) | No |
| 16 | `yield_slippage_threshold` | `Option<i64>` | No | `0..=10_000` bps, or `null` | `null` (0 = no check) | No |

---

## Detailed Parameter Descriptions

### 1. `admin` — Governance Address

The admin controls compliance holds, maturity updates, funding target changes,
investor caps, allowlist management, attestation binding, admin handover,
and funding cancellation.

| Aspect | Detail |
|--------|--------|
| **Recommendation** | Use a multisig address (e.g., 2-of-3 or 3-of-5) for production. A single-key admin that is lost strands funds if a legal hold is active. |
| **Recovery** | `propose_admin` + `accept_admin` — not gated by legal hold. Governance can rotate admin even under hold. |
| **Gas impact** | Admin operations cost ~2,000–8,000 CPU instructions each. Multisig auth adds ~500–1,000 per extra signer. |

### 2. `invoice_id` — Invoice Identifier

An ASCII identifier for the invoice, stored as a Soroban `Symbol`.

The canonical contract limit is `MAX_INVOICE_ID_STRING_LEN = 32`.

| Aspect | Detail |
|--------|--------|
| **Charset** | `[A-Za-z0-9_]` only — uppercase/lowercase ASCII letters, digits, and underscores. No spaces, hyphens, periods, or special characters. |
| **Length** | 1–32 bytes inclusive (`1..=MAX_INVOICE_ID_STRING_LEN`). |
| **Examples** | `INV001`, `INVOICE_2026_Q3`, `SUPPLIER_A_REF42` |
| **Validation errors** | Code 4 (`InvoiceIdInvalidLength`), Code 5 (`InvoiceIdInvalidCharset`) |
| **Gas impact** | Validation is O(n) on string length; negligible for 32 bytes. |

### 3. `sme_address` — Beneficiary / Invoice Issuer

The SME (Small/Medium Enterprise) is the invoice issuer who can call `settle` or
`withdraw` once the escrow is funded.

| Aspect | Detail |
|--------|--------|
| **Recommendation** | Use the SME's known operational address. Can be rotated via `rotate_beneficiary` before settlement if the SME changes wallets. |
| **Constraints** | Must differ from `admin` (no self-dealing without separate governance). |
| **Gas impact** | Settlement/withdrawal costs ~3,000–6,000 CPU instructions. |

### 4. `amount` — Invoice Face Value (Funding Target Seed)

The face value of the invoice in the **funding token's base units** (e.g., for a
token with 7 decimals, `10000_0000000` = 10,000 tokens).

| Aspect | Detail |
|--------|--------|
| **Range** | `> 0` (Code 1: `AmountMustBePositive`) |
| **Default `funding_target`** | Initially equals `amount`. Can be changed via `update_funding_target` while Open. |
| **Precision** | `i128` — max ~1.7e38 base units. More than sufficient for any realistic token amount. |
| **Gas impact** | Larger `i128` values have identical gas cost to smaller ones (fixed-width arithmetic). |

### 5. `yield_bps` — Base Annualized Yield

The base annualized yield in **basis points** (1 bps = 0.01%). Applied to all
investors who use `fund` (not `fund_with_commitment`).

| Value | Meaning |
|-------|---------|
| `0` | 0% — zero-yield invoice |
| `500` | 5% per annum |
| `800` | 8% per annum |
| `1000` | 10% per annum |
| `10000` | 100% per annum (maximum) |

| Aspect | Detail |
|--------|--------|
| **Range** | `0..=10_000` (Code 2: `YieldBpsOutOfRange`) |
| **Interaction with tiers** | When `yield_tiers` is configured, each tier's `yield_bps` must be ≥ `base_yield` (Code 11: `TierYieldBelowBase`). |
| **Gas impact** | Yield is read from storage on each claim; no per-call computation beyond storage access. |

### 6. `maturity` — Settlement Time Gate

A ledger timestamp (Unix seconds) before which `settle` is rejected. `0` means
"settle immediately once funded."

| Aspect | Detail |
|--------|--------|
| **`maturity = 0`** | No time lock. SME can settle as soon as funding target is met. |
| **`maturity > 0`** | `settle` requires `ledger.timestamp() >= maturity` (Code 122: `MaturityNotReached`). |
| **Mutable?** | Yes — admin can call `update_maturity` while escrow is Open (status 0). |
| **Trust model** | Uses validator-observed ledger time, not a wall-clock oracle. Expect possible skew on simulated vs. live networks. |
| **Gas impact** | Single `u64` comparison; negligible. |

### 7. `funding_token` — SEP-41 Token Address (IMMUTABLE)

The Stellar asset token contract investors use to fund the escrow. **Cannot be changed after init.**

| Aspect | Detail |
|--------|--------|
| **Token requirements** | Standard SEP-41 (`transfer`, `balance`, `decimals`). Fee-on-transfer, rebasing, and hook tokens are **explicitly out of scope** and will fail balance checks. |
| **Decimals** | Token-specific; the escrow does not validate or enforce a specific decimal count. The operator must ensure all amounts use the correct base-unit precision. |
| **Immutable** | Binding is permanent. To change the token, redeploy a new escrow instance. |
| **Gas impact** | Token transfers incur SEP-41 cross-contract call overhead (~2,000–4,000 CPU). |

### 8. `registry` — Off-Chain Registry Hint (IMMUTABLE)

Optional address of a registry contract for off-chain indexers. **Not an on-chain authority.**

| Aspect | Detail |
|--------|--------|
| **Purpose** | Discoverability hint for indexers and dashboards. |
| **Non-authority** | The contract never consults this address for auth or validation. Callers must query the registry directly to verify membership. |
| **Omission** | Setting to `null` simply omits the key from storage. |
| **Gas impact** | One extra `instance().set()` call at init (~500 CPU). |

### 9. `treasury` — Protocol Treasury (IMMUTABLE)

The address that receives terminal dust sweeps via `sweep_terminal_dust`.
**Cannot be changed after init.**

| Aspect | Detail |
|--------|--------|
| **Auth** | `sweep_terminal_dust` requires treasury's `require_auth()`. Admin cannot sweep unless it is also the treasury. |
| **Recommendation** | Use a protocol-owned multisig or DAO treasury address. |
| **Immutable** | Binding is permanent. |

### 10. `yield_tiers` — Tiered Yield Ladder (IMMUTABLE)

An optional ordered list of `YieldTier` structs that define yield incentives for
investors who commit to longer lock periods via `fund_with_commitment`.

```json
[
  { "min_lock_secs": 2592000,  "yield_bps": 1000 },
  { "min_lock_secs": 7776000,  "yield_bps": 1200 }
]
```

| Rule | Validation Error |
|------|-----------------|
| Each tier `yield_bps` in `0..=10_000` | Code 10: `TierYieldOutOfRange` |
| Each tier `yield_bps >= base_yield` | Code 11: `TierYieldBelowBase` |
| `min_lock_secs` strictly increasing across tiers | Code 12: `TierLockNotIncreasing` |
| `yield_bps` non-decreasing across tiers | Code 13: `TierYieldNotNonDecreasing` |

| Aspect | Detail |
|--------|--------|
| **Empty vec** | Same as `null` — tiers disabled; base yield applies to all. |
| **Max tiers** | No hard cap, but Vec is stored in instance storage. Practical limit ~20 tiers before storage cost dominates. |
| **Gas impact** | Tier lookup is O(n) on each `fund_with_commitment` call. Keep tiers ≤ 5 for optimal gas. |

### 11. `min_contribution` — Minimum Funding Floor

Minimum per-call deposit amount. Rejects `fund` / `fund_with_commitment` calls
with smaller amounts.

| Aspect | Detail |
|--------|--------|
| **Validation** | If set, must be `> 0` and `≤ amount` (target hint). |
| **Gas impact** | Single `i128` comparison; negligible. |
| **Recommendation** | Set to ~1% of target for retail-friendly invoices; omit for institutional-only. |

### 12. `max_unique_investors` — Distinct Investor Cap

Limits how many unique addresses can contribute to the escrow. Existing funders
can still add more principal even at the cap.

`Some(0)` is rejected during `init` with `MaxUniqueInvestorsNotPositive`; zero
is never treated as an active cap. Use `null` (`None`) for an unlimited number
of distinct investors.

| Aspect | Detail |
|--------|--------|
| **Default** | Unlimited when `null`. |
| **Mutable?** | Can only be **lowered** (not raised) via `lower_max_unique_investors` while Open. |
| **Lowering constraints** | New cap must be strictly lower (Code 77) and ≥ current funder count (Code 78). |
| **Gas impact** | Checked once per **new** investor. No cost for returning investors. |

### 13. `max_per_investor` — Per-Investor Contribution Cap

Limits total principal a single investor address can contribute across all deposits.

| Aspect | Detail |
|--------|--------|
| **Default** | Unlimited when `null`. |
| **Enforcement** | Checked on every `fund`/`fund_with_commitment` call against accumulated contribution. |
| **Gas impact** | Single `i128` comparison + storage read; negligible. |

### 14. `legal_hold_clear_delay` — Two-Phase Hold Clear Delay

Minimum delay (in seconds) between `request_clear_legal_hold` and `set_legal_hold(false)`.
Provides a timelock safety window when clearing a compliance hold.

| Value | Behavior |
|-------|----------|
| `null` or `0` | Hold can be cleared immediately by admin |
| `3600` | 1-hour delay — admin requests clear, must wait 1 hour before clearing |
| `86400` | 24-hour delay — common for production escrows |
| `604800` | 7-day delay — maximum safety for large-value escrows |

| Aspect | Detail |
|--------|--------|
| **Workflow** | `request_clear_legal_hold` → wait `delay` seconds → `clear_legal_hold` / `set_legal_hold(false)`. |
| **Bypass** | None. The delay is enforced on-chain. Code 150/151 if the workflow is violated. |
| **Gas impact** | One `u64` addition for delay calculation; negligible. |

### 15. `funding_deadline` — Funding Window Closure

A ledger timestamp after which `fund`/`fund_with_commitment`/`fund_batch` calls
are rejected with Code 164 (`FundingDeadlinePassed`).

| Aspect | Detail |
|--------|--------|
| **Default** | `null` — no deadline; funding remains open until target met or admin cancels. |
| **Validation** | Must be `> ledger.timestamp()` at init time. |
| **Recommendation** | Set a reasonable window (e.g., 30–90 days) for time-sensitive invoices. |

### 16. `yield_slippage_threshold` — Real-Time Slippage Detection

When set, `claim_investor_payout` compares actual vs. expected yield and emits
a `YieldSlippageWarning` event if deviation exceeds the threshold.

| Value | Behavior |
|-------|----------|
| `null` or `0` | No slippage check |
| `50` | Warn if actual yield deviates > 0.5% from expected |
| `100` | Warn if deviation > 1% |
| `500` | Warn if deviation > 5% |

| Aspect | Detail |
|--------|--------|
| **Range** | `0..=10_000` bps (Code 163: `YieldSlippageThresholdOutOfRange`) |
| **Effect** | Emits event only — does **not** block the claim. Integrators must monitor events. |
| **Gas impact** | Added comparison + optional event emission on each claim (~500–1,000 CPU). |

---

## Use Case Recommendations

### Fast Settlement (e.g., short-term trade finance, < 30 days)

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `maturity` | `0` | No time lock; settle immediately after funding |
| `yield_bps` | `300`–`500` | 3–5% for short-duration invoices |
| `yield_tiers` | `null` | Simple — no lock commitments needed |
| `min_contribution` | `null` or small | Low barrier to entry |
| `legal_hold_clear_delay` | `0` or `3600` | Immediate or 1-hour safety window |
| `funding_deadline` | 7–14 days | Urgent invoices close quickly |

### High Yield (e.g., long-term supply chain finance, 90–365 days)

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `maturity` | 90–365 days Unix | Long-dated settlement gate |
| `yield_bps` | `800`–`1500` | 8–15% for longer capital commitment |
| `yield_tiers` | 2–3 tiers | Incentivize longer locks for higher yield |
| `min_contribution` | 1–5% of target | Filter micro-deposits |
| `funding_deadline` | 30–90 days | Extended funding window |

**Example tier configuration for high-yield:**
```json
[
  { "min_lock_secs": 2592000,  "yield_bps": 900 },
  { "min_lock_secs": 7776000,  "yield_bps": 1200 },
  { "min_lock_secs": 15552000, "yield_bps": 1500 }
]
```

### Conservative / Institutional (e.g., large-value regulated invoices)

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `maturity` | Set to invoice due date | Enforce settlement timing |
| `yield_bps` | `500`–`800` | Moderate, stable yield |
| `yield_tiers` | `null` | Avoid tier complexity |
| `min_contribution` | 5–20% of target | Institutional-sized lots only |
| `max_unique_investors` | 5–20 | Limited syndicate |
| `max_per_investor` | 20–50% of target | Prevent single-investor dominance |
| `legal_hold_clear_delay` | `86400`–`604800` | 1–7 day safety window |
| `funding_deadline` | 30–60 days | Standard institutional window |
| `yield_slippage_threshold` | `100` | 1% slippage monitoring |

### Template-Based Fast Paths

The contract provides built-in templates via `init_from_template`:

| Template | `yield_bps` | `maturity` | Tiers | Best for |
|----------|-------------|------------|-------|----------|
| `fast` | 500 | 0 | None | Quick trade finance |
| `standard` | 800 | 0 | None | General invoice factoring |
| `conservative` | 600 | Future (+90d) | 3 tiers | Regulated / institutional |

Custom templates can be registered via `register_template`.

---

## Gas Cost Notes

All gas costs below are approximate for Stellar Soroban on a typical validator.

| Operation | Approx. CPU | Notes |
|-----------|-------------|-------|
| `init` (minimal) | ~15,000–25,000 | Baseline initialization with no optional features |
| `init` (full: tiers + caps + deadline + slippage) | ~25,000–40,000 | Each optional feature adds ~2,000–5,000 |
| `fund` / `fund_with_commitment` | ~5,000–12,000 | Higher with tier lookup, caps, allowlist checks |
| `settle` | ~3,000–6,000 | Simple status transition |
| `claim_investor_payout` | ~3,000–8,000 | Higher with slippage check |
| `sweep_terminal_dust` | ~6,000–12,000 | Includes cross-contract token transfer |
| Tier lookup (per `fund_with_commitment`) | ~500 per tier | O(n) — keep n ≤ 5 for optimal gas |

**Key takeaway:** The biggest gas drivers are:
1. Cross-contract calls (token transfers via `external_calls`)
2. Number of yield tiers in the ladder (linear scan)
3. Instance storage footprint (more keys = higher base load cost)

---

## Validation Rules Summary

The following conditions cause `init` to emit typed `EscrowError` codes:

| Condition | Code | Recovery |
|-----------|------|----------|
| `amount <= 0` | 1 | Pass a positive invoice amount |
| `yield_bps` outside `0..=10_000` | 2 | Correct yield configuration |
| Escrow already initialized | 3 | Deploy new contract instance |
| `invoice_id` length outside `1..=32` | 4 | Trim invoice ID to valid length |
| `invoice_id` has invalid characters | 5 | Use only `[A-Za-z0-9_]` |
| `min_contribution` set but `<= 0` | 6 | Omit or set positive floor |
| `min_contribution > amount` | 7 | Lower floor or raise target |
| `max_unique_investors` set but `<= 0` | 8 | Omit or set positive cap |
| `max_per_investor` set but `<= 0` | 9 | Omit or set positive cap |
| Tier `yield_bps` outside `0..=10_000` | 10 | Fix tier yield |
| Tier `yield_bps < base_yield` | 11 | Raise tier yield or lower base |
| Tier locks not strictly increasing | 12 | Sort tiers by `min_lock_secs` |
| Tier yields not non-decreasing | 13 | Sort tiers by non-decreasing yield |
| `funding_deadline <= ledger.timestamp()` | 164 | Set a future deadline |

---

## Related Documents

- [Escrow Lifecycle & State Machine](escrow-lifecycle.md)
- [Escrow Data Model](escrow-data-model.md)
- [Escrow Error Codes](escrow-error-messages.md)
- [CLI Simulation Recipes](escrow-sim-stellar-cli.md)
- [Operator Runbook](OPERATOR_RUNBOOK.md)
- [ADR-001: State Model](adr/ADR-001-state-model.md)
- [ADR-005: Tiered Yield](adr/ADR-005-tiered-yield.md)
- [ADR-008: Backup/Restore Rejection](adr/ADR-008-backup-restore-rejection.md)
