# TypeScript SDK Error Handling Guide

The karis-ky escrow contract emits typed Soroban contract errors, which surface in the TypeScript SDK as `ContractError(code)` exceptions. This guide explains how to detect, classify, and handle these errors in your SDK integration.

## Table of Contents

- [Error Detection](#error-detection)
- [Error Categories](#error-categories)
- [Error Code Reference](#error-code-reference)
- [Common Error Patterns by Method](#common-error-patterns-by-method)
- [Try/Catch Examples](#trycatch-examples)
- [Error Recovery Strategies](#error-recovery-strategies)

---

## Error Detection

### Understanding Error Types

The SDK can emit three distinct error types:

1. **ContractError(code)** — Typed Soroban contract error. Branch on the numeric code.
2. **Network Error** — RPC connection, timeout, or Stellar network issue.
3. **XDR Decoding Error** — Malformed response from contract.
4. **Unexpected Exception** — Other runtime errors.

### Distinguishing Contract Errors

Check if an exception is a contract error by inspecting the error object:

```typescript
import { EscrowErrorCode, EscrowClient } from "@karis-ky/escrow-sdk";

try {
  await client.fund(investor, amount);
} catch (err: unknown) {
  // Check if this is a Soroban contract error
  if (err instanceof SorobanContractError && err.code !== undefined) {
    // This is a typed contract error
    const code = err.code;
    const message = `Contract error ${code}: ${getErrorMessage(code)}`;
    console.error(message);
    // Handle based on code...
  } else if (err instanceof NetworkError) {
    // RPC/network problem
    console.error("Network error:", err.message);
    // Retry logic...
  } else {
    // Unknown error
    console.error("Unexpected error:", err);
  }
}
```

### Checking Against Specific Codes

```typescript
import { EscrowErrorCode } from "@karis-ky/escrow-sdk";

try {
  await client.fund(investor, amount);
} catch (err: unknown) {
  if (err instanceof SorobanContractError) {
    if (err.code === EscrowErrorCode.FundingAmountNotPositive) {
      // Handle: amount must be > 0
      console.error("Investor amount must be positive");
    } else if (err.code === EscrowErrorCode.LegalHoldBlocksFunding) {
      // Handle: legal hold in effect
      console.error("Funding blocked by legal hold; contact admin");
    } else if (err.code === EscrowErrorCode.InvestorNotAllowlisted) {
      // Handle: investor not on allowlist
      console.error("Investor not authorized; add to allowlist first");
    }
  }
}
```

### Using Error Labels and Classification

The SDK provides human-readable error descriptions and category helpers:

```typescript
import { ESCROW_ERROR_LABELS, classifyError } from "@karis-ky/escrow-sdk";

try {
  await client.fund(investor, amount);
} catch (err: unknown) {
  if (err instanceof SorobanContractError) {
    const label = ESCROW_ERROR_LABELS[err.code];
    const category = classifyError(err.code);
    console.error(`[${category}] ${label}`);
  }
}
```

---

## Error Categories

Error codes are grouped by domain so SDKs can map categories without parsing variant names:

| Category | Codes | Purpose |
| --- | --- | --- |
| `init` | 1–13 | Initialization and pricing configuration |
| `uninitialized` | 20–22 | Missing escrow or required addresses |
| `dustSweep` | 30–42 | Treasury dust sweep and token safety |
| `attestation` | 50–51 | Attestation binding and logging |
| `collateral` | 60–62 | SME collateral metadata |
| `adminValidation` | 70–83 | Administrative validation and batch bounds |
| `migration` | 90–92 | Schema migration version checks |
| `funding` | 100–111 | Investor deposits and contribution limits |
| `settlement` | 120–129 | Settlement, withdrawal, and payout math |
| `cancelRefund` | 140–143 | Funding cancellation and refunds |
| `legalHoldClear` | 150–152 | Legal hold clear workflow |
| `beneficiary` | 160–164 | Beneficiary rotation and admin handover |

---

## Error Code Reference

### Initialization Errors (1–13)

| Code | Name | Trigger | Recommended Action |
| ---: | --- | --- | --- |
| 1 | `AmountMustBePositive` | `amount <= 0` | Reject input; show user that amount must be > 0 |
| 2 | `YieldBpsOutOfRange` | `yield_bps` outside 0..=10,000 | Fix yield configuration; valid range is 0–10,000 bps |
| 3 | `EscrowAlreadyInitialized` | `init` called twice on same contract | Do not call `init` again; use read APIs to check state |
| 4 | `InvoiceIdInvalidLength` | `invoice_id` length outside 1..=32 | Ensure invoice ID is 1–32 characters |
| 5 | `InvoiceIdInvalidCharset` | `invoice_id` contains invalid characters | Use only `[A-Za-z0-9_]` in invoice ID |
| 6 | `MinContributionNotPositive` | `min_contribution` configured but <= 0 | Omit or set a positive floor |
| 7 | `MinContributionExceedsAmount` | `min_contribution > amount` | Lower floor or raise funding target |

### Uninitialized Metadata Errors (20–22)

| Code | Name | Trigger | Recommended Action |
| ---: | --- | --- | --- |
| 20 | `EscrowNotInitialized` | Escrow not yet created | Call `init` first with valid parameters |
| 21 | `FundingTokenNotSet` | Token address missing | Complete initialization with token address |
| 22 | `TreasuryNotSet` | Treasury address missing | Complete initialization with treasury address |

### Funding Errors (100–111)

| Code | Name | Trigger | Recommended Action |
| ---: | --- | --- | --- |
| 100 | `FundingAmountNotPositive` | `amount <= 0` in `fund` or `fund_with_commitment` | Pass a positive amount > 0 |
| 101 | `FundingBelowMinContribution` | Deposit less than configured floor | Increase deposit to meet minimum |
| 102 | `LegalHoldBlocksFunding` | Legal hold active | Wait for legal hold to clear |
| 103 | `EscrowNotOpenForFunding` | Escrow not in Open state | Check escrow status; funding may be closed |
| 104 | `InvestorNotAllowlisted` | Investor not on allowlist | Add investor to allowlist via `setInvestorsAllowlisted` |
| 105 | `InvestorContributionOverflow` | Contribution math overflows | Reduce deposit size |
| 106 | `InvestorContributionExceedsCap` | Contribution exceeds `max_per_investor` | Reduce deposit or contact admin to raise cap |
| 107 | `UniqueInvestorCapReached` | Max unique investors cap reached | Use an existing investor or wait |
| 108 | `TieredSecondDeposit` | Second deposit after tiered first deposit | Use `fund()` for additional deposits; use `fund_with_commitment()` only for first |
| 109 | `InvestorClaimTimeOverflow` | `timestamp + lock_secs` overflows | Reduce lock duration |
| 111 | `CommitmentLockExceedsMaturity` | Lock period extends beyond maturity | Shorten lock or extend maturity first |

### Settlement / Payout Errors (120–129)

| Code | Name | Trigger | Recommended Action |
| ---: | --- | --- | --- |
| 120 | `LegalHoldBlocksSettlement` | Legal hold active | Wait for legal hold to clear before settlement |
| 121 | `SettlementNotFunded` | Escrow not in Funded state | Fund escrow first via `fund` |
| 122 | `MaturityNotReached` | Maturity timestamp not yet reached | Wait until maturity time before settlement |
| 123 | `LegalHoldBlocksWithdrawal` | Legal hold blocks SME withdrawal | Wait for legal hold to clear |
| 124 | `WithdrawalNotFunded` | Escrow not in Funded state | Fund escrow first |
| 125 | `LegalHoldBlocksInvestorClaims` | Legal hold blocks payout claims | Wait for legal hold to clear |
| 126 | `NoContributionToClaim` | Investor has no contribution | Caller is not a funder; verify investor address |
| 127 | `InvestorClaimNotSettled` | Escrow not in Settled state | Wait for settlement before claiming |
| 128 | `InvestorCommitmentLockNotExpired` | Investor lock period not yet elapsed | Wait until lock expiration time |
| 129 | `ComputePayoutArithmeticOverflow` | Payout math overflows | Escalate; values exceed safe range |

### Legal Hold Errors (30, 102, 120, 123, 125, 140, 150–152, 160)

| Code | Name | Trigger | Recommended Action |
| ---: | --- | --- | --- |
| 30 | `LegalHoldBlocksTreasuryDustSweep` | Legal hold prevents dust sweep | Contact admin to clear legal hold first |
| 102 | `LegalHoldBlocksFunding` | Legal hold prevents deposits | Wait for legal hold to clear |
| 120 | `LegalHoldBlocksSettlement` | Legal hold prevents settlement | Wait for legal hold to clear |
| 123 | `LegalHoldBlocksWithdrawal` | Legal hold prevents withdrawal | Wait for legal hold to clear |
| 125 | `LegalHoldBlocksInvestorClaims` | Legal hold blocks investor claims | Wait for legal hold to clear |
| 140 | `LegalHoldBlocksCancelFunding` | Legal hold prevents cancellation | Wait for legal hold to clear |
| 150 | `LegalHoldClearRequestMissing` | Clearing hold without prior request | Call `requestClearLegalHold` first |
| 151 | `LegalHoldClearNotReady` | Hold clear delay not yet elapsed | Wait until clearable timestamp |
| 160 | `LegalHoldBlocksBeneficiaryRotation` | Legal hold prevents SME rotation | Clear hold before rotating beneficiary |

### Admin/Allowlist Errors (70–80, 82–83, 104, 163–164)

| Code | Name | Trigger | Recommended Action |
| ---: | --- | --- | --- |
| 70 | `InvestorBatchEmpty` | Empty investors list in `setInvestorsAllowlisted` | Pass at least one investor address |
| 71 | `InvestorBatchTooLarge` | More than 32 investors in batch | Split into smaller batches (max 32) |
| 72 | `TargetNotPositive` | Target <= 0 in `updateFundingTarget` | Set a positive target |
| 73 | `TargetUpdateNotOpen` | Target update in non-Open state | Only update target while Open |
| 74 | `TargetBelowFundedAmount` | New target less than already funded | Target must cover existing funding |
| 75 | `CapLowerNotOpen` | Cap lower in non-Open state | Only lower cap while Open |
| 76 | `NoInvestorCapConfigured` | Lower cap but no cap configured | Configure cap at init first |
| 77 | `NewCapNotLower` | New cap not strictly lower | Pass a strictly lower cap value |
| 78 | `NewCapBelowCurrentFunderCount` | Cap lower than existing funders | Cannot evict existing investors |
| 80 | `NewAdminSameAsCurrent` | Proposed admin equals current | Nominate a different admin |
| 82 | `FundingBatchEmpty` | Empty entries in `fundBatch` | Pass at least one (investor, amount) pair |
| 83 | `FundingBatchTooLarge` | More than 50 entries in `fundBatch` | Split into smaller batches (max 50) |
| 104 | `InvestorNotAllowlisted` | Investor not on allowlist | Add via `setInvestorsAllowlisted` |
| 163 | `NoPendingAdmin` | `acceptAdmin` with no pending | Call `proposeAdmin` first |
| 164 | `FundingDeadlinePassed` | Funding past deadline | Funding window closed |

---

## Common Error Patterns by Method

### `fund()` — Common Errors

Errors you may encounter when calling `fund(investor, amount)`:

| Code | When | Recovery |
| ---: | --- | --- |
| 100 | Amount ≤ 0 | Validate user input is positive |
| 101 | Amount < min_contribution | Show user minimum and prompt for higher amount |
| 102 | Legal hold active | Show message: "Funding temporarily restricted" |
| 103 | Escrow not Open | Check escrow status via `getEscrow()` |
| 104 | Investor not allowlisted | Add investor to allowlist (admin only) |
| 106 | Contribution exceeds per-investor cap | Show remaining cap and prompt for less |
| 107 | Unique investor cap reached | Use existing investor or contact admin |
| 20 | Escrow not initialized | Call `init` first |

### `settle()` — Common Errors

Errors you may encounter when calling `settle()`:

| Code | When | Recovery |
| ---: | --- | --- |
| 120 | Legal hold active | Wait for hold to clear |
| 121 | Escrow not Funded | Fund escrow first via `fund()` |
| 122 | Maturity not reached | Show user maturity time and wait |
| 20 | Escrow not initialized | Call `init` first |

### `claimInvestorPayout()` — Common Errors

Errors you may encounter when calling `claimInvestorPayout(investor)`:

| Code | When | Recovery |
| ---: | --- | --- |
| 125 | Legal hold active | Wait for hold to clear |
| 126 | Investor has no contribution | Verify investor address is correct |
| 127 | Escrow not settled | Wait for settlement |
| 128 | Investor lock period active | Show lock expiration time and wait |
| 129 | Payout math overflow | Escalate to developer |
| 20 | Escrow not initialized | Call `init` first |

---

## Try/Catch Examples

### Example 1: Safe Funding with Full Error Handling

```typescript
import {
  EscrowClient,
  EscrowErrorCode,
  ESCROW_ERROR_LABELS,
  classifyError,
  toBaseUnits,
  SorobanContractError,
} from "@karis-ky/escrow-sdk";

async function safeFund(
  client: EscrowClient,
  investor: string,
  amountDecimal: string,
): Promise<void> {
  try {
    // Validate input before sending
    const amount = toBaseUnits(amountDecimal);
    if (parseInt(amount) <= 0) {
      console.error("Amount must be positive");
      return;
    }

    console.log(`Funding ${amountDecimal} tokens for ${investor}...`);
    const result = await client.fund(investor, amount);
    console.log(`✓ Funding successful. Escrow funded amount: ${result.funded_amount}`);
  } catch (err: unknown) {
    if (err instanceof SorobanContractError) {
      const code = err.code;
      const label = ESCROW_ERROR_LABELS[code] || "Unknown error";
      const category = classifyError(code);

      console.error(`✗ Contract error [${category}] ${code}: ${label}`);

      // Handle specific errors
      switch (code) {
        case EscrowErrorCode.LegalHoldBlocksFunding:
          console.error("  → Funding is temporarily restricted due to legal hold.");
          console.error("  → Contact the administrator to clear the hold.");
          break;

        case EscrowErrorCode.InvestorNotAllowlisted:
          console.error("  → This investor is not on the allowlist.");
          console.error("  → Ask the admin to add them via setInvestorsAllowlisted().");
          break;

        case EscrowErrorCode.FundingBelowMinContribution:
          console.error(
            "  → Funding amount is below the minimum contribution floor.",
          );
          console.error("  → Increase the amount to meet the requirement.");
          break;

        case EscrowErrorCode.UniqueInvestorCapReached:
          console.error("  → The escrow has reached its unique investor limit.");
          console.error("  → Use an existing investor address or contact admin.");
          break;

        case EscrowErrorCode.EscrowNotOpenForFunding:
          console.error("  → This escrow is no longer open for funding.");
          console.error("  → Check the escrow status and state.");
          break;

        default:
          console.error(`  → Unrecognized contract error: ${label}`);
      }
    } else if (err instanceof Error && err.message.includes("network")) {
      console.error("✗ Network error; check your connection or RPC URL.");
      console.error(`  Details: ${err.message}`);
    } else {
      console.error(`✗ Unexpected error: ${err}`);
    }
  }
}

// Usage
await safeFund(client, "GINVESTOR....", "5000");
```

### Example 2: Settlement with Maturity and Hold Checks

```typescript
import {
  EscrowClient,
  EscrowErrorCode,
  EscrowStatus,
  ESCROW_STATUS_LABELS,
  ESCROW_ERROR_LABELS,
  SorobanContractError,
} from "@karis-ky/escrow-sdk";

async function settleWhenReady(
  client: EscrowClient,
  smeAddress: string,
): Promise<void> {
  try {
    // 1. Check escrow state before attempting settlement
    console.log("Checking escrow state...");
    const escrow = await client.getEscrow();
    const isLegalHoldActive = await client.getLegalHold();

    if (escrow.status !== EscrowStatus.Funded) {
      console.error(
        `Cannot settle: escrow is ${ESCROW_STATUS_LABELS[escrow.status]}, not Funded.`,
      );
      return;
    }

    if (isLegalHoldActive) {
      console.warn("⚠ Legal hold is active; settlement is blocked.");
      console.warn("  Waiting for legal hold to be cleared...");
      return;
    }

    // 2. Check maturity
    if (escrow.maturity && escrow.maturity !== "0") {
      const maturityTimestamp = parseInt(escrow.maturity);
      const now = Math.floor(Date.now() / 1000);

      if (now < maturityTimestamp) {
        const waitSeconds = maturityTimestamp - now;
        const waitDays = Math.ceil(waitSeconds / 86400);
        console.warn(`⚠ Escrow has not reached maturity yet.`);
        console.warn(`  Maturity in ${waitDays} day(s) (${new Date(maturityTimestamp * 1000).toISOString()})`);
        return;
      }
    }

    // 3. Attempt settlement
    console.log("Initiating settlement...");
    const settled = await client.settle(smeAddress);
    console.log(`✓ Settlement successful. Status: ${ESCROW_STATUS_LABELS[settled.status]}`);
  } catch (err: unknown) {
    if (err instanceof SorobanContractError) {
      const code = err.code;
      const label = ESCROW_ERROR_LABELS[code];

      console.error(`✗ Settlement failed [${code}]: ${label}`);

      switch (code) {
        case EscrowErrorCode.LegalHoldBlocksSettlement:
          console.error(
            "  → Legal hold is blocking settlement. Admin must clear the hold.",
          );
          break;

        case EscrowErrorCode.SettlementNotFunded:
          console.error("  → Escrow must be fully funded before settlement.");
          break;

        case EscrowErrorCode.MaturityNotReached:
          console.error("  → Escrow has not reached its maturity date yet.");
          break;

        default:
          console.error(`  → See docs: https://github.com/karis-ky/karis-ky-escrow/docs/escrow-error-messages.md`);
      }
    } else {
      console.error(`✗ Unexpected error: ${err}`);
    }
  }
}

// Usage
await settleWhenReady(client, "GSME....");
```

### Example 3: Investor Claim with Lock Expiration Handling

```typescript
import {
  EscrowClient,
  EscrowErrorCode,
  EscrowStatus,
  ESCROW_STATUS_LABELS,
  ESCROW_ERROR_LABELS,
  fromBaseUnits,
  SorobanContractError,
} from "@karis-ky/escrow-sdk";

async function claimPayoutWhenEligible(
  client: EscrowClient,
  investor: string,
): Promise<void> {
  try {
    console.log(`Checking claim eligibility for ${investor}...`);

    // 1. Check escrow is settled
    const escrow = await client.getEscrow();
    if (escrow.status !== EscrowStatus.Settled) {
      console.warn(
        `✗ Escrow is ${ESCROW_STATUS_LABELS[escrow.status]}, not Settled.`,
      );
      console.warn("  Investors can only claim after settlement.");
      return;
    }

    // 2. Check investor has a contribution
    const contribution = await client.getContribution(investor);
    if (parseInt(contribution) === 0) {
      console.warn(`✗ Investor ${investor} has no contribution to claim.`);
      return;
    }

    // 3. Check if investor has already claimed
    const isClaimed = await client.isInvestorClaimed(investor);
    if (isClaimed) {
      console.log("✓ Investor has already claimed their payout.");
      return;
    }

    // 4. Check tiered lock expiration
    const lockNotBefore = await client.getInvestorClaimNotBefore(investor);
    if (lockNotBefore && lockNotBefore !== "0") {
      const lockTimestamp = parseInt(lockNotBefore);
      const now = Math.floor(Date.now() / 1000);

      if (now < lockTimestamp) {
        const waitSeconds = lockTimestamp - now;
        const waitDays = Math.ceil(waitSeconds / 86400);
        console.warn(
          `⚠ Investor claim is locked until ${new Date(lockTimestamp * 1000).toISOString()}`,
        );
        console.warn(`  Wait ${waitDays} more day(s) before claiming.`);
        return;
      }
    }

    // 5. Claim payout
    console.log("✓ Investor is eligible. Claiming payout...");
    await client.claimInvestorPayout(investor);
    console.log(`✓ Payout claimed successfully for ${investor}.`);

    // 6. Show payout amount
    const summary = await client.getEscrowSummary();
    const yieldBps = await client.getInvestorYieldBps(investor);
    console.log(`  Contribution: ${fromBaseUnits(contribution)} tokens`);
    console.log(`  Yield tier: ${yieldBps} bps`);
  } catch (err: unknown) {
    if (err instanceof SorobanContractError) {
      const code = err.code;
      const label = ESCROW_ERROR_LABELS[code];

      console.error(`✗ Claim failed [${code}]: ${label}`);

      switch (code) {
        case EscrowErrorCode.LegalHoldBlocksInvestorClaims:
          console.error("  → Legal hold is preventing claims. Wait for hold to clear.");
          break;

        case EscrowErrorCode.NoContributionToClaim:
          console.error("  → Investor address has no contribution.");
          console.error("  → Verify the investor address is correct.");
          break;

        case EscrowErrorCode.InvestorClaimNotSettled:
          console.error("  → Escrow has not been settled yet.");
          break;

        case EscrowErrorCode.InvestorCommitmentLockNotExpired:
          console.error(
            "  → Investor is still in a tiered lock period.",
          );
          console.error("  → Check getInvestorClaimNotBefore() for expiration time.");
          break;

        case EscrowErrorCode.ComputePayoutArithmeticOverflow:
          console.error("  → Payout computation failed due to numeric overflow.");
          console.error("  → Contact developer; this is a critical issue.");
          break;

        default:
          console.error(`  → Unknown error. Check error reference.`);
      }
    } else {
      console.error(`✗ Unexpected error: ${err}`);
    }
  }
}

// Usage
await claimPayoutWhenEligible(client, "GINVESTOR....");
```

---

## Error Recovery Strategies

### Network Errors

```typescript
async function fundWithRetry(
  client: EscrowClient,
  investor: string,
  amount: string,
  maxRetries: number = 3,
): Promise<void> {
  let lastError: unknown;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      console.log(`Attempt ${attempt}/${maxRetries}...`);
      await client.fund(investor, amount);
      console.log("✓ Success");
      return;
    } catch (err: unknown) {
      lastError = err;

      // Only retry on network-like errors
      if (err instanceof SorobanContractError) {
        // Contract errors are not retryable
        throw err;
      }

      const message = String(err).toLowerCase();
      if (message.includes("network") || message.includes("timeout")) {
        const backoffMs = Math.pow(2, attempt) * 1000;
        console.warn(
          `Network error (attempt ${attempt}). Retrying in ${backoffMs}ms...`,
        );
        await new Promise((resolve) => setTimeout(resolve, backoffMs));
      } else {
        throw err;
      }
    }
  }

  console.error(`Failed after ${maxRetries} attempts.`);
  throw lastError;
}
```

### Preventive State Checks

Before calling state-changing methods, read the current state to catch errors early:

```typescript
async function fundSafely(
  client: EscrowClient,
  investor: string,
  amount: string,
): Promise<void> {
  // Preventive checks
  const escrow = await client.getEscrow();
  const isHeld = await client.getLegalHold();
  const allowlist = await client.isAllowlistActive();
  const summary = await client.getEscrowSummary();

  if (isHeld) {
    throw new Error("Legal hold is active; cannot fund.");
  }

  if (escrow.status !== 0) {
    throw new Error(
      `Escrow is not Open (status ${escrow.status}); cannot fund.`,
    );
  }

  if (allowlist && !summary.is_allowlist_active) {
    // Check if investor is allowlisted (custom check needed)
    throw new Error("Investor not on allowlist.");
  }

  // Safe to proceed
  await client.fund(investor, amount);
}
```

### Conditional Workflows

Use error codes to branch into different workflows:

```typescript
async function handleFundingError(err: unknown, context: string): Promise<void> {
  if (!(err instanceof SorobanContractError)) {
    throw err;
  }

  const code = err.code;

  if (code >= 100 && code <= 111) {
    // Funding error — inform investor
    await notifyInvestor(context, code);
  } else if (code >= 120 && code <= 152) {
    // Settlement/legal hold — inform admin
    await notifyAdmin(context, code);
  } else if (code >= 1 && code <= 13) {
    // Init error — developer issue
    console.error("Init configuration error; review parameters.");
    throw err;
  } else {
    // Unknown
    throw err;
  }
}
```

---

## Additional Resources

- **Full Error Code Reference:** [`docs/escrow-error-messages.md`](../../docs/escrow-error-messages.md)
- **Client Implementation:** [`src/client.ts`](../src/client.ts)
- **Type Definitions:** [`src/types.ts`](../src/types.ts)
- **Example Usage:** [`examples/basic-usage.ts`](../examples/basic-usage.ts)
- **Escrow State Machine:** [`docs/state-machine.md`](../../docs/state-machine.md)
- **CLI Simulation Recipes:** [`docs/escrow-sim-stellar-cli.md`](../../docs/escrow-sim-stellar-cli.md)

---

## Best Practices

1. **Always branch on error codes, not strings.** Error descriptions may change; codes are stable.
2. **Use `classifyError()` to group errors by category.** This simplifies broad error handling.
3. **Check state before calling methods.** Read via `getEscrow()`, `getLegalHold()`, etc. to catch state errors early.
4. **Distinguish contract errors from network errors.** Only retry on network failures, not contract errors.
5. **Preserve context in error messages.** Include escrow ID, investor address, and method name to aid debugging.
6. **Log error codes, not just text.** Numeric codes are stable and searchable across logs.
7. **Document expected errors per method.** Warn users which errors may occur and what recovery looks like.
