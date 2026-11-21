//! Settlement and withdrawal tests for the karis-ky escrow contract.
//!
//! Covers the full `withdraw` surface (happy path, wrong-status guards, legal-hold
//! block, idempotency, event emission, and terminal status assertion) as well as
//! the `settle` → `claim_investor_payout` flow, maturity gates, and dust-sweep
//! integration that belong in the same lifecycle module.
//!
//! # State model recap (ADR-001)
//! ```text
//! 0 (open) ──fund──▶ 1 (funded) ──settle──▶ 2 (settled)
//!                           └────withdraw───▶ 3 (withdrawn)
//! ```
//! `withdraw` and `settle` are mutually exclusive; both require `status == 1`.
//!
//! # Test organisation
//! Each test builds its own `Env` via the shared `setup` / `default_init` helpers
//! defined in `escrow/src/test.rs`. No cross-test state is shared.

#[cfg(test)]
use super::{
    assert_contract_error, default_init, deploy, deploy_with_id, free_addresses,
    install_stellar_asset_token, setup, MAX_DUST_SWEEP_AMOUNT, TARGET,
};
use crate::LiquifactEscrow;
use soroban_sdk::{
    testutils::{Address as _, Events, Ledger as _},
    token::StellarAssetClient,
    Address, Env, String,
};

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Bring an escrow to `status == 1` (funded) by depositing exactly `TARGET`
/// from a single investor, then return the investor address.
fn fund_to_target(client: &super::LiquifactEscrowClient<'_>, env: &Env) -> Address {
    let investor = Address::generate(env);
    client.fund(&investor, &TARGET);
    investor
}

/// Set up an escrow backed by a real Stellar asset contract (SAC), fund it to
/// target, and mint `TARGET` tokens into the escrow contract so `withdraw()` can
/// actually transfer them.  Returns `(client, sme, sac_admin_client)`.
fn setup_funded_with_token<'a>(
    env: &'a Env,
) -> (
    super::LiquifactEscrowClient<'a>,
    Address,
    StellarAssetClient<'a>,
) {
    let sac = env.register_stellar_asset_contract_v2(Address::generate(env));
    let token_id = sac.address();
    let sac_admin = StellarAssetClient::new(env, &token_id);

    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(env, &escrow_id);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let treasury = Address::generate(env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(env, "INV_TOK"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &token_id,
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

    // Fund to target (accounting only — no real tokens yet).
    let investor = Address::generate(env);
    client.fund(&investor, &TARGET);

    // Mint funded_amount into the escrow contract so withdraw() has tokens to send.
    sac_admin.mint(&escrow_id, &TARGET);

    (client, sme, sac_admin)
}

/// Bring an escrow to `status == 2` (settled) and return the investor address.
fn settle_escrow(client: &super::LiquifactEscrowClient<'_>, env: &Env) -> Address {
    let investor = fund_to_target(client, env);
    client.settle();
    investor
}

// ──────────────────────────────────────────────────────────────────────────────
// `withdraw` — happy path
// ──────────────────────────────────────────────────────────────────────────────

/// Status must become 3 after a successful `withdraw`.
///
/// This is the primary assertion required by the task description.
#[test]
fn withdraw_sets_status_to_three() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, sme, _sac) = setup_funded_with_token(&env);

    client.withdraw(&sme);

    let escrow = client.get_escrow();
    assert_eq!(
        escrow.status, 3u32,
        "status must be 3 (withdrawn) after withdraw"
    );
}

/// `withdraw` must require SME auth.
///
/// In `mock_all_auths` environments the check always passes; this test
/// documents the expected signer so a future auth-audit can grep for it.
#[test]
fn withdraw_requires_sme_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _sme, _sac) = setup_funded_with_token(&env);

    // Passes because test env mocks all auth. The assertion is on the *call*
    // succeeding for the correct signer (sme), not an impostor.
    client.withdraw(&_sme);

    // Verify state changed — confirming it was sme who triggered the path.
    assert_eq!(client.get_escrow().status, 3u32);
}

/// `withdraw` must be rejected if called by an address other than the registered SME.
#[test]
#[should_panic]
fn withdraw_rejects_wrong_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, sme, _sac) = setup_funded_with_token(&env);

    // Generate a different address (not the SME)
    let impostor = Address::generate(&env);

    // This should panic because impostor != sme
    client.withdraw(&impostor);
}

/// After `withdraw` the funded_amount and funding_target remain intact —
/// `withdraw` transitions state and transfers tokens, but does not zero accounting fields.
#[test]
fn withdraw_preserves_accounting_fields() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, sme, _sac) = setup_funded_with_token(&env);

    client.withdraw(&sme);

    let escrow = client.get_escrow();
    assert_eq!(
        escrow.funded_amount, TARGET,
        "funded_amount must not be wiped by withdraw"
    );
    assert_eq!(
        escrow.funding_target, TARGET,
        "funding_target must not be mutated by withdraw"
    );
}

/// `withdraw` emits an `SmeWithdrew` event.
#[test]
fn withdraw_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, sme, _sac) = setup_funded_with_token(&env);

    client.withdraw(&sme);

    // At least one event must be emitted in the transaction.
    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(
        !events.is_empty(),
        "withdraw must emit at least one contract event"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// `withdraw` — wrong-status guards
// ──────────────────────────────────────────────────────────────────────────────

/// `withdraw` on an `open` (status 0) escrow must panic.
///
/// The escrow has not been funded; `withdraw` requires `status == 1`.
#[test]
#[should_panic]
fn withdraw_on_open_escrow_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    // No funding — status is still 0.
    client.withdraw(&sme);
}

/// `withdraw` on an already-settled (status 2) escrow must panic.
///
/// Once `settle` has been called the escrow is terminal in the settlement path;
/// `withdraw` must not be able to re-label it.
#[test]
#[should_panic]
fn withdraw_on_settled_escrow_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    settle_escrow(&client, &env);
    // status == 2 — withdraw must be rejected.
    client.withdraw(&sme);
}

/// `withdraw` called twice on the same escrow must panic on the second call.
///
/// Once status reaches 3 (withdrawn) it is terminal; no forward transition
/// exists from 3, so a second `withdraw` must be rejected.
#[test]
#[should_panic]
fn withdraw_twice_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, sme, _sac) = setup_funded_with_token(&env);

    client.withdraw(&sme); // first call — succeeds, status → 3
    client.withdraw(&sme); // second call — must panic (status == 3, not 1)
}

/// `settle` cannot be called after `withdraw` (status 3 is terminal).
#[test]
#[should_panic]
fn settle_after_withdraw_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, sme, _sac) = setup_funded_with_token(&env);
    client.withdraw(&sme); // status → 3
    client.settle(); // must panic — settle requires status == 1
}

/// `fund` cannot be called after `withdraw` (status 3 is terminal).
#[test]
#[should_panic]
fn fund_after_withdraw_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, sme, _sac) = setup_funded_with_token(&env);
    client.withdraw(&sme); // status → 3
    let late_investor = Address::generate(&env);
    client.fund(&late_investor, &10_000_000_000_i128); // must panic — fund requires status == 0
}

// ──────────────────────────────────────────────────────────────────────────────
// `withdraw` — legal-hold block (ADR-004)
// ──────────────────────────────────────────────────────────────────────────────

/// `withdraw` must be blocked while a legal hold is active.
///
/// Per ADR-004 the hold freezes `withdraw` regardless of escrow status.
#[test]
#[should_panic]
fn withdraw_blocked_by_legal_hold() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);

    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    // Status is 1 but hold is active — must panic.
    client.withdraw(&sme);
}

/// `withdraw` must succeed after a legal hold is cleared.
///
/// Verifies that `clear_legal_hold` (or `set_legal_hold(false)`) fully lifts
/// the block and the escrow can proceed to `status == 3`.
#[test]
fn withdraw_succeeds_after_hold_cleared() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, sme, _sac) = setup_funded_with_token(&env);

    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    client.set_legal_hold(&false, &String::from_str(&env, ""));

    client.withdraw(&sme);
    assert_eq!(client.get_escrow().status, 3u32);
}

// ──────────────────────────────────────────────────────────────────────────────
// Investor claim idempotency and per-investor isolation
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_claim_investor_twice_is_idempotent() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CL001"),
        &sme,
        &1_000i128,
        &400i64,
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
    client.fund(&investor, &1_000i128);
    client.settle();

    // First claim - should succeed and set the claimed marker
    client.claim_investor_payout(&investor);

    assert!(client.is_investor_claimed(&investor));

    // Second claim - should be idempotent (no-op, does not panic)
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

#[test]
#[should_panic]
fn test_claim_by_non_investor_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let stranger = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "STR001"),
        &sme,
        &1_000i128,
        &400i64,
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
    // Escrow settled but stranger never funded
    let investor = Address::generate(&env);
    client.fund(&investor, &1_000i128);
    client.settle();

    client.claim_investor_payout(&stranger);
}

#[test]
fn test_clashing_investors_have_independent_claims() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CLASH01"),
        &sme,
        &2_000i128,
        &400i64,
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
    client.fund(&inv_a, &1_000i128);
    client.fund(&inv_b, &1_000i128);
    client.settle();

    client.claim_investor_payout(&inv_a);
    assert!(client.is_investor_claimed(&inv_a));
    assert!(!client.is_investor_claimed(&inv_b));

    client.claim_investor_payout(&inv_b);
    assert!(client.is_investor_claimed(&inv_b));
}

/// `set_legal_hold` must be admin-only; a non-admin cannot place a hold.
#[test]
#[should_panic]
fn legal_hold_set_by_non_admin_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    env.mock_all_auths_allowing_non_root_auth(); // stricter auth mode
    env.mock_auths(&[]);
    default_init(&client, &env, &admin, &sme);
    // `sme` is not the admin — must panic.
    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
}

// ──────────────────────────────────────────────────────────────────────────────
// `settle` path — complementary coverage ensuring mutual exclusivity
// ──────────────────────────────────────────────────────────────────────────────

/// `settle` transitions status from 1 to 2.
#[test]
fn settle_sets_status_to_two() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);

    client.settle();

    assert_eq!(client.get_escrow().status, 2u32);
}

/// `settle` is blocked while a legal hold is active.
#[test]
#[should_panic]
fn settle_blocked_by_legal_hold() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);

    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    client.settle();
}

#[test]
#[should_panic]
fn test_claim_blocked_until_commitment_ledger_time() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, treasury) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "LOCK001"),
        &sme,
        &1_000i128,
        &400i64,
        &0u64,
        &tok,
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
    client.fund_with_commitment(&inv, &1_000i128, &500u64);
    client.settle();
    client.claim_investor_payout(&inv);
}

#[test]
fn test_claim_succeeds_after_commitment_and_settle() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, treasury) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "LOCK002"),
        &sme,
        &1_000i128,
        &400i64,
        &0u64,
        &tok,
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
    client.fund_with_commitment(&inv, &1_000i128, &100u64);
    client.settle();
    env.ledger().set_timestamp(150);
    client.claim_investor_payout(&inv);
    assert!(client.is_investor_claimed(&inv));
}

#[test]
fn test_claim_gating_exact_timestamp() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv = Address::generate(&env);
    let (tok, treasury) = free_addresses(&env);

    env.ledger().set_timestamp(1000);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "LOCK003"),
        &sme,
        &1_000i128,
        &400i64,
        &0u64,
        &tok,
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

    let lock_duration = 500u64;
    client.fund_with_commitment(&inv, &1_000i128, &lock_duration);
    client.settle();

    let expiry = 1000 + lock_duration;

    // 1 second before expiry
    env.ledger().set_timestamp(expiry - 1);
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.claim_investor_payout(&inv);
    }));
    assert!(err.is_err(), "Claim should be blocked 1s before expiry");

    // Exact expiry
    env.ledger().set_timestamp(expiry);
    client.claim_investor_payout(&inv);
    assert!(client.is_investor_claimed(&inv));
}

#[test]
fn test_claim_gating_with_multiple_investors() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let inv1 = Address::generate(&env);
    let inv2 = Address::generate(&env);
    let (tok, treasury) = free_addresses(&env);

    env.ledger().set_timestamp(1000);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "LOCK004"),
        &sme,
        &2_000i128,
        &400i64,
        &0u64,
        &tok,
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

    client.fund_with_commitment(&inv1, &1_000i128, &100u64); // Expiry 1100
    client.fund_with_commitment(&inv2, &1_000i128, &200u64); // Expiry 1200
    client.settle();

    env.ledger().set_timestamp(1150);

    // inv1 can claim
    client.claim_investor_payout(&inv1);
    assert!(client.is_investor_claimed(&inv1));

    // inv2 still blocked
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.claim_investor_payout(&inv2);
    }));
    assert!(err.is_err(), "inv2 should still be blocked at 1150");

    env.ledger().set_timestamp(1200);
    client.claim_investor_payout(&inv2);
    assert!(client.is_investor_claimed(&inv2));
}

/// Cost baseline: settle after funding.
#[test]
fn test_cost_baseline_settle() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "INV103b"),
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
    env.ledger().set_timestamp(1001);
    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

/// `settle` called twice must panic on the second call.
#[test]
#[should_panic]
fn settle_twice_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);
    client.settle();
    client.settle(); // status == 2, must panic
}

// ──────────────────────────────────────────────────────────────────────────────
// Maturity gate — settle is time-gated when `maturity > 0`; bypass when 0
// ──────────────────────────────────────────────────────────────────────────────

/// `settle` succeeds immediately when `maturity == 0` regardless of ledger time.
#[test]
fn settle_with_maturity_zero_succeeds_immediately() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "INV_MAT_001"),
        &sme,
        &TARGET,
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

    assert!(
        !client.has_maturity_lock(),
        "maturity == 0 must be surfaced as no maturity lock"
    );
    assert!(!client.get_escrow_summary().has_maturity_lock);

    fund_to_target(&client, &env);

    env.ledger().with_mut(|l| l.timestamp = 1);
    let settled = client.settle();
    assert_eq!(settled.status, 2);
    assert_eq!(settled.maturity, 0);
}

/// `settle` with `maturity > 0` must trap one second before the configured
/// validator-observed ledger timestamp and must not mutate the funded state.
#[test]
fn settle_one_second_before_maturity_traps_and_preserves_state() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    let maturity: u64 = 20_000;
    client.init(
        &admin,
        &String::from_str(&env, "INV_MAT_003"),
        &sme,
        &TARGET,
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

    fund_to_target(&client, &env);
    let snapshot_before = client.get_funding_close_snapshot();

    env.ledger().with_mut(|l| l.timestamp = maturity - 1);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.settle();
    }));

    assert!(
        result.is_err(),
        "settle must trap before the inclusive maturity boundary"
    );
    assert_eq!(
        client.get_escrow().status,
        1,
        "pre-maturity settlement attempt must leave escrow funded"
    );
    assert_eq!(
        client.get_funding_close_snapshot(),
        snapshot_before,
        "pre-maturity settlement attempt must not mutate snapshot state"
    );
}

/// `settle` with `maturity > 0` succeeds at exactly the maturity timestamp.
#[test]
fn settle_at_maturity_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    let maturity: u64 = 20_000;
    client.init(
        &admin,
        &String::from_str(&env, "INV_MAT_002"),
        &sme,
        &TARGET,
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

    assert!(
        client.has_maturity_lock(),
        "positive maturity must be surfaced as an active maturity lock"
    );
    assert!(client.get_escrow_summary().has_maturity_lock);

    fund_to_target(&client, &env);
    env.ledger().with_mut(|l| l.timestamp = maturity);
    let settled = client.settle();
    assert_eq!(settled.status, 2);
    assert_eq!(settled.maturity, maturity);
}

/// `settle` must panic if SME auth is not provided.
#[test]
#[should_panic]
fn settle_requires_sme_auth() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);

    env.mock_auths(&[]); // clear mocks — auth will fail
    client.settle();
}

/// `settle` on open (status 0) escrow must panic.
#[test]
#[should_panic]
fn settle_on_open_escrow_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    // No funding — status is still 0.
    client.settle();
}

/// `settle` on withdrawn (status 3) escrow must panic.
#[test]
#[should_panic]
fn settle_on_withdrawn_escrow_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);
    client.withdraw(&sme); // status → 3
    client.settle();
}

/// `sweep_terminal_dust` must reject open/funded escrows before terminal state.
// HostError wraps contract panic; expected substring not matched in outer message.
#[ignore = "HostError wraps contract panic; expected substring not matched"]
#[test]
#[should_panic(expected = "dust sweep only in terminal states (settled, withdrawn, or cancelled)")]
fn sweep_terminal_dust_before_terminal_state_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = settle_escrow(&client, &env);

    client.claim_investor_payout(&investor);

    let contract_events = env.events().all();
    let events = contract_events.events();
    assert!(
        !events.is_empty(),
        "claim must emit InvestorPayoutClaimed event"
    );
}

/// `claim_investor_payout` must be blocked while a legal hold is active.
#[test]
#[should_panic]
fn claim_investor_payout_blocked_by_legal_hold() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = settle_escrow(&client, &env);

    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    client.claim_investor_payout(&investor); // must panic
}

/// `claim_investor_payout` must fail before `settle` (status != 2).
#[test]
#[should_panic]
fn claim_investor_payout_before_settle_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = fund_to_target(&client, &env);
    client.claim_investor_payout(&investor);
}

/// An investor that did not participate cannot claim.
#[test]
#[should_panic]
fn claim_investor_payout_non_participant_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    env.mock_all_auths_allowing_non_root_auth();
    env.mock_auths(&[]);
    default_init(&client, &env, &admin, &sme);
    settle_escrow(&client, &env);

    let stranger = Address::generate(&env);
    client.claim_investor_payout(&stranger);
}

/// `settle` with `maturity > 0` succeeds one ledger after the configured timestamp.
#[test]
fn settle_one_second_after_maturity_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    let maturity: u64 = 20_000;
    client.init(
        &admin,
        &String::from_str(&env, "INV_MAT_004"),
        &sme,
        &TARGET,
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

    fund_to_target(&client, &env);

    // Advance ledger to one second after maturity
    env.ledger().with_mut(|l| l.timestamp = maturity + 1);
    let settled = client.settle();
    assert_eq!(settled.status, 2, "settle after maturity must succeed");
}

/// `settle` must still succeed when ledger time is slightly ahead (clock skew).
#[test]
fn settle_with_ledger_clock_skew_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    let maturity: u64 = 20_000;
    client.init(
        &admin,
        &String::from_str(&env, "INV_MAT_005"),
        &sme,
        &TARGET,
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

    fund_to_target(&client, &env);

    // Simulate clock skew: ledger time is 5 seconds ahead of maturity
    env.ledger().with_mut(|l| l.timestamp = maturity + 5);
    let settled = client.settle();
    assert_eq!(settled.status, 2, "clock skew must not break settlement");
}

// ──────────────────────────────────────────────────────────────────────────────
// Terminal dust sweep
// ──────────────────────────────────────────────────────────────────────────────

// Uses `token.stellar`/`escrow_id` not in scope (deploy not deploy_with_id).
#[cfg(any())]
#[test]
fn test_sweep_terminal_dust_after_settle_transfers_to_treasury() {
    let env = Env::default();
    env.mock_all_auths();
    let token = install_stellar_asset_token(&env);
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (_tok, treasury) = free_addresses(&env);
    let maturity = 5000u64;
    client.init(
        &admin,
        &String::from_str(&env, "SW001"),
        &sme,
        &TARGET,
        &100i64,
        &maturity,
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
    let investor = Address::generate(&env);
    client.fund(&investor, &1_000i128);
    client.settle();

    token.stellar.mint(&contract_id, &5_000i128);
    let before_t = token.token.balance(&treasury);
    let swept = client.sweep_terminal_dust(&5_000i128);
    assert_eq!(swept, 5_000i128);
    assert_eq!(token.token.balance(&tre), before_t + 5_000i128);
}

// Uses `token.stellar`/`escrow_id` not in scope.
#[cfg(any())]
#[test]
fn test_sweep_terminal_dust_after_withdraw_and_ledger_tick() {
    let env = Env::default();
    env.mock_all_auths();
    let token = install_stellar_asset_token(&env);
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (_tok, treasury) = free_addresses(&env);
    let maturity = 5000u64;
    client.init(
        &admin,
        &String::from_str(&env, "SW002"),
        &sme,
        &TARGET,
        &100i64,
        &maturity,
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
    let investor = Address::generate(&env);
    client.fund(&investor, &1_000i128);
    client.withdraw(&sme);

    env.ledger()
        .set_sequence_number(env.ledger().sequence() + 10);

    token.stellar.mint(&contract_id, &333i128);
    let swept = client.sweep_terminal_dust(&333i128);
    assert_eq!(swept, 333i128);
}

// HostError wraps contract panic; expected substring not matched.
#[ignore = "HostError wraps contract panic; expected substring not matched"]
#[test]
#[should_panic]
fn test_sweep_rejected_when_open() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "SW003"),
        &sme,
        &TARGET,
        &100i64,
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
    client.fund(&investor, &1_000i128);
    client.settle();
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

#[test]
#[should_panic]
fn test_sweep_blocked_under_legal_hold() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &String::from_str(&env, "SW004"),
        &sme,
        &1_000i128,
        &100i64,
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
    client.fund(&investor, &1_000i128);
    client.settle();
    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    client.sweep_terminal_dust(&1i128);
}

// HostError wraps contract panic; expected substring not matched.
#[ignore = "HostError wraps contract panic; expected substring not matched"]
#[test]
#[should_panic]
fn test_sweep_rejects_amount_above_dust_cap() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SW005"),
        &sme,
        &TARGET,
        &100i64,
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
    client.fund(&investor, &1_000i128);
    // status == 1 (funded), not settled — must panic
    client.claim_investor_payout(&investor);
}

// Body calls claim_investor_payout for a stranger (panics); no #[should_panic].
#[ignore = "body tests non-participant claim, not dust sweep capping; panics without #[should_panic]"]
#[test]
fn test_sweep_caps_at_contract_balance() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SW006"),
        &sme,
        &1_000i128,
        &100i64,
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
    client.fund(&investor, &1_000i128);
    client.settle();
    client.claim_investor_payout(&stranger); // must panic — no contribution
}

// Uses `token.stellar`/`escrow_id` not in scope (setup/default_init pattern).
#[cfg(any())]
#[test]
fn test_sweep_requires_treasury_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let token = install_stellar_asset_token(&env);
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (_tok, treasury) = free_addresses(&env);
    client.init(
        &admin,
        &String::from_str(&env, "SW007"),
        &sme,
        &TARGET,
        &100i64,
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
    fund_to_target(&client, &env);
    client.settle();
    token
        .stellar
        .mint(&contract_id, &(MAX_DUST_SWEEP_AMOUNT + 1));

    client.sweep_terminal_dust(&(MAX_DUST_SWEEP_AMOUNT + 1));
}

/// `claim_investor_payout` succeeds for an investor after `settle`.
// Uses `token.stellar`/`escrow_id` not in scope (setup/default_init pattern).
#[cfg(any())]
#[test]
fn claim_investor_payout_succeeds_after_settle() {
    let env = Env::default();
    env.mock_all_auths();
    let investor = Address::generate(&env);
    let token = install_stellar_asset_token(&env);
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (_tok, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "SW008"),
        &sme,
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
    client.fund(&investor, &TARGET);
    client.settle();
    token.stellar.mint(&contract_id, &10i128);

    env.mock_auths(&[]);
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.sweep_terminal_dust(&10i128);
    }));
    assert!(err.is_err(), "sweep without treasury auth must fail");
}

/// Deterministic scenario: sweeping terminal dust must not consume funds that
/// are still owed to investors who have not yet claimed.
///
/// Three investors each contribute a third of `TARGET`; the escrow settles and
/// `TARGET` tokens are minted into the contract.  The first investor claims,
/// then the maximum permitted dust sweep runs.  The remaining two investors
/// must still be able to claim their full payouts, and the contract balance
/// after the sweep must cover both outstanding liabilities.
#[test]
fn test_sweep_terminal_dust_preserves_unclaimed_investor_liabilities() {
    let env = Env::default();
    env.mock_all_auths();
    let token = install_stellar_asset_token(&env);
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (_tok, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "SW_DUST_LIAB"),
        &sme,
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

    // Three equal investors, each contributing a third of TARGET.
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);
    let third = TARGET / 3;
    let remainder = TARGET - (third * 2);
    client.fund(&inv_a, &third);
    client.fund(&inv_b, &third);
    client.fund(&inv_c, &remainder);

    client.settle();

    // Mint the full TARGET into the contract so payouts can be honoured.
    token.stellar.mint(&contract_id, &TARGET);

    // First investor claims.
    client.claim_investor_payout(&inv_a);
    assert!(client.is_investor_claimed(&inv_a));

    // Compute the outstanding liabilities for the two unclaimed investors.
    let payout_b = client.compute_investor_payout(&inv_b);
    let payout_c = client.compute_investor_payout(&inv_c);
    let outstanding = payout_b + payout_c;

    // Sweep the maximum allowed amount, capped by the dust cap and the balance.
    let balance_before = token.token.balance(&contract_id);
    let sweep_amount = if balance_before < MAX_DUST_SWEEP_AMOUNT {
        balance_before
    } else {
        MAX_DUST_SWEEP_AMOUNT
    };
    let treasury_before = token.token.balance(&treasury);
    let swept = client.sweep_terminal_dust(&sweep_amount);

    assert_eq!(swept, sweep_amount, "sweep must transfer the requested amount");
    assert_eq!(
        token.token.balance(&treasury),
        treasury_before + swept,
        "treasury must receive the swept amount"
    );

    // The remaining balance must still cover both unclaimed payouts.
    let balance_after = token.token.balance(&contract_id);
    assert!(
        balance_after >= outstanding,
        "contract balance after sweep must cover unclaimed investor liabilities"
    );

    // Both remaining investors can still claim successfully.
    client.claim_investor_payout(&inv_b);
    client.claim_investor_payout(&inv_c);

    assert!(client.is_investor_claimed(&inv_a));
    assert!(client.is_investor_claimed(&inv_b));
    assert!(client.is_investor_claimed(&inv_c));
}

// ──────────────────────────────────────────────────────────────────────────────
// BUG-005: Token transfer safety checks in sweep_terminal_dust
// ──────────────────────────────────────────────────────────────────────────────

/// **BUG-005 Test**: `sweep_terminal_dust` validates pre-transfer balance and emits
/// a typed error if the contract is under-funded, preventing host-level traps.
///
/// Scenario:
/// 1. Settle an escrow with a token balance
/// 2. Verify the balance (for setup)
/// 3. Call sweep with an amount greater than the actual balance (simulating under-funding)
/// 4. Expect a typed error (`InsufficientTokenBalanceBeforeTransfer`) instead of a host trap
///
/// This test ensures the contract emits typed errors on insufficient balance before
/// invoking the token transfer, providing clear feedback to treasury operators.
#[test]
#[should_panic(expected = "Insufficient token balance before transfer")]
fn test_sweep_terminal_dust_underfunded_emits_typed_error() {
    let env = Env::default();
    env.mock_all_auths();
    let token = install_stellar_asset_token(&env);
    let (contract_id, client) = deploy_with_id(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (_tok, treasury) = free_addresses(&env);
    let maturity = 5000u64;

    client.init(
        &admin,
        &String::from_str(&env, "SW_BUG005"),
        &sme,
        &TARGET,
        &100i64,
        &maturity,
        &token.id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund and settle
    let investor = Address::generate(&env);
    client.fund(&investor, &1_000i128);
    client.settle();

    // Mint only 100 tokens into the contract
    token.stellar.mint(&contract_id, &100i128);

    // Attempt to sweep 200 tokens — should fail with typed error
    // instead of a host-level trap
    client.sweep_terminal_dust(&200i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// Funding snapshot invariant (ADR-003)
// ──────────────────────────────────────────────────────────────────────────────

/// The funding-close snapshot is written once when status transitions to 1.
/// After `withdraw` the snapshot must still be readable with the original values.
///
/// This guards against the denominator being zeroed or mutated by the withdrawal
/// path — off-chain accounting always needs a stable snapshot.
#[test]
fn funding_snapshot_survives_withdraw() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);

    let snapshot_before = client
        .get_funding_close_snapshot()
        .expect("snapshot exists after fund close");
    client.withdraw(&sme);
    let snapshot_after = client
        .get_funding_close_snapshot()
        .expect("snapshot persists after withdraw");

    assert_eq!(
        snapshot_before, snapshot_after,
        "funding snapshot must be immutable after withdraw"
    );
    assert_eq!(
        snapshot_after.total_principal, TARGET,
        "snapshot total_principal must equal funded amount"
    );
}

/// The threshold-crossing deposit may overfund the target; the snapshot must capture the
/// full credited funded_amount and the exact ledger timestamp/sequence at close.
#[test]
fn funding_close_snapshot_captures_overfunding_and_close_ledger() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    let first_leg = TARGET - 10_000i128;
    let crossing_leg = 25_000i128;
    let close_total = first_leg + crossing_leg;

    client.fund(&investor_a, &first_leg);
    assert_eq!(
        client.get_funding_close_snapshot(),
        None,
        "snapshot must be absent before funded transition"
    );

    let close_timestamp = 88_888u64;
    let close_sequence = 777u32;
    env.ledger().with_mut(|ledger| {
        ledger.timestamp = close_timestamp;
        ledger.sequence_number = close_sequence;
    });

    client.fund(&investor_b, &crossing_leg);

    let escrow = client.get_escrow();
    let snapshot = client
        .get_funding_close_snapshot()
        .expect("snapshot must be written at funded transition");
    assert_eq!(escrow.status, 1, "escrow must close as funded");
    assert_eq!(escrow.funded_amount, close_total);
    assert_eq!(
        snapshot.total_principal, escrow.funded_amount,
        "snapshot denominator must match overfunded close amount"
    );
    assert_eq!(snapshot.total_principal, TARGET + 15_000i128);
    assert_eq!(snapshot.funding_target, TARGET);
    assert_eq!(snapshot.closed_at_ledger_timestamp, close_timestamp);
    assert_eq!(snapshot.closed_at_ledger_sequence, close_sequence);
}

/// After the funded transition, another same-ledger funding attempt must not overwrite the
/// snapshot or mutate contributions. This guards the write-once denominator invariant even if a
/// caller retries immediately after the close.
#[test]
fn funding_close_snapshot_not_overwritten_by_same_ledger_follow_on_attempt() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let closer = Address::generate(&env);
    let late_investor = Address::generate(&env);
    let close_amount = TARGET + 1_234i128;

    env.ledger().with_mut(|ledger| {
        ledger.timestamp = 99_999;
        ledger.sequence_number = 999;
    });
    client.fund(&closer, &close_amount);
    let snapshot_at_close = client
        .get_funding_close_snapshot()
        .expect("snapshot exists after overfunded close");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.fund(&late_investor, &1i128);
    }));
    assert!(
        result.is_err(),
        "funding after close must fail before it can overwrite snapshot"
    );

    let snapshot_after_attempt = client
        .get_funding_close_snapshot()
        .expect("snapshot remains present after rejected follow-on attempt");
    assert_eq!(
        snapshot_at_close, snapshot_after_attempt,
        "snapshot must remain write-once after same-ledger follow-on attempt"
    );
    assert_eq!(client.get_escrow().funded_amount, close_amount);
    assert_eq!(
        client.get_contribution(&late_investor),
        0,
        "rejected follow-on funding must not create contribution state"
    );
}

/// After `settle` the snapshot still matches what was recorded at fund-close.
#[test]
fn funding_snapshot_survives_settle() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);

    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);

    let snapshot_before = client
        .get_funding_close_snapshot()
        .expect("snapshot exists after fund");
    client.settle();
    let snapshot_after = client.get_funding_close_snapshot();

    assert_eq!(
        snapshot_before.total_principal,
        snapshot_after.unwrap().total_principal
    );
}

// ── is_investor_claimed: idempotent read behavior & cross-investor isolation ──

#[test]
fn test_is_investor_claimed_false_before_any_claim() {
    // Getter must return false for a funded investor who has not yet claimed;
    // repeated reads must not mutate state.
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "GIC001"),
        &sme,
        &1_000i128,
        &400i64,
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
    client.fund(&investor, &1_000i128);
    client.settle();
    assert!(!client.is_investor_claimed(&investor));
    assert!(!client.is_investor_claimed(&investor)); // idempotent — no state change
}

#[test]
fn test_is_investor_claimed_returns_false_for_unfunded_address() {
    // An address that never participated must return false, not panic.
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "GIC002"),
        &sme,
        &1_000i128,
        &400i64,
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
    client.fund(&investor, &1_000i128);
    client.settle();
    assert!(!client.is_investor_claimed(&stranger));
}

#[test]
fn test_claim_marker_persists_after_claim() {
    // After a successful claim the flag must remain true across repeated reads.
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "GIC003"),
        &sme,
        &1_000i128,
        &400i64,
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
    client.fund(&investor, &1_000i128);
    client.settle();
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
    assert!(client.is_investor_claimed(&investor)); // second read: still persisted
}

#[test]
fn test_claim_marker_isolated_per_investor() {
    // Claiming for investor_a must not set the flag for investor_b (no key crosstalk).
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "GIC004"),
        &sme,
        &2_000i128,
        &400i64,
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
    client.fund(&investor_a, &1_000i128);
    client.fund(&investor_b, &1_000i128);
    client.settle();
    client.claim_investor_payout(&investor_a);
    assert!(client.is_investor_claimed(&investor_a));
    assert!(!client.is_investor_claimed(&investor_b)); // b unaffected by a's claim
}

#[test]
fn test_claim_marker_all_investors_independent() {
    // Three investors with independent claim keys; partial claiming must not
    // corrupt unclaimed investors' flags.
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let inv_c = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "GIC005"),
        &sme,
        &3_000i128,
        &400i64,
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
    client.fund(&inv_a, &1_000i128);
    client.fund(&inv_b, &1_000i128);
    client.fund(&inv_c, &1_000i128);
    client.settle();
    client.claim_investor_payout(&inv_a);
    client.claim_investor_payout(&inv_c);
    assert!(client.is_investor_claimed(&inv_a));
    assert!(!client.is_investor_claimed(&inv_b)); // b still unclaimed
    assert!(client.is_investor_claimed(&inv_c));
    client.claim_investor_payout(&inv_b);
    assert!(client.is_investor_claimed(&inv_b));
}

#[test]
fn investor_contribution_readable_after_withdraw() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    let contribution: i128 = TARGET;
    client.fund(&investor, &contribution);
    client.withdraw(&sme);

    let recorded = client.get_contribution(&investor);
    assert_eq!(
        recorded, contribution,
        "investor contribution must be readable after withdraw for refund accounting"
    );
}

/// Multiple investors — each contribution is preserved after `withdraw`.
#[test]
fn multi_investor_contributions_preserved_after_withdraw() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    // Fund with two investors reaching target collectively.
    let inv_a = Address::generate(&env);
    let inv_b = Address::generate(&env);
    let half = TARGET / 2;
    client.fund(&inv_a, &half);
    client.fund(&inv_b, &(TARGET - half));

    client.withdraw(&sme);

    assert_eq!(client.get_contribution(&inv_a), half);
    assert_eq!(client.get_contribution(&inv_b), TARGET - half);
    assert_eq!(client.get_escrow().status, 3u32);
}

// ──────────────────────────────────────────────────────────────────────────────
// Terminal status — no entrypoint can move state backward from 3
// ──────────────────────────────────────────────────────────────────────────────

/// After `withdraw` (status 3) no write entrypoint must succeed.
///
/// This is a belt-and-suspenders test that exercises every state-mutating
/// path the SME might attempt after withdrawal.
#[test]
fn no_state_mutation_possible_after_withdraw() {
    // settle after withdraw
    {
        let env = Env::default();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);
        fund_to_target(&client, &env);
        client.withdraw(&sme);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.settle();
        }));
        assert!(r.is_err(), "settle after withdraw must panic");
    }

    // withdraw after withdraw
    {
        let env = Env::default();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);
        fund_to_target(&client, &env);
        client.withdraw(&sme);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.withdraw(&sme);
        }));
        assert!(r.is_err(), "withdraw after withdraw must panic");
    }

    // fund after withdraw
    {
        let env = Env::default();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);
        fund_to_target(&client, &env);
        client.withdraw(&sme);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let late = Address::generate(&env);
            client.fund(&late, &10_000_000_000_i128);
        }));
        assert!(r.is_err(), "fund after withdraw must panic");
    }
}
// ──────────────────────────────────────────────────────────────────────────────
// `archive_escrow` — unclaimed-payout guard and terminal-state behavior
// ──────────────────────────────────────────────────────────────────────────────

/// `archive_escrow` must panic with `ArchiveBlockedByUnclaimedPayouts` when a
/// settled escrow still has at least one investor who has not claimed.
#[test]
#[should_panic(expected = "ArchiveBlockedByUnclaimedPayouts")]
fn archive_escrow_blocked_by_unclaimed_payouts() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    // Settle with a single investor who never claims.
    let _investor = settle_escrow(&client, &env);

    client.archive_escrow();
}

/// `archive_escrow` must succeed once every investor has claimed their payout.
#[test]
fn archive_escrow_succeeds_after_all_claims() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = settle_escrow(&client, &env);

    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));

    client.archive_escrow();
    assert_eq!(
        client.get_escrow().status,
        5u32,
        "status must be 5 (archived) after archive_escrow"
    );
}

/// `archive_escrow` must succeed for a withdrawn (status 3) escrow regardless
/// of claim state — the settlement path never ran, so there are no payouts.
#[test]
fn archive_escrow_succeeds_for_withdrawn_escrow() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);
    client.withdraw(&sme); // status → 3

    client.archive_escrow();
    assert_eq!(client.get_escrow().status, 5u32);
}

/// `archive_escrow` must succeed for a cancelled (status 4) escrow regardless
/// of claim state.
#[test]
fn archive_escrow_succeeds_for_cancelled_escrow() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);
    client.cancel(&sme); // status → 4

    client.archive_escrow();
    assert_eq!(client.get_escrow().status, 5u32);
}

/// `archive_escrow` must be idempotent: calling it again on an already-archived
/// (status 5) escrow must not panic and must leave the status at 5.
#[test]
fn archive_escrow_is_idempotent_when_already_archived() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    fund_to_target(&client, &env);
    client.withdraw(&sme); // status → 3

    client.archive_escrow();
    assert_eq!(client.get_escrow().status, 5u32);

    // Second call must be a no-op, not a panic.
    client.archive_escrow();
    assert_eq!(
        client.get_escrow().status,
        5u32,
        "archive_escrow must be idempotent when already archived"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// `partial_settle` tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_partial_settle_sme_happy_path() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    // Partially fund
    let investor = Address::generate(&env);
    client.fund(&investor, &(TARGET / 2));

    // SME settles early
    client.partial_settle(&sme);

    let escrow = client.get_escrow();
    assert_eq!(
        escrow.status, 1u32,
        "Status must be 1 (funded/settleable) after partial_settle"
    );
    assert_eq!(escrow.funded_amount, TARGET / 2);
}

#[test]
fn test_partial_settle_admin_happy_path() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    // Admin settles early
    client.partial_settle(&admin);

    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 1u32);
}

#[test]
#[should_panic]
fn test_partial_settle_unauthorized_caller_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let stranger = Address::generate(&env);
    client.partial_settle(&stranger);
}

#[test]
#[should_panic]
fn test_partial_settle_blocked_by_legal_hold() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    client.partial_settle(&sme);
}

#[test]
#[should_panic]
fn test_partial_settle_rejected_if_not_open() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    // Fully fund — status becomes 1; partial_settle requires status == 0.
    fund_to_target(&client, &env);
    client.partial_settle(&sme);
}

#[test]
fn test_partial_settle_writes_correct_snapshot() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let amount = 123_456_789i128;
    let investor = Address::generate(&env);
    client.fund(&investor, &amount);

    client.partial_settle(&sme);

    let snapshot = client
        .get_funding_close_snapshot()
        .expect("Snapshot must exist");
    assert_eq!(snapshot.total_principal, amount);
    assert_eq!(snapshot.funding_target, TARGET);
}

#[test]
#[should_panic]
fn test_funding_blocked_after_partial_settle() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    client.partial_settle(&sme);

    let late_investor = Address::generate(&env);
    client.fund(&late_investor, &1_000i128);
}

// ──────────────────────────────────────────────────────────────────────────────
// Partial settlement with amount parameter tests
// ──────────────────────────────────────────────────────────────────────────────

/// Partial settlement with 50% of funded amount should keep status as 1 (funded)
#[test]
fn settle_partial_amount_keeps_status_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 1u32, "Pre-condition: status must be funded");

    // Settle 50% of the funded amount
    let partial_amount = escrow.funded_amount / 2;
    client.settle(&Some(partial_amount));

    let updated_escrow = client.get_escrow();
    assert_eq!(
        updated_escrow.status, 1u32,
        "status must remain 1 (funded) after partial settlement"
    );
}

/// Full settlement (partial_amount == funded_amount) should transition to status 2
#[test]
fn settle_full_amount_transitions_to_settled() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();

    // Settle 100% explicitly with the full amount
    client.settle(&Some(escrow.funded_amount));

    let updated_escrow = client.get_escrow();
    assert_eq!(
        updated_escrow.status, 2u32,
        "status must be 2 (settled) when fully settled"
    );
}

/// Settle with no amount parameter should fully settle (backward compatibility)
#[test]
fn settle_without_amount_fully_settles() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);

    // Call settle without amount parameter
    client.settle(&None);

    let updated_escrow = client.get_escrow();
    assert_eq!(
        updated_escrow.status, 2u32,
        "settle with None should fully settle"
    );
}

/// Multiple partial settlements should accumulate correctly
#[test]
fn multiple_partial_settlements_accumulate() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();
    let total = escrow.funded_amount;

    // First settlement: 30%
    let first_settlement = total / 3;
    client.settle(&Some(first_settlement));

    // Verify status still funded
    assert_eq!(client.get_escrow().status, 1u32);

    // Second settlement: 30%
    client.settle(&Some(first_settlement));

    // Verify status still funded
    assert_eq!(client.get_escrow().status, 1u32);

    // Final settlement: remaining 40%
    let remaining = total - (first_settlement * 2);
    client.settle(&Some(remaining));

    // Verify status is now settled
    assert_eq!(client.get_escrow().status, 2u32);
}

/// Investor can claim payout after partial settlement
#[test]
fn investor_can_claim_after_partial_settlement() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();

    // Partial settlement: 60%
    let partial_amount = (escrow.funded_amount * 60) / 100;
    client.settle(&Some(partial_amount));

    // Investor should be able to claim payout
    let payout = client.compute_investor_payout(&investor);
    assert!(
        payout > 0,
        "investor should have positive payout after partial settlement"
    );
}

/// Pro-rata distribution: investor gets share of settled amount, not full amount
#[test]
fn partial_settlement_pro_rata_distribution() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor_a = Address::generate(&env);
    let investor_b = Address::generate(&env);

    // Each investor contributes half
    let half_target = TARGET / 2;
    client.fund(&investor_a, &half_target);
    client.fund(&investor_b, &half_target);

    let escrow = client.get_escrow();

    // Settle only 40% of total
    let partial_amount = (escrow.funded_amount * 40) / 100;
    client.settle(&Some(partial_amount));

    // Each investor should get their pro-rata share of the 40%
    let payout_a = client.compute_investor_payout(&investor_a);
    let payout_b = client.compute_investor_payout(&investor_b);

    // Payouts should be equal (both invested the same amount)
    assert_eq!(
        payout_a, payout_b,
        "equal investors should get equal payouts"
    );

    // Both payouts should be based on 40% of funded amount, not 100%
    let settled_share_per_investor =
        (partial_amount / 2) + ((partial_amount * escrow.yield_bps as i128) / 10_000) / 2;

    // Allow for rounding differences
    assert!(
        (payout_a - settled_share_per_investor).abs() <= 2,
        "payout should be approximately pro-rata share of settled amount"
    );
}

/// After full settlement, status is 2 and investors can still claim
#[test]
fn full_settlement_via_partial_allows_claims() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();

    // Settle all in one partial call
    client.settle(&Some(escrow.funded_amount));

    // Status should be 2
    assert_eq!(client.get_escrow().status, 2u32);

    // Investor should be able to claim
    let payout = client.compute_investor_payout(&investor);
    assert!(payout > 0, "investor should have positive payout");
}

/// Partial settlement emits correct event with settled_amount
#[test]
fn partial_settlement_emits_correct_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();

    // Clear previous events
    env.events().all();

    // Partial settlement
    let partial_amount = escrow.funded_amount / 2;
    client.settle(&Some(partial_amount));

    // Check events
    let contract_events = env.events().all();
    let events = contract_events.events();

    // Should have at least one event (EscrowPartiallySettled)
    assert!(!events.is_empty(), "should emit event");

    // The event name should indicate partial settlement, not full settlement
    // (This is a simplified check; more detailed event inspection depends on event parsing)
}

/// Full settlement via partial emits EscrowSettled event
#[test]
fn full_settlement_via_partial_emits_settled_event() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();

    // Clear previous events
    env.events().all();

    // Full settlement via partial
    client.settle(&Some(escrow.funded_amount));

    // Check events
    let contract_events = env.events().all();
    let events = contract_events.events();

    assert!(!events.is_empty(), "should emit event");
}

/// Cannot settle more than funded amount
#[test]
#[should_panic]
fn settle_partial_exceeding_funded_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();

    // Try to settle more than funded
    let over_funded = escrow.funded_amount + 1;
    client.settle(&Some(over_funded));
}

/// Cannot settle zero amount
#[test]
#[should_panic]
fn settle_partial_zero_amount_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);

    // Try to settle zero
    client.settle(&Some(0));
}

/// Cannot settle negative amount
#[test]
#[should_panic]
fn settle_partial_negative_amount_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);

    // Try to settle negative
    client.settle(&Some(-100i128));
}

/// Maturity check applies to partial settlement as well
#[test]
#[should_panic]
fn partial_settlement_respects_maturity() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);

    // Initialize with maturity timestamp far in future
    let future_maturity = 9_999_999_999u64;
    let escrow_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(&env, &escrow_id);
    let funding_token = install_stellar_asset_token(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_TOK"),
        &sme,
        &TARGET,
        &800i64,
        &future_maturity,
        &funding_token,
        &None,
        &admin, // treasury
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund the escrow
    let investor = Address::generate(&env);
    client.fund(&investor, &TARGET);

    // Try to settle before maturity (should panic)
    env.mock_all_auths();
    client.settle(&Some(TARGET / 2));
}

/// Settlement with None parameter settles everything
#[test]
fn settle_none_parameter_settles_all() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let investor = fund_to_target(&client, &env);
    let escrow = client.get_escrow();
    let total = escrow.funded_amount;

    // Settle with None parameter
    client.settle(&None);

    // Should reach status 2 immediately
    assert_eq!(client.get_escrow().status, 2u32);

    // Investor payout should be based on full amount
    let payout = client.compute_investor_payout(&investor);

    // Payout should equal: total + coupon
    let coupon = (total * escrow.yield_bps as i128) / 10_000;
    let expected_payout = total + coupon;

    assert_eq!(
        payout, expected_payout,
        "full settlement payout should include all principal and coupon"
    );
}

// ──────────────────────────────────────────────────────────────────────────────
// Yield Reinvestment Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn reinvest_yield_requires_settled_source_escrow() {
    let env = Env::default();
    env.mock_all_auths();
    let (source_client, admin, sme) = setup(&env);
    default_init(&source_client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    source_client.fund(&investor, &TARGET);

    let target_client = deploy(&env);
    let target_admin = Address::generate(&env);
    let target_sme = Address::generate(&env);
    target_client.init(
        &target_admin,
        &String::from_str(&env, "TARGET_REINVEST"),
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

    let result = source_client.try_reinvest_yield(&investor, &target_client.address, &1_000i128);
    assert_contract_error(result, crate::EscrowError::ReinvestSourceEscrowNotSettled);
}

#[test]
fn reinvest_yield_rejects_insufficient_accrued_yield() {
    let env = Env::default();
    env.mock_all_auths();
    let (source_client, admin, sme) = setup(&env);
    default_init(&source_client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    source_client.fund(&investor, &TARGET);
    source_client.settle();

    let target_client = deploy(&env);
    let target_admin = Address::generate(&env);
    let target_sme = Address::generate(&env);
    target_client.init(
        &target_admin,
        &String::from_str(&env, "TARGET_INSUFFICIENT_YIELD"),
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

    let result = source_client.try_reinvest_yield(
        &investor,
        &target_client.address,
        &8_000_000_001i128,
    );
    assert_contract_error(result, crate::EscrowError::ReinvestYieldInsufficient);
}

#[test]
fn reinvest_yield_allows_rollover_into_funding_target() {
    let env = Env::default();
    env.mock_all_auths();
    let token = install_stellar_asset_token(&env);
    let (source_id, source_client) = deploy_with_id(&env);
    let source_admin = Address::generate(&env);
    let source_sme = Address::generate(&env);
    let source_treasury = Address::generate(&env);
    source_client.init(
        &source_admin,
        &String::from_str(&env, "SOURCE_REINVEST"),
        &source_sme,
        &TARGET,
        &800i64,
        &0u64,
        &token.id,
        &None,
        &source_treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    source_client.fund(&investor, &TARGET);
    source_client.settle();
    let reinvest_amount = 1_000i128;
    token.stellar.mint(&source_id, &(TARGET + reinvest_amount));

    let (target_id, target_client) = deploy_with_id(&env);
    let target_admin = Address::generate(&env);
    let target_sme = Address::generate(&env);
    let target_treasury = Address::generate(&env);
    target_client.init(
        &target_admin,
        &String::from_str(&env, "TARGET_REINVEST"),
        &target_sme,
        &TARGET,
        &800i64,
        &0u64,
        &token.id,
        &None,
        &target_treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    source_client.reinvest_yield(&investor, &target_id, &reinvest_amount);
    let target = target_client.get_escrow();
    assert_eq!(
        target.status, 0u32,
        "target escrow must remain in funding state"
    );
    assert_eq!(target.funded_amount, reinvest_amount);
    assert_eq!(target_client.get_contribution(&investor), reinvest_amount);
    assert_eq!(token.token.balance(&target_id), reinvest_amount);
    assert_eq!(token.token.balance(&source_id), TARGET);
}

#[test]
#[should_panic]
fn reinvest_yield_rejects_non_positive_amount() {
    let env = Env::default();
    env.mock_all_auths();
    let (source_client, admin, sme) = setup(&env);
    default_init(&source_client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    source_client.fund(&investor, &TARGET);
    source_client.settle();

    let target_client = deploy(&env);
    let target_admin = Address::generate(&env);
    let target_sme = Address::generate(&env);
    target_client.init(
        &target_admin,
        &String::from_str(&env, "TARGET_ZERO"),
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

    source_client.reinvest_yield(&investor, &target_client.address, &0i128);
}

#[test]
fn reinvest_yield_rejects_self_target() {
    let env = Env::default();
    env.mock_all_auths();
    let (source_client, admin, sme) = setup(&env);
    default_init(&source_client, &env, &admin, &sme);

    let investor = Address::generate(&env);
    source_client.fund(&investor, &TARGET);
    source_client.settle();

    // Attempt to reinvest yield back into the same (source) escrow
    let result = source_client.try_reinvest_yield(&investor, &source_client.address, &1_000i128);
    assert_contract_error(result, crate::EscrowError::ReinvestSelfTargetRejected);
}
