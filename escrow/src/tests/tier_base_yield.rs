use super::*;

// Tier yields relative to the base yield at `init` (Issue #520).
// `validate_yield_tiers_table` rejects any tier whose `yield_bps` is below the
// escrow's base `yield_bps` with `TierYieldBelowBase` (code 11); a tier equal to
// or above the base is allowed.

const BASE_YIELD_BPS: i64 = 800;

/// `try_init` with base yield `BASE_YIELD_BPS` and a single tier of `tier_yield_bps`;
/// all other optional settings unset. A macro so the client's `try_init` result type
/// is inferred.
macro_rules! try_init_with_single_tier {
    ($env:expr, $tier_yield_bps:expr) => {{
        let env: &Env = $env;
        let client = deploy(env);
        let admin = Address::generate(env);
        let sme = Address::generate(env);
        let (token, treasury) = free_addresses(env);
        let mut tiers = SorobanVec::new(env);
        tiers.push_back(YieldTier {
            min_lock_secs: 100,
            yield_bps: $tier_yield_bps,
        });
        client.try_init(
            &admin,
            &String::from_str(env, "TIERS520"),
            &sme,
            &1_000i128,
            &BASE_YIELD_BPS,
            &0u64,
            &token,
            &None,
            &treasury,
            &Some(tiers),
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
fn tier_below_base_yield_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    assert_contract_error(
        try_init_with_single_tier!(&env, BASE_YIELD_BPS - 100),
        EscrowError::TierYieldBelowBase,
    );
}

#[test]
fn tier_equal_to_base_yield_is_allowed() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = try_init_with_single_tier!(&env, BASE_YIELD_BPS)
        .expect("init should succeed")
        .expect("return value should convert");
    assert_eq!(escrow.yield_bps, BASE_YIELD_BPS);
}

#[test]
fn tier_above_base_yield_is_allowed() {
    let env = Env::default();
    env.mock_all_auths();
    let escrow = try_init_with_single_tier!(&env, BASE_YIELD_BPS + 100)
        .expect("init should succeed")
        .expect("return value should convert");
    assert_eq!(escrow.yield_bps, BASE_YIELD_BPS);
}
