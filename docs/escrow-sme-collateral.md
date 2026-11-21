# SME Collateral Commitment Metadata

`record_sme_collateral_commitment(asset, amount, collateral_type)` in [`escrow/src/lib.rs`](../escrow/src/lib.rs) is a metadata-only Soroban escrow entrypoint. It allows the configured SME address to report collateral metadata for off-chain review, but it does **not** move, reserve, escrow, freeze, or verify any asset on-chain.

## Limitations & Contrast with Custody Flows

> [!WARNING]
> This entrypoint is **metadata-only**. It writes metadata to contract instance storage and emits a Soroban event. It does **not** act as an enforced lien or asset custody mechanism.

To prevent integration risks, integrators must understand how this metadata-only flow contrasts with on-chain asset custody:
- **No Token Transfers:** Calling this function does not transfer any tokens from the SME to the escrow contract, nor does it interact with any token contracts.
- **No Reserve Balances:** It does not freeze or lock any on-chain balances.
- **No Custody Verification:** The escrow contract does not verify that the SME actually owns, holds, or has custody of the referenced asset.
- **No Enforcement or Blocking:** Recording a collateral commitment does not block, gate, or restrict any other contract flows. Specifically, it has no effect on settlement ([`LiquifactEscrow::settle`]), SME withdrawal ([`LiquifactEscrow::withdraw`]), investor claims ([`LiquifactEscrow::claim_investor_payout`]), compliance holds, or any other state transition.

Future versions of the platform that enforce asset movement or custody must introduce distinct API endpoints. Historical records of this self-reported metadata are not proof of custody and must never be treated as proof of locked assets.

## On-chain Behavior

### 1. Authorization
Only the configured SME address (`InvoiceEscrow::sme_address`) is authorized to call this entrypoint. The contract enforces this by calling `sme_address.require_auth()` via the internal helper `load_escrow_require_sme`.

### 2. Validation Rules
The contract validates inputs and state before recording:
- **Positive Amount:** The `amount` parameter must be strictly positive (`amount > 0`). If it is zero or negative, the contract panics with [`EscrowError::CollateralAmountNotPositive`].
- **Non-empty Asset Symbol:** The `asset` parameter must be a non-empty Symbol (`asset != Symbol::new(&env, "")`). If an empty symbol is passed, the contract panics with [`EscrowError::CollateralAssetEmpty`].
- **Non-empty Collateral Type:** The `collateral_type` parameter must be a non-empty string. If an empty string is passed, the contract panics with [`EscrowError::CollateralTypeEmpty`].
- **Monotonic Timestamp on Replacement:** When replacing an existing commitment, the current ledger timestamp from `Env::ledger().timestamp()` must not be earlier than the previously recorded timestamp (`now >= prior_commitment.recorded_at`). This acts as a defense-in-depth against stale out-of-order writes. If the timestamp goes backwards, the contract panics with [`EscrowError::CollateralTimestampBackwards`].

### 3. Storage
The contract writes the metadata record under [`DataKey::SmeCollateralPledge`] in the instance storage. This completely replaces any previously recorded commitment.

The recorded data is represented by the [`SmeCollateralCommitment`] struct:
- `asset`: `Symbol` – the off-chain asset symbol.
- `amount`: `i128` – the reported amount.
- `recorded_at`: `u64` – the Soroban ledger timestamp when the commitment was written.
- `collateral_type`: `String` – a descriptive categorization of the collateral pledge (non-empty).

To retrieve the current record, external callers can use [`LiquifactEscrow::get_sme_collateral_commitment`].

### 4. Event Emission
The contract emits a [`CollateralRecordedEvt`] event upon successful execution, which contains:
- `name`: `Symbol` – hardcoded to `coll_rec` (`symbol_short!("coll_rec")`).
- `invoice_id`: `Symbol` – the invoice ID associated with the current escrow.
- `amount`: `i128` – the newly recorded collateral amount.
- `prior_amount`: `i128` – the previously recorded collateral amount (or `0` if no prior commitment existed). This provides clear replacement semantics for off-chain indexers.

## Off-chain Risk-Team Handling

Risk teams and off-chain services must treat the recorded data as self-reported metadata and verify its validity independently.

Recommended verification procedures:
1. **Verify Signer Context:** Confirm the transaction was signed by the correct SME address linked to the invoice.
2. **Resolve Asset Symbol:** Ensure the reported `asset` symbol maps to the correct physical asset or token contract.
3. **Verify Custody Separately:** Confirm custody accounts, statements, and security perfection outside the blockchain.
4. **Reconcile Independently:** Implement any asset-control or settlement actions in separate off-chain systems or dedicated contracts, completely detached from this metadata escrow record.
5. **Clear Labeling:** Label all indexed database fields as `reported_collateral_metadata` rather than implying locked balances or enforceable claims.
