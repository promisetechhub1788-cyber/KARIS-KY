# Escrow Contract Events

This document provides a reference for indexers and block explorers to consume events emitted by the Liquifact Escrow contract.

## 📡 Event Structure

All events follow the Soroban `contractevent` format. Key fields like `invoice_id` and `investor` are marked as **topics** to enable efficient filtering by indexers.

### Common Topics
- **Topic 0**: Contract ID (provided by Soroban host).
- **Topic 1**: Event Name (Symbol, e.g., `funded`, `escrow_sd`).
- **Topic 2**: `invoice_id` (Symbol) — present in most events.
- **Topic 3**: `investor` (Address) — present in funding and claim events.

---

## 📋 Event Catalog

### `EscrowInitialized`
Emitted once by `init()`. Carries the escrow snapshot plus immutable bound references so
indexers can register `funding_token`, `treasury`, and optional `registry` without follow-up reads.

**Topics:**
1. `escrow_ii` (Symbol)

**Data Payload:**
- `escrow` (`InvoiceEscrow`)
- `funding_token` (`Address`) — equals `DataKey::FundingToken`
- `treasury` (`Address`) — equals `DataKey::Treasury`
- `registry` (`Option<Address>`) — equals `DataKey::RegistryRef`
- `has_maturity_lock` (`bool`) — false when `maturity == 0`, meaning settlement has no maturity time lock

**Example (JSON Decoded):**
```json
{
  "topics": ["escrow_ii"],
  "data": {
    "escrow": { "invoice_id": "INV_001", "status": 0 },
    "funding_token": "CTOKEN...",
    "treasury": "GTREAS...",
    "registry": "GREG...",
    "has_maturity_lock": true
  }
}
```

### `MaxUniqueInvestorsCapLowered`
Emitted when admin calls `lower_max_unique_investors` while the escrow is open.

**Topics:**
1. `inv_cap` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `old_cap` (u32)
- `new_cap` (u32)

### `EscrowFunded`
Emitted when an investor deposits principal.

**Topics:**
1. `funded` (Symbol)
2. `invoice_id` (Symbol)
3. `investor` (Address)

**Data Payload:**
- `amount` (i128)
- `funded_amount` (i128)
- `status` (u32)
- `investor_effective_yield_bps` (i64)

**Example (JSON Decoded):**
```json
{
  "topics": ["funded", "INV_001", "G...INVESTOR"],
  "data": {
    "amount": "1000000000",
    "funded_amount": "5000000000",
    "status": 0,
    "investor_effective_yield_bps": 500
  }
}
```

### `BatchFunded`
Emitted once per `fund_batch` call after all per-investor deposits in the batch have
been applied. Indexers should treat this as the batch-level summary and continue to
rely on the per-investor `EscrowFunded` events for individual positions.

**Topics:**
1. `batch_fd` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `investor_count` (u32) — number of investors funded in the batch
- `total_amount` (i128) — sum of principal deposited across the batch
- `funded_amount` (i128) — escrow `funded_amount` after the batch was applied
- `status` (u32) — escrow status after the batch was applied

**Example (JSON Decoded):**
```json
{
  "topics": ["batch_fd", "INV_001"],
  "data": {
    "investor_count": 3,
    "total_amount": "3000000000",
    "funded_amount": "8000000000",
    "status": 0
  }
}
```

### `EscrowSettled`
Emitted when the SME finalizes the escrow after maturity.

**Topics:**
1. `escrow_sd` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `funded_amount` (i128)
- `yield_bps` (i64)
- `maturity` (u64)
- `settled_at_ledger_timestamp` (u64) — the ledger timestamp when `settle` was called

**Example (JSON Decoded):**
```json
{
  "topics": ["escrow_sd", "INV_001"],
  "data": {
    "funded_amount": "10000000000",
    "yield_bps": 500,
    "maturity": 1714184400,
    "settled_at_ledger_timestamp": 1714184400
  }
}
```

### `SmeWithdrew`
Emitted by `withdraw` after the funded principal has been transferred to the SME and the escrow status has moved to `3` (withdrawn).

**Topics:**
1. `sme_wd` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `amount` (i128) — the escrow's `funded_amount`, transferred to the SME
- `recipient` (Address) — the escrow's `sme_address`

**Example (JSON Decoded):**
```json
{
  "topics": ["sme_wd", "INV_001"],
  "data": {
    "amount": "10000000000",
    "recipient": "G...SME"
  }
}
```

### `InvestorPayoutClaimed`
Emitted when an investor records their payout claim.

**Topics:**
1. `inv_claim` (Symbol)
2. `invoice_id` (Symbol)
3. `investor` (Address)

**Example (JSON Decoded):**
```json
{
  "topics": ["inv_claim", "INV_001", "G...INVESTOR"],
  "data": null
}
```

### `InvestorAllowlistChanged`
Emitted when an admin adds or removes an investor from the allowlist. This event is
emitted per-address even when the change is performed via the batch entrypoint
`set_investors_allowlisted`, so indexers receive one `InvestorAllowlistChanged` event
for each address in the batch.

**Topics:**
1. `al_set` (Symbol)
2. `invoice_id` (Symbol)
3. `investor` (Address)

**Data Payload:**
- `allowed` (u32): `1` for allowed, `0` for blocked.

**Notes:**
- Batch mutations via `set_investors_allowlisted` emit one `al_set` event per affected
  investor to preserve parity with individual `set_investor_allowlisted` calls.

### `LegalHoldChanged`
Emitted when an admin toggles the compliance hold.

**Topics:**
1. `legalhld` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `active` (u32): `1` for enabled, `0` for cleared.

### `CollateralDeposited`
Emitted when collateral is posted against an escrow via `deposit_collateral`.

**Topics:**
1. `coll_dep` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `depositor` (`Address`) — account that supplied the collateral
- `amount` (i128) — collateral amount deposited in this call
- `total_collateral` (i128) — escrow collateral balance after the deposit

### `CollateralReleased`
Emitted when previously posted collateral is released back to the depositor via
`release_collateral`.

**Topics:**
1. `coll_rel` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `recipient` (`Address`) — account receiving the released collateral
- `amount` (i128) — collateral amount released in this call
- `total_collateral` (i128) — escrow collateral balance after the release

### `CollateralSwept`
Emitted when residual collateral is swept to the treasury via `sweep_collateral`.

**Topics:**
1. `coll_swp` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `treasury` (`Address`) — treasury account receiving the swept collateral
- `amount` (i128) — collateral amount swept in this call
- `total_collateral` (i128) — escrow collateral balance after the sweep

### `Swept`
Emitted when the escrow's residual funding-token balance is swept to the treasury via
`sweep`. This is distinct from `CollateralSwept`, which only moves collateral.

**Topics:**
1. `swept` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `treasury` (`Address`) — treasury account receiving the swept funds
- `amount` (i128) — funding-token amount swept in this call
- `remaining_balance` (i128) — escrow funding-token balance after the sweep

### `DisputePausedEvt`
Emitted when an admin activates a dispute pause. The `action` field is `1` for
this event; resumes use the separate `DisputeResumedEvt` event.

**Topics:**
1. `disppause` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `ticket_id` (`String`)
- `action` (`u32`): `1` = paused
- `paused_at` (`u64`): ledger timestamp when the pause began
- `expires_at` (`u64`): configured expiry timestamp

### `DisputeResumedEvt`
Emitted when an admin calls `resume_dispute`.

**Topics:**
1. `disp_res` (Symbol)
2. `invoice_id` (Symbol)

**Data Payload:**
- `admin` (`Address`)
- `resumed_by` (`DisputeResumedBy`): `Manual` or `AutoExpiry`
- `ledger_timestamp` (`u64`): ledger timestamp when the resume was recorded

Expired pauses emit `DisputeResumedEvt` with `AutoExpiry` when first observed
by a mutating operation. `is_dispute_paused` reads do not emit lifecycle
events.

### `AttestationBoundEvt`
Emitted after a successful `bind_primary_attestation_hash`. The legacy
`PrimaryAttestationBound` event is also emitted for existing consumers.

**Topics:**
1. `att_bound` (Symbol)

**Data Payload:**
- `hash` (`BytesN<32>`) — the bound SHA-256 digest; represent as 64 hexadecimal characters off-chain
- `ledger_timestamp` (`u64`) — ledger timestamp at binding, represented as a decimal string in JSON

### `UpgradeProposedEvt`
Emitted when an admin calls `propose_upgrade(new_wasm_hash)`. The proposal does not
take effect immediately; it can only be applied by `execute_upgrade` once the
minimum delay has elapsed. Indexers should use this event to surface pending
upgrades and their earliest executable time.

**Topics:**
1. `upg_prop` (Symbol)

**Data Payload:**
- `new_wasm_hash` (`BytesN<32>`) — the proposed WASM hash; represent as 64 hexadecimal characters off-chain
- `proposed_at` (`u64`) — ledger timestamp when the proposal was recorded
- `executable_at` (`u64`) — ledger timestamp at which `execute_upgrade` becomes callable (`proposed_at + delay`)

**Example (JSON Decoded):**
```json
{
  "topics": ["upg_prop"],
  "data": {
    "new_wasm_hash": "ab12...ef34",
    "proposed_at": 1714184400,
    "executable_at": 1714188000
  }
}
```

### `UpgradeExecutedEvt`
Emitted when an admin calls `execute_upgrade` and the pending WASM hash is applied.
A proposal is cleared once executed, so a subsequent `execute_upgrade` requires a
new `propose_upgrade`.

**Topics:**
1. `upg_exec` (Symbol)

**Data Payload:**
- `new_wasm_hash` (`BytesN<32>`) — the applied WASM hash; represent as 64 hexadecimal characters off-chain
- `executed_at` (`u64`) — ledger timestamp when the upgrade was applied

**Example (JSON Decoded):**
```json
{
  "topics": ["upg_exec"],
  "data": {
    "new_wasm_hash": "ab12...ef34",
    "executed_at": 1714188000
  }
}
```

---

## 🗑️ Deprecated Events

The following events are retained for backwards compatibility with existing indexers
but are no longer emitted by the current contract. New integrations should not rely
on them.

### `PrimaryAttestationBound` *(deprecated)*
Superseded by `AttestationBoundEvt`. The current contract emits `AttestationBoundEvt`
for all new bindings; `PrimaryAttestationBound` is only referenced for legacy
consumers and should be treated as deprecated.

**Topics:**
1. `att_bnd` (Symbol)

**Data Payload:**
- `hash` (`BytesN<32>`)

---

## 🛠️ Indexing Recommendations

### Filtering by Invoice
To track all activity for a specific invoice, indexers should filter for events where **Topic 2** matches the `invoice_id`.

### Filtering by Investor
To track an investor's portfolio, filter for events where **Topic 3** matches the investor's `Address`. This applies to `EscrowFunded` and `InvestorPayoutClaimed`.

### Decoding payloads
Payloads are XDR-encoded. Use the `karis-ky_escrow` WASM/interface or the `Stellar SDK` to decode the `data` field into the corresponding Rust structs.
