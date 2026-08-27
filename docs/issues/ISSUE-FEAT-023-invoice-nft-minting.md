# Issue: Mint One NFT per Invoice at Settlement

**Issue ID:** FEAT-023  
**Category:** FEAT  
**Type:** Feature  
**Status:** Backlog, ready for review and assignment  
**Priority:** Medium  
**Related:** [Basic workflow example](../../examples/basic_workflow.rs), [Settlement Flow ADR](../adr/ADR-003-settlement-flow.md), [Escrow Events](../escrow-events.md), [Escrow Data Model](../escrow-data-model.md)

## Description

Mint a single settlement NFT for each invoice escrow when the SME successfully
settles the invoice. The NFT is a tamper-evident receipt and proof that the
invoice reached settlement. It is not an investor payout, does not represent
fractional ownership of escrow funds, and must not change settlement or claim
accounting.

The NFT contract is optional and is supplied when the escrow is initialized.
When configured, settlement mints exactly one token to the configured SME. The
invoice escrow remains the source of truth for settlement state; the NFT is an
external certificate whose token ID and mint timestamp are recorded on-chain
and emitted for indexers.

## Current Behavior

`init` accepts an optional `nft_contract`, and read APIs expose the configured
NFT contract. The repository also contains `SettlementNftMetadata`,
`SettlementNftSnapshot`, and `SettlementNftMinted` scaffolding. The
`example_nft_mint_workflow` test configures an NFT contract and settles an
escrow, but it does not assert a mint or token ownership.

The settlement path does not currently invoke an NFT contract. The metadata
and event types are therefore not backed by a complete storage, minting, or
failure-handling flow. No public read API reliably returns a minted token ID,
and no integration test proves that one invoice cannot mint twice.

## Steps to Reproduce

1. Run `cargo test -p karis_ky_escrow example_nft_mint_workflow -- --nocapture`.
2. Inspect `examples/basic_workflow.rs` and note that an NFT contract address
   is passed to `init` before `settle()`.
3. Inspect the configured NFT contract or a mock NFT ledger after settlement.
4. Observe that no mint call, owner balance increase, persisted token ID, or
   verifiable `SettlementNftMinted` event is required by the test.
5. Attempt to query the settlement NFT for the invoice and observe that the
   result cannot distinguish “not configured”, “not settled”, and “minted”.

## Expected Behavior

- An escrow may be initialized with no NFT contract; settlement then behaves
  exactly as it does today and produces no NFT event.
- With an NFT contract configured, the first successful full settlement calls
  the approved NFT mint interface exactly once.
- The minted token is sent to `sme_address`, and its token ID is unique within
  the NFT contract.
- The NFT metadata identifies the invoice and escrow contract, and includes
  settlement timestamp and the finalized settlement terms. Metadata must not
  contain investor addresses, private data, or mutable payout claims.
- The contract stores the minted token ID and mint timestamp in a new additive
  settlement-NFT key and exposes them through the existing summary/read model.
- `settle()` emits `SettlementNftMinted` only after the mint succeeds. A failed
  mint atomically leaves the escrow unsettled and emits no successful mint or
  settlement event.
- Repeated settlement attempts are rejected by the existing terminal-state
  checks and cannot mint a second NFT.
- Partial settlement does not mint an NFT. The NFT is minted only when the
  escrow reaches terminal, full settlement; a later completion mints once.
- NFT minting does not transfer, reserve, or alter funding tokens, investor
  contributions, yield, claims, withdrawal, refunds, or dust accounting.
- Existing escrows and integrations with no NFT configuration remain
  backward-compatible.

## Actual Behavior

The optional NFT address is recorded and can be read, but settlement has no
implemented mint side effect. The existing example can pass while proving only
that settlement changed status. The contract therefore provides no reliable
invoice certificate, token ownership guarantee, idempotency guarantee, or
indexer event for this feature.

## Proposed Solution

### 1. Define the NFT contract boundary

Choose and document a minimal Soroban NFT interface, preferably a trusted
project-owned contract with a typed client. The mint operation must accept the
recipient and immutable metadata (or a metadata URI/hash), and return a token
ID. Validate that the configured address is a contract at initialization or
fail clearly on the first mint; do not silently treat a failed external call
as success.

The NFT contract owner/admin and upgrade policy must be explicit. The escrow
must not accept arbitrary callbacks, and the NFT contract must not be able to
change escrow state or pull funding tokens. If the project uses a standard
rather than a project-specific interface, document the exact interface and
version in the contract and SDK.

### 2. Add settlement-NFT storage and minting

Add additive `DataKey` variants for the minted token record, preserving the
existing storage compatibility policy. Keep `None` for unconfigured or not-yet
minted escrows and store a record equivalent to:

```rust
pub struct SettlementNftMetadata {
    pub token_id: u32,
    pub minted_at_ledger_timestamp: u64,
}
```

At the point where `settle()` has validated authorization, maturity, holds,
funding, and any partial-settlement rules, but before the successful state and
event commit, call the NFT mint operation when configured. Persist the returned
ID only after a successful call. Guard the path so an already stored token
record cannot mint again. Use Soroban transaction atomicity and a focused
reentrancy/idempotency test to ensure a failed external call does not leave a
settled escrow or stale NFT record.

For partial settlement, define completion as `settled_amount == funded_amount`.
Do not mint for intermediate calls, and ensure a retry after a failed final
mint can succeed without charging or recording the invoice twice.

### 3. Define metadata and events

Use deterministic metadata fields at minimum:

- invoice ID;
- escrow contract address;
- SME recipient address;
- settlement ledger timestamp;
- finalized funded amount and yield basis points; and
- a schema/version marker.

Emit `SettlementNftMinted` with the invoice ID, NFT contract, SME, settlement
timestamp, yield, and the returned token ID. Align the event token-ID type with
the actual NFT interface and the persisted read type; do not maintain separate
incompatible `u32` and `Symbol` representations without a documented adapter.
Update `docs/escrow-events.md`, read API documentation, and the TypeScript SDK.

### 4. Tests, example, and operational documentation

Extend `examples/basic_workflow.rs` with a mock NFT contract or a test double
that records mint calls, then assert recipient, exactly-one mint, token ID,
stored metadata, and event-visible values. Add tests for:

- no NFT contract (legacy behavior);
- successful full settlement mint;
- partial settlement followed by one final mint;
- repeated settlement rejection;
- failed mint atomicity and retry;
- malformed or unavailable NFT contract;
- unauthorized initialization/configuration;
- metadata determinism and privacy; and
- token ID uniqueness across two invoices using the same NFT contract.

Add an integration test against the real project NFT contract if one exists.
Document deployment ordering, the trusted NFT contract address, upgrade/admin
responsibilities, indexer handling, and recovery steps for a failed mint.

## Environment Context

- **Repository:** `KARIS-KY`
- **Platform:** Stellar Soroban
- **Language/toolchain:** Rust, Cargo, Soroban SDK, SEP-41 funding token
- **Settlement actor:** configured SME
- **NFT recipient:** `InvoiceEscrow::sme_address`
- **NFT scope:** one settlement certificate per invoice escrow
- **Configuration:** optional immutable NFT contract address set at `init`
- **Mint timing:** successful terminal/full settlement only
- **State source of truth:** escrow settlement state; NFT is a derived certificate
- **Compatibility:** absent NFT configuration means no external call and no NFT
- **Security constraints:** atomic external call, no investor PII in metadata, no
  NFT callback authority over escrow funds or state

## Acceptance Criteria

- [ ] FEAT-023 has an approved NFT interface, contract trust model, metadata
      schema, token-ID type, and deployment/upgrade policy.
- [ ] `init` accepts and stores an optional immutable NFT contract address;
      unset configuration preserves legacy settlement behavior.
- [ ] Full settlement with NFT configuration calls mint exactly once and sends
      the NFT to the configured SME.
- [ ] Partial settlement never mints; completion mints exactly one NFT.
- [ ] Minted token ID and timestamp are stored under additive storage and are
      returned by the settlement read/summary API.
- [ ] `SettlementNftMinted` is emitted with types and values matching storage
      and the NFT contract response.
- [ ] A failed mint reverts settlement atomically, leaves no NFT record, and
      can be retried successfully.
- [ ] A second settlement cannot mint a second NFT.
- [ ] NFT minting does not change principal, yield, claim, withdrawal, refund,
      or dust accounting.
- [ ] Metadata is deterministic, versioned, and excludes investor private data.
- [ ] The basic workflow example asserts recipient, token ID, persisted record,
      and exactly-one mint behavior.
- [ ] Unit/integration tests cover configured, unconfigured, partial, failure,
      retry, duplicate, authorization, and cross-invoice uniqueness cases.
- [ ] Event, read API, SDK, operator, and deployment documentation is updated.
- [ ] `cargo fmt --check`, `cargo test -p karis_ky_escrow`, and the relevant
      clippy checks pass.

## Open Questions Before Assignment

1. Should the project own and deploy the NFT contract, or may governance
   approve an external contract implementation?
2. Should metadata be stored fully in the NFT contract, or should the NFT hold
   a deterministic URI/content hash with details served off-chain?
3. Is `u32` sufficient for token IDs, or should the contract standardize on a
   wider integer or `Bytes` representation?
4. What operational action is required if a trusted NFT contract is paused or
   upgraded between escrow initialization and settlement?

These questions must be resolved in design review before implementation is
assigned.
