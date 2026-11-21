use super::*;

// `get_yield_tier_table` read API (Issue #527). `init` stores the tier table only
// when `yield_tiers` is `Some` and non-empty, so both "no tiers" and "empty tiers"
// read back as `None`.

/// `init` with base yield 800 bps and the given optional tier table; all other
/// optional settings unset. Returns the client for follow-up reads.
fn init_with_tiers<'a>(
    env: &'a Env,
    tiers: Option<SorobanVec<YieldTier>>,
) -> LiquifactEscrowClient<'a> {
    let client = deploy(env);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let (token, treasury) = free_addresses(env);
    client.init(
        &admin,
        &String::from_str(env, "TIERS527"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &tiers,
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
    );
    client
}

#[test]
fn yield_tier_table_is_none_without_tiers() {
    let env = Env::default();
    env.mock_all_auths();
    let client = init_with_tiers(&env, None);
    assert_eq!(client.get_yield_tier_table(), None);
}

#[test]
fn yield_tier_table_is_none_for_an_empty_table() {
    let env = Env::default();
    env.mock_all_auths();
    let client = init_with_tiers(&env, Some(SorobanVec::new(&env)));
    assert_eq!(client.get_yield_tier_table(), None);
}

#[test]
fn yield_tier_table_returns_the_full_stored_table() {
    let env = Env::default();
    env.mock_all_auths();
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 850,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 200,
        yield_bps: 900,
    });
    let client = init_with_tiers(&env, Some(tiers.clone()));
    assert_eq!(client.get_yield_tier_table(), Some(tiers));
}
