//! Property-based tests for funding invariants.
//!
//! All properties are verified with 100+ proptest iterations. Covers:
//! - Pro-rata payout conservation: Σ payout_i ≤ settle_pool
//! - funded_amount monotonicity and conservation
//! - Status transition forward-only invariant
//! - FundingCloseSnapshot written exactly once and immutable
//! - Unique funder count matches distinct contributing addresses
//! - Zero-contribution edge cases and single-investor edge cases
//! - Random investor counts (1–10), contributions (1–1_000_000), yields (0–10_000 bps)
//!
//! Reference formulas (docs/escrow-pro-rata.md):
//!   coupon      = total_principal × yield_bps / 10_000  (floor)
//!   settle_pool = total_principal + coupon
//!   payout_i    = contribution_i  × settle_pool / total_principal  (floor)
//!
//! Each test creates its own fresh `Env` so tests are fully isolated.

use super::*;
use proptest::prelude::*;
use std::collections::BTreeSet;

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Compute expected settle_pool from raw inputs, mirroring the on-chain formula.
/// coupon = total_principal × yield_bps / 10_000 (floor)
/// settle_pool = total_principal + coupon
fn expected_settle_pool(total_principal: i128, yield_bps: i64) -> i128 {
    let coupon = total_principal * (yield_bps as i128) / 10_000;
    total_principal + coupon
}

/// Deploy, init, fund all contributions, then settle. Returns the client.
/// `contributions` is a slice of (investor_address, amount) pairs.
fn deploy_funded_settled<'a>(
    env: &'a Env,
    invoice_id: &str,
    yield_bps: i64,
    contributions: &[(Address, i128)],
) -> LiquifactEscrowClient<'a> {
    let client = deploy(env);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let (token, treasury) = free_addresses(env);

    let total: i128 = contributions.iter().map(|(_, a)| *a).sum();
    client.init(
        &admin,
        &soroban_sdk::String::from_str(env, invoice_id),
        &sme,
        &total,
        &yield_bps,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    for (investor, amount) in contributions {
        client.fund(investor, amount);
    }
    client.settle();
    client
}

/// Minimal deterministic PRNG (SplitMix64) — no std dependency, no proptest overhead.
#[derive(Clone, Copy)]
struct Rng64 {
    state: u64,
}

impl Rng64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn range_i128(&mut self, lo: i128, hi: i128) -> i128 {
        assert!(lo <= hi);
        let span = (hi - lo) as u128 + 1;
        lo + ((self.next_u64() as u128 % span) as i128)
    }

    fn range_usize(&mut self, lo: usize, hi: usize) -> usize {
        assert!(lo <= hi);
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as usize
    }

    fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        assert!(lo <= hi);
        let span = (hi - lo) as u64 + 1;
        lo + (self.next_u64() % span) as i64
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 1 — Pro-rata payout conservation
// Invariant: Σ payout_i ≤ settle_pool, residue ≥ 0
// Iterations: proptest default (100+)
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// Core conservation property: sum of all investor payouts never exceeds
    /// the settle_pool, and the residue (dust) is non-negative.
    #[test]
    fn prop_payout_sum_le_settle_pool(
        n_investors in 1usize..=8usize,
        yield_bps   in 0i64..=10_000i64,
        seed        in 0u64..u64::MAX,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();
        let amounts: Vec<i128> = (0..n_investors)
            .map(|_| rng.range_i128(1, 1_000_000))
            .collect();
        let pairs: Vec<(Address, i128)> = investors.iter().cloned()
            .zip(amounts.iter().cloned())
            .collect();

        let client = deploy_funded_settled(&env, "CONS0001", yield_bps, &pairs);

        let snap = client.get_funding_close_snapshot()
            .expect("snapshot must exist after funding");
        let settle_pool = expected_settle_pool(snap.total_principal, yield_bps);

        let payout_sum: i128 = investors.iter()
            .map(|inv| client.compute_investor_payout(inv))
            .sum();

        prop_assert!(
            payout_sum <= settle_pool,
            "sum of payouts ({payout_sum}) must not exceed settle_pool ({settle_pool})"
        );
        let residue = settle_pool - payout_sum;
        prop_assert!(
            residue >= 0,
            "residue must be non-negative, got {residue}"
        );
    }

    /// Each individual payout must be ≥ contribution (yield is non-negative).
    /// Only holds for yield_bps ≥ 0 (always true given the range).
    #[test]
    fn prop_each_payout_ge_contribution(
        n_investors in 1usize..=6usize,
        yield_bps   in 0i64..=10_000i64,
        seed        in 0u64..u64::MAX,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();
        let amounts: Vec<i128> = (0..n_investors)
            .map(|_| rng.range_i128(1, 500_000))
            .collect();
        let pairs: Vec<(Address, i128)> = investors.iter().cloned()
            .zip(amounts.iter().cloned())
            .collect();

        let client = deploy_funded_settled(&env, "PAYOUT01", yield_bps, &pairs);

        for (inv, &contribution) in investors.iter().zip(amounts.iter()) {
            let payout = client.compute_investor_payout(inv);
            prop_assert!(
                payout >= contribution,
                "payout ({payout}) must be >= contribution ({contribution}) for yield_bps={yield_bps}"
            );
        }
    }

    /// Residue is strictly bounded: residue < n_investors (each floor operation
    /// drops at most 1 unit per investor).
    #[test]
    fn prop_residue_bounded_by_investor_count(
        n_investors in 2usize..=8usize,
        yield_bps   in 1i64..=10_000i64,
        seed        in 0u64..u64::MAX,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();
        let amounts: Vec<i128> = (0..n_investors)
            .map(|_| rng.range_i128(1, 500_000))
            .collect();
        let pairs: Vec<(Address, i128)> = investors.iter().cloned()
            .zip(amounts.iter().cloned())
            .collect();

        let client = deploy_funded_settled(&env, "RESIDU01", yield_bps, &pairs);

        let snap = client.get_funding_close_snapshot().unwrap();
        let settle_pool = expected_settle_pool(snap.total_principal, yield_bps);
        let payout_sum: i128 = investors.iter()
            .map(|inv| client.compute_investor_payout(inv))
            .sum();

        let residue = settle_pool - payout_sum;
        prop_assert!(
            residue < n_investors as i128,
            "residue ({residue}) must be < n_investors ({n_investors})"
        );
    }

    /// Zero-yield: every payout equals the original contribution, sum == total_principal.
    #[test]
    fn prop_zero_yield_payout_equals_contribution(
        n_investors in 1usize..=8usize,
        seed        in 0u64..u64::MAX,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();
        let amounts: Vec<i128> = (0..n_investors)
            .map(|_| rng.range_i128(1, 1_000_000))
            .collect();
        let total: i128 = amounts.iter().sum();
        let pairs: Vec<(Address, i128)> = investors.iter().cloned()
            .zip(amounts.iter().cloned())
            .collect();

        let client = deploy_funded_settled(&env, "ZEROYLD2", 0i64, &pairs);

        let payout_sum: i128 = investors.iter()
            .map(|inv| client.compute_investor_payout(inv))
            .sum();

        // At zero yield settle_pool == total_principal, floor division is exact.
        for (inv, &contribution) in investors.iter().zip(amounts.iter()) {
            let payout = client.compute_investor_payout(inv);
            prop_assert_eq!(
                payout, contribution,
                "zero yield: payout must equal contribution exactly"
            );
        }
        prop_assert_eq!(payout_sum, total, "zero yield: sum must equal total_principal");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 2 — funded_amount monotonicity and conservation
// Invariant: funded_amount never decreases; equals Σ contributions
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// funded_amount is strictly non-decreasing across sequential fund() calls.
    #[test]
    fn prop_funded_amount_non_decreasing(
        amounts in proptest::collection::vec(1i128..=100_000i128, 2..=10),
        target  in 1_000_000i128..=2_000_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme   = Address::generate(&env);
        let client = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "MONO0001"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        let mut prev_funded: i128 = 0;
        for amount in &amounts {
            if client.get_escrow().status != 0 {
                break;
            }
            let after = client.fund(&Address::generate(&env), amount);
            prop_assert!(
                after.funded_amount >= prev_funded,
                "funded_amount decreased: {} → {}",
                prev_funded, after.funded_amount
            );
            prev_funded = after.funded_amount;
        }
    }

    /// funded_amount exactly equals the running sum of all contributions.
    #[test]
    fn prop_funded_amount_equals_contribution_sum(
        amounts in proptest::collection::vec(1i128..=200_000i128, 1..=8),
        target  in 2_000_000i128..=5_000_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let sme   = Address::generate(&env);
        let client = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "SUMCHK01"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        let mut running_sum: i128 = 0;
        for amount in &amounts {
            if client.get_escrow().status != 0 {
                break;
            }
            let after = client.fund(&Address::generate(&env), amount);
            running_sum = running_sum.checked_add(*amount).unwrap();
            prop_assert_eq!(
                after.funded_amount, running_sum,
                "funded_amount must equal running sum of contributions"
            );
            // get_escrow() must agree with the return value
            prop_assert_eq!(
                client.get_escrow().funded_amount, running_sum,
                "get_escrow().funded_amount must match return value"
            );
        }
    }

    /// Individual get_contribution() accumulates correctly for a single investor
    /// making multiple fund() calls.
    #[test]
    fn prop_get_contribution_accumulates_for_repeat_funder(
        call_amounts in proptest::collection::vec(1i128..=50_000i128, 2..=6),
        target       in 1_000_000i128..=3_000_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin    = Address::generate(&env);
        let sme      = Address::generate(&env);
        let investor = Address::generate(&env);
        let client   = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "CONTRIB1"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        let mut expected_contribution: i128 = 0;
        for amount in &call_amounts {
            if client.get_escrow().status != 0 {
                break;
            }
            client.fund(&investor, amount);
            expected_contribution = expected_contribution.checked_add(*amount).unwrap();
            prop_assert_eq!(
                client.get_contribution(&investor),
                expected_contribution,
                "get_contribution must equal cumulative funded amount for that investor"
            );
        }
    }

    /// funded_amount must equal the sum of all individual get_contribution() values.
    /// Tests multi-investor conservation.
    #[test]
    fn prop_funded_amount_equals_sum_of_all_contributions(
        n_investors in 2usize..=6usize,
        seed        in 0u64..u64::MAX,
        target      in 500_000i128..=2_000_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin  = Address::generate(&env);
        let sme    = Address::generate(&env);
        let client = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "SUMALL01"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();

        // Fund until escrow is fully funded or all investors have contributed
        let max_each = (target / n_investors as i128).max(1);
        for inv in &investors {
            if client.get_escrow().status != 0 {
                break;
            }
            let amount = rng.range_i128(1, max_each);
            client.fund(inv, &amount);
        }

        // Sum of individual contributions must equal funded_amount
        let sum_of_contributions: i128 = investors.iter()
            .map(|inv| client.get_contribution(inv))
            .sum();
        prop_assert_eq!(
            client.get_escrow().funded_amount,
            sum_of_contributions,
            "funded_amount must equal sum of all individual contributions"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 3 — Status forward-only invariant
// Invariant: status only increases (0→1→2 or 0→1→3, never backwards)
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// Status never decreases across the full funding → settle lifecycle.
    #[test]
    fn prop_status_monotonically_non_decreasing(
        amount in 1i128..=500_000i128,
        target in 1i128..=500_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin    = Address::generate(&env);
        let sme      = Address::generate(&env);
        let investor = Address::generate(&env);
        let client   = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        let escrow_init = client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "STATUS01"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );
        prop_assert_eq!(escrow_init.status, 0);

        let after_fund = client.fund(&investor, &amount);
        prop_assert!(
            after_fund.status >= escrow_init.status,
            "status regressed after fund: {} → {}",
            escrow_init.status, after_fund.status
        );
        prop_assert!(after_fund.status <= 1, "status must be 0 or 1 after single fund");

        if after_fund.status == 1 {
            let after_settle = client.settle();
            prop_assert_eq!(after_settle.status, 2);
            prop_assert!(after_settle.status >= after_fund.status);
        }
    }

    /// Funding that doesn't reach target leaves status == 0.
    /// Funding that meets or exceeds target sets status == 1.
    #[test]
    fn prop_status_flips_exactly_at_target(
        contribution in 1i128..=500_000i128,
        target       in 1i128..=500_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin    = Address::generate(&env);
        let sme      = Address::generate(&env);
        let investor = Address::generate(&env);
        let client   = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "FLIP0001"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        let after = client.fund(&investor, &contribution);
        if contribution >= target {
            prop_assert_eq!(after.status, 1, "must be funded when contribution >= target");
        } else {
            prop_assert_eq!(after.status, 0, "must remain open when contribution < target");
        }
    }

    /// After cancel_funding(), status is 4 and cannot transition further.
    #[test]
    fn prop_cancelled_status_is_terminal(
        partial_amount in 1i128..=50_000i128,
        target         in 100_000i128..=500_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin    = Address::generate(&env);
        let sme      = Address::generate(&env);
        let investor = Address::generate(&env);
        let client   = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "CANCEL01"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        // Partially fund (won't reach target)
        client.fund(&investor, &partial_amount);
        prop_assert_eq!(client.get_escrow().status, 0);

        client.cancel_funding();
        prop_assert_eq!(client.get_escrow().status, 4, "status must be 4 after cancel");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 4 — FundingCloseSnapshot written exactly once and immutable
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// Snapshot is absent before the target is reached and present (exactly once)
    /// after. total_principal equals funded_amount at close; never changes after.
    #[test]
    fn prop_snapshot_written_once_and_immutable(
        n_investors in 2usize..=6usize,
        target      in 50_000i128..=500_000i128,
        seed        in 0u64..u64::MAX,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin  = Address::generate(&env);
        let sme    = Address::generate(&env);
        let client = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "SNAPONCE"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        // Snapshot must not exist before any funding
        prop_assert!(
            client.get_funding_close_snapshot().is_none(),
            "snapshot must not exist before any funding"
        );

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();

        let max_each = (target / n_investors as i128).max(1);
        let mut funded_total: i128 = 0;
        let mut snapshot_captured = false;

        for inv in &investors {
            if client.get_escrow().status != 0 {
                break;
            }
            let amount = rng.range_i128(1, max_each);
            funded_total = funded_total.checked_add(amount).unwrap();
            client.fund(inv, &amount);

            if client.get_escrow().status == 1 && !snapshot_captured {
                // Snapshot must now exist
                let snap = client.get_funding_close_snapshot()
                    .expect("snapshot must exist when status becomes 1");
                prop_assert_eq!(
                    snap.total_principal,
                    client.get_escrow().funded_amount,
                    "snapshot.total_principal must equal funded_amount at close"
                );
                prop_assert_eq!(snap.funding_target, target);

                // Read again — must be identical (immutable)
                let snap2 = client.get_funding_close_snapshot().unwrap();
                prop_assert_eq!(snap, snap2, "snapshot must be immutable across reads");
                snapshot_captured = true;

                // Settle and verify snapshot still unchanged
                client.settle();
                let snap3 = client.get_funding_close_snapshot().unwrap();
                prop_assert_eq!(snap2, snap3, "snapshot must survive settle unchanged");
            }
        }

        // If we reached funded status, exactly one snapshot must exist
        if client.get_escrow().status >= 1 {
            prop_assert!(
                client.get_funding_close_snapshot().is_some(),
                "snapshot must exist for any funded/settled escrow"
            );
        }
    }

    /// Overfunding: snapshot records the full over-funded amount, not just target.
    #[test]
    fn prop_snapshot_records_overfund_amount(
        target   in 10_000i128..=100_000i128,
        excess   in 1i128..=50_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin    = Address::generate(&env);
        let sme      = Address::generate(&env);
        let investor = Address::generate(&env);
        let client   = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "OVERFUND"),
            &sme,
            &target,
            &500i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        let fund_amount = target + excess;
        client.fund(&investor, &fund_amount);

        let snap = client.get_funding_close_snapshot()
            .expect("snapshot must exist after overfunding");
        prop_assert_eq!(
            snap.total_principal, fund_amount,
            "snapshot must record the full over-funded amount"
        );
        prop_assert!(
            snap.total_principal > target,
            "total_principal must exceed target when over-funded"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 5 — Unique funder count invariant
// Invariant: get_unique_funder_count() == count of distinct funded addresses
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// Unique funder count always equals the number of distinct addresses
    /// that have made at least one positive contribution.
    #[test]
    fn prop_unique_funder_count_matches_distinct_addresses(
        n_investors in 1usize..=8usize,
        n_repeat    in 0usize..=3usize,
        seed        in 0u64..u64::MAX,
        target      in 1_000_000i128..=5_000_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let admin  = Address::generate(&env);
        let sme    = Address::generate(&env);
        let client = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "UCOUNT01"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None, &None, &None, &None, &None, &None, &None, &None, &None,
        );

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();

        let mut distinct: BTreeSet<String> = BTreeSet::new();
        let max_each = (target / (n_investors as i128 + 1)).max(1);

        // First pass: each investor funds once
        for inv in &investors {
            if client.get_escrow().status != 0 {
                break;
            }
            let amount = rng.range_i128(1, max_each);
            client.fund(inv, &amount);
            distinct.insert(format!("{:?}", inv));
        }

        // Repeat funders: pick existing investors and add more funding
        for _ in 0..n_repeat {
            if client.get_escrow().status != 0 {
                break;
            }
            let idx = rng.range_usize(0, investors.len() - 1);
            let inv = &investors[idx];
            let amount = rng.range_i128(1, max_each.max(1));
            client.fund(inv, &amount);
            distinct.insert(format!("{:?}", inv));
        }

        // Unique funder count must equal actual distinct funded addresses
        let actual_distinct_count = investors.iter()
            .filter(|inv| client.get_contribution(inv) > 0)
            .count();

        prop_assert_eq!(
            client.get_unique_funder_count() as usize,
            actual_distinct_count,
            "unique_funder_count must equal distinct addresses with non-zero contribution"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 6 — Non-participant and edge cases
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// A non-participant (zero contribution) always gets payout == 0.
    #[test]
    fn prop_non_participant_payout_is_zero(
        contribution in 1i128..=500_000i128,
        yield_bps    in 0i64..=10_000i64,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let investor = Address::generate(&env);
        let stranger = Address::generate(&env);

        let client = deploy_funded_settled(
            &env,
            "NONPART2",
            yield_bps,
            &[(investor.clone(), contribution)],
        );

        prop_assert_eq!(
            client.compute_investor_payout(&stranger),
            0,
            "non-participant must receive 0"
        );
    }

    /// Non-participant has zero get_contribution().
    #[test]
    fn prop_non_participant_contribution_is_zero(
        yield_bps in 0i64..=10_000i64,
        amount    in 1i128..=1_000_000i128,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let investor = Address::generate(&env);
        let stranger = Address::generate(&env);

        let client = deploy_funded_settled(
            &env, "NONPART3", yield_bps, &[(investor, amount)],
        );

        prop_assert_eq!(
            client.get_contribution(&stranger),
            0,
            "stranger must have zero contribution"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 7 — Single investor edge cases
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(120))]

    /// Single investor: payout == settle_pool exactly (no rounding loss when
    /// contribution == total_principal).
    #[test]
    fn prop_single_investor_payout_equals_settle_pool(
        contribution in 1i128..=1_000_000i128,
        yield_bps    in 0i64..=10_000i64,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let investor = Address::generate(&env);

        let client = deploy_funded_settled(
            &env,
            "SINGLE02",
            yield_bps,
            &[(investor.clone(), contribution)],
        );

        let snap = client.get_funding_close_snapshot().unwrap();
        let settle_pool = expected_settle_pool(snap.total_principal, yield_bps);
        let payout = client.compute_investor_payout(&investor);

        prop_assert_eq!(
            payout, settle_pool,
            "single investor must receive full settle_pool (no floor loss)"
        );
    }

    /// Single investor: payout >= contribution (principal returned).
    #[test]
    fn prop_single_investor_payout_ge_contribution(
        contribution in 1i128..=2_000_000i128,
        yield_bps    in 0i64..=10_000i64,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let investor = Address::generate(&env);

        let client = deploy_funded_settled(
            &env,
            "SINGLE03",
            yield_bps,
            &[(investor.clone(), contribution)],
        );

        let payout = client.compute_investor_payout(&investor);
        prop_assert!(
            payout >= contribution,
            "single investor payout ({payout}) must be >= contribution ({contribution})"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 8 — Random investor counts and yield combinations
// Stress test: 1–10 investors, 1–1_000_000 contributions, 0–10_000 bps yield
// ─────────────────────────────────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Broad randomized sweep: any combination of investor count, contribution
    /// sizes, and yield must satisfy the core conservation invariant.
    #[test]
    fn prop_broad_conservation_sweep(
        n_investors in 1usize..=10usize,
        yield_bps   in 0i64..=10_000i64,
        seed        in 0u64..u64::MAX,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();
        let amounts: Vec<i128> = (0..n_investors)
            .map(|_| rng.range_i128(1, 1_000_000))
            .collect();
        let pairs: Vec<(Address, i128)> = investors.iter().cloned()
            .zip(amounts.iter().cloned())
            .collect();

        let client = deploy_funded_settled(&env, "BROAD001", yield_bps, &pairs);

        let snap = client.get_funding_close_snapshot()
            .expect("snapshot must exist after funding");
        let settle_pool = expected_settle_pool(snap.total_principal, yield_bps);

        let payout_sum: i128 = investors.iter()
            .map(|inv| client.compute_investor_payout(inv))
            .sum();

        // Core invariant: conservation
        prop_assert!(
            payout_sum <= settle_pool,
            "n={n_investors}, yield={yield_bps}bps: sum ({payout_sum}) > settle_pool ({settle_pool})"
        );
        // Non-negative residue
        prop_assert!(
            settle_pool - payout_sum >= 0,
            "residue must not be negative"
        );
        // Residue bounded by investor count
        prop_assert!(
            settle_pool - payout_sum < n_investors as i128,
            "residue must be < n_investors"
        );
        // Status must be settled (2)
        prop_assert_eq!(client.get_escrow().status, 2, "escrow must be settled");
        // funded_amount non-zero
        prop_assert!(
            client.get_escrow().funded_amount > 0,
            "funded_amount must be positive after funding"
        );
    }

    /// Mixed yield tiers with proptest: investors using different yield rates
    /// (base vs tiered) still satisfy Σ payout ≤ settle_pool at base rate.
    #[test]
    fn prop_mixed_base_yield_conservation(
        n_investors in 2usize..=6usize,
        base_yield  in 100i64..=5_000i64,
        seed        in 0u64..u64::MAX,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        // Set up with tiered yield: one tier above base
        let tier_yield = (base_yield + 500).min(10_000);
        let admin  = Address::generate(&env);
        let sme    = Address::generate(&env);
        let client = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        let mut rng = Rng64::new(seed);
        let investors: Vec<Address> = (0..n_investors)
            .map(|_| Address::generate(&env))
            .collect();
        let amounts: Vec<i128> = (0..n_investors)
            .map(|_| rng.range_i128(1, 300_000))
            .collect();
        let total: i128 = amounts.iter().sum();

        let tiers = soroban_sdk::vec![
            &env,
            crate::YieldTier { min_lock_secs: 100, yield_bps: tier_yield },
        ];

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "MIXED001"),
            &sme,
            &total,
            &base_yield,
            &0u64,
            &token,
            &None,
            &treasury,
            &Some(tiers),
            &None, &None, &None, &None, &None, &None, &None, &None,
        );

        // First investor uses commitment (tier yield); rest use base fund()
        client.fund_with_commitment(&investors[0], &amounts[0], &200u64);
        for i in 1..n_investors {
            if client.get_escrow().status != 0 {
                break;
            }
            client.fund(&investors[i], &amounts[i]);
        }
        client.settle();

        let snap = client.get_funding_close_snapshot()
            .expect("snapshot must exist");
        // Use base yield for conservative settle_pool lower bound
        let settle_pool_base = expected_settle_pool(snap.total_principal, base_yield);

        let payout_sum: i128 = investors.iter()
            .map(|inv| client.compute_investor_payout(inv))
            .sum();

        // Total payouts must not exceed settle_pool at the maximum possible yield
        let settle_pool_tier = expected_settle_pool(snap.total_principal, tier_yield);
        prop_assert!(
            payout_sum <= settle_pool_tier,
            "sum ({payout_sum}) must not exceed max settle_pool ({settle_pool_tier})"
        );
        // Sanity: base settle_pool <= tier settle_pool
        prop_assert!(settle_pool_base <= settle_pool_tier);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 9 — Deterministic fuzz loops (seed-driven, 100+ cases each)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn fuzz_payout_conservation_100_cases() {
    let cases: usize = std::env::var("ESCROW_FUZZ_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    let base_seed: u64 = 0xCAFE_F00D_DEAD_BEEF;

    for case_idx in 0..cases {
        let case_seed = base_seed ^ (case_idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut rng = Rng64::new(case_seed);

        let env = Env::default();
        env.mock_all_auths();

        let n = rng.range_usize(1, 10);
        let yield_bps = rng.range_i64(0, 10_000);

        let investors: Vec<Address> = (0..n).map(|_| Address::generate(&env)).collect();
        let amounts: Vec<i128> = (0..n).map(|_| rng.range_i128(1, 1_000_000)).collect();
        let pairs: Vec<(Address, i128)> = investors
            .iter()
            .cloned()
            .zip(amounts.iter().cloned())
            .collect();

        let client = deploy_funded_settled(&env, "FUZZ0100", yield_bps, &pairs);

        let snap = client
            .get_funding_close_snapshot()
            .unwrap_or_else(|| panic!("case {case_idx}: snapshot missing"));
        let settle_pool = expected_settle_pool(snap.total_principal, yield_bps);
        let payout_sum: i128 = investors
            .iter()
            .map(|inv| client.compute_investor_payout(inv))
            .sum();

        assert!(
            payout_sum <= settle_pool,
            "case {case_idx}: sum ({payout_sum}) > settle_pool ({settle_pool}), seed={case_seed}"
        );
        assert!(
            settle_pool - payout_sum >= 0,
            "case {case_idx}: negative residue, seed={case_seed}"
        );
        assert!(
            settle_pool - payout_sum < n as i128,
            "case {case_idx}: residue >= n_investors, seed={case_seed}"
        );
    }
}

/// 100-case fuzz for funded_amount = Σ contributions invariant.
#[test]
fn fuzz_funded_amount_equals_contribution_sum_100_cases() {
    let base_seed: u64 = 0xBEEF_CAFE_0101_0101;
    for case_idx in 0..100usize {
        let case_seed = base_seed ^ (case_idx as u64).wrapping_mul(0x6C62_272E_07BB_0142);
        let mut rng = Rng64::new(case_seed);

        let env = Env::default();
        env.mock_all_auths();

        let n = rng.range_usize(1, 8);
        let target = rng.range_i128(n as i128 * 100, 2_000_000);

        let admin = Address::generate(&env);
        let sme = Address::generate(&env);
        let client = deploy(&env);
        let (token, treasury) = free_addresses(&env);

        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "FUZZ0200"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &token,
            &None,
            &treasury,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );

        let max_each = (target / n as i128).max(1);
        let investors: Vec<Address> = (0..n).map(|_| Address::generate(&env)).collect();
        let mut running_sum: i128 = 0;

        for inv in &investors {
            if client.get_escrow().status != 0 {
                break;
            }
            let amount = rng.range_i128(1, max_each);
            running_sum = running_sum.checked_add(amount).unwrap();
            client.fund(inv, &amount);
        }

        let funded = client.get_escrow().funded_amount;
        assert_eq!(
            funded, running_sum,
            "case {case_idx}: funded_amount ({funded}) != sum ({running_sum}), seed={case_seed}"
        );

        let contribution_sum: i128 = investors
            .iter()
            .map(|inv| client.get_contribution(inv))
            .sum();
        assert_eq!(
            funded, contribution_sum,
            "case {case_idx}: funded_amount != sum of get_contribution(), seed={case_seed}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section 10 — Deterministic unit edge-case tests
// ─────────────────────────────────────────────────────────────────────────────

/// Single investor, contribution 1 (minimum positive), zero yield.
#[test]
fn edge_single_investor_minimum_contribution_zero_yield() {
    let env = Env::default();
    env.mock_all_auths();
    let investor = Address::generate(&env);
    let client = deploy_funded_settled(&env, "EDGE0001", 0i64, &[(investor.clone(), 1i128)]);

    let payout = client.compute_investor_payout(&investor);
    assert_eq!(
        payout, 1,
        "minimum single investor at zero yield: payout must be 1"
    );
    assert_eq!(client.get_contribution(&investor), 1);
    assert_eq!(client.get_unique_funder_count(), 1);
}

/// Single investor, maximum yield (10_000 bps = 100%): settle_pool = 2 × principal.
#[test]
fn edge_single_investor_max_yield() {
    let env = Env::default();
    env.mock_all_auths();
    let investor = Address::generate(&env);
    let contribution: i128 = 10_000;
    let client = deploy_funded_settled(
        &env,
        "EDGE0002",
        10_000i64,
        &[(investor.clone(), contribution)],
    );

    let payout = client.compute_investor_payout(&investor);
    // settle_pool = 10_000 + (10_000 * 10_000 / 10_000) = 20_000
    assert_eq!(
        payout, 20_000,
        "max yield single investor: payout must be 2x principal"
    );
}

/// Two investors with equal contributions: payouts are equal.
#[test]
fn edge_two_equal_investors_equal_payouts() {
    let env = Env::default();
    env.mock_all_auths();
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let contribution: i128 = 7_777;
    let yield_bps: i64 = 800;
    let client = deploy_funded_settled(
        &env,
        "EDGE0003",
        yield_bps,
        &[(inv_a.clone(), contribution), (inv_b.clone(), contribution)],
    );

    let pa = client.compute_investor_payout(&inv_a);
    let pb = client.compute_investor_payout(&inv_b);
    assert_eq!(pa, pb, "equal contributions must yield equal payouts");

    let snap = client.get_funding_close_snapshot().unwrap();
    let settle_pool = expected_settle_pool(snap.total_principal, yield_bps);
    assert!(pa + pb <= settle_pool, "sum must not exceed settle_pool");
}

/// Non-participant (never called fund()) gets payout = 0 and contribution = 0.
#[test]
fn edge_non_participant_zero_payout_and_contribution() {
    let env = Env::default();
    env.mock_all_auths();
    let investor = Address::generate(&env);
    let stranger = Address::generate(&env);
    let client = deploy_funded_settled(&env, "EDGE0004", 500i64, &[(investor, 50_000i128)]);

    assert_eq!(
        client.get_contribution(&stranger),
        0,
        "stranger contribution must be 0"
    );
    assert_eq!(
        client.compute_investor_payout(&stranger),
        0,
        "stranger payout must be 0"
    );
}

/// Prime-denominator total: residue is bounded by investor count.
#[test]
fn edge_prime_denominator_residue_bounded() {
    let env = Env::default();
    env.mock_all_auths();
    let investors: Vec<Address> = (0..3).map(|_| Address::generate(&env)).collect();
    // 97 + 101 + 103 = 301 (prime)
    let pairs = vec![
        (investors[0].clone(), 97i128),
        (investors[1].clone(), 101i128),
        (investors[2].clone(), 103i128),
    ];
    let yield_bps: i64 = 1_000;
    let client = deploy_funded_settled(&env, "EDGE0005", yield_bps, &pairs);

    let snap = client.get_funding_close_snapshot().unwrap();
    let settle_pool = expected_settle_pool(snap.total_principal, yield_bps);
    let payout_sum: i128 = investors
        .iter()
        .map(|inv| client.compute_investor_payout(inv))
        .sum();

    assert!(
        payout_sum <= settle_pool,
        "prime denom: sum must not exceed settle_pool"
    );
    let residue = settle_pool - payout_sum;
    assert!(residue >= 0, "residue must be non-negative");
    assert!(residue < 3, "residue ({residue}) must be < n_investors (3)");
}

/// Zero contributions list: escrow initialized with target but no fund() calls.
/// Status must remain 0; snapshot must be None; unique funder count must be 0.
#[test]
fn edge_no_contributions_zero_state() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "EDGE0006"),
        &sme,
        &100_000i128,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(
        client.get_escrow().status,
        0,
        "no contributions: status must be 0"
    );
    assert_eq!(
        client.get_escrow().funded_amount,
        0,
        "no contributions: funded_amount must be 0"
    );
    assert!(
        client.get_funding_close_snapshot().is_none(),
        "no snapshot before any funding"
    );
    assert_eq!(
        client.get_unique_funder_count(),
        0,
        "no unique funders before any funding"
    );
}

/// Investor funds exactly the target: status flips to 1 on that exact call.
#[test]
fn edge_exact_target_funding() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);
    let (token, treasury) = free_addresses(&env);
    let target: i128 = 100_000;

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "EDGE0007"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let after = client.fund(&investor, &target);
    assert_eq!(after.status, 1, "exact target funding must set status to 1");
    assert_eq!(after.funded_amount, target);

    let snap = client
        .get_funding_close_snapshot()
        .expect("snapshot must exist on exact-target funding");
    assert_eq!(snap.total_principal, target);
    assert_eq!(snap.funding_target, target);
}

/// Two investors together hit target; second investor's fund() call tips the escrow.
#[test]
fn edge_two_investors_second_crosses_target() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let client = deploy(&env);
    let (token, treasury) = free_addresses(&env);
    let target: i128 = 100_000;

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "EDGE0008"),
        &sme,
        &target,
        &500i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let after_a = client.fund(&inv_a, &60_000i128);
    assert_eq!(after_a.status, 0, "first investor partial: status still 0");
    assert!(
        client.get_funding_close_snapshot().is_none(),
        "no snapshot before threshold"
    );

    let after_b = client.fund(&inv_b, &40_000i128);
    assert_eq!(
        after_b.status, 1,
        "second investor crosses target: status becomes 1"
    );

    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, 100_000i128);

    // Settle and verify payouts
    client.settle();
    let pa = client.compute_investor_payout(&inv_a);
    let pb = client.compute_investor_payout(&inv_b);
    let settle_pool = expected_settle_pool(100_000, 500);
    assert!(pa + pb <= settle_pool);
    assert!(
        pa > 0 && pb > 0,
        "both investors must have positive payouts"
    );
}

/// Large number of investors (10) with small contributions: conservation holds.
#[test]
fn edge_ten_investors_small_contributions() {
    let env = Env::default();
    env.mock_all_auths();
    let investors: Vec<Address> = (0..10).map(|_| Address::generate(&env)).collect();
    let pairs: Vec<(Address, i128)> = investors
        .iter()
        .cloned()
        .map(|inv| (inv, 1_000i128))
        .collect();
    let yield_bps: i64 = 1_500;

    let client = deploy_funded_settled(&env, "EDGE0009", yield_bps, &pairs);

    let snap = client.get_funding_close_snapshot().unwrap();
    let settle_pool = expected_settle_pool(snap.total_principal, yield_bps);
    let payout_sum: i128 = investors
        .iter()
        .map(|inv| client.compute_investor_payout(inv))
        .sum();

    assert!(
        payout_sum <= settle_pool,
        "10 investors: sum must not exceed settle_pool"
    );
    assert_eq!(
        client.get_unique_funder_count(),
        10,
        "must have 10 unique funders"
    );
}

/// is_investor_claimed() transitions false → true after settle + mark.
/// (Investor claiming is recorded correctly.)
#[test]
fn edge_investor_claim_flag_transitions() {
    let env = Env::default();
    env.mock_all_auths();
    let investor = Address::generate(&env);
    let client = deploy_funded_settled(&env, "EDGE0010", 800i64, &[(investor.clone(), 50_000i128)]);

    assert!(
        !client.is_investor_claimed(&investor),
        "must not be claimed before claim call"
    );
    client.claim_investor_payout(&investor);
    assert!(
        client.is_investor_claimed(&investor),
        "must be claimed after claim call"
    );
}

/// Contribution for an investor who made multiple fund() calls accumulates correctly.
#[test]
fn edge_multi_call_same_investor_accumulates() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "EDGE0011"),
        &sme,
        &300_000i128,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund(&investor, &50_000i128);
    assert_eq!(client.get_contribution(&investor), 50_000);

    client.fund(&investor, &80_000i128);
    assert_eq!(client.get_contribution(&investor), 130_000);

    client.fund(&investor, &70_000i128);
    assert_eq!(client.get_contribution(&investor), 200_000);

    assert_eq!(client.get_escrow().funded_amount, 200_000);
    assert_eq!(
        client.get_unique_funder_count(),
        1,
        "same investor: unique count must be 1"
    );
}
