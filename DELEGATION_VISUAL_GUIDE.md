# Delegation Visual Guide

This guide illustrates how delegation works across the protocol, including how
investors can delegate the claiming of their yield.

## Overview

Delegation lets an account (the *delegator*) authorize another account (the
*delegate*) to act on its behalf for a specific capability. For investors, the
most common capability to delegate is **yield claiming** — allowing a trusted
party to collect accrued yield without transferring ownership of the position.

```
Delegator (Investor)                 Delegate (Manager / Custodian)
        |                                        |
        |  1. grant claim delegation             |
        |--------------------------------------->|
        |                                        |
        |  2. delegate calls claimYield()        |
        |<---------------------------------------|
        |                                        |
        |  3. yield transferred to investor      |
        |                                        |
        |  4. revoke delegation (optional)       |
        |--------------------------------------->|
```

## Why an Investor Would Delegate Claiming

- **Operational convenience:** a fund manager or custodian can sweep yield on a
  regular schedule without the investor signing every claim.
- **Automation:** a bot or treasury service can claim and reinvest on the
  investor's behalf.
- **Custody arrangements:** when assets are held by a custodian, the custodian
  can claim yield and route it to the investor's account.
- **Gas / UX:** a delegate can batch claims across multiple positions, reducing
  the number of transactions the investor must sign.

Delegation only grants the ability to *claim*; it does not transfer ownership of
principal or the underlying position.

## Granting a Claim Delegation

### CLI

```bash
# Grant <delegate-address> the right to claim yield for the caller
protocol delegation grant \
  --capability claimYield \
  --delegate 0xDelegateAddress \
  --expiry 1735689600
```

### SDK

```ts
import { DelegationClient } from "@protocol/sdk";

const client = new DelegationClient(provider);

await client.grant({
  capability: "claimYield",
  delegate: "0xDelegateAddress",
  expiry: 1735689600, // optional unix timestamp
});
```

## Revoking a Claim Delegation

A delegation can be revoked at any time by the delegator. Revocation takes
effect immediately and prevents any further claims by the delegate.

### CLI

```bash
# Revoke a previously granted claim delegation
protocol delegation revoke \
  --capability claimYield \
  --delegate 0xDelegateAddress
```

### SDK

```ts
await client.revoke({
  capability: "claimYield",
  delegate: "0xDelegateAddress",
});
```

## Inspecting Active Delegations

```bash
# List delegations granted by the caller
protocol delegation list --capability claimYield
```

```ts
const delegations = await client.list({ capability: "claimYield" });
```

## Notes

- Delegations are scoped per capability; granting `claimYield` does not grant
  any other rights.
- An optional `expiry` bounds how long a delegation remains valid.
- Revocation is idempotent — revoking an already-revoked delegation is a no-op.
