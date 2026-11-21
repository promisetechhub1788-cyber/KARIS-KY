//!
//! Tests for the `clone_settled_escrow` entrypoint.
//!
//! This module covers cloning a settled escrow template to create new independent
//! escrow instances with the same configuration parameters.
//!
//! # Clone model
//!
//! `clone_settled_escrow` takes a settled escrow (status == 2) as a template and
//! creates a fresh escrow with:
//! - Cloned: admin, sme_address, yield_bps, maturity, registry, token, treasury,
//!   yield_tiers, min_contribution, max_unique_investors, max_per_investor,
//!   legal_hold_clear_delay, funding_deadline
//! - New (caller-supplied): invoice_id, amount
//! - Reset: funded_amount = 0, status = 0 (open), all per-investor state

#[cfg(test)]
use super::{default_init, deploy, free_addresses, TARGET};
use crate::LiquifactEscrow;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, String,
};

// ──────────────────────────────────────────────────────────────────────────────
// Happy path tests
// ──────────────────────────────────────────────────────────────────────────────

/// Clone a settled escrow with minimal (no optional) config into a new instance.
#[test]
fn test_clone_settled_escrow_happy_path() {
    let env = Env::default();

    // Deploy and init template escrow
    let (template_client, _, _) = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    template_client.init(
        &admin,
        &String::from_str(&env, "TEMPLATE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &template_client.funding_token(),
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Fund and settle the template
    let investor = Address::generate(&env);
    template_client.fund(&investor, &TARGET);
    template_client.settle();

    // Deploy target escrow for cloning
    let target_id = env.register(LiquifactEscrow, ());
    let target_client = super::LiquifactEscrowClient::new(&env, &target_id);

    let new_invoice_id = String::from_str(&env, "NEW_INV_001");
    let new_amount = 500_000i128;

    // Clone the settled escrow
    target_client.clone_settled_escrow(&env, &new_invoice_id, &new_amount);

    // Verify new escrow state
    let template_summary = template_client.get_escrow_summary();
    let new_summary = target_client.get_escrow_summary();

    // Check cloned fields
    assert_eq!(new_summary.escrow.admin, admin, "admin should match");
    assert_eq!(new_summary.escrow.sme_address, sme, "sme should match");
    assert_eq!(
        new_summary.escrow.yield_bps, template_summary.escrow.yield_bps,
        "yield_bps should match"
    );
    assert_eq!(
        new_summary.escrow.maturity, template_summary.escrow.maturity,
        "maturity should match"
    );

    // Check reset fields
    assert_eq!(
        new_summary.escrow.amount, new_amount,
        "amount should be new"
    );
    assert_eq!(
        new_summary.escrow.funding_target, new_amount,
        "funding_target should be new"
    );
    assert_eq!(
        new_summary.escrow.funded_amount, 0,
        "funded_amount should be 0"
    );
    assert_eq!(new_summary.escrow.status, 0, "status should be open");
    assert_eq!(
        new_summary.unique_funder_count, 0,
        "unique_funder_count should be 0"
    );
}

/// Clone fails if template escrow is not settled (status != 2).
#[test]
fn test_clone_settled_escrow_not_settled() {
    let env = Env::default();
    let (template_client, _, _) = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    template_client.init(
        &admin,
        &String::from_str(&env, "OPEN_TEMPLATE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &template_client.funding_token(),
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    // Create target but don't fund/settle template
    let target_id = env.register(LiquifactEscrow, ());
    let target_client = super::LiquifactEscrowClient::new(&env, &target_id);

    // Try to clone open (not settled) escrow - should fail with CloneNotSettled (170)
    let result = target_client.try_clone_settled_escrow(
        &env,
        &String::from_str(&env, "SHOULD_FAIL"),
        &500_000i128,
    );

    assert!(result.is_err(), "clone should fail on non-settled escrow");
}

/// Clone fails with non-positive amount.
#[test]
fn test_clone_settled_escrow_zero_amount() {
    let env = Env::default();

    // Deploy and settle template
    let (template_client, _, _) = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    template_client.init(
        &admin,
        &String::from_str(&env, "TEMPLATE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &template_client.funding_token(),
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    template_client.fund(&investor, &TARGET);
    template_client.settle();

    // Deploy target
    let target_id = env.register(LiquifactEscrow, ());
    let target_client = super::LiquifactEscrowClient::new(&env, &target_id);

    // Try with zero amount - should fail with CloneAmountNotPositive (171)
    let result =
        target_client.try_clone_settled_escrow(&env, &String::from_str(&env, "ZERO"), &0i128);

    assert!(result.is_err(), "clone should fail with zero amount");
}

/// Clone fails when template escrow has zero funding target.
#[test]
fn test_clone_settled_escrow_zero_funding_target() {
    let env = Env::default();

    let (template_client, _, _) = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    template_client.init(
        &admin,
        &String::from_str(&env, "TEMPLATE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &template_client.funding_token(),
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    template_client.fund(&investor, &TARGET);
    template_client.settle();

    // Fabricate template with zero funding target directly in storage
    env.as_contract(&template_client.address, || {
        let mut escrow = env
            .storage()
            .instance()
            .get::<crate::DataKey, crate::InvoiceEscrow>(&crate::DataKey::Escrow)
            .unwrap();
        escrow.funding_target = 0i128;
        env.storage()
            .instance()
            .set(&crate::DataKey::Escrow, &escrow);
    });

    let target_id = env.register(LiquifactEscrow, ());
    let target_client = super::LiquifactEscrowClient::new(&env, &target_id);

    let result =
        target_client.try_clone_settled_escrow(&env, &String::from_str(&env, "ZERO_TGT"), &100_000i128);

    assert!(result.is_err(), "clone should fail when template has zero funding target");
}

/// Original template escrow is not modified after clone.
#[test]
fn test_clone_settled_escrow_template_unchanged() {
    let env = Env::default();

    let (template_client, _, _) = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    template_client.init(
        &admin,
        &String::from_str(&env, "TEMPLATE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &template_client.funding_token(),
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    template_client.fund(&investor, &TARGET);
    template_client.settle();

    let template_summary_before = template_client.get_escrow_summary();

    // Create target and clone
    let target_id = env.register(LiquifactEscrow, ());
    let target_client = super::LiquifactEscrowClient::new(&env, &target_id);

    target_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONE_1"), &500_000i128);

    let template_summary_after = template_client.get_escrow_summary();

    // Verify template is identical
    assert_eq!(
        template_summary_before.escrow.invoice_id, template_summary_after.escrow.invoice_id,
        "template invoice_id should not change"
    );
    assert_eq!(
        template_summary_before.escrow.amount, template_summary_after.escrow.amount,
        "template amount should not change"
    );
    assert_eq!(
        template_summary_before.escrow.status, template_summary_after.escrow.status,
        "template status should not change"
    );
}

/// After cloning, the new escrow can be funded normally.
#[test]
fn test_clone_settled_escrow_then_fund() {
    let env = Env::default();

    let (template_client, _, _) = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    template_client.init(
        &admin,
        &String::from_str(&env, "TEMPLATE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &template_client.funding_token(),
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    template_client.fund(&investor, &TARGET);
    template_client.settle();

    // Create and clone
    let target_id = env.register(LiquifactEscrow, ());
    let target_client = super::LiquifactEscrowClient::new(&env, &target_id);

    let new_invoice_id = String::from_str(&env, "FUND_TEST");
    let new_amount = 500_000i128;

    target_client.clone_settled_escrow(&env, &new_invoice_id, &new_amount);

    // Fund the cloned escrow
    let investor2 = Address::generate(&env);
    target_client.fund(&investor2, &new_amount);

    let summary = target_client.get_escrow_summary();
    assert_eq!(
        summary.escrow.funded_amount, new_amount,
        "cloned escrow should be fundable"
    );
    assert_eq!(summary.escrow.status, 1, "status should become funded");
}

/// After cloning, the new escrow can be settled normally.
#[test]
fn test_clone_settled_escrow_then_settle() {
    let env = Env::default();

    let (template_client, _, _) = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    template_client.init(
        &admin,
        &String::from_str(&env, "TEMPLATE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &template_client.funding_token(),
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    template_client.fund(&investor, &TARGET);
    template_client.settle();

    // Create and clone
    let target_id = env.register(LiquifactEscrow, ());
    let target_client = super::LiquifactEscrowClient::new(&env, &target_id);

    let new_invoice_id = String::from_str(&env, "SETTLE_TEST");
    let new_amount = 500_000i128;

    target_client.clone_settled_escrow(&env, &new_invoice_id, &new_amount);

    // Fund and settle the cloned escrow
    let investor2 = Address::generate(&env);
    target_client.fund(&investor2, &new_amount);
    target_client.settle();

    let summary = target_client.get_escrow_summary();
    assert_eq!(
        summary.escrow.status, 2,
        "cloned escrow should be settleable"
    );
}

/// Multiple independent clones can be created from the same template.
#[test]
fn test_clone_settled_escrow_idempotent() {
    let env = Env::default();

    let (template_client, _, _) = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);

    template_client.init(
        &admin,
        &String::from_str(&env, "TEMPLATE"),
        &sme,
        &TARGET,
        &800i64,
        &0u64,
        &template_client.funding_token(),
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    template_client.fund(&investor, &TARGET);
    template_client.settle();

    // Create first clone
    let clone1_id = env.register(LiquifactEscrow, ());
    let clone1_client = super::LiquifactEscrowClient::new(&env, &clone1_id);

    clone1_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONE_1"), &500_000i128);

    // Create second clone (template still unchanged)
    let clone2_id = env.register(LiquifactEscrow, ());
    let clone2_client = super::LiquifactEscrowClient::new(&env, &clone2_id);

    clone2_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONE_2"), &750_000i128);

    // Verify both clones exist and have correct amounts
    let summary_1 = clone1_client.get_escrow_summary();
    let summary_2 = clone2_client.get_escrow_summary();

    assert_eq!(summary_1.escrow.amount, 500_000i128, "clone 1 amount");
    assert_eq!(summary_2.escrow.amount, 750_000i128, "clone 2 amount");
    assert_eq!(
        summary_1.escrow.admin, summary_2.escrow.admin,
        "both clones should have same admin"
    );

    // Template should still be settled
    let template_summary = template_client.get_escrow_summary();
    assert_eq!(template_summary.escrow.status, 2, "template still settled");
}

// ─────────────────────────────────────────────────────────────────────────────
// TEST-008: clone_settled_escrow — full parameter propagation assertions
// ─────────────────────────────────────────────────────────────────────────────
//
// Verifies that every optional configuration field present in the template
// escrow is faithfully copied to the clone, while invoice_id and amount are
// always fresh (caller-supplied). Each test isolates one or a few related fields.

/// Helper: build and settle a template escrow with the given init call, then
/// return (template_client, template_id, admin, sme, token_addr, treasury_addr).
fn settle_template<'a>(
    env: &'a Env,
    admin: &Address,
    sme: &Address,
    invoice_id: &str,
    amount: i128,
    yield_bps: i64,
    maturity: u64,
    token: &Address,
    registry: &Option<Address>,
    treasury: &Address,
    yield_tiers: &Option<soroban_sdk::Vec<crate::YieldTier>>,
    min_contribution: &Option<i128>,
    max_unique_investors: &Option<u32>,
    max_per_investor: &Option<i128>,
    legal_hold_clear_delay: &Option<u64>,
    funding_deadline: &Option<u64>,
) -> (super::LiquifactEscrowClient<'a>, Address) {
    let template_id = env.register(LiquifactEscrow, ());
    let client = super::LiquifactEscrowClient::new(env, &template_id);

    client.init(
        admin,
        &String::from_str(env, invoice_id),
        sme,
        &amount,
        &yield_bps,
        &maturity,
        token,
        registry,
        treasury,
        yield_tiers,
        min_contribution,
        max_unique_investors,
        max_per_investor,
        legal_hold_clear_delay,
        funding_deadline,
        &None, // max_funding_rate
        &None, // yield_slippage_threshold
    );

    // Fund and settle.
    let investor = Address::generate(env);
    client.fund(&investor, &amount);
    client.settle();

    (client, template_id)
}

// ── invoice_id and amount are always fresh ────────────────────────────────────

/// The clone's invoice_id must equal `new_invoice_id`, never the template's.
#[test]
fn test_clone_invoice_id_is_new() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let (template, _template_id) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLINVID1",
        100_000i128,
        800i64,
        0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "FRESHID001"), &50_000i128);

    let clone_escrow = clone_client.get_escrow();
    let template_escrow = template.get_escrow();

    assert_ne!(
        clone_escrow.invoice_id, template_escrow.invoice_id,
        "clone invoice_id must differ from template"
    );
    assert_eq!(
        clone_escrow.invoice_id,
        soroban_sdk::symbol_short!("FRESHID001"),
        "clone must use the caller-supplied invoice_id"
    );
}

/// The clone's amount (and funding_target) must equal `new_amount`, never the template's.
#[test]
fn test_clone_amount_is_new() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let template_amount = 100_000i128;
    let new_amount = 75_000i128;

    let (_template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLAMNT1",
        template_amount,
        500i64,
        0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "NEWAMT001"), &new_amount);

    let clone_escrow = clone_client.get_escrow();
    assert_eq!(
        clone_escrow.amount, new_amount,
        "clone amount must be new_amount"
    );
    assert_eq!(
        clone_escrow.funding_target, new_amount,
        "clone funding_target must equal new_amount"
    );
    assert_ne!(
        clone_escrow.amount, template_amount,
        "clone amount must differ from template"
    );
    // funded_amount is always reset to zero.
    assert_eq!(
        clone_escrow.funded_amount, 0,
        "funded_amount must be reset to 0"
    );
}

// ── Core cloned fields ────────────────────────────────────────────────────────

/// admin, sme_address, yield_bps, and maturity are all propagated from the template.
#[test]
fn test_clone_core_fields_propagated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    let maturity = 99_999u64;

    let (template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLCORE1",
        80_000i128,
        750i64,
        maturity,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONECORE1"), &40_000i128);

    let t = template.get_escrow();
    let c = clone_client.get_escrow();

    assert_eq!(c.admin, t.admin, "admin must be propagated");
    assert_eq!(
        c.sme_address, t.sme_address,
        "sme_address must be propagated"
    );
    assert_eq!(c.yield_bps, t.yield_bps, "yield_bps must be propagated");
    assert_eq!(c.maturity, t.maturity, "maturity must be propagated");
    // Status resets to open.
    assert_eq!(c.status, 0, "status must be reset to 0 (open)");
}

/// funding_token and treasury are propagated from the template.
#[test]
fn test_clone_funding_token_and_treasury_propagated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let (template, _) = settle_template(
        &env, &admin, &sme, "TMPLTOK1", 60_000i128, 400i64, 0u64, &token, &None, &treasury, &None,
        &None, &None, &None, &None, &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONETOK1"), &30_000i128);

    assert_eq!(
        clone_client.get_funding_token(),
        template.get_funding_token(),
        "funding_token must be propagated from template"
    );
    assert_eq!(
        clone_client.get_treasury(),
        template.get_treasury(),
        "treasury must be propagated from template"
    );
}

/// registry (when present) is propagated from the template.
#[test]
fn test_clone_registry_propagated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    let registry = Address::generate(&env);

    let (template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLREG1",
        90_000i128,
        300i64,
        0u64,
        &token,
        &Some(registry.clone()),
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONEREG1"), &45_000i128);

    assert_eq!(
        clone_client.get_registry_ref(),
        template.get_registry_ref(),
        "registry must be propagated from template"
    );
    assert_eq!(
        clone_client.get_registry_ref(),
        Some(registry),
        "clone registry must equal the original registry address"
    );
}

/// When template has no registry, clone also has no registry.
#[test]
fn test_clone_registry_none_not_populated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let (_template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLRNONE",
        30_000i128,
        200i64,
        0u64,
        &token,
        &None,
        &treasury, // no registry
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONERNONE"), &15_000i128);

    assert!(
        clone_client.get_registry_ref().is_none(),
        "clone must not have a registry if template did not"
    );
}

// ── yield_tiers propagation ───────────────────────────────────────────────────

/// When the template has a yield tier table, the clone inherits it.
#[test]
fn test_clone_yield_tiers_propagated() {
    use crate::YieldTier;

    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Build a 2-tier table: 30-day lock at 900bps, 90-day lock at 1200bps.
    let mut tiers = soroban_sdk::Vec::new(&env);
    tiers.push_back(YieldTier {
        min_lock_secs: 30 * 86400,
        yield_bps: 900,
    });
    tiers.push_back(YieldTier {
        min_lock_secs: 90 * 86400,
        yield_bps: 1200,
    });

    // init with base yield_bps=800 so tiers (≥ base) are valid
    let template_id = env.register(LiquifactEscrow, ());
    let template = super::LiquifactEscrowClient::new(&env, &template_id);
    template.init(
        &admin,
        &String::from_str(&env, "TMPLTIERS1"),
        &sme,
        &100_000i128,
        &800i64,
        &0u64,
        &token,
        &None,
        &treasury,
        &Some(tiers.clone()),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    template.fund(&investor, &100_000i128);
    template.settle();

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONETIERS1"), &50_000i128);

    // The cloned escrow's yield_bps should match the template's (800).
    let clone_escrow = clone_client.get_escrow();
    assert_eq!(
        clone_escrow.yield_bps, 800i64,
        "yield_bps must be propagated"
    );

    // A new investor funding with a lock should select the higher tier.
    let investor2 = Address::generate(&env);
    // fund_with_commitment selects the best tier for the given lock.
    clone_client.fund_with_commitment(&investor2, &50_000i128, &(30 * 86400u64));
    // The investor's effective yield should be at the 30-day tier (900 bps).
    let effective_yield = clone_client.get_investor_yield_bps(&investor2);
    assert_eq!(effective_yield, 900i64, "cloned tier table must be active");
}

/// When the template has no yield tiers, the clone also has none (falls back to base yield_bps).
#[test]
fn test_clone_no_yield_tiers_when_template_has_none() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let (_template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLNOTIER",
        50_000i128,
        600i64,
        0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None, // no tiers
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONENOTI1"), &25_000i128);

    // Without tiers, a fresh investor gets the base yield_bps.
    let investor2 = Address::generate(&env);
    clone_client.fund(&investor2, &25_000i128);
    let effective_yield = clone_client.get_investor_yield_bps(&investor2);
    assert_eq!(
        effective_yield, 600i64,
        "base yield_bps must apply when no tiers configured"
    );
}

// ── Cap propagation ───────────────────────────────────────────────────────────

/// max_unique_investors cap is propagated from the template.
#[test]
fn test_clone_max_unique_investors_cap_propagated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let cap: u32 = 3;
    let (template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLCAP1",
        200_000i128,
        400i64,
        0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &Some(cap),
        &None,
        &None,
        &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONECAP1"), &80_000i128);

    assert_eq!(
        clone_client.get_max_unique_investors_cap(),
        template.get_max_unique_investors_cap(),
        "max_unique_investors cap must be propagated"
    );
    assert_eq!(
        clone_client.get_max_unique_investors_cap(),
        Some(cap),
        "clone cap must equal the template cap"
    );
}

/// max_per_investor cap is propagated from the template.
#[test]
fn test_clone_max_per_investor_cap_propagated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let per_investor_cap: i128 = 20_000i128;
    let (template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLPIC1",
        100_000i128,
        350i64,
        0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &Some(per_investor_cap),
        &None,
        &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONEPIC1"), &60_000i128);

    assert_eq!(
        clone_client.get_max_per_investor_cap(),
        template.get_max_per_investor_cap(),
        "max_per_investor cap must be propagated"
    );
    assert_eq!(
        clone_client.get_max_per_investor_cap(),
        Some(per_investor_cap),
        "clone per-investor cap must match template"
    );
}

/// min_contribution_floor is propagated from the template.
#[test]
fn test_clone_min_contribution_floor_propagated() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let floor: i128 = 5_000i128;
    let (template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLFLOOR1",
        100_000i128,
        600i64,
        0u64,
        &token,
        &None,
        &treasury,
        &None,
        &Some(floor),
        &None,
        &None,
        &None,
        &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(
        &env,
        &String::from_str(&env, "CLONEFLOOR1"),
        &100_000i128, // new amount ≥ floor so init passes
    );

    assert_eq!(
        clone_client.get_min_contribution_floor(),
        template.get_min_contribution_floor(),
        "min_contribution_floor must be propagated"
    );
    assert_eq!(
        clone_client.get_min_contribution_floor(),
        floor,
        "clone floor must equal template floor"
    );
}

/// All optional caps absent in template ⇒ clone also has no caps.
#[test]
fn test_clone_caps_absent_when_template_has_none() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let (_template, _) = settle_template(
        &env,
        &admin,
        &sme,
        "TMPLNOCAPS",
        50_000i128,
        200i64,
        0u64,
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None, // no caps
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONENOCAPS"), &25_000i128);

    assert!(
        clone_client.get_max_unique_investors_cap().is_none(),
        "no cap expected"
    );
    assert!(
        clone_client.get_max_per_investor_cap().is_none(),
        "no per-investor cap expected"
    );
    assert_eq!(
        clone_client.get_min_contribution_floor(),
        0i128,
        "floor defaults to 0"
    );
}

// ── EscrowCloned event ────────────────────────────────────────────────────────

/// The EscrowCloned event emitted by clone_settled_escrow must reference the
/// template's invoice_id, the new invoice_id, admin, sme_address, yield_bps,
/// maturity, and new_amount.
#[test]
fn test_clone_emits_escrow_cloned_event() {
    use crate::EscrowCloned;
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    let (template, _template_id) = settle_template(
        &env, &admin, &sme, "TMPLEVT1", 50_000i128, 700i64, 0u64, &token, &None, &treasury, &None,
        &None, &None, &None, &None, &None,
    );

    let clone_id = env.register(LiquifactEscrow, ());
    let clone_client = super::LiquifactEscrowClient::new(&env, &clone_id);

    clone_client.clone_settled_escrow(&env, &String::from_str(&env, "CLONEEVT1"), &25_000i128);

    let template_escrow = template.get_escrow();
    let clone_escrow = clone_client.get_escrow();

    // The EscrowCloned event must appear in the event log.
    let events = env.events().all();
    let expected = EscrowCloned {
        name: soroban_sdk::symbol_short!("escrow_cl"),
        template_invoice_id: template_escrow.invoice_id.clone(),
        new_invoice_id: clone_escrow.invoice_id.clone(),
        admin: admin.clone(),
        sme_address: sme.clone(),
        yield_bps: 700i64,
        maturity: 0u64,
        new_amount: 25_000i128,
    }
    .to_xdr(&env, &clone_id);

    assert!(
        events.contains(expected),
        "EscrowCloned event must be emitted with correct fields"
    );
}
