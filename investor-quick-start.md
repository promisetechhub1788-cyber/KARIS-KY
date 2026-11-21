# Investor Quick Start

This guide walks an investor through the basics of participating in a
Handsoff yield-bearing pool: depositing, tracking your position, and
claiming the yield you have earned.

## 1. Deposit into a pool

```bash
handsoff pool deposit \
  --pool <POOL_ID> \
  --amount 1000 \
  --asset USDC
```

Your deposit mints a position token that represents your share of the pool.

## 2. Track your position

```bash
handsoff position show --pool <POOL_ID>
```

This prints your principal, accrued yield, and the timestamp of the last
accrual checkpoint.

## 3. Claim your yield

Yield accrues continuously and can be claimed at any time:

```bash
handsoff yield claim --pool <POOL_ID>
```

By default the claim is executed by the wallet that holds the position.

## 4. Delegate yield claiming

An investor does not have to be the one who submits the claim transaction.
Claiming can be **delegated** to another address, which is then authorized to
call the claim entrypoint on the investor's behalf. The position, principal,
and accrued yield always remain owned by the investor — only the right to
*trigger* a claim is transferred.

### Why delegate?

- **Custody and operations.** A fund administrator, treasury multisig, or
  operations team can sweep yield on a schedule without holding the investor's
  keys.
- **Automation.** A bot or keeper can claim on a fixed cadence so yield is
  realized and compounded without manual intervention.
- **Gas sponsorship.** A relayer can pay gas for the claim while the investor
  keeps custody of the position.
- **Cold-storage safety.** The position stays in a cold wallet while a hot
  operational key performs the routine claim transactions.

### Delegate via the CLI

```bash
handsoff yield delegate \
  --pool <POOL_ID> \
  --delegate <DELEGATE_ADDRESS>
```

After this, `<DELEGATE_ADDRESS>` can claim on your behalf:

```bash
handsoff yield claim \
  --pool <POOL_ID> \
  --on-behalf-of <YOUR_ADDRESS>
```

### Delegate via the SDK

```ts
import { Handsoff } from "@handsoff/sdk";

const client = new Handsoff({ /* ... */ });

// Grant the delegation.
await client.yield.delegate({
  pool: POOL_ID,
  delegate: DELEGATE_ADDRESS,
});

// The delegate claims on the investor's behalf.
await client.yield.claim({
  pool: POOL_ID,
  onBehalfOf: INVESTOR_ADDRESS,
});
```

### Inspect an existing delegation

```bash
handsoff yield delegation show --pool <POOL_ID>
```

### Revoke a delegation

Revoking removes the delegate's authority immediately; any claim submitted
after revocation will revert. The investor can always claim directly again.

```bash
handsoff yield delegate revoke \
  --pool <POOL_ID> \
  --delegate <DELEGATE_ADDRESS>
```

Or via the SDK:

```ts
await client.yield.revokeDelegation({
  pool: POOL_ID,
  delegate: DELEGATE_ADDRESS,
});
```

### Notes

- A delegation is scoped to a single pool; delegating on one pool does not
  grant authority on any other pool.
- Only the position owner can create or revoke a delegation.
- Revocation is immediate and does not require the delegate's cooperation.
