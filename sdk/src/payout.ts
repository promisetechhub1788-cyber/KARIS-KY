export interface ProRataPayoutInput {
  contribution: bigint | number | string;
  totalPrincipal: bigint | number | string;
  effectiveYieldBps: bigint | number | string;
}

export interface ProRataPayoutResult {
  contribution: bigint;
  totalPrincipal: bigint;
  effectiveYieldBps: bigint;
  coupon: bigint;
  settlePool: bigint;
  grossPayout: bigint;
}

const BPS_DENOMINATOR = 10_000n;

function asBigInt(value: bigint | number | string, field: string): bigint {
  try {
    return BigInt(value);
  } catch {
    throw new TypeError(`${field} must be an integer`);
  }
}

/** Calculate the contract-compatible payout before submitting a claim. */
export function computeProRataPayout(input: ProRataPayoutInput): ProRataPayoutResult {
  const contribution = asBigInt(input.contribution, "contribution");
  const totalPrincipal = asBigInt(input.totalPrincipal, "totalPrincipal");
  const effectiveYieldBps = asBigInt(input.effectiveYieldBps, "effectiveYieldBps");

  if (contribution < 0n) {
    throw new RangeError("contribution must be non-negative");
  }
  if (totalPrincipal <= 0n) {
    throw new RangeError("totalPrincipal must be positive");
  }
  if (effectiveYieldBps < 0n || effectiveYieldBps > BPS_DENOMINATOR) {
    throw new RangeError("effectiveYieldBps must be between 0 and 10000");
  }

  const coupon = (totalPrincipal * effectiveYieldBps) / BPS_DENOMINATOR;
  const settlePool = totalPrincipal + coupon;
  const grossPayout = (contribution * settlePool) / totalPrincipal;

  return {
    contribution,
    totalPrincipal,
    effectiveYieldBps,
    coupon,
    settlePool,
    grossPayout,
  };
}