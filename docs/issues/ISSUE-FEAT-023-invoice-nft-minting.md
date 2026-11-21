# Issue: Mint One NFT per Invoice at Settlement

**Issue ID:** FEAT-023  
**Category:** FEAT  
**Type:** Feature  
**Status:** Backlog, ready for review and assignment  
**Priority:** Medium  
**Related:** [Basic workflow example](../../examples/basic_workflow.rs), [Settlement Flow ADR](../adr/ADR-003-settlement-flow.md), [Escrow Events](../escrow-events.md), [Escrow Data Model](../escrow-data-model.md)

## Description

Mint a single settlement NFT for each invoice escrow when the SME successfully settles the invoice. The NFT is a tamper-evident receipt and proof that the invoice reached settlement. It is not an investor payout, does not represent fractional ownership of escrow funds, and must not change settlement or claim accounting.

The NFT contract is optional and is supplied when the escrow is initialized. When configured, settlement mints exactly one token to the configured SME. The invoice escrow remains the source of truth for settlement state; the NFT is an external certificate whose token ID and mint timestamp are recorded on-chain and emitted for indexers.

## Current Behavior

`init` accepts an optional `nft_contract`, and read APIs expose the configured NFT contract. The repository also contains `SettlementNftMetadata`, `SettlementNftSnapshot`, and `SettlementNftMinted` scaffolding. The `example_nft_mint_workflow` test configures an NFT contract and settles an escrow, but it does not assert a mint or token ownership.

The settlement path does not currently invoke an NFT contract. The metadata and event types are therefore not backed by a complete storage, minting, or failure-handling flow. No public read API reliably returns a minted token ID, and no integration test proves that one invoice cannot mint twice.

## Steps to Reproduce

1. Run `cargo test -p karis_ky_escrow example_nft_mint_workflow -- --nocapture`.
2. Inspect `examples/basic_workflow.rs` and note that an NFT contract address is passed to `init` before `settle()`.
3. Inspect the configured NFT contract or a mock NFT ledger after settlement.
4. Observe that no mint call, owner balance increase, persisted token ID, or verifiable `SettlementNftMinted` event is required by the test.
5. Attempt to query the settlement NFT for the invoice and observe that the result cannot distinguish “not configured”, “not settled”, and “minted”.

## Expected Behavior

- An escrow without an NFT contract settles as it does today and produces no NFT event.
- With an NFT contract configured, the first successful full settlement calls the approved NFT mint interface exactly once.
- The minted token is sent to `sme_address`, has a unique ID within the NFT contract, and identifies the invoice and escrow contract without exposing investor or private data.
- The minted token ID and timestamp are stored under an additive settlement-NFT key and exposed through the existing summary/read model.
- `SettlementNftMinted` is emitted only after mint succeeds. A failed mint atomically leaves the escrow unsettled and emits no successful mint or settlement event.
- Repeated settlement attempts cannot mint a second NFT.
- Partial settlement does not mint; completion mints exactly once.
- NFT minting does not alter funding tokens, contributions, yield, claims, withdrawal, refunds, or dust accounting.
- Existing escrows and integrations with no NFT configuration remain backward-compatible.

## Actual Behavior

The optional NFT address is recorded and can be read, but settlement has no implemented mint side effect. The existing example can pass while proving only that settlement changed status. The contract therefore provides no reliable invoice certificate, token ownership guarantee, idempotency guarantee, or indexer event for this feature.

## Proposed Solution

### 1. Define the NFT contract boundary

Choose and document a minimal Soroban NFT interface, preferably a trusted project-owned contract with a typed client. The mint operation must accept the recipient and immutable metadata (or a metadata URI/hash), and return a token ID. Validate the configured address at initialization or fail clearly on the first mint; never treat a failed external call as success.

Document the NFT contract owner, upgrade policy, interface version, and trust model. The escrow must not accept arbitrary callbacks, and the NFT contract must not be able to change escrow state or pull funding tokens.

### 2. Add settlement-NFT storage and minting

Add additive `DataKey` variants for the minted token record, preserving the storage compatibility policy. Keep `None` for unconfigured or not-yet-minted escrows and store a record equivalent to:

```rust
pub struct SettlementNftMetadata {
    pub token_id: u32,
    pub minted_at_ledger_timestamp: u64,
}
```

After `settle()` validates authorization, maturity, holds, funding, and partial-settlement rules, call the NFT mint operation when configured. Persist the returned ID only after a successful call and guard against an existing token record. Soroban transaction atomicity must leave no settled state or NFT record after a failed mint.

For partial settlement, completion is `settled_amount == funded_amount`. Intermediate calls must not mint; a retry after a failed final mint must remain possible.

### 3. Define metadata and events

Metadata must be deterministic and versioned, containing at minimum:

- invoice ID;
- escrow contract address;
- SME recipient address;
- settlement ledger timestamp;
- finalized funded amount and yield basis points; and
- a schema/version marker.

Emit `SettlementNftMinted` with the invoice ID, NFT contract, SME, settlement timestamp, yield, and returned token ID. Align the event token-ID type with the actual NFT interface and persisted read type. Update event, read API, and TypeScript SDK documentation.

### 4. Tests, example, and operations

Extend `examples/basic_workflow.rs` with a mock NFT contract or test double that records calls. Assert recipient, exactly-one mint, token ID, stored metadata, and event values. Add tests for:

- no NFT contract and legacy settlement;
- successful full settlement mint;
- partial settlement followed by one final mint;
- repeated settlement rejection;
- failed mint atomicity and retry;
- malformed or unavailable NFT contract;
- unauthorized initialization/configuration;
- deterministic, private-data-free metadata; and
- token ID uniqueness across two invoices sharing one NFT contract.

Document deployment ordering, trusted contract addresses, admin/upgrade responsibilities, indexer handling, and failed-mint recovery.

## Environment Context

- **Repository:** `KARIS-KY`
- **Platform:** Stellar Soroban
- **Language/toolchain:** Rust, Cargo, Soroban SDK, SEP-41 funding token
- **Settlement actor and NFT recipient:** configured `InvoiceEscrow::sme_address`
- **NFT scope:** one settlement certificate per invoice escrow
- **Configuration:** optional immutable NFT contract address set at `init`
- **Mint timing:** successful terminal/full settlement only
- **Source of truth:** escrow settlement state; NFT is a derived certificate
- **Compatibility:** absent NFT configuration means no external call and no NFT
- **Security:** atomic external call, no investor PII, and no callback authority over escrow funds/state

## Acceptance Criteria

- [ ] Approved NFT interface, trust model, metadata schema, token-ID type, and deployment/upgrade policy.
- [ ] `init` stores an optional immutable NFT contract; unset configuration preserves legacy settlement.
- [ ] Full settlement with NFT configuration calls mint exactly once and sends the NFT to the SME.
- [ ] Partial settlement never mints; completion mints exactly one NFT.
- [ ] Token ID and timestamp are stored under additive storage and returned by the summary/read API.
- [ ] `SettlementNftMinted` values and types match storage and the NFT response.
- [ ] Failed mint reverts settlement atomically, leaves no NFT record, and can be retried.
- [ ] A second settlement cannot mint a second NFT.
- [ ] NFT minting does not change principal, yield, claim, withdrawal, refund, or dust accounting.
- [ ] Metadata is deterministic, versioned, and excludes investor private data.
- [ ] The basic workflow example asserts recipient, token ID, persisted record, and exactly-one mint behavior.
- [ ] Unit/integration tests cover configured, unconfigured, partial, failure, retry, duplicate, authorization, and cross-invoice uniqueness cases.
- [ ] Event, read API, SDK, operator, and deployment documentation is updated.
- [ ] `cargo fmt --check`, `cargo test -p karis_ky_escrow`, and relevant clippy checks pass.

## Open Questions Before Assignment

1. Should the project own and deploy the NFT contract, or may governance approve an external implementation?
2. Should metadata be stored fully in the NFT contract, or should the NFT hold a deterministic URI/content hash with details served off-chain?
3. Is `u32` sufficient for token IDs, or should the contract standardize on a wider integer or `Bytes` representation?
4. What operational action is required if a trusted NFT contract is paused or upgraded between initialization and settlement?

These questions must be resolved in design review before implementation is assigned.