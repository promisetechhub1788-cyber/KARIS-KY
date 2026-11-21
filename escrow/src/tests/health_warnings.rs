//! Tests for Escrow Health Warning System (#231).
//!
//! Verifies health warnings are emitted when escrow enters unhealthy states.

use soroban_sdk::{Address, Env, String as SorobanString};

const TARGET: i128 = 100_000_000_000i128;

#[test]
fn test_health_warning_low_funding_ratio() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    // Initialize with 1M target
    let target = 1_000_000i128;
    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_LOW_FUND"),
        &sme,
        &target,
        &800i64,
        &0u64, // No maturity
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund only 40% of target => should trigger low funding ratio warning
    let funded_amount = 400_000i128;
    client.fund(&investor, &funded_amount);

    // Check health metrics
    let (warning_type, funded_ratio_bps, _) = client.check_escrow_health();

    assert_eq!(
        warning_type, 4001,
        "Low funding ratio (40%) should emit warning code 4001"
    );
    assert_eq!(
        funded_ratio_bps, 4000,
        "Funded ratio should be 4000 bps (40%)"
    );
}

#[test]
fn test_health_warning_close_to_maturity() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;

    // Set maturity to 1 hour in the future
    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_CLOSE_MAT"),
        &sme,
        &target,
        &800i64,
        &(now + 3600), // 1 hour
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund to target (healthy ratio)
    client.fund(&investor, &target);

    // Advance to 30 minutes before maturity
    env.ledger().set_timestamp(now + 1800);

    let (warning_type, funded_ratio_bps, time_to_maturity_secs) =
        client.check_escrow_health();

    assert_eq!(
        warning_type, 4002,
        "Close to maturity (30 min) with healthy funding should emit code 4002"
    );
    assert!(funded_ratio_bps >= 5000, "Funding ratio should be healthy");
    assert!(
        time_to_maturity_secs > 0 && time_to_maturity_secs < 86400,
        "Time to maturity should be between 0 and 1 day"
    );
}

#[test]
fn test_health_warning_low_funding_close_to_maturity() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_LOW_CLOSE"),
        &sme,
        &target,
        &800i64,
        &(now + 3600),
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund only 40%
    client.fund(&investor, &(400_000i128));

    // Advance to 30 minutes before maturity
    env.ledger().set_timestamp(now + 1800);

    let (warning_type, funded_ratio_bps, _) = client.check_escrow_health();

    assert_eq!(
        warning_type, 4001,
        "Low funding + close to maturity should emit code 4001 (takes priority)"
    );
    assert!(funded_ratio_bps < 5000, "Funded ratio should be low");
}

#[test]
fn test_health_warning_over_maturity_unfunded() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_OVER_MAT"),
        &sme,
        &target,
        &800i64,
        &(now + 3600),
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Do NOT fund - status remains 0
    // Advance to 30 minutes PAST maturity
    env.ledger().set_timestamp(now + 3600 + 1800);

    let (warning_type, _, time_to_maturity_secs) = client.check_escrow_health();

    assert_eq!(
        warning_type, 4003,
        "Past maturity + unfunded + open should emit code 4003"
    );
    assert!(
        time_to_maturity_secs < 0,
        "Time to maturity should be negative"
    );
}

#[test]
fn test_no_health_warning_healthy_escrow() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_HEALTHY"),
        &sme,
        &target,
        &800i64,
        &(now + 90 * 86400), // 90 days
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund to 100%
    client.fund(&investor, &target);

    let (warning_type, funded_ratio_bps, time_to_maturity_secs) =
        client.check_escrow_health();

    assert_eq!(warning_type, 0, "Healthy escrow should emit no warning");
    assert!(funded_ratio_bps >= 10_000, "Funded ratio should be >= 100%");
    assert!(time_to_maturity_secs > 86400, "Time to maturity should be > 1 day");
}

#[test]
fn test_no_health_warning_no_maturity_constraint() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let target = 1_000_000i128;

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_NO_MAT"),
        &sme,
        &target,
        &800i64,
        &0u64, // No maturity
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund at 60% (not low ratio)
    client.fund(&investor, &(600_000i128));

    let (warning_type, funded_ratio_bps, time_to_maturity_secs) =
        client.check_escrow_health();

    assert_eq!(
        warning_type, 0,
        "Healthy funding with no maturity should emit no warning"
    );
    assert!(funded_ratio_bps >= 5000, "Funded ratio is 60%, which is healthy");
    assert_eq!(
        time_to_maturity_secs, i64::MAX,
        "Time to maturity should be i64::MAX when no maturity set"
    );
}

#[test]
fn test_no_health_warning_settled_escrow() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_SETTLED"),
        &sme,
        &target,
        &800i64,
        &(now + 3600),
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund and settle
    client.fund(&investor, &target);
    env.ledger().set_timestamp(now + 3600);
    client.settle();

    let (warning_type, _, _) = client.check_escrow_health();

    assert_eq!(
        warning_type, 0,
        "Settled escrow (status 2) should emit no warning"
    );
}


#[test]
fn test_health_warning_funding_stalled_after_threshold() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;
    let stall_threshold = 7200u64; // 2 hours

    // Initialize with stall threshold
    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_STALL"),
        &sme,
        &target,
        &800i64,
        &(now + 100 * 86400), // 100 days (far future)
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &Some(stall_threshold),
    );

    // Fund only 30% of target (underfunded)
    client.fund(&investor, &(300_000i128));

    // Immediately check - no warning (just funded)
    let (warning_type, _, _) = client.check_escrow_health();
    assert_eq!(
        warning_type, 0,
        "Immediately after funding, should emit no stall warning"
    );

    // Advance time past the stall threshold
    env.ledger().set_timestamp(now + stall_threshold + 1);

    // Check health again
    let (warning_type, funded_ratio_bps, _) = client.check_escrow_health();
    assert_eq!(
        warning_type, 4004,
        "After stall threshold with underfunded escrow should emit code 4004"
    );
    assert!(funded_ratio_bps < 5000, "Should still be underfunded");
}

#[test]
fn test_no_health_warning_funding_not_stalled() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;
    let stall_threshold = 7200u64; // 2 hours

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_NOT_STALL"),
        &sme,
        &target,
        &800i64,
        &(now + 100 * 86400),
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &Some(stall_threshold),
    );

    // Fund only 30%
    client.fund(&investor, &(300_000i128));

    // Advance time but NOT past the stall threshold
    env.ledger().set_timestamp(now + stall_threshold - 1);

    let (warning_type, _, _) = client.check_escrow_health();
    assert_eq!(
        warning_type, 0,
        "Before stall threshold should emit no warning"
    );
}

#[test]
fn test_no_warning_stall_threshold_not_configured() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;

    // Initialize WITHOUT stall threshold
    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_NO_STALL_CFG"),
        &sme,
        &target,
        &800i64,
        &(now + 100 * 86400),
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None, // No stall threshold
    );

    // Fund only 30%
    client.fund(&investor, &(300_000i128));

    // Advance time by many hours
    env.ledger().set_timestamp(now + 86400);

    let (warning_type, _, _) = client.check_escrow_health();
    assert_eq!(
        warning_type, 0,
        "Without stall threshold configured, should not emit 4004 warning"
    );
}

#[test]
fn test_no_warning_funding_stalled_but_fully_funded() {
    let env = Env::default();
    let (client, admin, sme) = super::setup(&env);
    let investor = Address::generate(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;
    let stall_threshold = 3600u64;

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_STALL_FULL"),
        &sme,
        &target,
        &800i64,
        &(now + 100 * 86400),
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &Some(stall_threshold),
    );

    // Fund 100% (meets target)
    client.fund(&investor, &target);

    // Advance past stall threshold (escrow is now funded, status = 1)
    env.ledger().set_timestamp(now + stall_threshold + 1);

    let (warning_type, _, _) = client.check_escrow_health();
    assert_eq!(
        warning_type, 0,
        "Funded escrow (status != 0) should not trigger stall warning even if time has passed"
    );
}

#[test]
fn test_funding_stalled_never_funded_escrow() {
    let env = Env::default();
    let (client, admin, _sme) = super::setup(&env);

    let now = env.ledger().timestamp();
    let target = 1_000_000i128;
    let stall_threshold = 3600u64;

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV_STALL_NEVER"),
        &Address::generate(&env), // SME
        &target,
        &800i64,
        &(now + 100 * 86400),
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &Some(stall_threshold),
    );

    // DO NOT fund at all

    // Advance past stall threshold
    env.ledger().set_timestamp(now + stall_threshold + 1);

    let (warning_type, _, _) = client.check_escrow_health();
    assert_eq!(
        warning_type, 4004,
        "Unfunded escrow that never received funding should trigger stall after threshold"
    );
}
