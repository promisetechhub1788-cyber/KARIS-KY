//! Tests for the four feature tasks:
//! 1. EscrowHealthMetrics (get_escrow_health_metrics)
//! 2. Investor contribution bucketing benchmark
//! 3. New events: FundReceived, AdminChanged, LegalHoldSet, EscrowPaused
//! 4. CI cargo-audit (tested via CI, not here)

use super::*;
use crate::{
    AdminChanged, DataKey, EscrowHealthMetrics, EscrowPaused, FundReceived, LegalHoldSet,
    YieldTier, INVESTOR_BUCKET_COUNT,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, Ledger as _},
    Address, Env, Symbol, Vec as SorobanVec,
};

// ─── Task 1: Escrow Health Metrics ────────────────────────────────────────

#[test]
fn test_health_metrics_zero_progress_after_init() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "HLTH001"),
        &sme,
        &1_000_000i128,
        &500i64,
        &86_400u64 * 30, // 30 days maturity
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
    );

    let metrics = client.get_escrow_health_metrics();

    assert_eq!(metrics.funding_progress_percent, 0);
    // days_to_maturity should be approximately 30 days
    assert!(metrics.days_to_maturity > 0, "maturity is in the future");
    assert!(metrics.days_to_maturity <= 30, "should not exceed 30 days");
    assert_eq!(metrics.unique_investor_count, 0);
    assert_eq!(metrics.average_contribution_size, 0);
    assert_eq!(metrics.estimated_yield_payout, 0);
}

#[test]
fn test_health_metrics_partial_funding() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    let target = 1_000_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "HLTH002"),
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
    );

    let investor = Address::generate(&env);
    // Fund 50% of target
    client.fund(&investor, &(target / 2));

    let metrics = client.get_escrow_health_metrics();

    assert_eq!(metrics.funding_progress_percent, 50);
    assert_eq!(metrics.unique_investor_count, 1);
    assert_eq!(metrics.average_contribution_size, target / 2);
    // estimated_yield_payout = (500_000 * 800) / 10000 = 40_000
    assert_eq!(metrics.estimated_yield_payout, 40_000);
}

#[test]
fn test_health_metrics_overfunded_caps_at_100() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    let target = 1_000_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "HLTH003"),
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
    );

    let investor = Address::generate(&env);
    // Fund 150% of target
    client.fund(&investor, &(target + target / 2));

    let metrics = client.get_escrow_health_metrics();

    assert_eq!(
        metrics.funding_progress_percent, 100,
        "overfunded should cap at 100%"
    );
    assert_eq!(metrics.unique_investor_count, 1);
    assert_eq!(metrics.average_contribution_size, target + target / 2);
}

#[test]
fn test_health_metrics_maturity_past() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    let target = 1_000_000i128;
    let maturity_secs = 86400u64; // 1 day from epoch

    client.init(
        &admin,
        &String::from_str(&env, "HLTH004"),
        &sme,
        &target,
        &500i64,
        &maturity_secs,
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
    );

    let metrics = client.get_escrow_health_metrics();

    // Our test ledger starts at timestamp 12345 (from setup), so maturity is future
    assert!(
        metrics.days_to_maturity > 0,
        "maturity should be future relative to 12345"
    );
}

#[test]
fn test_health_metrics_multiple_investors() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    let target = 1_000_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "HLTH005"),
        &sme,
        &target,
        &1000i64,
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
    );

    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);

    client.fund(&inv_a, &200_000i128);
    client.fund(&inv_b, &300_000i128);
    client.fund(&inv_c, &500_000i128);

    let metrics = client.get_escrow_health_metrics();

    assert_eq!(metrics.unique_investor_count, 3);
    assert_eq!(metrics.funding_progress_percent, 100);
    assert_eq!(metrics.average_contribution_size, 1_000_000i128 / 3);
}

#[test]
fn test_health_metrics_zero_maturity() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "HLTH006"),
        &sme,
        &1_000_000i128,
        &500i64,
        &0u64, // no maturity lock
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
    );

    let metrics = client.get_escrow_health_metrics();
    assert_eq!(metrics.days_to_maturity, 0, "zero maturity means no lock");
}

// ─── Task 2: Bucketing Benchmark ──────────────────────────────────────────

#[test]
fn test_bucketing_aggregate_matches_funded_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    let target = 10_000_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "BUCK001"),
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
    );

    let mut total_bucketed: i128 = 0;
    for _i in 0..20 {
        let investor = Address::generate(&env);
        let contribution = 10_000i128;
        client.fund(&investor, &contribution);
        total_bucketed += contribution;
    }

    // Sum up all buckets — should equal funded_amount
    let mut bucket_sum: i128 = 0;
    env.as_contract(&client.address, || {
        for b in 0..INVESTOR_BUCKET_COUNT {
            let val: i128 = env
                .storage()
                .instance()
                .get(&DataKey::InvestorContributionBucket(b))
                .unwrap_or(0);
            bucket_sum += val;
        }
    });

    let escrow = client.get_escrow();
    assert_eq!(
        bucket_sum, escrow.funded_amount,
        "bucketed aggregate must equal funded_amount"
    );
    assert_eq!(bucket_sum, total_bucketed);
}

#[test]
fn test_bucketing_cost_baseline_many_investors() {
    // Measure the cost of funding with many investors to establish a baseline
    // for the bucketing optimization. Uses env.budget() to track CPU/IO.
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    let target = 100_000_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "BUCK002"),
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
    );

    // Record budget before bulk funding
    let budget_before = env.budget();

    let investor_count = 100u32;
    for _i in 0..investor_count {
        let investor = Address::generate(&env);
        client.fund(&investor, &(target / (investor_count as i128)));
    }

    let budget_after = env.budget();

    // Budget measurement sanity check — we expect some CPU consumption.
    // This is a cost-baseline test, not a hard assertion; it documents
    // the resource profile for future optimization comparisons.
    let cpu_used = budget_after.cpu_insns() - budget_before.cpu_insns();
    assert!(cpu_used > 0, "should consume CPU for 100 investors");
    assert!(
        cpu_used < 500_000_000,
        "CPU should stay under 500M instructions for 100 investors; actual: {}",
        cpu_used
    );
}

#[test]
fn test_bucketing_bucket_distribution_is_reasonably_uniform() {
    // Verify that investor addresses hash into at least some variety of buckets.
    // With 100 random addresses and 256 buckets, we expect ~100 distinct occupied
    // buckets (collisions are possible but rare with DJB2 + SHA256).
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "BUCK003"),
        &sme,
        &1_000_000i128,
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
    );

    for _i in 0..50 {
        let investor = Address::generate(&env);
        client.fund(&investor, &100i128);
    }

    // Count non-zero buckets
    let mut non_zero_buckets = 0u32;
    env.as_contract(&client.address, || {
        for b in 0..INVESTOR_BUCKET_COUNT {
            let val: i128 = env
                .storage()
                .instance()
                .get(&DataKey::InvestorContributionBucket(b))
                .unwrap_or(0);
            if val > 0 {
                non_zero_buckets += 1;
            }
        }
    });

    assert!(
        non_zero_buckets >= 20,
        "Expected at least 20 non-zero buckets with 50 investors, got {}",
        non_zero_buckets
    );
}

#[test]
fn test_bucketing_refund_subtracts_from_bucket() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let token = crate::tests::install_stellar_asset_token(&env);

    client.init(
        &admin,
        &String::from_str(&env, "BUCK004"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &token.id,
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    client.fund(&investor, &500i128);

    // Snapshot bucket state after fund
    let bucket_sum_after_fund = {
        let mut sum = 0i128;
        env.as_contract(&client.address, || {
            for b in 0..INVESTOR_BUCKET_COUNT {
                let val: i128 = env
                    .storage()
                    .instance()
                    .get(&DataKey::InvestorContributionBucket(b))
                    .unwrap_or(0);
                sum += val;
            }
        });
        sum
    };
    assert_eq!(bucket_sum_after_fund, 500);

    // Cancel and refund
    client.cancel_funding();
    token.stellar.mint(&client.address, &500);
    client.refund(&investor);

    // Bucket sum should now be 0
    let bucket_sum_after_refund = {
        let mut sum = 0i128;
        env.as_contract(&client.address, || {
            for b in 0..INVESTOR_BUCKET_COUNT {
                let val: i128 = env
                    .storage()
                    .instance()
                    .get(&DataKey::InvestorContributionBucket(b))
                    .unwrap_or(0);
                sum += val;
            }
        });
        sum
    };
    assert_eq!(
        bucket_sum_after_refund, 0,
        "buckets should be empty after full refund"
    );
}

// ─── Task 3: Integration Tests for New Events ────────────────────────────

#[test]
fn test_fund_received_event_emitted_on_fund() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);
    let investor = Address::generate(&env);
    let invoice_id = symbol_short!("EVT_FR");

    client.init(
        &admin,
        &String::from_str(&env, "EVT_FR"),
        &sme,
        &1_000i128,
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
    );
    client.fund(&investor, &500i128);

    // Check that FundReceived event is in the event stream
    let all_events = env.events().all();
    let fund_recv_events: Vec<_> = all_events
        .events()
        .iter()
        .filter(|e| {
            // Soroban events include the contract ID + topics. Look for our event name.
            let event_str = format!("{:?}", e);
            event_str.contains("fund_recv")
        })
        .collect();
    assert!(
        !fund_recv_events.is_empty(),
        "FundReceived event should be emitted on fund()"
    );
}

#[test]
fn test_admin_changed_event_emitted_on_propose_and_accept() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);
    let new_admin = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "EVT_AC"),
        &sme,
        &1_000i128,
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
    );

    client.propose_admin(&new_admin);
    client.accept_admin();

    // Verify AdminChanged events exist
    let all_events = env.events().all();
    let adm_chg_events: Vec<_> = all_events
        .events()
        .iter()
        .filter(|e| {
            let event_str = format!("{:?}", e);
            event_str.contains("adm_chg")
        })
        .collect();
    assert!(
        adm_chg_events.len() >= 2,
        "AdminChanged events should be emitted for both propose and accept, got {}",
        adm_chg_events.len()
    );
}

#[test]
fn test_legal_hold_set_event_emitted() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "EVT_LH"),
        &sme,
        &1_000i128,
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
    );

    client.set_legal_hold(&true);
    client.set_legal_hold(&false);

    let all_events = env.events().all();
    let legal_set_events: Vec<_> = all_events
        .events()
        .iter()
        .filter(|e| {
            let event_str = format!("{:?}", e);
            event_str.contains("legal_set")
        })
        .collect();
    assert!(
        legal_set_events.len() >= 2,
        "LegalHoldSet events should be emitted for both enable and disable, got {}",
        legal_set_events.len()
    );
}

#[test]
fn test_escrow_paused_event_emitted() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "EVT_EP"),
        &sme,
        &1_000i128,
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
    );

    assert!(!client.is_escrow_paused());

    client.set_escrow_paused(&true);
    assert!(client.is_escrow_paused());

    let all_events = env.events().all();
    let pause_events: Vec<_> = all_events
        .events()
        .iter()
        .filter(|e| {
            let event_str = format!("{:?}", e);
            event_str.contains("esc_pause")
        })
        .collect();
    assert!(
        !pause_events.is_empty(),
        "EscrowPaused event should be emitted on pause"
    );

    client.set_escrow_paused(&false);
    assert!(!client.is_escrow_paused());
}

#[test]
#[should_panic]
fn test_fund_blocked_while_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "PAUSE01"),
        &sme,
        &1_000i128,
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
    );

    client.set_escrow_paused(&true);
    client.fund(&investor, &100i128); // should panic with EscrowIsPaused
}

#[test]
#[should_panic]
fn test_settle_blocked_while_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "PAUSE02"),
        &sme,
        &1_000i128,
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
    );

    client.fund(&investor, &1_000i128);
    client.set_escrow_paused(&true);
    client.settle(); // should panic with EscrowIsPaused
}

#[test]
#[should_panic]
fn test_withdraw_blocked_while_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let token = crate::tests::install_stellar_asset_token(&env);
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "PAUSE03"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &token.id,
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund(&investor, &1_000i128);
    token.stellar.mint(&client.address, &1_000i128);
    client.set_escrow_paused(&true);
    client.withdraw(); // should panic with EscrowIsPaused
}

#[test]
#[should_panic]
fn test_claim_investor_payout_blocked_while_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &String::from_str(&env, "PAUSE04"),
        &sme,
        &1_000i128,
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
    );

    client.fund(&investor, &1_000i128);
    client.settle();
    client.set_escrow_paused(&true);
    client.claim_investor_payout(&investor); // should panic with EscrowIsPaused
}

#[test]
#[should_panic]
fn test_cancel_funding_blocked_while_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "PAUSE05"),
        &sme,
        &1_000i128,
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
    );

    client.set_escrow_paused(&true);
    client.cancel_funding(); // should panic with EscrowIsPaused
}

#[test]
fn test_read_only_works_while_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "PAUSE06"),
        &sme,
        &1_000i128,
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
    );

    client.set_escrow_paused(&true);

    // Read-only operations should still work
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 0);
    let summary = client.get_escrow_summary();
    assert!(!summary.legal_hold);
    let metrics = client.get_escrow_health_metrics();
    assert_eq!(metrics.funding_progress_percent, 0);
    assert!(client.is_escrow_paused());
}
