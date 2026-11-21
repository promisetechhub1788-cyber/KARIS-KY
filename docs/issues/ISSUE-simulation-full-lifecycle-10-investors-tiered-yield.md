# Issue: Add Full-Lifecycle Simulation with Ten Investors and Tiered Yield

**Type:** Test / integration coverage
**Status:** Backlog, ready for review and assignment
**Priority:** Medium
**Severity:** Medium
**Related:** [Simulation tests](../../escrow/tests/simulation.rs), [ADR-005: Tiered yield and commitment locks](../adr/ADR-005-tiered-yield.md), [Investor yield-tier guide](../escrow-investor-yield-tier-guide.md), [Simulation CLI recipes](../escrow-sim-stellar-cli.md), [Pro-rata payout mathematics](../escrow-pro-rata.md)

## Description

Extend `escrow/tests/simulation.rs` with a full lifecycle scenario involving
ten distinct investors and an immutable tiered-yield table. The test should
exercise the same workflow an integration client uses:

`init -> simulate funding -> actual funding -> simulate settlement -> actual
settlement -> simulate claims -> actual claims`

Each investor should make a first deposit with `fund_with_commitment`, using a
range of commitment durations that selects different tiers, and then complete
any required follow-on funding through `fund`. The scenario must verify that
simulation results predict the corresponding actual operation without mutating
contract state, while tier selection, per-investor effective yield, claim locks,
pro-rata payouts, rounding, and final claim state remain correct for all ten
investors.

This is intended as a deterministic integration fixture, not a performance
benchmark. It should use small, explicit token-base-unit values and assert
invariants that remain stable across Soroban test runs.

## Current Behavior

`escrow/tests/simulation.rs` covers `simulate_fund`, `simulate_settle`, and
`simulate_claim_investor_payout` independently. Existing scenarios use one or
two investors, the default base yield, and no tier table. Tier-selection and
commitment-lock rules are tested separately in `escrow/src/tests/funding.rs`,
but there is no simulation test proving that those per-investor values survive
a complete ten-investor workflow or agree with actual claims.

The current simulation tests also compare selected state fields rather than a
complete lifecycle snapshot. They do not verify every investor's effective
yield, claim-not-before timestamp, payout, claim marker, aggregate payout
conservation, or the absence of writes after repeated simulations.

## Steps to Reproduce

1. Open `escrow/tests/simulation.rs` and run the existing simulation tests.
2. Observe that the fixture initializes a flat-yield escrow and exercises
   funding, settlement, and claims in separate small scenarios.
3. Configure an equivalent escrow with a tier table, for example a base yield
   of `500` bps and tiers at `1,000` seconds / `750` bps and `2,000` seconds /
   `1,000` bps.
4. Create ten investors with distinct addresses and commitment durations that
   cover base-yield, first-tier, and second-tier selections, including exact
   threshold and below-threshold values.
5. Attempt to use the simulation entrypoints as a complete dry-run before
   performing the actual lifecycle.
6. Observe that no existing test proves all ten projected investor outcomes
   match the actual lifecycle, or that claim locks and tier-specific payouts
   are preserved through settlement.

## Expected Behavior

- Simulation entrypoints are read-only and require no investor or SME auth,
  while actual operations enforce their normal authorization.
- `simulate_fund` predicts the next escrow state without changing funded amount,
  status, investor contributions, tier records, claim locks, events, or any
  other storage.
- Each first `fund_with_commitment` selects the highest tier whose
  `min_lock_secs` is at or below the commitment, and stores that effective
  yield immutably for the investor.
- Follow-on deposits use `fund`, preserve the first deposit's effective yield
  and claim lock, and never allow tier reselection.
- Once the ten deposits reach the target, simulated and actual funding state
  agree and the escrow is funded.
- `simulate_settle` predicts settled status and does not write settlement
  state, snapshots, or events. Actual settlement then produces the same
  lifecycle result.
- `simulate_claim_investor_payout` returns the same payout as the corresponding
  eligible actual claim, using each investor's effective yield and the shared
  pro-rata/rounding rules.
- Claims before an investor's `claim_not_before` timestamp are rejected by the
  actual path; claims at or after the boundary succeed after settlement.
- All ten eligible investors can claim exactly once. Repeated simulations do
  not affect actual claims, and repeated actual claims remain idempotent.
- Aggregate principal and yield accounting is conserved within the documented
  integer rounding and terminal-dust policy; no investor receives another
  investor's tier or payout.

## Actual Behavior

There is no single simulation test covering ten investors, tier selection,
commitment locks, settlement, and claims as one lifecycle. The existing tests
therefore leave integration regressions undetected, including a simulation
path that reads flat-yield assumptions, loses per-investor tier state, applies
claim locks incorrectly, mutates storage, or disagrees with actual payout
calculation.

Tier behavior is currently validated in separate unit tests, and basic
simulation behavior is validated with small flat-yield fixtures. Neither suite
alone proves the complete user-visible workflow for a multi-investor tiered
escrow.

## Proposed Solution

1. Add a dedicated test in `escrow/tests/simulation.rs`, or a clearly named
   companion section in that file, using shared test helpers rather than
   duplicating contract setup logic.
2. Initialize a deterministic escrow with a valid immutable tier table. Include
   at least base yield, one lower tier, and one higher tier. Ensure thresholds
   are strictly increasing and yields are non-decreasing as required by
   ADR-005.
3. Generate exactly ten investors and define an explicit fixture table with
   amount, commitment duration, expected matched tier, expected effective
   yield, and expected claim-not-before timestamp for each investor. Include:
   - a zero-lock/base-yield investor;
   - commitments below the first threshold;
   - commitments exactly at each threshold; and
   - commitments above the highest threshold.
4. Run simulations before each actual funding call and assert projected
   `InvoiceEscrow` state and unchanged storage/events. Use `fund_with_commitment`
   for each first deposit, then at least one `fund` follow-on deposit for a
   subset of investors to verify tier preservation.
5. Simulate and repeat settlement, asserting status and aggregate amounts match
   while no settlement mutation occurs during simulation.
6. Simulate every claim, advance the mock ledger to exercise both locked and
   unlocked boundaries, then perform actual claims. Compare simulated and
   actual payouts using the contract's integer arithmetic rather than floating
   point calculations.
7. Assert all ten claim markers, payout history records, events, and aggregate
   balances. Verify repeated simulations remain identical and repeated actual
   claims do not double-pay or duplicate claim events.
8. Keep the fixture deterministic and bounded. Avoid using this test to assert
   instruction-cost thresholds; add a separate benchmark if scale metrics are
   needed.
9. Document the scenario in the simulation test index or related developer
   documentation and run the full escrow test suite before assignment closure.

## Environment Context

- **Repository:** `KARIS-KY`
- **Contract:** `escrow` Soroban smart contract
- **Test target:** `escrow/tests/simulation.rs`
- **Language/toolchain:** Rust, Cargo, Soroban SDK test environment
- **Lifecycle:** initialization, tiered funding, settlement, investor claims
- **Investors:** exactly ten distinct addresses
- **Tier configuration:** `Vec<YieldTier>` passed during `init`; each tier has
  `min_lock_secs` and `yield_bps`
- **Tier rule:** first deposit selects the best matching tier; follow-on
  deposits use `fund` and retain the first selection
- **Claim rule:** settlement plus `ledger.timestamp() >= claim_not_before`
- **Simulation APIs:** `simulate_fund`, `simulate_settle`, and
  `simulate_claim_investor_payout`
- **Actual APIs:** `fund_with_commitment`, `fund`, `settle`, and
  `claim_investor_payout`
- **Verification:** focused simulation test, tier/funding tests, settlement and
  claim tests, then the full Cargo test suite

## Acceptance Criteria

- [ ] `simulation.rs` contains a named full-lifecycle test with exactly ten
      investors and a configured tier table.
- [ ] The fixture covers base yield, below-threshold, exact-threshold, and
      above-highest-threshold commitment durations.
- [ ] Every investor's expected tier, effective yield, commitment lock, and
      claim-not-before timestamp is asserted after funding.
- [ ] At least one follow-on `fund` deposit per selected fixture path preserves
      the investor's original tier and claim lock.
- [ ] Simulated funding matches actual funding for escrow state and does not
      mutate contributions, tier records, locks, events, or other storage.
- [ ] Simulated settlement matches actual settlement while leaving state
      unchanged during simulation.
- [ ] Simulated payouts match actual payouts for all ten investors under the
      contract's integer rounding rules.
- [ ] Claim attempts before each lock boundary fail, and claims at or after the
      boundary succeed after settlement.
- [ ] All ten investors can claim once; repeated actual claims are idempotent,
      and repeated simulations produce identical results without new events.
- [ ] Aggregate accounting verifies principal conservation and documented
      rounding/terminal-dust behavior, with no cross-investor tier leakage.
- [ ] The test uses deterministic ledger timestamps and does not rely on
      floating-point arithmetic, wall-clock time, or random expected values.
- [ ] Focused simulation, tiered-yield, settlement, and claim tests pass, along
      with the full escrow test suite.
- [ ] The test's purpose and fixture assumptions are documented for future
      maintainers, and it remains separate from performance benchmarking.

## Assignment Notes

Before assignment, confirm the exact tier schedule and payout expectations to
use as the stable fixture contract. Decide whether the ten investors should
fund equal amounts or deliberately varied amounts; varied amounts provide
stronger pro-rata coverage, while equal amounts simplify expected values. Also
confirm whether the test should include actual token transfers or remain a
state/simulation consistency test using the repository's existing lightweight
address setup.
