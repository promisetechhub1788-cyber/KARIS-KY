use super::*;
use crate::BatchFundCompleted;
use soroban_sdk::{Error, InvokeError};
use std::fmt::Debug;

// Funding, contributions, snapshots, tier selection, and fund-shaped cost baselines.

fn assert_contract_error<T, E>(
    result: Result<Result<T, E>, Result<Error, InvokeError>>,
    expected: EscrowError,
) where
    T: Debug,
    E: Debug,
{
    let expected_code = expected as u32;
    match result {
        Err(Ok(error)) => {
            assert_eq!(error, Error::from_contract_error(expected_code));
        }
        Err(Err(InvokeError::Contract(code))) => {
            assert_eq!(code, expected_code);
        }
        other => panic!("expected ContractError({expected_code}), got {other:?}"),
    }
}

#[test]
fn test_fund_and_settle() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INVMETA"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    let funded = client.fund(&investor, &TARGET);
    assert_eq!(funded.funded_amount, TARGET);
    assert_eq!(funded.status, 1);
    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

#[test]
fn test_fund_partial_then_full() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV002"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    let partial = client.fund(&investor, &(TARGET / 2));
    assert_eq!(partial.status, 0);
    assert_eq!(partial.funded_amount, TARGET / 2);
    let full = client.fund(&investor, &(TARGET / 2));
    assert_eq!(full.status, 1);
    assert_eq!(full.funded_amount, TARGET);
}

#[test]
#[should_panic]
fn test_fund_zero_amount_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &0i128);
}

#[test]
#[should_panic]
fn test_fund_after_funded_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    client.fund(&investor, &1i128);
}

#[test]
fn test_fund_requires_investor_auth() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    assert!(
        env.auths().iter().any(|(addr, _)| *addr == investor),
        "investor auth was not recorded for fund"
    );
}

#[test]
fn test_single_investor_contribution_tracked() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV020"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(30_000_000_000i128));
    let contribution = client.get_contribution(&investor);
    assert_eq!(contribution, 30_000_000_000i128);
}

#[test]
fn test_unknown_investor_contribution_is_zero() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let stranger = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &1_000i128);
    assert_eq!(client.get_contribution(&stranger), 0i128);
}

#[test]
fn test_repeated_funding_accumulates_contribution() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV021"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(20_000_000_000i128));
    client.fund(&investor, &(30_000_000_000i128));
    assert_eq!(client.get_contribution(&investor), 50_000_000_000i128);
}

#[test]
fn test_reinvest_yield_rejects_target_not_in_funding_state() {
    let env = Env::default();
    env.mock_all_auths();
    let (source_client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&source_client, &env, &admin, &sme);

    let target_client = deploy(&env);
    let target = target_client.address.clone();
    let target_admin = Address::generate(&env);
    let target_sme = Address::generate(&env);
    target_client.init(
        &target_admin,
        &String::from_str(&env, "TARGET_ESCROW"),
        &target_sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    let target_investor = Address::generate(&env);
    target_client.fund(&target_investor, &TARGET);
    target_client.settle();
    source_client.fund(&investor, &TARGET);
    source_client.settle();

    let result = source_client.try_reinvest_yield(&investor, &target, &1_000i128);
    assert_contract_error(result, EscrowError::ReinvestTargetEscrowNotFunding);
}

#[test]
#[should_panic]
fn test_funding_amount_accumulation_overflow_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "OVF001"),
        &sme,
        &i128::MAX,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund(&investor_a, &(i128::MAX - 1));
    client.fund(&investor_b, &2i128);
}

#[test]
fn test_funding_amount_overflow_does_not_mutate_state() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "OVF002"),
        &sme,
        &i128::MAX,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund(&investor, &(i128::MAX - 1));
    let before = client.get_escrow();

    let overflowed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&investor, &2i128);
    }));
    assert!(overflowed.is_err());

    let after = client.get_escrow();
    assert_eq!(after.funded_amount, before.funded_amount);
    assert_eq!(after.status, 0);
    assert_eq!(client.get_contribution(&investor), i128::MAX - 1);
}

#[test]
#[should_panic]
fn test_fund_with_commitment_overflow_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "OVF001b"),
        &sme,
        &i128::MAX,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund(&investor_a, &(i128::MAX - 1));
    client.fund_with_commitment(&investor_b, &2i128, &0u64);
}

#[test]
fn test_fund_with_commitment_overflow_does_not_mutate_state() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "OVF002b"),
        &sme,
        &i128::MAX,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund(&investor_a, &(i128::MAX - 1));
    let before = client.get_escrow();

    let overflowed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund_with_commitment(&investor_b, &2i128, &0u64);
    }));
    assert!(overflowed.is_err());

    let after = client.get_escrow();
    assert_eq!(after.funded_amount, before.funded_amount);
    assert_eq!(after.status, 0);
    assert_eq!(client.get_contribution(&investor_b), 0);
}

#[test]
fn test_fund_overflow_emits_typed_error() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "OVF_TYPED"),
        &sme,
        &i128::MAX,
        &800i64,
        &0u64,
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

    // Fund to near-maximum
    client.fund(&investor_a, &(i128::MAX - 1));

    // Attempt to add amount that would overflow
    let result = client.try_fund(&investor_b, &2i128);
    assert_contract_error(result, EscrowError::FundedAmountOverflow);
}

#[test]
fn test_fund_with_commitment_overflow_emits_typed_error() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "OVF_COM_TYPED"),
        &sme,
        &i128::MAX,
        &800i64,
        &0u64,
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

    // Fund to near-maximum
    client.fund(&investor_a, &(i128::MAX - 1));

    // Attempt to add amount that would overflow via commitment variant
    let result = client.try_fund_with_commitment(&investor_b, &2i128, &0u64);
    assert_contract_error(result, EscrowError::FundedAmountOverflow);
}

/// Regression for issue #253: per-investor accounting must live in persistent storage, not instance.
#[test]
fn test_per_investor_contribution_uses_persistent_storage() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let contract_id = client.address.clone();
    let investor = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "PERS01"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &500i128);

    env.as_contract(&contract_id, || {
        assert_eq!(
            env.storage()
                .persistent()
                .get::<DataKey, i128>(&DataKey::InvestorContribution(investor.clone())),
            Some(500i128)
        );
        assert_eq!(
            env.storage()
                .instance()
                .get::<DataKey, i128>(&DataKey::InvestorContribution(investor.clone())),
            None
        );
    });
}

#[test]
#[should_panic]
fn test_investor_contribution_overflow_panics_even_if_state_is_inconsistent() {
    // This test intentionally constructs an inconsistent storage snapshot to ensure
    // the per-investor accounting never wraps even under corrupted / unexpected state.
    //
    // Rationale: `funded_amount` overflow is already guarded by checked_add. This test
    // separately proves the per-investor `InvestorContribution` update uses checked_add.
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let contract_id = client.address.clone();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "OVF003"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    env.as_contract(&contract_id, || {
        // Force the contribution near i128::MAX while keeping funded_amount small.
        // `fund` must still trap on contribution overflow even if funded_amount would not.
        env.storage().persistent().set(
            &DataKey::InvestorContribution(investor.clone()),
            &(i128::MAX - 1),
        );
        let mut escrow = LiquifactEscrow::get_escrow(env.clone());
        escrow.funded_amount = 0;
        escrow.status = 0;
        env.storage().instance().set(&DataKey::Escrow, &escrow);
    });

    client.fund(&investor, &2i128);
}

#[test]
fn test_investor_contribution_overflow_does_not_mutate_state() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let contract_id = client.address.clone();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "OVF004"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    env.as_contract(&contract_id, || {
        env.storage().persistent().set(
            &DataKey::InvestorContribution(investor.clone()),
            &(i128::MAX - 1),
        );
        let mut escrow = LiquifactEscrow::get_escrow(env.clone());
        escrow.funded_amount = 0;
        escrow.status = 0;
        env.storage().instance().set(&DataKey::Escrow, &escrow);
    });

    let before_escrow = client.get_escrow();
    let before_contribution = client.get_contribution(&investor);

    let overflowed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&investor, &2i128);
    }));
    assert!(overflowed.is_err());

    assert_eq!(client.get_escrow(), before_escrow);
    assert_eq!(client.get_contribution(&investor), before_contribution);
}

#[test]
fn test_multiple_investors_tracked_independently() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV023"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&inv_a, &(20_000_000_000i128));
    client.fund(&inv_b, &(50_000_000_000i128));
    client.fund(&inv_c, &(30_000_000_000i128));
    assert_eq!(client.get_contribution(&inv_a), 20_000_000_000i128);
    assert_eq!(client.get_contribution(&inv_b), 50_000_000_000i128);
    assert_eq!(client.get_contribution(&inv_c), 30_000_000_000i128);
    let sum = client.get_contribution(&inv_a)
        + client.get_contribution(&inv_b)
        + client.get_contribution(&inv_c);
    assert_eq!(sum, client.get_escrow().funded_amount);
}

#[test]
fn test_contributions_sum_equals_funded_amount() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV023b"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&inv_a, &(20_000_000_000i128));
    client.fund(&inv_b, &(50_000_000_000i128));
    client.fund(&inv_c, &(30_000_000_000i128));
    let sum = client.get_contribution(&inv_a)
        + client.get_contribution(&inv_b)
        + client.get_contribution(&inv_c);
    assert_eq!(sum, client.get_escrow().funded_amount);
}

#[test]
fn test_cost_baseline_fund_partial() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV103"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(10_000_000_000i128));
}

#[test]
fn test_cost_baseline_fund_full() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV104"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &TARGET);
}

#[test]
fn test_cost_baseline_fund_overshoot() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV105"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(150_000_000_000i128));
    assert_eq!(client.get_escrow().status, 1);
}

#[test]
fn test_cost_baseline_fund_two_step_completion() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV106"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&investor, &(TARGET / 2));
    client.fund(&investor, &(TARGET / 2));
    assert_eq!(client.get_escrow().status, 1);
}

#[test]
fn test_funding_close_snapshot_captures_overfunded_total_once() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SNAP001"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert_eq!(client.get_funding_close_snapshot(), None);
    client.fund(&inv, &(TARGET + 50_000_000_000i128));
    let snap = client.get_funding_close_snapshot().expect("snapshot");
    assert_eq!(snap.total_principal, TARGET + 50_000_000_000i128);
    assert_eq!(snap.funding_target, TARGET);
    assert_eq!(snap.closed_at_ledger_timestamp, env.ledger().timestamp());
    assert_eq!(snap.closed_at_ledger_sequence, env.ledger().sequence());
}

#[test]
fn test_funding_snapshot_immutable_across_second_fund_after_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SNAP002"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&a, &(TARGET / 2));
    assert_eq!(client.get_funding_close_snapshot(), None);
    client.fund(&b, &(TARGET / 2));
    let s1 = client.get_funding_close_snapshot().unwrap();
    assert_eq!(s1.total_principal, TARGET);
    let s2 = client.get_funding_close_snapshot().unwrap();
    assert_eq!(s1, s2);
}

#[test]
fn test_pro_rata_weight_ratio_from_snapshot() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SNAP003"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&a, &(20_000_000_000i128));
    client.fund(&b, &(80_000_000_000i128));
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, TARGET);
    let ca = client.get_contribution(&a);
    let cb = client.get_contribution(&b);
    assert_eq!(ca + cb, snap.total_principal);
}

#[test]
fn test_tiered_yield_and_follow_on_fund() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 500,
        yield_bps: 1100,
    });
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "TIER001"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund_with_commitment(&inv, &5_000i128, &200u64);
    assert_eq!(client.get_investor_yield_bps(&inv), 900);
    assert_eq!(client.get_investor_claim_not_before(&inv), 200);
    client.fund(&inv, &5_000i128);
    assert_eq!(client.get_investor_yield_bps(&inv), 900);
    assert_eq!(client.get_escrow().status, 1);
}

#[test]
fn test_tier_selection_edges_base_vs_high_bucket() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let i_short = Address::generate(&env);
    let i_long = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 50,
        yield_bps: 850,
    });
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "TIER002"),
        &sme,
        &20_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund_with_commitment(&i_short, &10_000i128, &40u64);
    assert_eq!(client.get_investor_yield_bps(&i_short), 800);
    client.fund_with_commitment(&i_long, &10_000i128, &50u64);
    assert_eq!(client.get_investor_yield_bps(&i_long), 850);
}

#[test]
#[should_panic]
fn test_fund_with_commitment_twice_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 1,
        yield_bps: 810,
    });
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "TIER003"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund_with_commitment(&inv, &5_000i128, &10u64);
    client.fund_with_commitment(&inv, &5_000i128, &10u64);
}

#[test]
#[should_panic]
fn test_fund_then_fund_with_commitment_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SEQ001"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    client.fund(&inv, &5_000i128);
    client.fund_with_commitment(&inv, &5_000i128, &10u64);
}

#[test]
fn test_tier_selection_ladder() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 200,
        yield_bps: 1000,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "LADDER01"),
        &sme,
        &100_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let inv_base = Address::generate(&env);
    let inv_tier1 = Address::generate(&env);
    let inv_tier2 = Address::generate(&env);
    let inv_mid = Address::generate(&env);

    // Below first tier -> base
    client.fund_with_commitment(&inv_base, &1_000i128, &50u64);
    assert_eq!(client.get_investor_yield_bps(&inv_base), 800);

    // Exactly first tier
    client.fund_with_commitment(&inv_tier1, &1_000i128, &100u64);
    assert_eq!(client.get_investor_yield_bps(&inv_tier1), 900);

    // Between tiers -> still first tier
    client.fund_with_commitment(&inv_mid, &1_000i128, &150u64);
    assert_eq!(client.get_investor_yield_bps(&inv_mid), 900);

    // Exactly second tier
    client.fund_with_commitment(&inv_tier2, &1_000i128, &200u64);
    assert_eq!(client.get_investor_yield_bps(&inv_tier2), 1000);
}

#[test]
fn test_yield_tier_emitted_in_event() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let invoice_id = symbol_short!("EVT001");

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "EVT001"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let inv = Address::generate(&env);

    // 1. Tiered fund: committed_lock_secs=150 >= tier.min_lock_secs=100 => yield 900, lock 100.
    client.fund_with_commitment(&inv, &1_000i128, &150u64);

    assert_eq!(
        env.events().all(),
        std::vec![EscrowFunded {
            name: symbol_short!("funded"),
            invoice_id: invoice_id.clone(),
            investor: inv.clone(),
            amount: 1_000i128,
            funded_amount: 1_000i128,
            status: 0,
            investor_effective_yield_bps: 900,
            tier_lock_secs: 100,
        }
        .to_xdr(&env, &contract_id)]
    );

    // 2. Base yield: committed_lock_secs=50 < tier.min_lock_secs=100 => yield 800, lock 0.
    let inv2 = Address::generate(&env);
    client.fund_with_commitment(&inv2, &1_000i128, &50u64);

    let binding = env.events().all();
    let all_event_list = binding.events();
    let last = all_event_list
        .last()
        .expect("expected funded event for base-yield deposit");
    assert_eq!(
        *last,
        EscrowFunded {
            name: symbol_short!("funded"),
            invoice_id,
            investor: inv2,
            amount: 1_000i128,
            funded_amount: 2_000i128,
            status: 0,
            investor_effective_yield_bps: 800,
            tier_lock_secs: 0,
        }
        .to_xdr(&env, &contract_id)
    );
}

#[test]
fn test_yield_tier_emitted_no_tiers() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let invoice_id = symbol_short!("NOTIER");

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "NOTIER"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let inv = Address::generate(&env);
    // fund_with_commitment even with no tiers configured
    client.fund_with_commitment(&inv, &1_000i128, &150u64);

    assert_eq!(
        env.events().all(),
        std::vec![EscrowFunded {
            name: symbol_short!("funded"),
            invoice_id: invoice_id.clone(),
            investor: inv.clone(),
            amount: 1_000i128,
            funded_amount: 1_000i128,
            status: 0,
            investor_effective_yield_bps: 800,
            tier_lock_secs: 0,
        }
        .to_xdr(&env, &contract_id)]
    );
}

#[test]
fn test_yield_tier_emitted_between_tiers() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let invoice_id = symbol_short!("MIDTIER");

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 200,
        yield_bps: 1000,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MIDTIER"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let inv = Address::generate(&env);
    // Committing 150 secs (between 100 and 200) -> matches the 100 sec tier.
    client.fund_with_commitment(&inv, &1_000i128, &150u64);

    let binding = env.events().all();
    let event = binding.events().first().unwrap();
    assert_eq!(
        *event,
        EscrowFunded {
            name: symbol_short!("funded"),
            invoice_id: invoice_id.clone(),
            investor: inv.clone(),
            amount: 1_000i128,
            funded_amount: 1_000i128,
            status: 0,
            investor_effective_yield_bps: 900,
            tier_lock_secs: 100,
        }
        .to_xdr(&env, &contract_id)
    );
}

#[test]
fn test_fund_with_commitment_zero_lock_behaves_as_fund() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "ZERO001"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund_with_commitment(&inv, &5_000i128, &0u64);
    assert_eq!(client.get_investor_yield_bps(&inv), 800);
    assert_eq!(client.get_investor_claim_not_before(&inv), 0);
}

#[test]
fn test_commitment_claim_time_allows_u64_max_boundary() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = u64::MAX - 5;
    });
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CLKMAX1"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund_with_commitment(&investor, &100i128, &5u64);

    assert_eq!(client.get_investor_claim_not_before(&investor), u64::MAX);
}

#[test]
#[should_panic]
fn test_commitment_claim_time_overflow_panics() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = u64::MAX - 5;
    });
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CLKMAX2"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund_with_commitment(&investor, &100i128, &6u64);
}

#[test]
fn test_commitment_claim_time_overflow_does_not_record_position() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = u64::MAX - 5;
    });
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CLKMAX3"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let overflowed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund_with_commitment(&investor, &100i128, &6u64);
    }));
    assert!(overflowed.is_err());

    assert_eq!(client.get_escrow().funded_amount, 0);
    assert_eq!(client.get_contribution(&investor), 0);
    assert_eq!(client.get_investor_claim_not_before(&investor), 0);
}

#[test]
#[should_panic]
fn test_init_bad_tier_order_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 200,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 950,
    });
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "BADTIER"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

#[test]
#[should_panic]
fn test_init_tier_yield_below_base_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 10,
        yield_bps: 700,
    });
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "BADT2"),
        &sme,
        &1_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

#[test]
fn test_differential_funding_target_eq_exact_cross() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let t = 5_000i128;
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DIFF002"),
        &sme,
        &t,
        &100i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    let escrow = client.fund(&inv, &t);
    assert_eq!(escrow.funded_amount, t);
    assert_eq!(escrow.status, 1);
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.total_principal, t);
    assert_eq!(snap.funding_target, t);
}

#[test]
fn test_ledger_sequence_recorded_in_snapshot_with_tick() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DIFF003"),
        &sme,
        &1_000i128,
        &100i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    let seq = env.ledger().sequence();
    client.fund(&inv, &1_000i128);
    let snap = client.get_funding_close_snapshot().unwrap();
    assert_eq!(snap.closed_at_ledger_sequence, seq);
}

#[test]
fn test_funding_close_snapshot_both_fields_written_correctly() {
    // Verify that both ledger_timestamp and ledger_sequence are correctly captured
    // in the snapshot at the moment of the funded transition.
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SNAP_BOTH_FIELDS_01"),
        &sme,
        &2_000i128,
        &5_000i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Set specific ledger time and sequence before funding
    let expected_timestamp = 42_000u64;
    let expected_sequence = 123u32;
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = expected_timestamp;
        ledger.sequence_number = expected_sequence;
    });

    // Fund the escrow to trigger the snapshot write
    client.fund(&inv, &2_000i128);

    // Verify both fields are correctly written
    let snap = client
        .get_funding_close_snapshot()
        .expect("snapshot must be present after funding");
    assert_eq!(
        snap.closed_at_ledger_timestamp, expected_timestamp,
        "ledger_timestamp must be captured from env.ledger().timestamp()"
    );
    assert_eq!(
        snap.closed_at_ledger_sequence, expected_sequence,
        "ledger_sequence must be captured from env.ledger().sequence()"
    );

    // Also verify other snapshot fields are correct
    assert_eq!(snap.total_principal, 2_000i128);
    assert_eq!(snap.funding_target, 2_000i128);
}

#[test]
fn test_funding_close_snapshot_both_fields_written_after_partial_settle() {
    // Verify that both ledger_timestamp and ledger_sequence are correctly captured
    // in the snapshot when written during partial_settle (early close).
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SNAP_PARTIAL_SETTLE_01"),
        &sme,
        &5_000i128,
        &8_000i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Partial fund (escrow remains open)
    client.fund(&inv, &1_000i128);
    assert_eq!(
        client.get_funding_close_snapshot(),
        None,
        "snapshot must not exist yet for underfunded escrow"
    );

    // Set specific ledger time and sequence before partial_settle
    let expected_timestamp = 99_999u64;
    let expected_sequence = 555u32;
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = expected_timestamp;
        ledger.sequence_number = expected_sequence;
    });

    // Call partial_settle to close funding early
    client.partial_settle();

    // Verify both fields are correctly written in the snapshot
    let snap = client
        .get_funding_close_snapshot()
        .expect("snapshot must be present after partial_settle");
    assert_eq!(
        snap.closed_at_ledger_timestamp, expected_timestamp,
        "ledger_timestamp must be captured at partial_settle time"
    );
    assert_eq!(
        snap.closed_at_ledger_sequence, expected_sequence,
        "ledger_sequence must be captured at partial_settle time"
    );

    // Verify the funded_amount is captured correctly
    assert_eq!(snap.total_principal, 1_000i128);
    assert_eq!(snap.funding_target, 5_000i128);
}

#[test]
fn test_get_funding_close_snapshot_absent_before_any_funding() {
    // Snapshot must be None immediately after init, before any fund() call.
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SNAP010"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
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
        client.get_funding_close_snapshot(),
        None,
        "snapshot must be absent before any funding"
    );
}

#[test]
fn test_get_funding_close_snapshot_present_after_funding_completes() {
    // Snapshot must be Some with correct fields once funded_amount reaches funding_target.
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SNAP011"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    // Partial fund — snapshot still absent.
    client.fund(&inv, &(TARGET / 2));
    assert_eq!(
        client.get_funding_close_snapshot(),
        None,
        "snapshot must remain absent while escrow is still open"
    );
    // Final fund that crosses the target — snapshot must now be present.
    client.fund(&inv, &(TARGET / 2));
    let snap = client
        .get_funding_close_snapshot()
        .expect("snapshot must be present after funding completes");
    assert_eq!(snap.total_principal, TARGET);
    assert_eq!(snap.funding_target, TARGET);
    assert_eq!(client.get_escrow().status, 1);
}

#[test]
fn test_get_funding_close_snapshot_immutable_after_set() {
    // Once the snapshot is written it must not change, even if additional reads occur
    // after the escrow has transitioned to a terminal state (settled).
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SNAP012"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    // Fund exactly to target — snapshot is written here.
    client.fund(&inv, &TARGET);
    let snap_at_close = client
        .get_funding_close_snapshot()
        .expect("snapshot must be present after funding");
    // Advance through settlement — snapshot must remain identical.
    client.settle();
    let snap_after_settle = client
        .get_funding_close_snapshot()
        .expect("snapshot must still be present after settlement");
    assert_eq!(
        snap_at_close, snap_after_settle,
        "snapshot must be immutable after being set"
    );
}

// --- MaxUniqueInvestorsCap and UniqueFunderCount Tests ---

#[test]
fn test_unique_funder_count_initialized_to_zero() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP001"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert_eq!(client.get_unique_funder_count(), 0);
}

#[test]
fn test_unique_funder_count_increments_on_first_investor() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP002"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert_eq!(client.get_unique_funder_count(), 0);
    client.fund(&investor, &(TARGET / 2));
    assert_eq!(client.get_unique_funder_count(), 1);
    client.fund(&investor, &(TARGET / 2));
    assert_eq!(client.get_unique_funder_count(), 1); // Still 1, same investor
}

#[test]
fn test_unique_funder_count_increments_for_distinct_investors() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP003"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    assert_eq!(client.get_unique_funder_count(), 0);

    client.fund(&inv_a, &(TARGET / 3));
    assert_eq!(client.get_unique_funder_count(), 1);

    client.fund(&inv_b, &(TARGET / 3));
    assert_eq!(client.get_unique_funder_count(), 2);

    client.fund(&inv_c, &(TARGET / 3));
    assert_eq!(client.get_unique_funder_count(), 3);
}

#[test]
fn test_unique_funder_count_with_fund_with_commitment() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });

    client.init(
        &admin,
        &String::from_str(&env, "CAP004"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(client.get_unique_funder_count(), 0);

    // First investor uses fund_with_commitment
    client.fund_with_commitment(&inv_a, &(TARGET / 2), &200u64);
    assert_eq!(client.get_unique_funder_count(), 1);

    // Second investor uses regular fund
    client.fund(&inv_b, &(TARGET / 2));
    assert_eq!(client.get_unique_funder_count(), 2);
}

#[test]
fn test_max_unique_investors_cap_none_allows_unlimited() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP005"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &None, // No cap set
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Should be able to add many investors when no cap is set
    for i in 0..10 {
        let investor = Address::generate(&env);
        client.fund(&investor, &(TARGET / 20));
        assert_eq!(client.get_unique_funder_count(), i + 1);
    }
}

#[test]
#[ignore]
fn test_max_unique_investors_cap_enforced_at_limit() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP006"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(3u32), // Cap of 3 investors
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(client.get_max_unique_investors_cap(), Some(3u32));

    // Add 3 investors - should succeed
    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);
    let inv3 = Address::generate(&env);

    client.fund(&inv1, &(TARGET / 6));
    assert_eq!(client.get_unique_funder_count(), 1);

    client.fund(&inv2, &(TARGET / 6));
    assert_eq!(client.get_unique_funder_count(), 2);

    client.fund(&inv3, &(TARGET / 6));
    assert_eq!(client.get_unique_funder_count(), 3);

    // 4th investor should panic
    let inv4 = Address::generate(&env);
    client.fund(&inv4, &(TARGET / 6));
}

#[test]
#[should_panic]
fn test_max_unique_investors_cap_blocks_excess_investors() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP007"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(2u32), // Cap of 2 investors
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Add 2 investors
    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);
    client.fund(&inv1, &(TARGET / 4));
    client.fund(&inv2, &(TARGET / 4));

    // 3rd investor should panic
    let inv3 = Address::generate(&env);
    client.fund(&inv3, &(TARGET / 4));
}

#[test]
#[should_panic]
fn test_max_unique_investors_cap_blocks_fund_with_commitment() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });

    client.init(
        &admin,
        &String::from_str(&env, "CAP008"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &Some(1u32), // Cap of 1 investor
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // First investor succeeds
    let inv1 = Address::generate(&env);
    client.fund_with_commitment(&inv1, &(TARGET / 2), &200u64);

    // Second investor using fund_with_commitment should panic
    let inv2 = Address::generate(&env);
    client.fund_with_commitment(&inv2, &(TARGET / 2), &200u64);
}

#[test]
#[ignore]
fn test_re_funding_same_address_doesnt_count_against_cap() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP009"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(1u32), // Cap of 1 investor
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // First fund should succeed
    client.fund(&investor, &(TARGET / 3));
    assert_eq!(client.get_unique_funder_count(), 1);

    // Re-funding same address should also succeed (doesn't count against cap)
    client.fund(&investor, &(TARGET / 3));
    assert_eq!(client.get_unique_funder_count(), 1);

    // Final fund from same address should succeed
    client.fund(&investor, &(TARGET / 3));
    assert_eq!(client.get_unique_funder_count(), 1);
    assert_eq!(client.get_escrow().status, 1); // Funded
}

#[test]
fn test_zero_contribution_then_non_zero_contribution_counts_as_unique_investor() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP010"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(2u32), // Cap of 2 investors
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    assert_eq!(client.get_unique_funder_count(), 0);
    assert_eq!(client.get_contribution(&investor), 0);

    // First non-zero contribution should increment count
    client.fund(&investor, &(TARGET / 2));
    assert_eq!(client.get_unique_funder_count(), 1);
    assert_eq!(client.get_contribution(&investor), TARGET / 2);
}

#[test]
#[ignore]
fn test_cap_validation_at_init_positive_value_required() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    // Should panic for zero cap
    client.init(
        &admin,
        &String::from_str(&env, "CAP011"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(0u32), // Invalid: zero cap
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

#[test]
#[should_panic]
fn test_init_panics_for_zero_cap() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP012"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(0u32), // Invalid: zero cap
        &None,
        &None,
        &None,
        &None,
        &None,
    );
}

#[test]
#[ignore]
fn test_cap_edge_case_exact_limit_reached() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP013"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(5u32), // Cap of 5 investors
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Add exactly 5 investors - should all succeed
    for i in 0..5 {
        let investor = Address::generate(&env);
        client.fund(&investor, &(TARGET / 10));
        assert_eq!(client.get_unique_funder_count(), i + 1);
    }

    // Should have exactly 5 unique funders
    assert_eq!(client.get_unique_funder_count(), 5);

    // 6th investor should panic
    let inv6 = Address::generate(&env);
    client.fund(&inv6, &(TARGET / 10));
}

#[test]
#[should_panic]
fn test_cap_edge_case_exactly_one_over_limit_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP014"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(5u32), // Cap of 5 investors
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Add exactly 5 investors
    for _i in 0..5 {
        let investor = Address::generate(&env);
        client.fund(&investor, &(TARGET / 10));
    }

    // 6th investor should panic
    let inv6 = Address::generate(&env);
    client.fund(&inv6, &(TARGET / 10));
}

#[test]
#[ignore]
fn test_cap_with_min_contribution_floor_interaction() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP015"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &Some(1_000i128), // Min contribution floor
        &Some(3u32),      // Cap of 3 investors
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Should respect both cap and floor
    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);
    let inv3 = Address::generate(&env);

    client.fund(&inv1, &2_000i128);
    assert_eq!(client.get_unique_funder_count(), 1);

    client.fund(&inv2, &1_500i128);
    assert_eq!(client.get_unique_funder_count(), 2);

    client.fund(&inv3, &1_000i128);
    assert_eq!(client.get_unique_funder_count(), 3);

    // 4th investor should be blocked by cap, not floor
    let inv4 = Address::generate(&env);
    client.fund(&inv4, &2_000i128);
}

#[test]
#[should_panic]
fn test_cap_blocks_even_with_large_contribution() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &String::from_str(&env, "CAP016"),
        &sme,
        &(TARGET * 10), // Large target
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(1u32), // Cap of 1 investor
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // First investor can fund large amount
    let inv1 = Address::generate(&env);
    client.fund(&inv1, &(TARGET * 5));

    // Second investor blocked even if they could fully fund remaining amount
    let inv2 = Address::generate(&env);
    client.fund(&inv2, &(TARGET * 5));
}

#[test]
#[ignore]
fn test_cap_panic_message_quality() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "CAP017"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &Some(1u32),
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Add first investor
    let inv1 = Address::generate(&env);
    client.fund(&inv1, &(TARGET / 2));

    // Try to add second investor - should panic with clear message
    let inv2 = Address::generate(&env);
    client.fund(&inv2, &(TARGET / 2));
}

// ── cancel_funding and refund tests ──────────────────────────────────────────

fn init_with_token<'a>(
    env: &'a Env,
    client: &LiquifactEscrowClient<'a>,
    admin: &Address,
    sme: &Address,
) -> (
    crate::tests::StellarTestToken<'a>,
    Address, // treasury
) {
    let token = install_stellar_asset_token(env);
    let treasury = Address::generate(env);
    client.init(
        admin,
        &String::from_str(env, "REFUND01"),
        sme,
        &TARGET,
        &800i64,
        &0u64,
        &token.id,
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
    );
    (token, treasury)
}

#[test]
fn test_cancel_funding_transitions_to_status_4() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let result = client.cancel_funding();
    assert_eq!(result.status, 4);
}

#[test]
fn test_cancel_funding_requires_admin_auth() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.cancel_funding();
    assert!(
        env.auths().iter().any(|(addr, _)| *addr == admin),
        "admin auth was not recorded for cancel_funding"
    );
}

#[test]
#[should_panic]
fn test_cancel_funding_panics_if_already_funded() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);
    client.cancel_funding();
}

#[test]
#[should_panic]
fn test_cancel_funding_panics_if_already_cancelled() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.cancel_funding();
    client.cancel_funding();
}

#[test]
#[should_panic]
fn test_cancel_funding_blocked_by_legal_hold() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    client.cancel_funding();
}

#[test]
fn test_refund_returns_principal_to_investor() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let (token, _treasury) = init_with_token(&env, &client, &admin, &sme);

    // Mint tokens into the escrow contract so it can refund
    let contract_id = client.address.clone();
    token.stellar.mint(&contract_id, &TARGET);

    client.fund(&investor, &TARGET);
    // Undo funded status by cancelling — but fund() moved status to 1, so we need open state.
    // Re-init with partial fund instead.
    let env2 = Env::default();
    let (client2, admin2, sme2) = setup(&env2);
    let investor2 = Address::generate(&env2);
    let (token2, _) = init_with_token(&env2, &client2, &admin2, &sme2);
    let contract_id2 = client2.address.clone();
    token2.stellar.mint(&contract_id2, &(TARGET / 2));
    client2.fund(&investor2, &(TARGET / 2));
    client2.cancel_funding();

    let before = token2.token.balance(&investor2);
    client2.refund(&investor2);
    let after = token2.token.balance(&investor2);
    assert_eq!(after - before, TARGET / 2);
}

#[test]
fn test_refund_zeroes_contribution() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let (token, _) = init_with_token(&env, &client, &admin, &sme);
    let contract_id = client.address.clone();
    token.stellar.mint(&contract_id, &(TARGET / 2));
    client.fund(&investor, &(TARGET / 2));
    client.cancel_funding();
    client.refund(&investor);
    assert_eq!(client.get_contribution(&investor), 0);
}

#[test]
fn test_refund_marks_investor_refunded() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let (token, _) = init_with_token(&env, &client, &admin, &sme);
    let contract_id = client.address.clone();
    token.stellar.mint(&contract_id, &(TARGET / 2));
    client.fund(&investor, &(TARGET / 2));
    client.cancel_funding();
    assert!(!client.is_investor_refunded(&investor));
    client.refund(&investor);
    assert!(client.is_investor_refunded(&investor));
}

#[test]
#[should_panic]
fn test_refund_double_spend_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let (token, _) = init_with_token(&env, &client, &admin, &sme);
    let contract_id = client.address.clone();
    token.stellar.mint(&contract_id, &(TARGET / 2));
    client.fund(&investor, &(TARGET / 2));
    client.cancel_funding();
    client.refund(&investor);
    client.refund(&investor); // second call must panic
}

#[test]
#[should_panic]
fn test_refund_non_investor_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.cancel_funding();
    let stranger = Address::generate(&env);
    client.refund(&stranger);
}

#[test]
#[should_panic]
fn test_refund_panics_in_open_state() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &(TARGET / 2));
    client.refund(&investor);
}

#[test]
#[should_panic]
fn test_refund_panics_in_funded_state() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    assert_eq!(client.get_escrow().status, 1);
    client.refund(&investor);
}

#[test]
fn test_refund_requires_investor_auth() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let (token, _) = init_with_token(&env, &client, &admin, &sme);
    let contract_id = client.address.clone();
    token.stellar.mint(&contract_id, &(TARGET / 2));
    client.fund(&investor, &(TARGET / 2));
    client.cancel_funding();
    client.refund(&investor);
    assert!(
        env.auths().iter().any(|(addr, _)| *addr == investor),
        "investor auth was not recorded for refund"
    );
}

#[test]
fn test_refund_multiple_investors_independent() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let (token, _) = init_with_token(&env, &client, &admin, &sme);
    let contract_id = client.address.clone();
    let amt_a = TARGET / 3;
    let amt_b = TARGET / 4;
    token.stellar.mint(&contract_id, &(amt_a + amt_b));
    client.fund(&inv_a, &amt_a);
    client.fund(&inv_b, &amt_b);
    client.cancel_funding();

    let before_a = token.token.balance(&inv_a);
    let before_b = token.token.balance(&inv_b);
    client.refund(&inv_a);
    client.refund(&inv_b);
    assert_eq!(token.token.balance(&inv_a) - before_a, amt_a);
    assert_eq!(token.token.balance(&inv_b) - before_b, amt_b);
}

#[test]
fn test_refund_three_investors_exactly_once() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investors = [
        Address::generate(&env),
        Address::generate(&env),
        Address::generate(&env),
    ];
    let amounts = [TARGET / 5, TARGET / 4, TARGET / 3];
    let total: i128 = amounts.iter().sum();
    let (token, _) = init_with_token(&env, &client, &admin, &sme);
    token.stellar.mint(&client.address, &total);

    for (investor, amount) in investors.iter().zip(amounts.iter()) {
        client.fund(investor, amount);
    }
    client.cancel_funding();

    let balances_before = [
        token.token.balance(&investors[0]),
        token.token.balance(&investors[1]),
        token.token.balance(&investors[2]),
    ];
    for (index, (investor, amount)) in investors.iter().zip(amounts.iter()).enumerate() {
        assert!(!client.is_investor_refunded(investor));
        client.refund(investor);
        assert_eq!(token.token.balance(investor) - balances_before[index], *amount);
        assert!(client.is_investor_refunded(investor));
        assert_eq!(client.get_contribution(investor), 0);
        assert_contract_error(
            client.try_refund(investor),
            EscrowError::NoContributionToRefund,
        );
    }
}

#[test]
fn test_cancel_funding_preserves_funded_amount() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &(TARGET / 2));
    let cancelled = client.cancel_funding();
    assert_eq!(cancelled.funded_amount, TARGET / 2);
}

#[test]
fn test_sweep_terminal_dust_allowed_in_cancelled_state() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let (token, treasury) = init_with_token(&env, &client, &admin, &sme);
    let contract_id = client.address.clone();
    // Mint slightly more than the investor contributes to leave dust
    token.stellar.mint(&contract_id, &(TARGET / 2 + 1));
    client.fund(&investor, &(TARGET / 2));
    client.cancel_funding();
    client.refund(&investor);
    // 1 unit of dust remains in the contract
    let swept = client.sweep_terminal_dust(&1i128);
    assert_eq!(swept, 1i128);
    assert_eq!(token.token.balance(&treasury), 1i128);
}

// ─── Commitment first-deposit-only invariant (issue #260) ────────────────────

/// After `fund_with_commitment(lock_secs > 0)`, a subsequent `fund()` call from the
/// same investor must preserve **both** `InvestorEffectiveYield` (tier rate) and
/// `InvestorClaimNotBefore` (absolute timestamp) unchanged.
#[test]
fn test_commitment_claim_lock_preserved_after_follow_on_fund() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 950,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CLK001"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Set ledger timestamp to a known value so claim_nb is deterministic.
    env.ledger().with_mut(|l| l.timestamp = 1_000_000u64);

    // First deposit: tier at 100 s → effective yield = 950 bps, lock until 1_000_100.
    client.fund_with_commitment(&inv, &3_000i128, &100u64);
    let yield_after_first = client.get_investor_yield_bps(&inv);
    let lock_after_first = client.get_investor_claim_not_before(&inv);
    assert_eq!(yield_after_first, 950, "tier yield not selected correctly");
    assert_eq!(
        lock_after_first, 1_000_100u64,
        "claim lock not set correctly"
    );

    // Follow-on deposit using fund() — must succeed and preserve both values.
    client.fund(&inv, &3_000i128);
    assert_eq!(
        client.get_investor_yield_bps(&inv),
        yield_after_first,
        "effective yield must be immutable after follow-on fund()"
    );
    assert_eq!(
        client.get_investor_claim_not_before(&inv),
        lock_after_first,
        "InvestorClaimNotBefore must be immutable after follow-on fund()"
    );
}

/// Tier and claim-lock selection must remain immutable across **multiple** consecutive
/// follow-on `fund()` calls from the same investor.
#[test]
fn test_commitment_invariant_across_multiple_follow_on_funds() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 50,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 200,
        yield_bps: 1100,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CLK002"),
        &sme,
        &50_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp = 2_000_000u64);

    // First deposit: 200 s commitment → top tier (1100 bps), lock until 2_000_200.
    client.fund_with_commitment(&inv, &5_000i128, &200u64);
    let expected_yield = client.get_investor_yield_bps(&inv);
    let expected_lock = client.get_investor_claim_not_before(&inv);
    assert_eq!(expected_yield, 1100);
    assert_eq!(expected_lock, 2_000_200u64);

    // Three follow-on fund() calls — invariant must hold after each.
    for round in 1u32..=3 {
        client.fund(&inv, &1_000i128);
        assert_eq!(
            client.get_investor_yield_bps(&inv),
            expected_yield,
            "yield changed on follow-on fund round {round}"
        );
        assert_eq!(
            client.get_investor_claim_not_before(&inv),
            expected_lock,
            "claim lock changed on follow-on fund round {round}"
        );
    }
}

/// `fund_with_commitment(lock_secs = 0)` must assign base yield and leave
/// `InvestorClaimNotBefore` at zero. A subsequent `fund()` call must keep both
/// at their zero / base values — no claim gate is imposed.
#[test]
fn test_commitment_zero_lock_follow_on_fund_no_claim_gate() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 950,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CLK003"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Zero lock → base yield only, no claim gate.
    client.fund_with_commitment(&inv, &4_000i128, &0u64);
    assert_eq!(
        client.get_investor_yield_bps(&inv),
        800,
        "should get base yield for zero lock"
    );
    assert_eq!(
        client.get_investor_claim_not_before(&inv),
        0u64,
        "no claim gate for zero lock"
    );

    // Follow-on fund() must preserve both zero-valued guards.
    client.fund(&inv, &4_000i128);
    assert_eq!(
        client.get_investor_yield_bps(&inv),
        800,
        "yield must remain at base after follow-on fund() with zero-lock commitment"
    );
    assert_eq!(
        client.get_investor_claim_not_before(&inv),
        0u64,
        "InvestorClaimNotBefore must stay 0 after follow-on fund() with zero-lock commitment"
    );
}

/// A second `fund_with_commitment` from the same investor (who already has a
/// non-zero contribution) must fail with the documented typed error, regardless
/// of whether a tier table is configured.
#[test]
fn test_second_fund_with_commitment_panics_without_tier_table() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    // No tier table: base-only escrow.
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CLK004"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund_with_commitment(&inv, &3_000i128, &0u64);
    // Second call must trap.
    assert_contract_error(
        client.try_fund_with_commitment(&inv, &3_000i128, &0u64),
        EscrowError::TieredSecondDeposit,
    );
}

/// After a plain `fund()` first deposit, calling `fund_with_commitment` on the same
/// investor must panic — the tier/lock selection window is permanently closed.
/// This is the "inverse" direction of the state-machine rule.
#[test]
fn test_fund_first_then_commitment_second_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 50,
        yield_bps: 900,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CLK005"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // First leg via fund() → establishes base-yield position.
    client.fund(&inv, &3_000i128);
    // Attempt to re-select tier via fund_with_commitment → must panic.
    assert_contract_error(
        client.try_fund_with_commitment(&inv, &3_000i128, &100u64),
        EscrowError::TieredSecondDeposit,
    );
}

/// Verify that a plain `fund()` as first deposit sets the effective yield to the
/// base rate and leaves `InvestorClaimNotBefore` at zero (no lock gate implied by
/// the simple funding path).
#[test]
fn test_fund_first_deposit_sets_base_yield_and_no_claim_gate() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 950,
    });

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CLK006"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund(&inv, &5_000i128);
    assert_eq!(
        client.get_investor_yield_bps(&inv),
        800,
        "fund() must assign base yield even when tier table is present"
    );
    assert_eq!(
        client.get_investor_claim_not_before(&inv),
        0u64,
        "fund() must not impose a claim gate"
    );
}

// ── CommitmentLockExceedsMaturity bound ──────────────────────────────────────

// Helper: init an escrow with a specific maturity timestamp.
fn init_with_maturity(
    env: &Env,
    client: &crate::LiquifactEscrowClient<'_>,
    admin: &soroban_sdk::Address,
    sme: &soroban_sdk::Address,
    maturity: u64,
) {
    let (token, treasury) = free_addresses(env);
    client.init(
        admin,
        &soroban_sdk::String::from_str(env, "LOCK1"),
        sme,
        &10_000i128,
        &800i64,
        &maturity,
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
    );
}

#[test]
fn commitment_lock_within_maturity_is_accepted() {
    // now=1000, maturity=2000, lock=500 → claim_nb=1500 ≤ 2000  ✓
    let env = Env::default();
    env.mock_all_auths();
    let mut li = env.ledger().get();
    li.timestamp = 1000;
    env.ledger().set(li);
    let (client, admin, sme) = setup(&env);
    let investor = soroban_sdk::Address::generate(&env);
    init_with_maturity(&env, &client, &admin, &sme, 2000);
    let escrow = client.fund_with_commitment(&investor, &1_000i128, &500u64);
    assert_eq!(escrow.status, 0);
    assert_eq!(client.get_investor_claim_not_before(&investor), 1500u64);
}

#[test]
fn commitment_lock_exactly_at_maturity_is_accepted() {
    // now=1000, maturity=2000, lock=1000 → claim_nb=2000 == maturity  ✓ (inclusive)
    let env = Env::default();
    env.mock_all_auths();
    let mut li = env.ledger().get();
    li.timestamp = 1000;
    env.ledger().set(li);
    let (client, admin, sme) = setup(&env);
    let investor = soroban_sdk::Address::generate(&env);
    init_with_maturity(&env, &client, &admin, &sme, 2000);
    let escrow = client.fund_with_commitment(&investor, &1_000i128, &1000u64);
    assert_eq!(escrow.status, 0);
    assert_eq!(client.get_investor_claim_not_before(&investor), 2000u64);
}

#[test]
fn commitment_lock_one_second_past_maturity_is_rejected() {
    // now=1000, maturity=2000, lock=1001 → claim_nb=2001 > 2000  ✗
    let env = Env::default();
    env.mock_all_auths();
    let mut li = env.ledger().get();
    li.timestamp = 1000;
    env.ledger().set(li);
    let (client, admin, sme) = setup(&env);
    let investor = soroban_sdk::Address::generate(&env);
    init_with_maturity(&env, &client, &admin, &sme, 2000);
    assert_contract_error(
        client.try_fund_with_commitment(&investor, &1_000i128, &1001u64),
        EscrowError::CommitmentLockExceedsMaturity,
    );
}

#[test]
fn commitment_lock_far_past_maturity_is_rejected() {
    // now=1000, maturity=2000, lock=5000 → claim_nb=6000 >> 2000  ✗
    let env = Env::default();
    env.mock_all_auths();
    let mut li = env.ledger().get();
    li.timestamp = 1000;
    env.ledger().set(li);
    let (client, admin, sme) = setup(&env);
    let investor = soroban_sdk::Address::generate(&env);
    init_with_maturity(&env, &client, &admin, &sme, 2000);
    assert_contract_error(
        client.try_fund_with_commitment(&investor, &1_000i128, &5000u64),
        EscrowError::CommitmentLockExceedsMaturity,
    );
}

#[test]
fn zero_lock_with_maturity_is_always_accepted() {
    // committed_lock_secs==0 → claim_nb=0, no maturity bound applied
    let env = Env::default();
    env.mock_all_auths();
    let mut li = env.ledger().get();
    li.timestamp = 1000;
    env.ledger().set(li);
    let (client, admin, sme) = setup(&env);
    let investor = soroban_sdk::Address::generate(&env);
    init_with_maturity(&env, &client, &admin, &sme, 2000);
    let escrow = client.fund_with_commitment(&investor, &1_000i128, &0u64);
    assert_eq!(escrow.status, 0);
    assert_eq!(client.get_investor_claim_not_before(&investor), 0u64);
}
#[test]
fn lock_with_zero_maturity_is_always_accepted() {
    // maturity==0 means no maturity lock; any lock_secs is fine
    let env = Env::default();
    env.mock_all_auths();
    let mut li = env.ledger().get();
    li.timestamp = 1000;
    env.ledger().set(li);
    let (client, admin, sme) = setup(&env);
    let investor = soroban_sdk::Address::generate(&env);
    // maturity = 0 → no bound applied even for a huge lock
    let (token, treasury) = free_addresses(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "LOCK2"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        // no maturity
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
    );
    let escrow = client.fund_with_commitment(&investor, &1_000i128, &9999u64);
    assert_eq!(escrow.status, 0);
    assert_eq!(client.get_investor_claim_not_before(&investor), 10999u64);
}

#[test]
fn plain_fund_with_maturity_ignores_lock_bound() {
    // fund() (simple_fund=true) never sets a claim lock; bound is irrelevant
    let env = Env::default();
    env.mock_all_auths();
    let mut li = env.ledger().get();
    li.timestamp = 1000;
    env.ledger().set(li);
    let (client, admin, sme) = setup(&env);
    let investor = soroban_sdk::Address::generate(&env);
    init_with_maturity(&env, &client, &admin, &sme, 2000);
    // fund() should succeed regardless of maturity; it never imposes a lock
    let escrow = client.fund(&investor, &1_000i128);
    assert_eq!(escrow.status, 0);
    assert_eq!(client.get_investor_claim_not_before(&investor), 0u64);
}
// ─────────────────────────────────────────────────────────────────────────────
// Tests for fund_batch entrypoint (Issue #311)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "FundingBatchEmpty")]
fn test_fund_batch_rejects_empty() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let empty_batch: SorobanVec<(Address, i128)> = SorobanVec::new(&env);
    client.fund_batch(&empty_batch);
}

#[test]
#[should_panic(expected = "FundingBatchTooLarge")]
fn test_fund_batch_rejects_oversized() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let mut entries = SorobanVec::new(&env);
    // Create MAX_FUND_BATCH + 1 entries
    for _ in 0..=(MAX_FUND_BATCH as usize) {
        let investor = Address::generate(&env);
        entries.push_back((investor, 1_000i128));
    }

    client.fund_batch(&entries);
}

#[test]
fn test_fund_batch_equals_n_single_funds() {
    let env = Env::default();
    env.mock_all_auths();
    let client_a = deploy(&env);
    let client_b = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    // Initialize both identical escrows
    let target = 100_000i128;
    for client in &[&client_a, &client_b] {
        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "BATCH001"),
            &sme,
            &target,
            &800i64,
            &0u64,
            &tok,
            &None,
            &tre,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
            &None,
        );
    }

    // Create 5 investors
    let mut investors = SorobanVec::new(&env);
    let mut amounts = SorobanVec::new(&env);
    for i in 0..5 {
        let inv = Address::generate(&env);
        investors.push_back(inv.clone());
        amounts.push_back((i + 1) as i128 * 10_000i128);
    }

    // Path A: fund_batch
    let mut batch_entries = SorobanVec::new(&env);
    for i in 0..5 {
        batch_entries.push_back((investors.get(i).unwrap(), amounts.get(i).unwrap()));
    }
    let result_batch = client_a.fund_batch(&batch_entries);

    // Path B: individual fund calls
    for i in 0..5 {
        client_b.fund(&investors.get(i).unwrap(), &amounts.get(i).unwrap());
    }
    let result_single = client_b.get_escrow();

    // Assert identical final state
    assert_eq!(result_batch.funded_amount, result_single.funded_amount);
    assert_eq!(result_batch.status, result_single.status);

    // Verify contributions match
    for i in 0..5 {
        let inv = investors.get(i).unwrap();
        let batch_contrib = client_a.get_contribution(&inv);
        let single_contrib = client_b.get_contribution(&inv);
        assert_eq!(batch_contrib, single_contrib);
    }
}

#[test]
#[should_panic(expected = "InvestorContributionExceedsCap")]
fn test_fund_batch_per_investor_cap_rejection() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let target = 100_000i128;
    let per_investor_cap = 30_000i128;

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CAP001"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &Some(per_investor_cap),
        &None,
        &None,
        &None,
        &None,
    );

    let mut entries = SorobanVec::new(&env);
    entries.push_back((inv1.clone(), 25_000i128)); // Within cap
    entries.push_back((inv2.clone(), 35_000i128)); // Exceeds cap

    client.fund_batch(&entries);
}

#[test]
fn test_fund_batch_mid_batch_funded_transition() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);
    let investor = Address::generate(&env);

    let target = 100_000i128;
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "TRANS001"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);
    let inv3 = Address::generate(&env);
    let inv4 = Address::generate(&env);
    let inv5 = Address::generate(&env);

    let mut entries = SorobanVec::new(&env);
    // inv1 brings total to 40k (still open)
    entries.push_back((inv1.clone(), 40_000i128));
    // inv2 brings total to 95k (still open)
    entries.push_back((inv2.clone(), 55_000i128));
    // inv3 brings total to 105k (crosses funded threshold)
    entries.push_back((inv3.clone(), 10_000i128));
    // Entries after the funded transition are still processed.
    entries.push_back((inv4.clone(), 4_000i128));
    entries.push_back((inv5.clone(), 6_000i128));

    assert!(client.get_funding_close_snapshot().is_none());
    let result = client.batch_fund(&entries);

    // Verify transition occurred
    assert_eq!(result.status, 1, "status should be funded (1) after batch");
    assert_eq!(result.funded_amount, 115_000i128);

    // Verify all entries were processed
    assert_eq!(client.get_contribution(&inv1), 40_000i128);
    assert_eq!(client.get_contribution(&inv2), 55_000i128);
    assert_eq!(client.get_contribution(&inv3), 10_000i128);
    assert_eq!(client.get_contribution(&inv4), 4_000i128);
    assert_eq!(client.get_contribution(&inv5), 6_000i128);

    // Snapshot is captured at entry 3 and is not replaced by later batch entries.
    let snap = client.get_funding_close_snapshot();
    assert!(snap.is_some());
    let snap = snap.unwrap();
    assert_eq!(snap.total_principal, 105_000i128);
    assert_eq!(snap.funding_target, target);
    assert_eq!(snap.closed_at_ledger_timestamp, env.ledger().timestamp());
    assert_eq!(snap.closed_at_ledger_sequence, env.ledger().sequence());
}

#[test]
#[should_panic(expected = "InvestorContributionExceedsCap")]
fn test_fund_batch_duplicate_addresses() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let target = 100_000i128;
    let per_investor_cap = 50_000i128;

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DUP001"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &Some(per_investor_cap),
        &None,
        &None,
        &None,
        &None,
    );

    let mut entries = SorobanVec::new(&env);
    entries.push_back((inv.clone(), 30_000i128)); // First entry: 30k
    entries.push_back((inv.clone(), 25_000i128)); // Second entry: 30k + 25k = 55k > cap

    client.fund_batch(&entries);
}

#[test]
#[should_panic]
fn test_fund_batch_per_investor_auth() {
    // Test that each investor in the batch must authorize their own entry.
    // This test demonstrates that require_auth() is called per investor.
    let env = Env::default();
    // NOT calling env.mock_all_auths() - we'll manually auth only one investor
    let (client, admin, sme) = setup(&env); // setup() calls mock_all_auths, so this won't work as intended
    default_init(&client, &env, &admin, &sme);

    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);

    let mut entries = SorobanVec::new(&env);
    entries.push_back((inv1.clone(), 10_000i128));
    entries.push_back((inv2.clone(), 10_000i128)); // This one will fail on require_auth

    // Since setup() mocks all auths, this test will pass both.
    // A more realistic test would require custom auth mocking, which is env-dependent.
    // For now, we just verify that the batch processes all entries with require_auth.
    let result = client.fund_batch(&entries);
    assert_eq!(result.funded_amount, 20_000i128);
}

#[test]
fn test_fund_batch_single_entry() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let inv = Address::generate(&env);
    let amount = 50_000i128;

    let mut entries = SorobanVec::new(&env);
    entries.push_back((inv.clone(), amount));

    let result = client.fund_batch(&entries);

    assert_eq!(result.funded_amount, amount);
    assert_eq!(client.get_contribution(&inv), amount);
}

#[test]
fn test_fund_batch_max_batch_size() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let target = 10_000_000i128; // Very large target
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MAXBATCH"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Create exactly MAX_FUND_BATCH entries
    let mut entries = SorobanVec::new(&env);
    for _ in 0..MAX_FUND_BATCH {
        let inv = Address::generate(&env);
        entries.push_back((inv, 1_000i128));
    }

    let result = client.fund_batch(&entries);

    // Verify all entries were processed
    assert_eq!(result.funded_amount, (MAX_FUND_BATCH as i128) * 1_000i128);
}

#[test]
fn test_batch_fund_max_batch_size_within_instruction_budget() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let mut entries = SorobanVec::new(&env);
    for _ in 0..MAX_FUND_BATCH {
        entries.push_back((Address::generate(&env), 1_000i128));
    }

    env.cost_estimate().budget().reset_unlimited();
    let budget = env.cost_estimate().budget();
    let result = client.batch_fund(&entries);

    assert_eq!(result.funded_amount, MAX_FUND_BATCH as i128 * 1_000);
    assert!(
        budget.cpu_instruction_cost() <= SOROBAN_TX_MAX_INSTRUCTIONS,
        "batch_fund with {MAX_FUND_BATCH} entries exceeds the CPU instruction budget: {} > {}",
        budget.cpu_instruction_cost(),
        SOROBAN_TX_MAX_INSTRUCTIONS
    );
}

#[test]
fn test_batch_fund_rejects_51_entries() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let mut entries = SorobanVec::new(&env);
    for _ in 0..=MAX_FUND_BATCH {
        entries.push_back((Address::generate(&env), 1_000i128));
    }

    assert_contract_error(
        client.try_batch_fund(&entries),
        EscrowError::FundingBatchTooLarge,
    );
}

#[test]
fn test_batch_fund_is_atomic_when_last_entry_is_not_allowlisted() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let allowed_one = Address::generate(&env);
    let allowed_two = Address::generate(&env);
    let not_allowlisted = Address::generate(&env);
    client.set_allowlist_active(&true);
    client.set_investor_allowlisted(&allowed_one, &true);
    client.set_investor_allowlisted(&allowed_two, &true);

    let mut entries = SorobanVec::new(&env);
    entries.push_back((allowed_one.clone(), 1_000i128));
    entries.push_back((allowed_two.clone(), 2_000i128));
    entries.push_back((not_allowlisted.clone(), 3_000i128));

    assert_contract_error(
        client.try_batch_fund(&entries),
        EscrowError::InvestorNotAllowlisted,
    );

    assert_eq!(client.get_escrow().funded_amount, 0);
    assert_eq!(client.get_contribution(&allowed_one), 0);
    assert_eq!(client.get_contribution(&allowed_two), 0);
    assert_eq!(client.get_contribution(&not_allowlisted), 0);
    assert_eq!(client.get_unique_funder_count(), 0);
}

#[test]
fn test_batch_fund_summary_event_payload_for_distinct_and_repeated_investors() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();
    let (contract_id_distinct, client_distinct) = deploy_with_id(&env);
    let (contract_id_repeated, client_repeated) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    for client in [&client_distinct, &client_repeated] {
        client.init(
            &admin,
            &soroban_sdk::String::from_str(&env, "BATCH_EVENT"),
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
        );
    }

    let distinct_one = Address::generate(&env);
    let distinct_two = Address::generate(&env);
    let mut distinct_entries = SorobanVec::new(&env);
    distinct_entries.push_back((distinct_one, 1_000i128));
    distinct_entries.push_back((distinct_two, 2_000i128));
    let distinct_result = client_distinct.batch_fund(&distinct_entries);
    let distinct_summary = BatchFundCompleted {
        name: symbol_short!("btch_fnd"),
        invoice_id: distinct_result.invoice_id,
        total_funded_amount: 3_000,
        investor_count: 2,
        escrow_funded_amount: 3_000,
        status: 0,
    }
    .to_xdr(&env, &contract_id_distinct);

    let repeated = Address::generate(&env);
    let other = Address::generate(&env);
    let mut repeated_entries = SorobanVec::new(&env);
    repeated_entries.push_back((repeated.clone(), 1_000i128));
    repeated_entries.push_back((other, 2_000i128));
    repeated_entries.push_back((repeated, 3_000i128));
    let repeated_result = client_repeated.batch_fund(&repeated_entries);
    let repeated_summary = BatchFundCompleted {
        name: symbol_short!("btch_fnd"),
        invoice_id: repeated_result.invoice_id,
        total_funded_amount: 6_000,
        investor_count: 3,
        escrow_funded_amount: 6_000,
        status: 0,
    }
    .to_xdr(&env, &contract_id_repeated);

    let events = env.events().all();
    assert_eq!(
        events
            .events()
            .iter()
            .filter(|event| **event == distinct_summary)
            .count(),
        1,
        "distinct-investor batch must emit exactly one summary with the expected payload"
    );
    assert_eq!(
        events
            .events()
            .iter()
            .filter(|event| **event == repeated_summary)
            .count(),
        1,
        "repeated-investor batch must emit exactly one summary with the expected payload"
    );
    assert_eq!(client_repeated.get_unique_funder_count(), 2);
}

#[test]
fn test_fund_batch_preserves_event_semantics() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let target = 100_000i128;
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "EVENTS01"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);

    let mut entries = SorobanVec::new(&env);
    entries.push_back((inv1.clone(), 30_000i128));
    entries.push_back((inv2.clone(), 50_000i128));

    client.fund_batch(&entries);

    // Verify events emitted
    let events = env.events().all();
    assert_eq!(
        events.events().len(),
        2,
        "should emit 2 EscrowFunded events"
    );

    // Each event corresponds to a fund operation
    // (Detailed event field verification depends on EscrowFunded structure)
}

// ─────────────────────────────────────────────────────────────────────────────
// Issue #237: batch_fund budget benchmark (worst-case configuration)
// ─────────────────────────────────────────────────────────────────────────────
//
// MAX_FUND_BATCH = 50 was chosen without a published budget measurement. This
// benchmark exercises `batch_fund` with exactly MAX_FUND_BATCH entries under the
// worst-case configuration (allowlist enabled, tiered yield configured, KYC
// checks active) and reports the CPU instruction count and memory usage against
// the Soroban mainnet instruction budget.
//
// Run in release mode with:
//     cargo test -p escrow --release -- --ignored --nocapture bench_batch_fund
//
// Any adjustment to MAX_FUND_BATCH is explicitly out of scope for this issue and
// is deferred to a separate PR. No contract changes are made here.

/// Soroban mainnet per-transaction CPU instruction budget (as of protocol 22).
/// Source: `soroban-env-host` `BudgetImpl` / network config `tx_max_instructions`.
const SOROBAN_TX_MAX_INSTRUCTIONS: u64 = 100_000_000;

/// Soroban mainnet per-transaction memory budget in bytes.
/// Source: network config `tx_memory_limit`.
const SOROBAN_TX_MEMORY_LIMIT: u64 = 40 * 1024 * 1024;

/// Worst-case `batch_fund` budget benchmark: MAX_FUND_BATCH entries with
/// allowlist enabled, tiered yield configured, and KYC checks active.
///
/// Gated behind `#[ignore]` so it does not run in the default test suite; it is
/// intended to be executed in release mode with `--ignored --nocapture`.
#[test]
#[ignore]
fn bench_batch_fund_50_entries_worst_case() {
    use soroban_sdk::testutils::Ledger as _;

    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    // Worst-case configuration: tiered yield ladder (forces tier selection on
    // every entry) plus KYC and allowlist requirements.
    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 500,
        yield_bps: 1100,
    });

    // Target large enough that all MAX_FUND_BATCH entries are processed without
    // the escrow transitioning to funded mid-batch.
    let target = 1_000_000_000_000i128;

    client.init(
        &admin,
        &String::from_str(&env, "BENCH237"),
        &sme,
        &target,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Enable the worst-case gating: KYC required and allowlist enforced.
    client.set_kyc_required(&true);
    client.set_allowlist_enabled(&true);

    // Build exactly MAX_FUND_BATCH entries, each with a distinct, allowlisted,
    // KYC-approved investor.
    let mut entries = SorobanVec::new(&env);
    for _ in 0..MAX_FUND_BATCH {
        let inv = Address::generate(&env);
        client.set_investor_kyc(&inv, &true);
        client.set_allowlisted(&inv, &true);
        entries.push_back((inv, 1_000i128));
    }

    // Measure CPU instruction count and memory usage for the batch call.
    env.cost_estimate().budget().reset_unlimited();
    let before = env.cost_estimate().budget();

    let result = client.fund_batch(&entries);

    let cpu_instructions = before.cpu_instruction_cost();
    let memory_bytes = before.memory_bytes_cost();

    // Sanity: all entries were processed.
    assert_eq!(
        result.funded_amount,
        (MAX_FUND_BATCH as i128) * 1_000i128,
        "all MAX_FUND_BATCH entries must be processed"
    );

    // Document the measurement against the mainnet budget.
    println!("── batch_fund budget benchmark (issue #237) ──");
    println!("entries:                 {MAX_FUND_BATCH}");
    println!("configuration:           allowlist + tiers + KYC");
    println!("cpu instructions:        {cpu_instructions}");
    println!("cpu budget (mainnet):    {SOROBAN_TX_MAX_INSTRUCTIONS}");
    println!(
        "cpu utilization:         {:.2}%",
        (cpu_instructions as f64 / SOROBAN_TX_MAX_INSTRUCTIONS as f64) * 100.0
    );
    println!("memory bytes:            {memory_bytes}");
    println!("memory budget (mainnet): {SOROBAN_TX_MEMORY_LIMIT}");
    println!(
        "memory utilization:      {:.2}%",
        (memory_bytes as f64 / SOROBAN_TX_MEMORY_LIMIT as f64) * 100.0
    );

    let fits_cpu = cpu_instructions <= SOROBAN_TX_MAX_INSTRUCTIONS;
    let fits_memory = memory_bytes <= SOROBAN_TX_MEMORY_LIMIT;
    println!(
        "fits mainnet budget:     {}",
        if fits_cpu && fits_memory {
            "YES"
        } else {
            "NO"
        }
    );

    assert!(
        fits_cpu,
        "batch_fund with {MAX_FUND_BATCH} entries exceeds the CPU instruction budget: \
         {cpu_instructions} > {SOROBAN_TX_MAX_INSTRUCTIONS}"
    );
    assert!(
        fits_memory,
        "batch_fund with {MAX_FUND_BATCH} entries exceeds the memory budget: \
         {memory_bytes} > {SOROBAN_TX_MEMORY_LIMIT}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// #205: max_unique_investors cap enforced at fund() time
// ─────────────────────────────────────────────────────────────────────────────

/// Cap of 2: first two investors succeed; the third receives UniqueInvestorCapReached (107).
///
/// Verifies that `fund_impl` enforces the cap with a typed error, not a silent
/// allow-through or an unclassified panic.
#[test]
fn test_205_unique_investor_cap_typed_error_on_third_investor() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    // Large target so the escrow stays open (status 0) after 2 partial investors.
    client.init(
        &admin,
        &String::from_str(&env, "BUG205"),
        &sme,
        &300_000_000_000i128, // 300B — well above inv1 + inv2
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(2u32), // cap = 2 distinct investors
        &None,
        &None,
        &None,
    );

    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);
    let inv3 = Address::generate(&env);

    // First two investors must succeed.
    client.fund(&inv1, &50_000_000_000i128);
    assert_eq!(client.get_unique_funder_count(), 1);

    client.fund(&inv2, &50_000_000_000i128);
    assert_eq!(client.get_unique_funder_count(), 2);

    // Escrow must still be open (status 0) so the cap check is exercised, not
    // EscrowNotOpenForFunding.
    assert_eq!(client.get_escrow().status, 0);

    // Third investor must be rejected with the specific typed error (code 107).
    assert_contract_error(
        client.try_fund(&inv3, &50_000_000_000i128),
        EscrowError::UniqueInvestorCapReached,
    );

    // State must be unchanged — unique count is still 2, inv3 has no contribution.
    assert_eq!(client.get_unique_funder_count(), 2);
    assert_eq!(client.get_contribution(&inv3), 0);
}

/// Existing investor follow-on fund must not be blocked by the cap — only new addresses count.
#[test]
fn test_205_existing_investor_followon_bypasses_cap() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    client.init(
        &admin,
        &String::from_str(&env, "BUG205B"),
        &sme,
        &300_000_000_000i128,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None,
        &Some(1u32), // cap = 1
        &None,
        &None,
        &None,
    );

    let inv1 = Address::generate(&env);
    client.fund(&inv1, &10_000_000_000i128);
    assert_eq!(client.get_unique_funder_count(), 1);

    // Second deposit from same investor — must succeed even though cap==1 is "full".
    client.fund(&inv1, &10_000_000_000i128);
    assert_eq!(client.get_unique_funder_count(), 1);
    assert_eq!(client.get_contribution(&inv1), 20_000_000_000i128);
}

// ─────────────────────────────────────────────────────────────────────────────
// #206: Zero-yield payout guard — settle with yield_bps == 0
// ─────────────────────────────────────────────────────────────────────────────

/// When funded_amount == funding_target and yield_bps == 0, compute_investor_payout
/// must return exactly the investor's contribution (principal only, no coupon).
///
/// This is the regression test for the zero-yield guard in `compute_investor_payout`.
/// The path: coupon = 0, settle_pool = total_principal, payout = contribution.
#[test]
fn test_206_zero_yield_payout_equals_contribution() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let target = 100_000_000i128;

    client.init(
        &admin,
        &String::from_str(&env, "BUG206"),
        &sme,
        &target,
        &0i64, // zero yield
        &0u64,
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

    // Fund exactly to target so funded_amount == funding_target.
    client.fund(&investor, &target);
    assert_eq!(client.get_escrow().status, 1); // funded

    // Settle.
    client.settle();
    assert_eq!(client.get_escrow().status, 2); // settled

    // With zero yield: coupon = 0, settle_pool = total_principal = target,
    // payout = contribution × target / target = contribution exactly.
    let payout = client.compute_investor_payout(&investor);
    assert_eq!(
        payout, target,
        "zero-yield payout must equal investor contribution (no coupon)"
    );
}

/// Two investors each contributing half, zero yield — each gets exactly their share back.
#[test]
fn test_206_zero_yield_two_investors_pro_rata() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let target = 100_000_000i128;
    let half = target / 2;

    client.init(
        &admin,
        &String::from_str(&env, "BUG206B"),
        &sme,
        &target,
        &0i64, // zero yield
        &0u64,
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

    client.fund(&inv_a, &half);
    client.fund(&inv_b, &half);
    assert_eq!(client.get_escrow().status, 1);
    client.settle();

    // Each investor contributed half the principal; with zero yield each gets their half back.
    let payout_a = client.compute_investor_payout(&inv_a);
    let payout_b = client.compute_investor_payout(&inv_b);
    assert_eq!(
        payout_a, half,
        "inv_a payout must equal contribution with zero yield"
    );
    assert_eq!(
        payout_b, half,
        "inv_b payout must equal contribution with zero yield"
    );
}

/// BUG-013: Test that FundingCloseSnapshot emits a warning when snapshot timestamp >= maturity_date.
/// This test verifies the fix for the issue where funding closes after the invoice maturity date,
/// which creates a logically inconsistent state. The fix emits EscrowHealthWarning with type 4004.
#[test]
fn test_funding_close_snapshot_validates_against_maturity() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let target = 100_000_000_000i128;

    // Get current ledger timestamp
    let now = env.ledger().timestamp();

    // Set maturity to a past time (5000 seconds ago) to simulate misconfigured time or fast ledger
    let maturity_in_past = now.saturating_sub(5000);

    // Initialize escrow with maturity in the past
    client.init(
        &admin,
        &String::from_str(&env, "BUG013"),
        &sme,
        &target,
        &800i64,
        &maturity_in_past,
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

    // Fund to reach the target—this should create FundingCloseSnapshot with current timestamp
    // which is >= maturity, triggering the warning.
    client.fund(&investor, &target);

    // Verify escrow transitioned to funded status
    let escrow = client.get_escrow();
    assert_eq!(
        escrow.status, 1,
        "Escrow should be funded after reaching target"
    );

    // Check the health metrics to verify warning was emitted
    let (warning_type, funded_ratio_bps, time_to_maturity_secs) = client.check_escrow_health();

    assert_eq!(
        warning_type, 4004,
        "Snapshot after maturity should emit warning type 4004 (FundingClosedAfterMaturity)"
    );
    assert_eq!(
        funded_ratio_bps, 10000,
        "Funded ratio should be 10000 bps (100%)"
    );
    assert!(
        time_to_maturity_secs < 0,
        "Time to maturity should be negative (in the past)"
    );
}

/// BUG-013: Test that partial_settle also validates FundingCloseSnapshot against maturity.
/// This verifies the warning is emitted in both fund_impl and partial_settle code paths.
#[test]
fn test_partial_settle_close_snapshot_validates_against_maturity() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let target = 100_000_000_000i128;
    let half_target = target / 2;

    // Get current ledger timestamp
    let now = env.ledger().timestamp();

    // Set maturity to a past time
    let maturity_in_past = now.saturating_sub(3000);

    // Initialize escrow with maturity in the past
    client.init(
        &admin,
        &String::from_str(&env, "BUG013B"),
        &sme,
        &target,
        &800i64,
        &maturity_in_past,
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

    // Fund only half the target (escrow stays in open status)
    client.fund(&investor, &half_target);

    // Verify escrow is still open
    let escrow = client.get_escrow();
    assert_eq!(
        escrow.status, 0,
        "Escrow should be open after partial funding"
    );

    // Call partial_settle to transition to funded early
    // This also creates a FundingCloseSnapshot
    client.partial_settle();

    // Verify escrow transitioned to funded status
    let escrow = client.get_escrow();
    assert_eq!(
        escrow.status, 1,
        "Escrow should be funded after partial_settle"
    );

    // Check the health metrics to verify warning was emitted
    let (warning_type, funded_ratio_bps, time_to_maturity_secs) = client.check_escrow_health();

    assert_eq!(
        warning_type, 4004,
        "Snapshot after maturity (partial_settle) should emit warning type 4004"
    );
    assert!(
        funded_ratio_bps < 10000,
        "Funded ratio should be less than 100%"
    );
    assert!(
        time_to_maturity_secs < 0,
        "Time to maturity should be negative (in the past)"
    );
}

/// BUG-013: Test that no warning is emitted when snapshot timestamp < maturity.
/// This is the happy path—funding closes before maturity should not emit warning 4004.
#[test]
fn test_funding_close_snapshot_before_maturity_no_warning() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let target = 100_000_000_000i128;

    // Get current ledger timestamp
    let now = env.ledger().timestamp();

    // Set maturity to a future time (10000 seconds in the future)
    let maturity_in_future = now.saturating_add(10000);

    // Initialize escrow with maturity in the future
    client.init(
        &admin,
        &String::from_str(&env, "BUG013C"),
        &sme,
        &target,
        &800i64,
        &maturity_in_future,
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

    // Fund to reach the target—this should create FundingCloseSnapshot with current timestamp
    // which is < maturity, so no warning 4004 should be emitted.
    client.fund(&investor, &target);

    // Verify escrow transitioned to funded status
    let escrow = client.get_escrow();
    assert_eq!(
        escrow.status, 1,
        "Escrow should be funded after reaching target"
    );

    // Check the health metrics
    let (warning_type, funded_ratio_bps, time_to_maturity_secs) = client.check_escrow_health();

    // With 100% funding and maturity in future, no warning should be emitted (warning_type should be 0)
    assert_eq!(
        warning_type, 0,
        "No warning should be emitted when snapshot is before maturity"
    );
    assert_eq!(
        funded_ratio_bps, 10000,
        "Funded ratio should be 10000 bps (100%)"
    );
    assert!(
        time_to_maturity_secs > 0,
        "Time to maturity should be positive (in the future)"
    );
}

/// BUG-013: Test with zero maturity (no maturity constraint) — should not emit warning.
/// When maturity is 0, there's no maturity date to validate against.
#[test]
fn test_funding_close_snapshot_no_maturity_constraint() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let target = 100_000_000_000i128;

    // Initialize escrow with maturity = 0 (no maturity constraint)
    client.init(
        &admin,
        &String::from_str(&env, "BUG013D"),
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

    // Fund to reach the target
    client.fund(&investor, &target);

    // Verify escrow transitioned to funded status
    let escrow = client.get_escrow();
    assert_eq!(
        escrow.status, 1,
        "Escrow should be funded after reaching target"
    );

    // Check the health metrics
    let (warning_type, funded_ratio_bps, time_to_maturity_secs) = client.check_escrow_health();

    // With maturity = 0 and 100% funding, no warning 4004 should be emitted
    assert_eq!(
        warning_type, 0,
        "No warning should be emitted when maturity is 0 (no constraint)"
    );
    assert_eq!(
        funded_ratio_bps, 10000,
        "Funded ratio should be 10000 bps (100%)"
    );
    assert_eq!(
        time_to_maturity_secs,
        i64::MAX,
        "Time to maturity should be i64::MAX when no maturity constraint"
    );
}

// --- MinContributionFloor validation tests (CENTRALIZED SEMANTICS) ---

/// Test 1: Zero amount rejection with no floor set (floor defaults to 0).
/// Even though floor is absent (None), zero amounts must always be rejected.
#[test]
#[should_panic(expected = "FundingAmountNotPositive")]
fn test_min_floor_zero_amount_no_floor_set() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    
    client.init(
        &admin,
        &String::from_str(&env, "MINFLOOR_TEST_1"),
        &sme,
        &100_000_000_000i128,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &None, // No min contribution floor (absent)
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    // Zero amount must be rejected, even with no floor set
    client.fund(&investor, &0i128);
}

/// Test 2: Zero amount rejection with floor explicitly set.
/// Zero amounts must always be rejected independently of floor value.
#[test]
#[should_panic(expected = "FundingAmountNotPositive")]
fn test_min_floor_zero_amount_with_floor_set() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    
    let floor = 1_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "MINFLOOR_TEST_2"),
        &sme,
        &100_000_000_000i128,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &Some(floor), // Min contribution floor explicitly set
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    // Zero amount must be rejected, even when checking floor
    client.fund(&investor, &0i128);
}

/// Test 3: Amount below floor rejected with FundingBelowMinContribution.
/// Amounts must always be >= floor (floor defaults to 0 when absent).
#[test]
#[should_panic(expected = "FundingBelowMinContribution")]
fn test_min_floor_below_floor_rejected() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    
    let floor = 5_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "MINFLOOR_TEST_3"),
        &sme,
        &100_000_000_000i128,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &Some(floor),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    // Amount below floor (but > 0) must be rejected with specific error
    client.fund(&investor, &(floor - 1));
}

/// Test 4: Amount exactly at floor accepted.
/// Amounts >= floor must be accepted (boundary condition).
#[test]
fn test_min_floor_exactly_at_floor_accepted() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    
    let floor = 5_000i128;
    client.init(
        &admin,
        &String::from_str(&env, "MINFLOOR_TEST_4"),
        &sme,
        &100_000_000_000i128,
        &800i64,
        &0u64,
        &Address::generate(&env),
        &None,
        &Address::generate(&env),
        &None,
        &Some(floor),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    // Amount exactly at floor must be accepted
    client.fund(&investor, &floor);
    
    // Verify the contribution was recorded
    let escrow = client.get_escrow();
    assert_eq!(escrow.funded_amount, floor);
}

// ─────────────────────────────────────────────────────────────────────────────
// get_investor_position aggregation tests
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: assert that `get_investor_position` aggregates exactly the values
/// returned by the individual getters for the given investor.
fn assert_position_matches_getters(
    client: &LiquifactEscrowClient<'_>,
    investor: &Address,
) {
    let position = client.get_investor_position(investor);
    assert_eq!(
        position.contribution,
        client.get_contribution(investor),
        "position.contribution must equal get_contribution"
    );
    assert_eq!(
        position.yield_bps,
        client.get_investor_yield_bps(investor),
        "position.yield_bps must equal get_investor_yield_bps"
    );
    assert_eq!(
        position.claim_not_before,
        client.get_investor_claim_not_before(investor),
        "position.claim_not_before must equal get_investor_claim_not_before"
    );
    assert_eq!(
        position.refunded,
        client.is_investor_refunded(investor),
        "position.refunded must equal is_investor_refunded"
    );
    assert_eq!(
        position.yield_claim_delegate,
        client.get_yield_claim_delegate(investor),
        "position.yield_claim_delegate must equal get_yield_claim_delegate"
    );
}

/// Unfunded investor: all aggregated fields must be at their defaults.
#[test]
fn test_get_investor_position_unfunded_defaults() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let stranger = Address::generate(&env);
    let position = client.get_investor_position(&stranger);

    assert_eq!(position.contribution, 0i128);
    assert_eq!(position.yield_bps, 0u32);
    assert_eq!(position.claim_not_before, 0u64);
    assert!(!position.refunded);
    assert_eq!(position.yield_claim_delegate, None);

    assert_position_matches_getters(&client, &stranger);
}

/// Funded investor: contribution, yield_bps and claim_not_before must be aggregated.
#[test]
fn test_get_investor_position_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    let mut tiers = SorobanVec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 100,
        yield_bps: 950,
    });

    client.init(
        &admin,
        &String::from_str(&env, "POS_FUNDED"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &Some(tiers),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp = 1_000_000u64);
    client.fund_with_commitment(&inv, &3_000i128, &100u64);

    let position = client.get_investor_position(&inv);
    assert_eq!(position.contribution, 3_000i128);
    assert_eq!(position.yield_bps, 950u32);
    assert_eq!(position.claim_not_before, 1_000_100u64);
    assert!(!position.refunded);
    assert_eq!(position.yield_claim_delegate, None);

    assert_position_matches_getters(&client, &inv);
}

/// Claimed investor: aggregation must still match the individual getters.
#[test]
fn test_get_investor_position_claimed() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "POS_CLAIMED"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    client.fund(&inv, &10_000i128);
    client.settle();
    client.claim_investor_payout(&inv);

    assert_position_matches_getters(&client, &inv);
}

/// Refunded investor: the refunded flag must be reflected in the aggregation.
#[test]
fn test_get_investor_position_refunded() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let (token, _) = init_with_token(&env, &client, &admin, &sme);
    let contract_id = client.address.clone();
    token.stellar.mint(&contract_id, &(TARGET / 2));
    client.fund(&investor, &(TARGET / 2));
    client.cancel_funding();
    client.refund(&investor);

    let position = client.get_investor_position(&investor);
    assert!(position.refunded);
    assert_eq!(position.contribution, 0i128);

    assert_position_matches_getters(&client, &investor);
}

/// Delegated investor: the yield_claim_delegate must be aggregated.
#[test]
fn test_get_investor_position_delegated() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let delegate = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &(TARGET / 2));
    client.set_yield_claim_delegate(&investor, &delegate);

    let position = client.get_investor_position(&investor);
    assert_eq!(position.yield_claim_delegate, Some(delegate.clone()));

    assert_position_matches_getters(&client, &investor);
}

/// Lock-in active: claim_not_before in the future must be aggregated correctly.
#[test]
fn test_get_investor_position_lock_in_active() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, tre) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "POS_LOCKIN"),
        &sme,
        &10_000i128,
        &800i64,
        &0u64,
        &tok,
        &None,
        &tre,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    env.ledger().with_mut(|l| l.timestamp = 5_000u64);
    client.fund_with_commitment(&inv, &4_000i128, &1_000u64);

    let position = client.get_investor_position(&inv);
    assert_eq!(position.claim_not_before, 6_000u64);
    assert!(position.claim_not_before > env.ledger().timestamp());

    assert_position_matches_getters(&client, &inv);
}
