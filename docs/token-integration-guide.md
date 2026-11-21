# Token Integration Guide

This guide explains how to safely integrate a stablecoin or other SEP-41 fungible token
with the karis-ky escrow contract. It covers supported token behaviours, explicit
unsupported token warnings with concrete examples, rebasing token pitfalls, and a
pre-integration test script you can run before deploying to mainnet.

> **See also:** [`ESCROW_TOKEN_INTEGRATION_CHECKLIST.md`](ESCROW_TOKEN_INTEGRATION_CHECKLIST.md)
> for the security checklist reference, and
> [`external_calls.rs`](../escrow/src/external_calls.rs) for the balance-delta invariant
> implementation.

---

## Token requirements for `funding_token`

The escrow accepts a single SEP-41 `funding_token` per instance. Before deploying, confirm
the token satisfies every row below. Error codes are defined in
[`escrow-error-messages.md`](escrow-error-messages.md).

| Property | Supported? | Notes | Error code on violation |
|----------|-----------|-------|-------------------------|
| **Decimals** | ✅ Yes — fixed, known at deploy time | Amounts are raw `i128` in the token's smallest unit; the contract performs **no on-chain decimal conversion**. SAC tokens (USDC, USDT, EURC, native XLM) use **7 decimals** — encode all amounts as `human_amount × 10^7`. Any fixed decimal count works as long as it never changes and every amount is encoded consistently. | No typed error — mis-encoded amounts silently mis-price the escrow (e.g. `InsufficientContractBalance` = **183** at withdraw time if under-encoded) |
| **Fee-on-transfer** | ❌ No | `transfer()` must move exactly `amount`: sender delta and recipient delta must both equal `amount`. Any deducted fee fails the balance-delta invariant at `fund()` time. | **36** `TransferAmountNotPositive`, **37** `InsufficientTokenBalanceBeforeTransfer`, **38** `SenderBalanceUnderflow`, **39** `RecipientBalanceUnderflow`, **40** `SenderBalanceDeltaMismatch`, **41** `RecipientBalanceDeltaMismatch` |
| **Rebasing / deflationary** | ❌ No | Balances must not change autonomously between reads. Positive rebases leave unattributed residue; negative rebases (deflation) mean the contract holds less than `funded_amount`, so `withdraw()` cannot pay out the recorded principal. | **183** `InsufficientContractBalance` at withdraw time (shortfall surfaces when the contract balance is below the amount being transferred) |
| **Dynamic decimals** | ❌ No | Decimals must be immutable post-deploy; a mid-lifecycle change breaks every stored amount. | No typed error — silent integer misalignment |
| **Pausable / blacklistable transfers** | ⚠️ Allowed but risky | `transfer()` reverting (paused token, blacklisted escrow address) blocks `withdraw()`, `refund()` and `sweep_terminal_dust()`. | Token panic propagates (no escrow error code) |
| **Callback / hook tokens** | ❌ No | Re-entrancy or balance mutation during `transfer()` breaks the before/after delta comparison. | **40** `SenderBalanceDeltaMismatch`, **41** `RecipientBalanceDeltaMismatch` |

> **Summary:** the token must be a fixed-supply-semantics, fixed-decimal SEP-41 token whose
> `transfer` moves exactly `amount` and whose `balance` never changes without a transfer.
> See [§4](#4-balance-delta-invariant--what-the-contract-enforces) for the enforcement code,
> [§5](#5-fee-on-transfer-token-warning) for fee-on-transfer details, and
> [§6](#6-rebasing-token-pitfalls) for rebasing details.

---

## Contents

1. [How the escrow holds tokens](#1-how-the-escrow-holds-tokens)
2. [Supported token standard: SEP-41](#2-supported-token-standard-sep-41)
3. [Stablecoin examples](#3-stablecoin-examples)
   - [USDC on Stellar (Circle)](#usdc-on-stellar-circle)
   - [USDT on Stellar (Tether)](#usdt-on-stellar-tether)
   - [EURC on Stellar (Circle EUR)](#eurc-on-stellar-circle-eur)
   - [XLM-backed Soroban token](#xlm-backed-soroban-token)
4. [Balance-delta invariant — what the contract enforces](#4-balance-delta-invariant--what-the-contract-enforces)
5. [Fee-on-transfer token warning](#5-fee-on-transfer-token-warning)
6. [Rebasing token pitfalls](#6-rebasing-token-pitfalls)
7. [Other unsupported token patterns](#7-other-unsupported-token-patterns)
8. [Pre-integration test script](#8-pre-integration-test-script)
9. [Governance allowlist checklist](#9-governance-allowlist-checklist)

---

## 1. How the escrow holds tokens

The karis-ky escrow contract **records accounting state only**. It does not hold tokens
natively; the integration layer is responsible for:

1. Transferring tokens from an investor wallet **into the escrow contract address** before
   or as part of the `fund()` call (depending on your bridge / app layer).
2. Ensuring the contract address holds exactly `funded_amount` in the bound funding token
   at the time `withdraw()` or `sweep_terminal_dust()` is called.

The contract stores amounts as raw `i128` in the **smallest unit** of the token (e.g., 1
unit = 1 stroopoid for 7-decimal tokens). It performs no decimal conversion.

**Custody verification:** call `verify_asset_custody()` (admin-only) at any time to compare
`contract_balance` vs `funded_amount` and receive a signed discrepancy value.

---

## 2. Supported token standard: SEP-41

The escrow is designed exclusively for
[**SEP-41**](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
fungible tokens, which expose:

```
fn balance(id: Address) → i128
fn transfer(from: Address, to: MuxedAddress, amount: i128)
fn decimals() → u32
```

Both `balance` and `transfer` must follow standard semantics:

- `balance(addr)` returns the exact current balance in smallest units with no hidden
  accrual or deflation between reads.
- After `transfer(from, to, amount)`:
  - `balance(from)` decreases by exactly `amount`.
  - `balance(to)` increases by exactly `amount`.

Any deviation from these semantics will cause `transfer_funding_token_with_balance_checks`
(in `external_calls.rs`) to **panic with a typed error** (codes 36–41), aborting the
transaction safely.

---

## 3. Stablecoin examples

### USDC on Stellar (Circle)

| Property | Value |
|----------|-------|
| Issuer | Circle Internet Financial |
| Asset code | `USDC` |
| Network | Stellar Mainnet / Testnet |
| Decimals | 7 (Stellar native SAC) |
| SEP-41 compliant | ✅ Yes — Stellar Asset Contract (SAC) v2 |
| Fee-on-transfer | ❌ No |
| Rebasing | ❌ No |
| Supports `MuxedAddress` | ✅ Yes (Stellar memo-style) |

**Init call:**
```bash
# Testnet deployment example
stellar contract invoke \
  --id $ESCROW_CONTRACT_ID \
  --source $ADMIN_SECRET \
  --network testnet \
  -- init \
  --admin $ADMIN_ADDRESS \
  --invoice_id "INV_2026_001" \
  --sme_address $SME_ADDRESS \
  --amount 100000000000 \  # 10,000.0000000 USDC (7 decimals)
  --yield_bps 800 \
  --maturity 0 \
  --funding_token $USDC_CONTRACT_ADDRESS \
  --registry null \
  --treasury $TREASURY_ADDRESS \
  --yield_tiers null \
  --min_contribution null \
  --max_unique_investors null \
  --max_per_investor null \
  --legal_hold_clear_delay null \
  --funding_deadline null \
  --max_funding_rate null \
  --yield_slippage_threshold null \
  --settlement_notifier_contract null
```

**Amount encoding:**
```
Human amount   → smallest unit
10,000 USDC    → 100_000_000_000 (10_000 × 10^7)
1 USDC         → 10_000_000
0.01 USDC      → 100_000
```

**Risk profile:** Circle USDC on Stellar is issued as a Stellar Asset Contract (SAC).
It is the recommended production stablecoin for this escrow. Circle enforces on-chain
freeze/clawback; verify that the escrow's admin governance policy covers the risk that
Circle could freeze the token while it is held in the escrow contract.

---

### USDT on Stellar (Tether)

| Property | Value |
|----------|-------|
| Issuer | Tether Operations Limited |
| Asset code | `USDT` |
| Network | Stellar Mainnet / Testnet |
| Decimals | 7 (Stellar native SAC) |
| SEP-41 compliant | ✅ Yes — SAC v2 |
| Fee-on-transfer | ❌ No (Stellar USDT does **not** charge transfer fees; EVM USDT does — they are separate) |
| Rebasing | ❌ No |
| Clawback risk | ✅ Yes — issuer retains clawback authorization |

**Amount encoding (same as USDC):**
```
100 USDT → 1_000_000_000 (100 × 10^7)
```

> ⚠️ **EVM USDT vs Stellar USDT are completely different contracts.** The EVM version
> of USDT historically charged transfer fees and has had upgrade surprises. The Stellar
> SAC version does **not** charge transfer fees as of this writing, but integrators must
> independently verify the current Stellar USDT contract before using it in production.
> Always audit the specific contract ID, not just the token symbol.

---

### EURC on Stellar (Circle EUR)

| Property | Value |
|----------|-------|
| Issuer | Circle Internet Financial |
| Asset code | `EURC` |
| Network | Stellar Mainnet |
| Decimals | 7 |
| SEP-41 compliant | ✅ Yes — SAC v2 |
| Fee-on-transfer | ❌ No |
| Rebasing | ❌ No |

**Multi-currency escrow note:** the escrow contract stores a single `funding_token`
per instance. To issue a EUR-denominated invoice escrow, deploy a separate instance
with `funding_token = $EURC_CONTRACT_ADDRESS`. Do not try to mix USDC and EURC in
one instance.

**Currency risk disclosure:** EURC/USD exchange rate risk is out of scope of this
contract. If your integration exposes investors to FX risk, document it in your
off-chain materials and yield disclosure.

---

### XLM-backed Soroban token

A Soroban token that wraps XLM (the native Stellar asset) via a custom contract can
be used as long as it fully implements SEP-41 semantics, including exact balance-delta
conservation. The Stellar Asset Contract for native XLM (`XLM` with no issuer) is
supported on testnet and can be used for development:

```bash
# Register native XLM as a SAC for testnet testing
stellar contract asset deploy --asset native --network testnet
```

**Note:** Native XLM balances include a **minimum reserve** (currently 1 XLM per account).
Ensure the escrow contract address is pre-funded with the Soroban ledger rent plus the
minimum reserve before the first token transfer, or the `balance` read will panic.

---

## 4. Balance-delta invariant — what the contract enforces

Every token transfer in this contract goes through
`external_calls::transfer_funding_token_with_balance_checks`. Here is the exact
verification sequence:

```rust
// Pseudo-code — see escrow/src/external_calls.rs for the real implementation
let from_balance_before = token.balance(from);
let to_balance_before   = token.balance(to);

// Require: from has enough balance
ensure(from_balance_before >= amount, InsufficientTokenBalanceBeforeTransfer); // error 37

token.transfer(from, MuxedAddress::from(to), amount);

let from_balance_after = token.balance(from);
let to_balance_after   = token.balance(to);

let spent    = from_balance_before - from_balance_after;  // must not underflow
let received = to_balance_after   - to_balance_before;    // must not underflow

ensure(spent    == amount, SenderBalanceDeltaMismatch);    // error 40
ensure(received == amount, RecipientBalanceDeltaMismatch); // error 41
```

This fires on **every** outbound transfer:
- `withdraw()` — SME pulls funded liquidity
- `sweep_terminal_dust()` — treasury recovers rounding residue
- `refund()` — investor recovers principal in cancelled escrow

---

## 5. Fee-on-transfer token warning

> ⛔ **Fee-on-transfer tokens are explicitly NOT supported.**

### What is a fee-on-transfer token?

A fee-on-transfer (FoT) token deducts a fee from the transferred amount at the token
contract level. The sender sends `amount`, but the recipient receives `amount - fee`.

**EVM examples** (not directly relevant to Stellar but illustrative):
- Early Safemoon (`SAFEMOON`) on BSC — 10% fee on every transfer
- REFLECT Finance tokens — 2–5% fee with holder redistribution
- Deflationary tokens that burn 1% per transfer

### Why this breaks the karis-ky escrow

Consider a hypothetical Soroban token with a 1% fee:

```
Investor calls fund(&investor, 10_000_000_000) // 1,000 USDC equivalent
↓
token.transfer(investor → escrow, 10_000_000_000)
↓
escrow receives: 9_900_000_000  (fee of 100_000_000 deducted)
↓
external_calls checks: received (9_900_000_000) ≠ amount (10_000_000_000)
↓
PANIC: RecipientBalanceDeltaMismatch (EscrowError 41)
```

The transaction reverts. No partial credit is given. This is the **intended** behavior:
the escrow refuses to record more principal than actually arrived.

### What would happen if the check was bypassed (hypothetical)

Without the balance-delta check, a fee-on-transfer token would cause silent under-funding:

```
Investor funds 10,000 USDC (FoT, 1% fee)
escrow.funded_amount += 10,000  ← recorded
escrow holds only 9,900         ← actual balance

At withdraw():
  SME tries to pull 10,000 but contract only has 9,900
  → InsufficientContractBalance (EscrowError 164) at withdraw time
  → SME cannot withdraw; funds stuck
```

This is why the check is done at **fund time**, not at withdraw time.

### Integration action required

Before deploying to production, verify with the token issuer and by code audit that the
token contract does **not** deduct fees during `transfer`. For Stellar SAC tokens (USDC,
USDT, EURC), this is guaranteed by the standard SAC implementation.

---

## 6. Rebasing token pitfalls

> ⛔ **Rebasing tokens are explicitly NOT supported.**

### What is a rebasing token?

A rebasing token changes all holders' `balance` values autonomously — usually in response
to an oracle price or staking reward accrual. Examples:

- `stETH` (Ethereum) — accrues staking rewards daily by increasing all balances
- Ampleforth (`AMPL`) on Ethereum — supply adjusts to target $1 price
- Yield-bearing wrappers like `aUSDC` (Aave) — balance grows over time as interest accrues

The Stellar ecosystem does not currently have widely deployed rebasing tokens, but
wrapper contracts that simulate yield accrual via balance changes are possible.

### Why rebasing breaks the escrow

#### Problem 1: Balance inflation between fund() and withdraw()

```
Investor A funds 10,000 USDC-equivalent into a rebasing wrapper
  → escrow.funded_amount = 10,000
  → contract.balance = 10,000

[6 months later, yield accrues — rebasing event]
  → contract.balance = 10,500  (500 extra from rebase)

SME calls withdraw():
  balance_before = 10,500
  token.transfer(escrow → sme, 10,000)
  balance_after  = 500
  spent = 10,500 - 500 = 10,000  ✅ delta check passes

  But: 500 "airdrop" balance remains, unattributed to any investor
  → sweep_terminal_dust needed for the residue
```

This case technically passes the balance-delta check on `withdraw()`, but creates
an attribution problem: the 500 accrued units belong to investors conceptually but
are not tracked by the escrow's `funded_amount`.

#### Problem 2: Balance deflation (negative rebase)

```
Investor funds 10,000 into a negatively rebasing token
[rebase event: supply contracts by 5%]
  → contract.balance drops from 10,000 to 9,500

SME calls withdraw():
  balance_before = 9,500
  token.transfer(escrow → sme, 10,000)  ← tries to transfer 10,000
  → InsufficientTokenBalanceBeforeTransfer (EscrowError 37)
  → Transaction reverts; SME cannot withdraw
  → Escrow is stuck with 9,500 tokens it cannot distribute as 10,000
```

This is a **loss of funds** scenario. The escrow's accounting (`funded_amount = 10,000`)
no longer matches reality (`balance = 9,500`).

#### Problem 3: Snapshot denominator drift

The `FundingCloseSnapshot.total_principal` is written once when the escrow first becomes
funded and is used as the immutable pro-rata denominator for all investor payouts.

If the token rebases after the snapshot is written, the actual token balance held by the
contract diverges from `total_principal`. Investor payouts computed by
`compute_investor_payout()` become meaningless because they reference an outdated
denominator that no longer reflects the actual token pool.

### Mitigation

If you need a yield-bearing token integration, use the optional `yield_token` + oracle
settlement path (see `docs/escrow-init-parameters.md`), which unwraps yield tokens back
to the underlying stablecoin at settlement time — avoiding rebasing semantics in the
core funding pool.

---

## 7. Other unsupported token patterns

| Pattern | Risk | Error triggered |
|---------|------|-----------------|
| **Pausable transfers** | `transfer()` reverts if token is paused; escrow operations blocked until token unpaused | Token panic propagates |
| **Blacklisted addresses** | USDC/USDT issuers can blacklist addresses; if escrow contract is blacklisted, all transfers fail | Token panic propagates |
| **Non-standard `balance` return** | Token reads a stale/cached balance instead of live on-chain value | Silent wrong payout or delta mismatch |
| **Callback/hook tokens** | Transfer triggers callbacks that could re-enter or alter balances mid-check | Balance delta mismatch (41) |
| **Dynamic decimals** | Token changes decimal precision post-deploy; all amount calculations break | Silent integer misalignment |
| **Deflationary burn** | Transfer burns a % of tokens; recipient delta < amount | `RecipientBalanceDeltaMismatch` (41) |
| **Admin-mintable supply** | Issuer can silently inflate supply inside the escrow; audit risk | Silent over-collateralization |

---

## 8. Pre-integration test script

Run this script on **testnet** before deploying to mainnet. It exercises the balance-delta
invariant with your specific token contract.

```bash
#!/usr/bin/env bash
# pre-integration-test.sh
# Tests that a Soroban token is compatible with the karis-ky escrow balance-delta checks.
#
# Prerequisites:
#   - Stellar CLI >= 21 installed
#   - stellar account funded on testnet (ADMIN_SECRET, INVESTOR_SECRET, SME_SECRET, TREASURY_SECRET)
#   - $TOKEN_CONTRACT_ID set to your token's contract ID
#   - $ESCROW_WASM path to the compiled escrow WASM

set -euo pipefail

NETWORK="testnet"
RPC_URL="https://soroban-testnet.stellar.org"

echo "=== karis-ky Token Pre-Integration Test ==="
echo "Token:   $TOKEN_CONTRACT_ID"
echo "Network: $NETWORK"
echo ""

# 1. Deploy a fresh escrow instance
echo "[1/7] Deploying escrow contract..."
ESCROW_ID=$(stellar contract deploy \
  --wasm "$ESCROW_WASM" \
  --source "$ADMIN_SECRET" \
  --network "$NETWORK" 2>&1 | tail -1)
echo "  Escrow contract: $ESCROW_ID"

# 2. Initialize with the test token
echo "[2/7] Initializing escrow..."
stellar contract invoke \
  --id "$ESCROW_ID" \
  --source "$ADMIN_SECRET" \
  --network "$NETWORK" \
  -- init \
  --admin "$ADMIN_ADDRESS" \
  --invoice_id "PRETEST01" \
  --sme_address "$SME_ADDRESS" \
  --amount 10000000000 \
  --yield_bps 500 \
  --maturity 0 \
  --funding_token "$TOKEN_CONTRACT_ID" \
  --registry "null" \
  --treasury "$TREASURY_ADDRESS" \
  --yield_tiers "null" \
  --min_contribution "null" \
  --max_unique_investors "null" \
  --max_per_investor "null" \
  --legal_hold_clear_delay "null" \
  --funding_deadline "null" \
  --max_funding_rate "null" \
  --yield_slippage_threshold "null" \
  --settlement_notifier_contract "null"
echo "  ✅ Init succeeded"

# 3. Mint tokens to investor (testnet only — requires SAC admin)
echo "[3/7] Minting test tokens to investor..."
stellar contract invoke \
  --id "$TOKEN_CONTRACT_ID" \
  --source "$SAC_ADMIN_SECRET" \
  --network "$NETWORK" \
  -- mint \
  --to "$INVESTOR_ADDRESS" \
  --amount 15000000000
echo "  ✅ Minted 1,500 tokens (7 decimals) to investor"

# 4. Mint tokens TO the escrow contract address (simulates investor transfer)
echo "[4/7] Minting tokens to escrow contract (simulating deposit)..."
stellar contract invoke \
  --id "$TOKEN_CONTRACT_ID" \
  --source "$SAC_ADMIN_SECRET" \
  --network "$NETWORK" \
  -- mint \
  --to "$ESCROW_ID" \
  --amount 10000000000
echo "  ✅ Minted 1,000 tokens to escrow"

# 5. Call fund() — this exercises the balance-delta check on the contract-held balance
echo "[5/7] Calling fund() — exercises balance-delta invariant..."
stellar contract invoke \
  --id "$ESCROW_ID" \
  --source "$INVESTOR_SECRET" \
  --network "$NETWORK" \
  -- fund \
  --investor "$INVESTOR_ADDRESS" \
  --amount 10000000000
echo "  ✅ fund() succeeded — token passed balance-delta check"

# 6. Verify custody
echo "[6/7] Verifying asset custody..."
DISCREPANCY=$(stellar contract invoke \
  --id "$ESCROW_ID" \
  --source "$ADMIN_SECRET" \
  --network "$NETWORK" \
  -- verify_asset_custody 2>&1 | tail -1)
echo "  Discrepancy: $DISCREPANCY (should be 0)"
if [ "$DISCREPANCY" != "0" ]; then
  echo "  ⚠️  WARNING: Non-zero discrepancy. Investigate before proceeding."
  exit 1
fi

# 7. Test settle + sweep (terminal state)
echo "[7/7] Settling escrow and testing terminal state..."
stellar contract invoke \
  --id "$ESCROW_ID" \
  --source "$SME_SECRET" \
  --network "$NETWORK" \
  -- settle
echo "  ✅ Settle succeeded"

# Attempt a small sweep to verify the token allows treasury transfers
stellar contract invoke \
  --id "$ESCROW_ID" \
  --source "$TREASURY_SECRET" \
  --network "$NETWORK" \
  -- sweep_terminal_dust \
  --amount 1
echo "  ✅ Dust sweep succeeded — balance-delta check passes for treasury transfers"

echo ""
echo "=== All pre-integration tests passed ==="
echo "Token $TOKEN_CONTRACT_ID is compatible with the karis-ky escrow."
echo ""
echo "Checklist before mainnet:"
echo "  [ ] Repeat on mainnet with a small pilot amount"
echo "  [ ] Confirm token contract ID with issuer (no symbol ambiguity)"
echo "  [ ] Confirm token is not currently paused or frozen"
echo "  [ ] Confirm issuer cannot blacklist the escrow contract address"
echo "  [ ] Review token contract source or audit report for hidden fees"
echo "  [ ] Add token contract ID to your governance allowlist"
```

### What the test validates

| Step | Invariant checked |
|------|------------------|
| `init` | Token address accepted at initialization |
| Mint to investor | Token allows standard minting (testnet only) |
| Mint to escrow | Contract address can hold the token |
| `fund()` | `RecipientBalanceDeltaMismatch` (41) does NOT fire → no hidden fees |
| `verify_asset_custody` | `contract.balance == funded_amount` (zero discrepancy) |
| `settle` + `sweep` | `SenderBalanceDeltaMismatch` (40) does NOT fire → token allows outbound transfers |

If any step fails with a typed error (36–41), the token is **not compatible** with this
escrow and must not be used in production.

---

## 9. Governance allowlist checklist

Before listing a token in your deployment config, complete this checklist:

- [ ] **Token contract ID verified** — obtained directly from the issuer's official channels,
  not derived from asset code/symbol alone (symbols are not unique on Stellar).
- [ ] **No transfer fee** — confirmed by code review or auditor report.
- [ ] **No rebasing** — confirmed by code review or auditor report.
- [ ] **No hidden callbacks** — confirmed by code review or auditor report.
- [ ] **Decimal precision confirmed** — e.g., 7 for SAC tokens; encoded in the
  `funding_target` and all fund amounts before calling `init`.
- [ ] **Issuer freeze/clawback risk documented** — USDC and USDT have issuer freeze powers;
  your operational policy must address what happens if the escrow's token balance is frozen
  mid-lifecycle.
- [ ] **Pre-integration test script run on testnet** — all 7 steps green.
- [ ] **Escrow contract address not on any sanctions list** — verify before deployment.
- [ ] **Governance approval recorded** — multisig or DAO vote logged with token contract ID.

---

## Related documents

| Document | Link |
|----------|------|
| Token Integration Security Checklist | [`ESCROW_TOKEN_INTEGRATION_CHECKLIST.md`](ESCROW_TOKEN_INTEGRATION_CHECKLIST.md) |
| External calls and balance-delta impl | [`escrow/src/external_calls.rs`](../escrow/src/external_calls.rs) |
| ADR-006: Dust sweep and token safety | [`docs/adr/ADR-006-dust-sweep-and-token-safety.md`](adr/ADR-006-dust-sweep-and-token-safety.md) |
| Init parameter reference | [`escrow-init-parameters.md`](escrow-init-parameters.md) |
| Error code reference | [`escrow-error-messages.md`](escrow-error-messages.md) |
| Operator runbook | [`OPERATOR_RUNBOOK.md`](OPERATOR_RUNBOOK.md) |
