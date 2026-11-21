use super::*;

// Yield tier table ordering at `init` (Issue #519). `validate_yield_tiers_table`
// requires `min_lock_secs` strictly increasing (`TierLockNotIncreasing`, 12) and
// `yield_bps` non-decreasing (`TierYieldNotNonDecreasing`, 13); lock ordering is
// checked first, so a table misordered in both dimensions reports code 12.

const BASE_YIELD_BPS: i64 = 800;

fn tiers(env: &Env, rows: &[(u64, i64)]) -> SorobanVec<YieldTier> {
    let mut out = SorobanVec::new(env);
    for &(min_lock_secs, yield_bps) in rows {
        out.push_back(YieldTier {
            min_lock_secs,
            yield_bps,
        });
    }
    out
}

/// `try_init` with base yield `BASE_YIELD_BPS` and the given tier rows; all other
/// optional settings unset. A macro so the client's `try_init` result type is inferred.
macro_rules! try_init_with_tiers {
    ($env:expr, $rows:expr) => {{
        let env: &Env = $env;
        let client = deploy(env);
        let admin = Address::generate(env);
        let sme = Address::generate(env);
        let (token, treasury) = free_addresses(env);
        client.try_init(
            &admin,
            &String::from_str(env, "TIERS519"),
            &sme,
            &1_000i128,
            &BASE_YIELD_BPS,
            &0u64,
            &token,
            &None,
            &treasury,
            &Some(tiers(env, $rows)),
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        )
    }};
}

#[test]
fn sorted_tier_table_is_accepted() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = try_init_with_tiers!(&env, &[(100, 850), (200, 900), (300, 900)])
        .expect("init should succeed")
        .expect("return value should convert");
    assert_eq!(escrow.yield_bps, BASE_YIELD_BPS);
}

#[test]
fn tier_table_misordered_by_lock_secs_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    assert_contract_error(
        try_init_with_tiers!(&env, &[(200, 850), (100, 900)]),
        EscrowError::TierLockNotIncreasing,
    );
}

#[test]
fn tier_table_misordered_by_yield_bps_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    assert_contract_error(
        try_init_with_tiers!(&env, &[(100, 950), (200, 900)]),
        EscrowError::TierYieldNotNonDecreasing,
    );
}

#[test]
fn tier_table_misordered_in_both_dimensions_reports_lock_order_first() {
    let env = Env::default();
    env.mock_all_auths();
    assert_contract_error(
        try_init_with_tiers!(&env, &[(200, 950), (100, 900)]),
        EscrowError::TierLockNotIncreasing,
    );
}
