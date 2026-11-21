import { describe, expect, it } from "vitest";
import { computeProRataPayout } from "./payout.js";

describe("computeProRataPayout", () => {
  it("computes a proportional principal plus yield payout", () => {
    const result = computeProRataPayout({
      contribution: 1_000n,
      totalPrincipal: 10_050n,
      effectiveYieldBps: 500n,
    });

    expect(result.coupon).toBe(502n);
    expect(result.settlePool).toBe(10_552n);
    expect(result.grossPayout).toBe(1_049n);
  });

  it("floors coupon and payout at base-unit precision", () => {
    const result = computeProRataPayout({
      contribution: 1n,
      totalPrincipal: 3n,
      effectiveYieldBps: 1n,
    });

    expect(result.coupon).toBe(0n);
    expect(result.grossPayout).toBe(1n);
  });

  it("returns the full settle pool for a sole investor", () => {
    const result = computeProRataPayout({
      contribution: 10_000_000_000n,
      totalPrincipal: 10_000_000_000n,
      effectiveYieldBps: 1_200n,
    });

    expect(result.grossPayout).toBe(result.settlePool);
  });

  it("rejects invalid payout inputs", () => {
    expect(() => computeProRataPayout({ contribution: 1, totalPrincipal: 0, effectiveYieldBps: 0 })).toThrow(
      "totalPrincipal must be positive",
    );
    expect(() => computeProRataPayout({ contribution: -1, totalPrincipal: 1, effectiveYieldBps: 0 })).toThrow(
      "contribution must be non-negative",
    );
    expect(() => computeProRataPayout({ contribution: 1, totalPrincipal: 1, effectiveYieldBps: 10_001 })).toThrow(
      "effectiveYieldBps must be between 0 and 10000",
    );
  });
});