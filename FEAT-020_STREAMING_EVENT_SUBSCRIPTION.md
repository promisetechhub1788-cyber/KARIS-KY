# FEAT-020: TypeScript SDK Streaming Event Subscription

## Description

Add a typed, cancellable event stream to the TypeScript SDK for monitoring an escrow contract's lifecycle. The SDK polls Soroban RPC `getEvents`, scopes requests to one escrow contract, preserves Soroban paging cursors, and exposes events as an async iterable.

Lifecycle events include initialization, funding, settlement, withdrawal, claims, refunds, cancellation, legal-hold changes, administrative updates, and attestation or collateral updates already declared in `sdk-ts/spec.json`.

## Steps To Reproduce

1. Create an `EscrowClient` with a Soroban RPC adapter.
2. Attempt to monitor an escrow after calling `init`, `fund`, or `settle`.
3. Observe that the SDK provides read and write methods but no event subscription API; applications must implement RPC polling, cursor handling, filtering, and shutdown themselves.

## Expected Behavior

Applications can call `subscribeEscrowEvents()` and consume typed events with `for await`. The stream should:

- query only the configured escrow contract;
- start at a requested ledger or the current ledger;
- resume from a supplied Soroban paging cursor;
- optionally filter by decoded event name;
- avoid duplicate pages by forwarding the returned cursor;
- continue polling when no events are available; and
- stop cleanly through an `AbortSignal`.

## Actual Behavior

No SDK event API exists. `SorobanRpcClient` has no event retrieval capability, so consumers duplicate transport and lifecycle logic outside the SDK.

## Environment

- TypeScript SDK: `@karis-ky/sdk` 0.1.x
- Runtime: Node.js 18 or later
- Network: Stellar Soroban RPC endpoint
- Contract: `LiquifactEscrow`, schema version 6, interface version 1

## Proposed Solution

Extend the RPC adapter with optional `getEvents` support returning normalized `SorobanEventPage` values. Add `EscrowClient.subscribeEscrowEvents(options)`, returning `AsyncGenerator<EscrowEvent>`. Keep the adapter responsible for XDR decoding and the client responsible for contract scoping, polling, filtering, cursor progression, and cancellation. Existing adapters remain source-compatible and receive a clear error until they implement `getEvents`.

## Acceptance Criteria

- [x] Public types exist for escrow events, event pages, query filters, and subscription options.
- [x] The client scopes every event query to its configured contract ID.
- [x] The client supports starting ledger, cursor, event-name filter, page limit, poll interval, and `AbortSignal`.
- [x] Returned cursors are sent on subsequent requests; ledger advancement is used when no cursor is returned.
- [x] Empty pages poll again without yielding duplicates.
- [x] Aborting stops the async generator without an additional event.
- [x] Missing adapter support produces a descriptive error.
- [x] The SDK builds under strict TypeScript settings and has focused tests for paging, filtering, and unsupported adapters.
