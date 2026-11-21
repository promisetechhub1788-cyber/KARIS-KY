# KARIS-KY Escrow SDK

## Compute a payout before claiming

Use `computeProRataPayout` with values in the token's base units. It mirrors
the contract's `compute_investor_payout` formula and returns floor-rounded
integer values without making an RPC call.

```ts
import { computeProRataPayout } from "./src/index.js";

const payout = computeProRataPayout({
  contribution: "1000000000",
  totalPrincipal: "10050000000",
  effectiveYieldBps: 500,
});

console.log(payout.grossPayout);
```

Use the immutable `FundingCloseSnapshot.total_principal` as
`totalPrincipal`, and the investor's effective yield (or the escrow base yield)
as `effectiveYieldBps`. The result is a `bigint`; convert it only at the UI or
serialization boundary.