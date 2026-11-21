# Escrow Yield Reinvestment (RFC-002)

This guide documents the user-facing flow for the implemented `reinvest_yield`
entrypoint. It covers the preconditions required to call the entrypoint, the
errors that can be returned, and concrete usage examples.

## Overview

`reinvest_yield` allows an authorized caller to take the yield that has accrued
on an escrow and reinvest it back into the escrow principal instead of
withdrawing it. This compounds the position without moving funds out of the
escrow.

## Preconditions

Before calling `reinvest_yield`, all of the following must hold:

1. **Escrow exists and is active.** The escrow referenced by the call must have
   been created and must not be in a terminal state (released, refunded, or
   cancelled).
2. **Caller is authorized.** Only the escrow owner or an account explicitly
   granted reinvestment authority may call `reinvest_yield`.
3. **Yield is available.** The escrow must have accrued a non-zero amount of
   yield that has not already been reinvested or withdrawn.
4. **Reinvestment is enabled.** The escrow must have been configured with
   reinvestment enabled at creation time.
5. **No pending settlement.** The escrow must not have an in-flight settlement
   or withdrawal that would conflict with the reinvestment.

If any precondition is not met, the call fails with one of the errors below.

## Errors

After the error renumbering, `reinvest_yield` returns the following errors:

| Code | Name | Description | Remediation |
| ---- | ---- | ----------- | ----------- |
| 181 | `ReinvestmentNotEnabled` | The escrow was not configured to allow reinvestment. | Recreate the escrow with reinvestment enabled, or use the standard withdrawal flow. |
| 182 | `NoYieldToReinvest` | The escrow has no accrued yield available to reinvest. | Wait for yield to accrue, or verify the yield accounting for the escrow. |
| 183 | `UnauthorizedReinvestor` | The caller is not the escrow owner or an authorized reinvestor. | Call from the owner account or grant reinvestment authority to the caller. |
| 184 | `EscrowNotActive` | The escrow is not in an active state (released, refunded, or cancelled). | Reinvestment is only possible while the escrow is active; no action is available once it is terminal. |

## Usage Examples

### Reinvest accrued yield

```ts
// Reinvest all accrued yield back into the escrow principal.
await escrowClient.reinvestYield({
  escrowId: "escrow-123",
  caller: ownerAccount,
});
```

### Reinvest a specific amount

```ts
// Reinvest a portion of the accrued yield.
await escrowClient.reinvestYield({
  escrowId: "escrow-123",
  caller: ownerAccount,
  amount: 500n,
});
```

### Handling errors

```ts
try {
  await escrowClient.reinvestYield({ escrowId, caller: ownerAccount });
} catch (err) {
  switch (err.code) {
    case 181:
      // Reinvestment not enabled for this escrow.
      break;
    case 182:
      // No yield available to reinvest yet.
      break;
    case 183:
      // Caller is not authorized to reinvest.
      break;
    case 184:
      // Escrow is no longer active.
      break;
    default:
      throw err;
  }
}
```

## Related Documents

- `RFC-002-yield-reinvestment.md` — design and rationale for yield reinvestment.
