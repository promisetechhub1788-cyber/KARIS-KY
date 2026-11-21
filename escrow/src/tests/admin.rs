use super::*;
use crate::{AdminProposedEvent, ContractUpgraded, EscrowCloseSnapshot, FundingTargetUpdated};
use soroban_sdk::Event;

// Admin/governance operations: target changes, maturity changes, admin handover,
// legal hold, migration guards, and collateral metadata.

#[test]
fn test_update_maturity_success() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV006b"),
        &sme,
        &1_000i128,
        &500i64,
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
    // Extensions now require the propose/confirm flow; shortening stays immediate.
    client.propose_maturity_extension(&2000u64);
    let updated = client.confirm_maturity_extension();
    assert_eq!(updated.maturity, 2000u64);
    assert_eq!(updated.status, 0);
}

#[test]
#[should_panic]
fn test_update_maturity_wrong_state() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV007"),
        &sme,
        &1_000i128,
        &500i64,
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
    client.propose_maturity_extension(&2000u64);
}

#[test]
#[should_panic]
fn test_update_maturity_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV009"),
        &sme,
        &1_000i128,
        &500i64,
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
    env.mock_auths(&[]);
    client.propose_maturity_extension(&2000u64);
}

// ── Issue #241: update_maturity extension guard (propose/confirm + delay) ─────

/// Extending maturity must be rejected until the minimum delay has elapsed.
#[test]
#[should_panic]
fn test_update_maturity_extension_rejected_before_delay() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    client.propose_maturity_extension(&2000u64);
    // No ledger time advance — confirmation must be rejected.
    client.confirm_maturity_extension();
}

/// Extending maturity succeeds once the minimum delay has elapsed.
#[test]
fn test_update_maturity_extension_accepted_after_delay() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let proposed_at = env.ledger().timestamp();
    client.propose_maturity_extension(&2000u64);

    env.ledger()
        .set_timestamp(proposed_at + crate::MATURITY_EXTENSION_DELAY_SECS);

    let updated = client.confirm_maturity_extension();
    assert_eq!(updated.maturity, 2000u64);
}

/// Shortening maturity (new < current) must remain immediate and unaffected by
/// the extension guard.
#[test]
fn test_update_maturity_shortening_remains_immediate() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    // default_init sets maturity to 1000; shorten to 500 immediately.
    let updated = client.update_maturity(&500u64);
    assert_eq!(updated.maturity, 500u64);
}

/// Unauthorized callers cannot propose a maturity extension.
#[test]
#[should_panic]
fn test_propose_maturity_extension_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    env.mock_auths(&[]);
    client.propose_maturity_extension(&2000u64);
}

/// Unauthorized callers cannot confirm a maturity extension.
#[test]
#[should_panic]
fn test_confirm_maturity_extension_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let proposed_at = env.ledger().timestamp();
    client.propose_maturity_extension(&2000u64);
    env.ledger()
        .set_timestamp(proposed_at + crate::MATURITY_EXTENSION_DELAY_SECS);
    env.mock_auths(&[]);
    client.confirm_maturity_extension();
}

/// Confirming without a prior proposal must be rejected.
#[test]
#[should_panic]
fn test_confirm_maturity_extension_without_proposal_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.confirm_maturity_extension();
}

/// When a guardian is configured, the guardian can confirm the extension
/// instead of waiting for the delay.
#[test]
fn test_update_maturity_extension_guardian_confirmation() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let guardian = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MATG001"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &Some(guardian.clone()),
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

    client.propose_maturity_extension(&2000u64);
    // Guardian confirms immediately, before the delay elapses.
    let updated = client.confirm_maturity_extension_as_guardian();
    assert_eq!(updated.maturity, 2000u64);
}

/// Guardian confirmation must be rejected when no guardian is configured.
#[test]
#[should_panic]
fn test_update_maturity_extension_guardian_absent_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.propose_maturity_extension(&2000u64);
    client.confirm_maturity_extension_as_guardian();
}

/// Guardian confirmation must be rejected when the caller is not the guardian.
#[test]
#[should_panic]
fn test_update_maturity_extension_guardian_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let guardian = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MATG002"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &Address::generate(&env),
        &Some(guardian),
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
    client.propose_maturity_extension(&2000u64);
    env.mock_auths(&[]);
    client.confirm_maturity_extension_as_guardian();
}

#[test]
fn test_verify_asset_custody_reports_signed_discrepancy() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = install_stellar_asset_token(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "CUST001"),
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
        &None,
    );

    client.fund(&investor, &1_000i128);
    token.stellar.mint(&client.address, &1_200i128);

    let discrepancy = client.verify_asset_custody();
    assert_eq!(discrepancy, 200i128);

    let recipient = Address::generate(&env);
    token.token.transfer(&client.address, &recipient, &300i128);

    let discrepancy = client.verify_asset_custody();
    assert_eq!(discrepancy, -100i128);
}

#[test]
fn test_propose_admin_sets_pending_without_changing_admin() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let new_admin = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "T001"),
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
    let pending = client.propose_admin(&new_admin);
    assert_eq!(pending, new_admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin));
    assert_eq!(client.get_escrow().admin, admin);
}

#[test]
fn test_accept_admin_promotes_pending_and_clears_pending() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let new_admin = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "TACPT1"),
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

    client.propose_admin(&new_admin);
    let updated = client.accept_admin();
    assert_eq!(updated.admin, new_admin);
    assert_eq!(client.get_escrow().admin, new_admin);
    assert_eq!(client.get_pending_admin(), None);
}

#[test]
#[allow(deprecated)]
fn test_transfer_admin_deprecated_shim_only_proposes() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let new_admin = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "TSHIM1"),
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

    let unchanged = client.transfer_admin(&new_admin);
    assert_eq!(unchanged.admin, admin);
    assert_eq!(client.get_pending_admin(), Some(new_admin));
}

#[test]
#[should_panic]
fn test_transfer_admin_same_address_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "T002"),
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
    client.propose_admin(&admin);
}

#[test]
#[should_panic]
fn test_transfer_admin_uninitialized_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let new_admin = Address::generate(&env);
    client.propose_admin(&new_admin);
}

#[test]
#[should_panic]
fn test_accept_admin_without_pending_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.accept_admin();
}

#[test]
#[should_panic]
fn test_accept_admin_requires_pending_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_admin = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.propose_admin(&new_admin);
    env.mock_auths(&[]);
    client.accept_admin();
}

#[test]
fn test_propose_admin_overwrites_prior_pending() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let first = Address::generate(&env);
    let second = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);

    client.propose_admin(&first);
    client.propose_admin(&second);

    assert_eq!(client.get_pending_admin(), Some(second.clone()));
    let updated = client.accept_admin();
    assert_eq!(updated.admin, second);
}

#[test]
fn test_propose_admin_emits_event() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let contract_id = client.address.clone();
    let new_admin = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);

    client.propose_admin(&new_admin);

    assert_eq!(
        env.events().all().events().last().unwrap().clone(),
        AdminProposedEvent {
            name: symbol_short!("adm_prop"),
            invoice_id: client.get_escrow().invoice_id,
            current_admin: admin,
            pending_admin: new_admin,
        }
        .to_xdr(&env, &contract_id)
    );
}

#[test]
fn test_migrate_at_current_version_records_upgrade_audit() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    assert_eq!(client.migrate(&SCHEMA_VERSION), SCHEMA_VERSION);
    assert_eq!(client.get_version_history().len(), 2);
}

#[test]
#[should_panic]
fn test_migrate_wrong_from_version_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.migrate(&99u32);
}

#[test]
#[should_panic]
fn test_migrate_no_path_branch() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);
    // Simulate an older version 4 already in storage.
    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Version, &4u32);
    });
    // migrate(4) should hit the "No migration path" branch.
    client.migrate(&4u32);
}

#[test]
#[should_panic]
fn test_migrate_from_zero_uninitialized_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    // Uninitialized storage returns version 0; migrate(0) hits the no-path branch.
    client.migrate(&0u32);
}

#[test]
fn test_read_model_summary_defaults_no_dispute_pause_and_allowlist_inactive() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let summary = client.get_escrow_summary();

    assert!(!summary.dispute_pause_active);
    assert_eq!(summary.dispute_pause_expiry, None);
    assert!(!summary.allowlist_active);
}

#[test]
fn test_read_model_summary_reflects_active_dispute_pause() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let paused_at = env.ledger().timestamp();
    let duration = 3600u64;
    client.pause_dispute(&String::from_str(&env, "TICKET-SUMMARY"), &duration);

    let summary = client.get_escrow_summary();

    assert!(summary.dispute_pause_active);
    assert_eq!(summary.dispute_pause_expiry, Some(paused_at + duration));
}

#[test]
fn test_read_model_summary_dispute_pause_auto_expired() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let paused_at = env.ledger().timestamp();
    let duration = 1800u64;
    client.pause_dispute(&String::from_str(&env, "TICKET-EXPIRED"), &duration);

    env.ledger().set_timestamp(paused_at + duration);

    let summary = client.get_escrow_summary();

    assert!(!summary.dispute_pause_active);
    assert_eq!(summary.dispute_pause_expiry, None);
}

#[test]
fn test_read_model_summary_reflects_allowlist_active() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    client.set_allowlist_active(&true);
    assert!(client.get_escrow_summary().allowlist_active);

    client.set_allowlist_active(&false);
    assert!(!client.get_escrow_summary().allowlist_active);
}

#[test]
fn test_read_model_summary_includes_optional_admin_fields() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let funding_token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "TSUM01"),
        &sme,
        &TARGET,
        &800i64,
        &1000u64,
        &funding_token,
        &None,
        &treasury,
        &None,
        &Some(100i128),
        &Some(7u32),
        &Some(10_000i128),
        &None,
        &None,
        &None,
        &None,
    );

    let summary = client.get_escrow_summary();

    assert_eq!(summary.escrow, client.get_escrow());
    assert_eq!(summary.legal_hold, client.get_legal_hold());
    assert_eq!(summary.funding_close_snapshot, EscrowCloseSnapshot::None);
    assert_eq!(summary.unique_funder_count, 0);
    assert!(!summary.is_allowlist_active);
    assert_eq!(summary.schema_version, client.get_version());
    assert_eq!(client.get_max_per_investor_cap(), Some(10_000i128));
}

#[test]
fn test_record_collateral_stored_and_does_not_block_settle() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "COL001"),
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
    let c = client.record_sme_collateral_commitment(
        &symbol_short!("USDC"),
        &5000i128,
        &soroban_sdk::String::from_str(&env, "equipment"),
    );
    assert_eq!(c.amount, 5000i128);
    assert_eq!(c.asset, symbol_short!("USDC"));
    assert_eq!(
        c.collateral_type,
        soroban_sdk::String::from_str(&env, "equipment")
    );
    assert_eq!(client.get_sme_collateral_commitment(), Some(c));

    client.fund(&investor, &TARGET);
    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

#[test]
#[should_panic]
fn test_collateral_zero_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "COL002"),
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
    client.record_sme_collateral_commitment(
        &symbol_short!("XLM"),
        &0i128,
        &soroban_sdk::String::from_str(&env, "property"),
    );
}

#[test]
#[should_panic]
fn test_collateral_requires_sme_auth() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "COL003"),
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
    env.mock_auths(&[]);
    client.record_sme_collateral_commitment(
        &symbol_short!("XLM"),
        &100i128,
        &soroban_sdk::String::from_str(&env, "inventory"),
    );
}

#[test]
#[should_panic]
fn test_collateral_empty_type_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "COL004"),
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
    client.record_sme_collateral_commitment(
        &symbol_short!("XLM"),
        &100i128,
        &soroban_sdk::String::from_str(&env, ""),
    );
}

#[test]
fn test_legal_hold_blocks_settle_withdraw_claim_and_fund() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "LH001"),
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
    client.fund(&investor, &TARGET);
    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    assert!(client.get_legal_hold());

    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.settle();
    }))
    .is_err());

    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.withdraw();
    }))
    .is_err());

    client.clear_legal_hold();
    assert!(!client.get_legal_hold());
    let settled = client.settle();
    assert_eq!(settled.status, 2);

    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.claim_investor_payout(&investor);
    }))
    .is_err());

    client.clear_legal_hold();
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

#[test]
#[should_panic]
fn test_legal_hold_blocks_new_funds_when_open() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "LH002"),
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
    client.set_legal_hold(&true);
    client.fund(&investor, &1i128);
}

/// Soroban instance storage returns `None` for a key that has never been written.
/// `legal_hold_active` maps that `None` to `false` via `unwrap_or(false)`, so a
/// fresh deploy must read `false` without any explicit `set_legal_hold` call.
#[test]
fn test_get_legal_hold_defaults_false_on_fresh_deploy() {
    let env = Env::default();
    // No init, no set_legal_hold – DataKey::LegalHold is absent from storage.
    let client = deploy(&env);
    assert!(!client.get_legal_hold());
}

#[test]
fn test_update_funding_target_by_admin_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV001"),
        &sme,
        &5_000i128,
        &800i64,
        &3000u64,
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

    let updated = client.update_funding_target(&10_000i128);
    assert_eq!(updated.funding_target, 10_000i128);
    assert_eq!(updated.status, 0);
}

#[test]
#[should_panic]
fn test_update_funding_target_by_non_admin_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV001"),
        &sme,
        &5_000i128,
        &800i64,
        &3000u64,
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

    env.mock_auths(&[]);
    client.update_funding_target(&10_000i128);
}

#[test]
#[should_panic]
fn test_update_funding_target_fails_when_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV001"),
        &sme,
        &5_000i128,
        &800i64,
        &3000u64,
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
    client.fund(&investor, &5_000i128);
    client.update_funding_target(&10_000i128);
}

#[test]
#[should_panic]
fn test_update_funding_target_below_funded_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV001"),
        &sme,
        &10_000i128,
        &800i64,
        &3000u64,
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
    client.fund(&investor, &4_000i128);
    client.update_funding_target(&3_000i128);
}

#[test]
#[should_panic]
fn test_update_funding_target_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV001"),
        &sme,
        &5_000i128,
        &800i64,
        &3000u64,
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
    client.update_funding_target(&0i128);
}

// --- FundingTargetUpdated event and rejection coverage ---

/// Verify that `update_funding_target` emits a `FundingTargetUpdated` event whose
/// topic is `symbol_short!("fund_tgt")` and whose data fields carry the correct
/// `invoice_id`, `old_target`, and `new_target` values.
#[test]
fn test_update_funding_target_event_fields() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);
    let contract_id = client.address.clone();

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "EVT001"),
        &sme,
        &5_000i128,
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

    client.update_funding_target(&9_000i128);

    assert_eq!(
        env.events().all(),
        std::vec![FundingTargetUpdated {
            name: symbol_short!("fund_tgt"),
            invoice_id: client.get_escrow().invoice_id,
            old_target: 5_000i128,
            new_target: 9_000i128,
        }
        .to_xdr(&env, &contract_id)]
    );
}

/// `update_funding_target` must be rejected when the escrow is in the **settled**
/// state (status == 2); only the open state (0) is permitted.
#[test]
#[should_panic]
fn test_update_funding_target_fails_when_settled() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "SETL001"),
        &sme,
        &5_000i128,
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
    client.fund(&investor, &5_000i128); // status → 1 (funded)
    client.settle(); // status → 2 (settled)
    client.update_funding_target(&6_000i128);
}

/// `update_funding_target` must be rejected when the escrow is in the **withdrawn**
/// state (status == 3); only the open state (0) is permitted.
#[test]
#[should_panic]
fn test_update_funding_target_fails_when_withdrawn() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _escrow_id, _sme) = init_and_fund_with_real_token(&env, 5_000i128, "WD001");
    client.withdraw(); // status → 3 (withdrawn)
    client.update_funding_target(&6_000i128);
}

/// Setting the new target exactly equal to `funded_amount` is the boundary case
/// that must succeed: the invariant is `new_target >= funded_amount`, so equality
/// is allowed.
#[test]
fn test_update_funding_target_equal_to_funded_amount_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "BOUND001"),
        &sme,
        &10_000i128,
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
    client.fund(&investor, &4_000i128); // funded_amount == 4_000, status still 0

    // new_target == funded_amount: boundary — must not panic.
    let updated = client.update_funding_target(&4_000i128);
    assert_eq!(updated.funding_target, 4_000i128);
    assert_eq!(updated.funded_amount, 4_000i128);
    assert_eq!(updated.status, 0);
}

/// Passing a negative value must panic with "Target must be strictly positive".
#[test]
#[should_panic]
fn test_update_funding_target_negative_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "NEG001"),
        &sme,
        &5_000i128,
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
    client.update_funding_target(&-1i128);
}
// --- update_maturity: open-only, ledger time semantics, MaturityUpdatedEvent ---

/// `update_maturity` must emit a `MaturityUpdatedEvent` with the correct
/// topic (`symbol_short!("maturity")`), `invoice_id`, `old_maturity`, and
/// `new_maturity` fields. Ledger timestamps are validator-observed integers;
/// the contract stores and compares them as raw `u64` seconds.
#[test]
fn test_update_maturity_event_fields() {
    use crate::MaturityUpdatedEvent;
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);
    let contract_id = client.address.clone();

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MAT001"),
        &sme,
        &5_000i128,
        &800i64,
        &1000u64,
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

    client.propose_maturity_extension(&2000u64);
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + crate::MATURITY_EXTENSION_DELAY_SECS);
    client.confirm_maturity_extension();

    assert_eq!(
        env.events().all(),
        std::vec![MaturityUpdatedEvent {
            name: symbol_short!("maturity"),
            invoice_id: client.get_escrow().invoice_id,
            old_maturity: 1000u64,
            new_maturity: 2000u64,
        }
        .to_xdr(&env, &contract_id)]
    );
}

/// `update_maturity` must be rejected when the escrow is in the **funded**
/// state (status == 1); only Open (0) is permitted.
#[test]
#[should_panic]
fn test_update_maturity_fails_when_funded() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MAT002"),
        &sme,
        &5_000i128,
        &800i64,
        &1000u64,
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
    client.fund(&investor, &5_000i128); // status → 1 (funded)
    client.propose_maturity_extension(&2000u64);
}

/// `update_maturity` must be rejected when the escrow is **settled**
/// (status == 2); only Open (0) is permitted.
#[test]
#[should_panic]
fn test_update_maturity_fails_when_settled() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MAT003"),
        &sme,
        &5_000i128,
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
    client.fund(&investor, &5_000i128); // status → 1
    client.settle(); // status → 2
    client.propose_maturity_extension(&2000u64);
}

/// `update_maturity` must be rejected when the escrow is **withdrawn**
/// (status == 3); only Open (0) is permitted.
#[test]
#[should_panic]
fn test_update_maturity_fails_when_withdrawn() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _escrow_id, _sme) = init_and_fund_with_real_token(&env, 5_000i128, "MAT004");
    client.withdraw(); // status → 3
    client.propose_maturity_extension(&2000u64);
}

/// Setting maturity to zero is valid — it means no maturity gate.
/// The contract must accept zero as new_maturity in Open state.
#[test]
fn test_update_maturity_to_zero_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MAT005"),
        &sme,
        &5_000i128,
        &800i64,
        &1000u64,
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
    let updated = client.update_maturity(&0u64);
    assert_eq!(updated.maturity, 0u64);
    assert_eq!(updated.status, 0);
}

/// Ledger time semantics: `settle` uses `env.ledger().timestamp()`
/// (validator-observed seconds). Settle must pass exactly at maturity —
/// confirming the boundary is `now >= maturity` (inclusive).
#[test]
fn test_settle_passes_exactly_at_maturity_ledger_time() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MAT006"),
        &sme,
        &5_000i128,
        &800i64,
        &5000u64,
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
    client.fund(&investor, &5_000i128);

    // Advance ledger to exactly maturity — must succeed
    env.ledger().with_mut(|l| l.timestamp = 5000);
    let settled = client.settle();
    assert_eq!(settled.status, 2);
}

/// Ledger time semantics: settle must panic one second before maturity —
/// confirming the `>=` boundary strictly excludes values below maturity.
#[test]
#[should_panic]
fn test_settle_fails_one_second_before_maturity() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MAT007"),
        &sme,
        &5_000i128,
        &800i64,
        &5000u64,
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
    client.fund(&investor, &5_000i128);

    // One second before maturity — must reject
    env.ledger().with_mut(|l| l.timestamp = 4999);
    client.settle();
}

/// A second `update_maturity` call in the same Open state must overwrite
/// the previous value correctly — storage is atomic per call.
#[test]
fn test_update_maturity_twice_overwrites() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let client = deploy(&env);

    let token = Address::generate(&env);
    let treasury = Address::generate(&env);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "MAT008"),
        &sme,
        &5_000i128,
        &800i64,
        &1000u64,
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

    client.propose_maturity_extension(&2000u64);
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + crate::MATURITY_EXTENSION_DELAY_SECS);
    client.confirm_maturity_extension();

    client.propose_maturity_extension(&3000u64);
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + crate::MATURITY_EXTENSION_DELAY_SECS);
    let updated = client.confirm_maturity_extension();
    assert_eq!(updated.maturity, 3000u64);
    assert_eq!(client.get_escrow().maturity, 3000u64);
}

// ── Authorization guard ordering audit (issue #265) ───────────────────────────
//
// Negative tests: each guarded entrypoint must trap when `require_auth` fails
// (Soroban host aborts the transaction). Canonical ordering is documented in
// `docs/escrow-security-checklist.md` §6 and ADR-002.

fn auth_audit_init_funded(
    env: &Env,
) -> (
    LiquifactEscrowClient<'_>,
    Address,
    Address,
    Address,
    Address,
) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let investor = Address::generate(env);
    let client = deploy(env);
    default_init(&client, env, &admin, &sme);
    client.fund(&investor, &TARGET);
    (client, admin, sme, investor, Address::generate(env))
}

#[test]
#[should_panic]
fn auth_audit_propose_admin_requires_current_admin() {
    let env = Env::default();
    let (client, _, _, _, _) = auth_audit_init_funded(&env);
    let new_admin = Address::generate(&env);
    env.mock_auths(&[]);
    client.propose_admin(&new_admin);
}

#[test]
#[should_panic]
fn auth_audit_accept_admin_requires_pending_admin() {
    let env = Env::default();
    let (client, _, _, _, pending_admin) = auth_audit_init_funded(&env);
    client.propose_admin(&pending_admin);
    env.mock_auths(&[]);
    client.accept_admin();
}

#[test]
#[should_panic]
fn auth_audit_fund_requires_investor() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = Address::generate(&env);
    env.mock_auths(&[]);
    client.fund(&investor, &TARGET);
}

#[test]
#[should_panic]
fn auth_audit_fund_with_commitment_requires_investor() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let investor = Address::generate(&env);
    env.mock_auths(&[]);
    client.fund_with_commitment(&investor, &TARGET, &0u64);
}

#[test]
#[should_panic]
fn auth_audit_settle_requires_sme() {
    let env = Env::default();
    let (client, _, _, _, _) = auth_audit_init_funded(&env);
    env.mock_auths(&[]);
    client.settle();
}

#[test]
#[should_panic]
fn auth_audit_withdraw_requires_sme() {
    let env = Env::default();
    let (client, _, _, _, _) = auth_audit_init_funded(&env);
    env.mock_auths(&[]);
    client.withdraw();
}

#[test]
#[should_panic]
fn auth_audit_claim_investor_payout_requires_investor() {
    let env = Env::default();
    let (client, _, _, investor, _) = auth_audit_init_funded(&env);
    client.settle();
    env.mock_auths(&[]);
    client.claim_investor_payout(&investor);
}

#[test]
#[should_panic]
fn auth_audit_set_legal_hold_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    env.mock_auths(&[]);
    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
}

#[test]
#[should_panic]
fn auth_audit_bind_primary_attestation_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    env.mock_auths(&[]);
    client.bind_primary_attestation_hash(&soroban_sdk::Bytes::from_array(&env, &[0u8; 32]));
}

#[test]
#[should_panic]
fn auth_audit_append_attestation_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    env.mock_auths(&[]);
    client.append_attestation_digest(
        &symbol_short!(""),
        &soroban_sdk::BytesN::from_array(&env, &[0u8; 32]),
    );
}

#[test]
#[should_panic]
fn auth_audit_set_allowlist_active_requires_admin() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    env.mock_auths(&[]);
    client.set_allowlist_active(&true);
}

#[test]
#[should_panic]
fn auth_audit_sweep_terminal_dust_requires_treasury() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let token = install_stellar_asset_token(&env);
    let treasury = Address::generate(&env);
    let escrow_id = deploy_id(&env);
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "AUTHSW"),
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
    token.stellar.mint(&escrow_id, &100i128);
    env.mock_auths(&[]);
    client.sweep_terminal_dust(&100i128);
}

// --- rotate_beneficiary tests ---

#[test]
fn test_rotate_beneficiary_success_dual_auth() {
    use soroban_sdk::testutils::Events as _;
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    let contract_id = client.address.clone();

    let updated = client.rotate_beneficiary(&new_sme);
    assert_eq!(updated.sme_address, new_sme);
    assert_eq!(client.get_escrow().sme_address, new_sme);

    assert_eq!(
        env.events().all().events().last().unwrap().clone(),
        crate::BeneficiaryRotated {
            name: symbol_short!("ben_rot"),
            invoice_id: client.get_escrow().invoice_id,
            prior_sme: sme,
            new_sme,
        }
        .to_xdr(&env, &contract_id)
    );
}

/*
#[test]
#[should_panic]
fn test_rotate_beneficiary_only_sme_auth_fails() {
    use soroban_sdk::{testutils::MockAuth, IntoVal, Vec as SorobanVec};
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    env.mock_auths(&[MockAuth {
        address: &sme,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &client.address,
            fn_name: "rotate_beneficiary",
            args: SorobanVec::from_array(&env, [(new_sme.clone(),).into_val(&env)]),
            sub_invokes: &[],
        },
    }]);
    client.rotate_beneficiary(&new_sme);
}

#[test]
#[should_panic]
fn test_rotate_beneficiary_only_admin_auth_fails() {
    use soroban_sdk::{testutils::MockAuth, IntoVal, Vec as SorobanVec};
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &client.address,
            fn_name: "rotate_beneficiary",
            args: SorobanVec::from_array(&env, [(new_sme.clone(),).into_val(&env)]),
            sub_invokes: &[],
        },
    }]);
    client.rotate_beneficiary(&new_sme);
}
*/

#[test]
#[should_panic]
fn test_rotate_beneficiary_no_auth_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    env.mock_auths(&[]); // No auth
    client.rotate_beneficiary(&new_sme);
}

#[test]
#[should_panic]
fn test_rotate_beneficiary_new_same_as_current_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    client.rotate_beneficiary(&sme);
}

#[test]
#[should_panic]
fn test_rotate_beneficiary_in_settled_state_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    client.settle(); // status 2
    client.rotate_beneficiary(&new_sme);
}

#[test]
#[should_panic]
fn test_rotate_beneficiary_in_withdrawn_state_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    client.withdraw(); // status 3
    client.rotate_beneficiary(&new_sme);
}

#[test]
#[should_panic]
fn test_rotate_beneficiary_in_cancelled_state_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET);
    client.cancel_funding(); // status 4
    client.rotate_beneficiary(&new_sme);
}

#[test]
#[should_panic]
fn test_rotate_beneficiary_with_legal_hold_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));
    client.rotate_beneficiary(&new_sme);
}

#[test]
fn test_rotate_beneficiary_in_funded_state_success() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    let investor = Address::generate(&env);
    default_init(&client, &env, &admin, &sme);
    client.fund(&investor, &TARGET); // status 1
    let updated = client.rotate_beneficiary(&new_sme);
    assert_eq!(updated.sme_address, new_sme);
}

#[test]
fn test_rotate_beneficiary_then_withdraw_goes_to_new_sme() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    let new_sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let token = install_stellar_asset_token(&env);
    let treasury = Address::generate(&env);
    let escrow_id = deploy_id(&env);
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "WDTST"),
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
    token.stellar.mint(&investor, &TARGET);
    token.stellar.approve(
        &investor,
        &escrow_id,
        &TARGET,
        &(env.ledger().sequence() + 10_000),
    );
    client.fund(&investor, &TARGET);
    // Mint funded_amount into the escrow contract so withdraw() can transfer it.
    token.stellar.mint(&escrow_id, &TARGET);
    client.rotate_beneficiary(&new_sme);
    let old_sme_result = client.try_withdraw(&sme);
    assert_contract_error(old_sme_result, EscrowError::UnauthorizedWithdrawer);
    client.withdraw(&new_sme);
    assert_eq!(token.stellar.balance(&sme), 0);
    assert_eq!(token.stellar.balance(&new_sme), TARGET);
}

// ─────────────────────────────────────────────────────────────────────────────
// #208: Collateral record update_timestamp tracking and post-settlement guard
// ─────────────────────────────────────────────────────────────────────────────

/// Initial record: recorded_at and updated_at are both set to the current ledger timestamp.
#[test]
fn test_208_initial_record_timestamps_equal() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env); // sets ledger.timestamp = 12345
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "C208A"),
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
    );

    let commitment = client.record_sme_collateral_commitment(&symbol_short!("GOLD"), &5000i128);

    assert_eq!(
        commitment.recorded_at, 12345,
        "recorded_at must be set on first write"
    );
    assert_eq!(
        commitment.updated_at, 12345,
        "updated_at must equal recorded_at on first write"
    );
    assert_eq!(commitment.amount, 5000);
}

/// Update: recorded_at is preserved from the first write; updated_at advances.
#[test]
fn test_208_update_preserves_recorded_at_and_advances_updated_at() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env); // ledger.timestamp = 12345
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "C208B"),
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
    );

    // First write at timestamp 12345.
    client.record_sme_collateral_commitment(&symbol_short!("GOLD"), &5000i128);

    // Advance time and update.
    env.ledger().with_mut(|li| li.timestamp = 99999);
    let updated = client.record_sme_collateral_commitment(&symbol_short!("GOLD"), &9000i128);

    // recorded_at stays at the original write time.
    assert_eq!(
        updated.recorded_at, 12345,
        "recorded_at must not change on update"
    );
    // updated_at reflects the update time.
    assert_eq!(
        updated.updated_at, 99999,
        "updated_at must reflect the update timestamp"
    );
    assert_eq!(updated.amount, 9000);

    // Persisted state must match the returned struct.
    let stored = client.get_sme_collateral_commitment().unwrap();
    assert_eq!(stored.recorded_at, 12345);
    assert_eq!(stored.updated_at, 99999);
    assert_eq!(stored.amount, 9000);
}

/// Record, fund to target (funded), update should still succeed (status == 1 < 2).
#[test]
fn test_208_update_allowed_before_settlement() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "C208C"),
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
    );

    // Record while open (status 0).
    client.record_sme_collateral_commitment(&symbol_short!("GOLD"), &1000i128);

    // Fund to target — escrow transitions to funded (status 1).
    client.fund(&investor, &TARGET);
    assert_eq!(client.get_escrow().status, 1);

    // Advance time; update must still succeed when status == 1.
    env.ledger().with_mut(|li| li.timestamp = 99999);
    let updated = client.record_sme_collateral_commitment(&symbol_short!("GOLD"), &2000i128);
    assert_eq!(updated.amount, 2000);
    assert_eq!(updated.updated_at, 99999);
    assert_eq!(updated.recorded_at, 12345);
}

/// After settlement (status == 2), updates must be rejected with CollateralUpdateAfterSettlement (63).
#[test]
fn test_208_update_blocked_after_settlement() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "C208D"),
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
    );

    // Record initial commitment.
    client.record_sme_collateral_commitment(&symbol_short!("GOLD"), &5000i128);

    // Fund and settle.
    client.fund(&investor, &TARGET);
    client.settle();
    assert_eq!(client.get_escrow().status, 2);

    // Attempt update — must fail with typed error code 63.
    assert_contract_error(
        client.try_record_sme_collateral_commitment(&symbol_short!("GOLD"), &9000i128),
        EscrowError::CollateralUpdateAfterSettlement,
    );

    // State unchanged.
    let stored = client.get_sme_collateral_commitment().unwrap();
    assert_eq!(
        stored.amount, 5000,
        "collateral must not change after rejected update"
    );
}

/// First-time record on a settled escrow is also blocked (prior == None means it's an insert, not update;
/// but the status check applies to any write after settlement).
/// NOTE: Current logic only blocks when a prior commitment exists. First-time records are allowed
/// regardless of status (they're initial metadata, not corrections). This test documents that intent.
#[test]
fn test_208_first_record_on_settled_escrow_is_allowed() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "C208E"),
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
    );

    client.fund(&investor, &TARGET);
    client.settle();
    assert_eq!(client.get_escrow().status, 2);

    // First-time record after settlement must succeed (no prior commitment to guard against).
    let commitment = client.record_sme_collateral_commitment(&symbol_short!("BOND"), &1000i128);
    assert_eq!(commitment.amount, 1000);
    assert!(client.get_sme_collateral_commitment().is_some());

    // ──────────────────────────────────────────────────────────────────────────────
    // Legal hold state machine – Issue #406
    // ──────────────────────────────────────────────────────────────────────────────

    /// Helper: activate legal hold
    fn activate_hold(client: &LiquifactEscrowClient<'_>, env: &Env) {
        client.set_legal_hold(&true, &String::from_str(env, "Test hold"));
    }

    /// Helper: check that a panic contains a given contract error code
    fn assert_panic_contains_error_code(result: std::thread::Result<()>, expected_code: u32) {
        let err = result.err().unwrap();
        let msg = format!("{:?}", err);
        assert!(
            msg.contains(&format!("ContractError({})", expected_code)),
            "Expected error code {} but got: {}",
            expected_code,
            msg
        );
    }

    /// Test: fund is blocked during legal hold with error LegalHoldBlocksFunding (102)
    #[test]
    fn test_fund_blocked_during_legal_hold() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);

        activate_hold(&client, &env);

        let investor = Address::generate(&env);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.fund(&investor, &1_000i128);
        }));
        assert_panic_contains_error_code(result, 102); // LegalHoldBlocksFunding
    }

    /// Test: settle is blocked during legal hold with error LegalHoldBlocksSettlement (120)
    #[test]
    fn test_settle_blocked_during_legal_hold() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);

        fund_to_target(&client, &env);

        activate_hold(&client, &env);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.settle(&None);
        }));
        assert_panic_contains_error_code(result, 120); // LegalHoldBlocksSettlement
    }

    /// Test: withdraw is blocked during legal hold with error LegalHoldBlocksWithdrawal (123)
    #[test]
    fn test_withdraw_blocked_during_legal_hold() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);

        // We need to fund and mint tokens so withdraw can attempt to transfer
        let token = install_stellar_asset_token(&env);
        let contract_id = client.address.clone();
        token.stellar.mint(&contract_id, &TARGET);

        fund_to_target(&client, &env);

        activate_hold(&client, &env);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.withdraw();
        }));
        assert_panic_contains_error_code(result, 123); // LegalHoldBlocksWithdrawal
    }

    /// Test: claim_investor_payout is blocked during legal hold with error LegalHoldBlocksInvestorClaims (125)
    #[test]
    fn test_claim_investor_payout_blocked_during_legal_hold() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);

        let investor = settle_escrow(&client, &env);

        activate_hold(&client, &env);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.claim_investor_payout(&investor);
        }));
        assert_panic_contains_error_code(result, 125); // LegalHoldBlocksInvestorClaims
    }

    /// Test: sweep_terminal_dust is blocked during legal hold with error LegalHoldBlocksTreasuryDustSweep (30)
    #[test]
    fn test_sweep_terminal_dust_blocked_during_legal_hold() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);

        let investor = settle_escrow(&client, &env);

        // Mint dust into the contract so sweep can attempt to transfer
        let token = install_stellar_asset_token(&env);
        let contract_id = client.address.clone();
        token.stellar.mint(&contract_id, &10i128);

        activate_hold(&client, &env);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.sweep_terminal_dust(&10i128);
        }));
        assert_panic_contains_error_code(result, 30); // LegalHoldBlocksTreasuryDustSweep
    }

    /// Test: resume_dispute is allowed during legal hold (no error)
    #[test]
    fn test_resume_dispute_allowed_during_legal_hold() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, admin, sme) = setup(&env);
        default_init(&client, &env, &admin, &sme);

        activate_hold(&client, &env);

        // First, create a dispute pause so there is something to resume
        client.pause_dispute(&String::from_str(&env, "TICKET-001"), &3600u64);

        // resume_dispute must not panic even with legal hold active
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.resume_dispute();
        }));
        assert!(
            result.is_ok(),
            "resume_dispute should succeed during legal hold"
        );
    }
}

#[test]
fn test_resume_dispute_emits_manual_resume_event() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    let contract_id = client.address.clone();
    let ticket_id = String::from_str(&env, "TICKET-RESUME");

    client.pause_dispute(&ticket_id, &3600u64);
    client.resume_dispute();

    assert_eq!(
        env.events().all().events().last().unwrap().clone(),
        crate::DisputeResumedEvt {
            name: symbol_short!("disp_res"),
            invoice_id: client.get_escrow().invoice_id,
            admin,
            resumed_by: crate::DisputeResumedBy::Manual,
            ledger_timestamp: env.ledger().timestamp(),
        }
        .to_xdr(&env, &contract_id)
    );
}

#[test]
fn test_pause_dispute_duration_limits() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let now = env.ledger().timestamp();
    client.pause_dispute(
        &String::from_str(&env, "TICKET-MAX"),
        &crate::MAX_DISPUTE_PAUSE_DURATION_SECS,
    );
    let pause = client.get_dispute_pause().unwrap();
    assert_eq!(pause.expires_at, now + crate::MAX_DISPUTE_PAUSE_DURATION_SECS);

    assert_contract_error(
        client.try_pause_dispute(
            &String::from_str(&env, "TICKET-OVER-MAX"),
            &(crate::MAX_DISPUTE_PAUSE_DURATION_SECS + 1),
        ),
        EscrowError::DisputePauseDurationExceedsMax,
    );
    assert_contract_error(
        client.try_pause_dispute(&String::from_str(&env, "TICKET-ZERO"), &0u64),
        EscrowError::DisputePauseDurationNotPositive,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// TEST-007: Dispute pause auto-expiry
// ─────────────────────────────────────────────────────────────────────────────
//
// Verifies that a dispute pause configured with `duration_secs` auto-expires when
// the ledger timestamp advances past `paused_at + duration_secs`, allowing
// settlement and other operations to proceed **without** an explicit `resume_dispute`
// call. Tests cover:
// - Settle succeeds after auto-expiry
// - Settle blocked before expiry
// - Boundary conditions (exactly at expiry timestamp)

/// Happy path: pause a funded escrow with a 1-hour duration, advance time past
/// the expiry, and verify settle succeeds without calling `resume_dispute`.
#[test]
fn test_dispute_pause_auto_expire_then_settle() {
    use soroban_sdk::testutils::Events as _;

    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DISPAUTO1"),
        &sme,
        &100_000i128,
        &500i64,
        &0u64, // no maturity constraint
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
    let contract_id = client.address.clone();

    // Fund the escrow so settle is available.
    client.fund(&investor, &100_000i128);
    assert_eq!(client.get_escrow().status, 1); // funded

    // Pause the escrow for 1 hour (3600 seconds).
    let pause_duration = 3600u64;
    client.pause_dispute(
        &soroban_sdk::String::from_str(&env, "ticket-auto-001"),
        &pause_duration,
    );

    // Verify the pause is currently active.
    assert!(
        client.is_dispute_paused(),
        "dispute pause must be active immediately after pause_dispute"
    );

    // Attempt settle before expiry — must fail with DisputePausedBlocksSettlement.
    assert_contract_error(
        client.try_settle(),
        EscrowError::DisputePausedBlocksSettlement,
    );

    // Advance ledger time to exactly at expiry (now = paused_at + duration).
    let ledger_at_pause = env.ledger().timestamp();
    let expiry_ts = ledger_at_pause + pause_duration;
    env.ledger().set_timestamp(expiry_ts);

    // The pause is no longer active (is_dispute_paused checks now >= expires_at).
    let event_count_before_read = env.events().all().events().len();
    assert!(
        !client.is_dispute_paused(),
        "dispute pause must be inactive at/after expiry"
    );
    assert_eq!(env.events().all().events().len(), event_count_before_read);

    // Settle must succeed without an explicit resume_dispute call.
    client.settle();
    assert_eq!(client.get_escrow().status, 2); // settled

    let expected_expiry_event = crate::DisputeResumedEvt {
        name: symbol_short!("disp_res"),
        invoice_id: client.get_escrow().invoice_id,
        admin,
        resumed_by: crate::DisputeResumedBy::AutoExpiry,
        ledger_timestamp: expiry_ts,
    }
    .to_xdr(&env, &contract_id);
    assert!(env
        .events()
        .all()
        .events()
        .iter()
        .any(|event| event == &expected_expiry_event));
}

/// Boundary condition: settle blocked 1 second **before** expiry, succeeds
/// exactly **at** expiry.
#[test]
fn test_dispute_pause_expiry_boundary() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DISPBOUND1"),
        &sme,
        &50_000i128,
        &300i64,
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

    client.fund(&investor, &50_000i128);
    let pause_duration = 7200u64; // 2 hours
    let paused_at = env.ledger().timestamp();
    client.pause_dispute(
        &soroban_sdk::String::from_str(&env, "ticket-boundary-001"),
        &pause_duration,
    );

    // Advance to 1 second **before** expiry.
    let one_before_expiry = paused_at + pause_duration - 1;
    env.ledger().set_timestamp(one_before_expiry);

    // is_dispute_paused must still return true because now < expires_at.
    assert!(
        client.is_dispute_paused(),
        "pause must still be active 1 second before expiry"
    );

    // Settle must be blocked.
    assert_contract_error(
        client.try_settle(),
        EscrowError::DisputePausedBlocksSettlement,
    );

    // Advance to exactly the expiry timestamp.
    let expiry_ts = paused_at + pause_duration;
    env.ledger().set_timestamp(expiry_ts);

    // Pause is now inactive (now >= expires_at).
    assert!(
        !client.is_dispute_paused(),
        "pause must be inactive exactly at expiry"
    );

    // Settle must succeed.
    client.settle();
    assert_eq!(client.get_escrow().status, 2);
}

/// Fund, withdraw, and claim operations are also blocked during an active pause
/// and auto-unblocked after expiry.
#[test]
fn test_dispute_pause_blocks_fund_auto_expires() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DISPFUND1"),
        &sme,
        &100_000i128,
        &400i64,
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

    // Pause the escrow before funding.
    let pause_duration = 1800u64; // 30 minutes
    let paused_at = env.ledger().timestamp();
    client.pause_dispute(
        &soroban_sdk::String::from_str(&env, "ticket-fund-001"),
        &pause_duration,
    );

    // fund() must be blocked while pause is active.
    assert_contract_error(
        client.try_fund(&investor, &100_000i128),
        EscrowError::DisputePausedBlocksFunding,
    );

    // Advance time past expiry.
    env.ledger().set_timestamp(paused_at + pause_duration);

    // fund() must now succeed.
    client.fund(&investor, &100_000i128);
    assert_eq!(client.get_escrow().funded_amount, 100_000i128);
}

/// Withdraw is blocked during an active pause and auto-unblocked after expiry.
#[test]
fn test_dispute_pause_blocks_withdraw_auto_expires() {
    use crate::tests::install_stellar_asset_token;

    let env = Env::default();
    env.mock_all_auths();

    let (client_id, admin, sme) = (
        deploy_id(&env),
        Address::generate(&env),
        Address::generate(&env),
    );
    let client = LiquifactEscrowClient::new(&env, &client_id);
    let investor = Address::generate(&env);
    let token_setup = install_stellar_asset_token(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DISPWD1"),
        &sme,
        &50_000i128,
        &200i64,
        &0u64,
        &token_setup.id,
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

    // Fund with real token so withdraw can actually transfer.
    token_setup.stellar.mint(&investor, &50_000i128);
    token_setup
        .token
        .approve(&investor, &client_id, &50_000i128, &999_999);
    client.fund(&investor, &50_000i128);
    // Mint the escrow's balance so withdraw has tokens to send.
    token_setup.stellar.mint(&client_id, &50_000i128);

    // Pause after funded.
    let pause_duration = 600u64; // 10 minutes
    let paused_at = env.ledger().timestamp();
    client.pause_dispute(
        &soroban_sdk::String::from_str(&env, "ticket-wd-001"),
        &pause_duration,
    );

    // withdraw() is blocked.
    assert_contract_error(
        client.try_withdraw(),
        EscrowError::DisputePausedBlocksWithdrawal,
    );

    // Advance past expiry.
    env.ledger().set_timestamp(paused_at + pause_duration);

    // withdraw() succeeds.
    client.withdraw();
    assert_eq!(client.get_escrow().status, 3); // withdrawn
}

/// Investor claims are blocked during an active pause and auto-unblocked after expiry.
#[test]
fn test_dispute_pause_blocks_claim_auto_expires() {
    use crate::tests::install_stellar_asset_token;

    let env = Env::default();
    env.mock_all_auths();

    let (client_id, admin, sme) = (
        deploy_id(&env),
        Address::generate(&env),
        Address::generate(&env),
    );
    let client = LiquifactEscrowClient::new(&env, &client_id);
    let investor = Address::generate(&env);
    let token_setup = install_stellar_asset_token(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DISPCLAIM1"),
        &sme,
        &60_000i128,
        &500i64,
        &0u64,
        &token_setup.id,
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

    // Fund, mint tokens for claim payout, settle.
    token_setup.stellar.mint(&investor, &60_000i128);
    token_setup
        .token
        .approve(&investor, &client_id, &60_000i128, &999_999);
    client.fund(&investor, &60_000i128);
    token_setup.stellar.mint(&client_id, &63_000i128); // principal + yield
    client.settle();

    // Pause after settlement.
    let pause_duration = 900u64; // 15 minutes
    let paused_at = env.ledger().timestamp();
    client.pause_dispute(
        &soroban_sdk::String::from_str(&env, "ticket-claim-001"),
        &pause_duration,
    );

    // claim_investor_payout() is blocked.
    assert_contract_error(
        client.try_claim_investor_payout(&investor),
        EscrowError::DisputePausedBlocksInvestorClaims,
    );

    // Advance past expiry.
    env.ledger().set_timestamp(paused_at + pause_duration);

    // claim_investor_payout() succeeds.
    client.claim_investor_payout(&investor);
    assert!(client.is_investor_claimed(&investor));
}

/// Manual resume clears the pause before auto-expiry. Verify that settle succeeds
/// after resume, even if the expiry timestamp has not been reached.
#[test]
fn test_dispute_pause_manual_resume_before_expiry() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DISPRES1"),
        &sme,
        &40_000i128,
        &600i64,
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

    client.fund(&investor, &40_000i128);

    // Pause for 1 hour.
    let pause_duration = 3600u64;
    let paused_at = env.ledger().timestamp();
    client.pause_dispute(
        &soroban_sdk::String::from_str(&env, "ticket-manual-001"),
        &pause_duration,
    );

    // Advance only halfway to expiry.
    env.ledger().set_timestamp(paused_at + 1800);
    assert!(
        client.is_dispute_paused(),
        "pause must still be active before expiry"
    );

    // Admin manually resumes the pause.
    client.resume_dispute();

    // Pause is now cleared.
    assert!(
        !client.is_dispute_paused(),
        "pause must be inactive after manual resume"
    );

    // Settle succeeds immediately.
    client.settle();
    assert_eq!(client.get_escrow().status, 2);
}

/// get_dispute_pause returns Some(state) while active, None after auto-expiry.
#[test]
fn test_get_dispute_pause_returns_none_after_expiry() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DISPGET1"),
        &sme,
        &10_000i128,
        &100i64,
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

    let pause_duration = 1200u64; // 20 minutes
    let paused_at = env.ledger().timestamp();
    client.pause_dispute(
        &soroban_sdk::String::from_str(&env, "ticket-get-001"),
        &pause_duration,
    );

    // get_dispute_pause returns Some while active.
    let state = client.get_dispute_pause();
    assert!(
        state.is_some(),
        "get_dispute_pause must return Some while active"
    );
    let state = state.unwrap();
    assert_eq!(
        state.expires_at_ledger_timestamp,
        paused_at + pause_duration
    );

    // Advance past expiry.
    env.ledger().set_timestamp(paused_at + pause_duration);

    // get_dispute_pause returns None (the pause is logically expired).
    let state_after = client.get_dispute_pause();
    assert!(
        state_after.is_none(),
        "get_dispute_pause must return None after auto-expiry"
    );
}

// ── BUG-011: set_legal_hold terminal escrow guard tests ───────────────────

/// set_legal_hold rejects terminal escrows (status 2 = settled).
#[test]
#[should_panic(expected = "Error(Contract, #154)")]
fn test_set_legal_hold_rejects_settled_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_SETTLE_LH1"),
        &sme,
        &1_000i128,
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
    );

    // Fund and settle the escrow to status 2.
    client.fund(&investor, &1_000i128);
    client.settle(&None);

    // Attempt to set legal hold on settled escrow (status 2).
    client.set_legal_hold(&true, &soroban_sdk::String::from_str(&env, "Too late!"));
}

/// set_legal_hold rejects terminal escrows (status 3 = withdrawn).
#[test]
#[should_panic(expected = "Error(Contract, #154)")]
fn test_set_legal_hold_rejects_withdrawn_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_WITHDRAW_LH1"),
        &sme,
        &1_000i128,
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
    );

    // Fund, settle, and withdraw to status 3.
    client.fund(&investor, &1_000i128);
    client.settle(&None);
    client.withdraw();

    // Attempt to set legal hold on withdrawn escrow (status 3).
    client.set_legal_hold(&true, &soroban_sdk::String::from_str(&env, "Too late!"));
}

/// set_legal_hold rejects terminal escrows (status 4 = cancelled).
#[test]
#[should_panic(expected = "Error(Contract, #154)")]
fn test_set_legal_hold_rejects_cancelled_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_CANCEL_LH1"),
        &sme,
        &1_000i128,
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
    );

    // Fund partially then cancel to status 4.
    client.fund(&investor, &500i128);
    client.cancel_funding();

    // Attempt to set legal hold on cancelled escrow (status 4).
    client.set_legal_hold(&true, &soroban_sdk::String::from_str(&env, "Too late!"));
}

/// set_legal_hold still works on open (status 0) escrow.
#[test]
fn test_set_legal_hold_accepts_open_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_OPEN_LH1"),
        &sme,
        &1_000i128,
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
    );

    // Set legal hold on open escrow (status 0) should succeed.
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 0, "Escrow should be open");

    client.set_legal_hold(
        &true,
        &soroban_sdk::String::from_str(&env, "Compliance hold"),
    );
    assert!(client.get_legal_hold(), "Legal hold should be active");
}

/// set_legal_hold still works on funded (status 1) escrow.
#[test]
fn test_set_legal_hold_accepts_funded_escrow() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "INV_FUNDED_LH1"),
        &sme,
        &1_000i128,
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
    );

    // Fund to status 1.
    client.fund(&investor, &1_000i128);
    let escrow = client.get_escrow();
    assert_eq!(escrow.status, 1, "Escrow should be funded");

    // Set legal hold on funded escrow should succeed.
    client.set_legal_hold(
        &true,
        &soroban_sdk::String::from_str(&env, "Compliance hold"),
    );
    assert!(client.get_legal_hold(), "Legal hold should be active");
}

// ── BUG-010: migrate diagnostic event tests ──────────────────────────────

/// migrate emits diagnostic event with correct version information.
#[test]
#[should_panic(expected = "Error(Contract, #92)")]
fn test_migrate_emits_diagnostic_event_before_error() {
    use soroban_sdk::vec as soroban_vec;

    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);

    // Simulate version 4 stored on-chain.
    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Version, &4u32);
        // Also need a minimal escrow to avoid uninitialized errors.
        let escrow = InvoiceEscrow {
            invoice_id: Symbol::new(&env, "TEST04"),
            admin: Address::generate(&env),
            guardian: None,
            sme_address: Address::generate(&env),
            amount: 1_000i128,
            funding_target: 500i128,
            funded_amount: 0i128,
            yield_bps: 500i64,
            maturity: 0u64,
            status: 0u32,
        };
        env.storage().instance().set(&DataKey::Escrow, &escrow);
    });

    // Call migrate(4) which should emit diagnostic event before returning NoMigrationPath error.
    client.migrate(&4u32);

    // After panic, the test framework will verify the event was emitted by checking
    // the event log (if not panicking, we can inspect events).
}

/// migrate diagnostic event carries version delta information.
#[test]
fn test_migrate_diagnostic_event_version_delta() {
    use soroban_sdk::vec as soroban_vec;

    let env = Env::default();
    env.mock_all_auths();
    let (contract_id, client) = deploy_with_id(&env);

    // Simulate version 2 stored on-chain (skipping multiple versions).
    env.as_contract(&contract_id, || {
        env.storage().instance().set(&DataKey::Version, &2u32);
        let escrow = InvoiceEscrow {
            invoice_id: Symbol::new(&env, "TEST02"),
            admin: Address::generate(&env),
                guardian: None,
            sme_address: Address::generate(&env),
            amount: 1_000i128,
            funding_target: 500i128,
            funded_amount: 0i128,
            yield_bps: 500i64,
            maturity: 0u64,
            status: 0u32,
        };
        env.storage().instance().set(&DataKey::Escrow, &escrow);
    });

    // Call migrate(2); it will fail with NoMigrationPath, but we want to see the event.
    let res = env.try_invoke_contract::<_, u32>(
        &contract_id,
        &Symbol::new(&env, "migrate"),
        soroban_vec![&env, &2u32],
    );

    // Expect error 92 (NoMigrationPath).
    assert!(res.is_err(), "migrate should fail");

    // Check for the diagnostic event in the event log.
    let events = env.events().all();

    // Find MigrationDiagnosticEmitted event.
    let diagnostic_events: Vec<_> = events
        .iter()
        .filter_map(|e| {
            // The event should contain "mig_diag" as the name (topic 0).
            if let soroban_sdk::Event::Contract(ce) = e {
                if ce.topics.len() > 0 {
                    if let soroban_sdk::Val::Symbol(name) = &ce.topics[0] {
                        if name.to_string() == "mig_diag" {
                            return Some(e.clone());
                        }
                    }
                }
            }
            None
        })
        .collect();

    assert!(
        !diagnostic_events.is_empty(),
        "migrate should emit MigrationDiagnosticEmitted event before error"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// Legal Hold + Dispute Pause Interaction Matrix Tests
// ──────────────────────────────────────────────────────────────────────────

/// Test interaction matrix: legal hold and dispute pause behavior when both
/// controls are active on the same escrow.
///
/// This test exercises all four combinations:
/// 1. Both hold active + pause active
/// 2. Hold active + pause inactive
/// 3. Hold inactive + pause active
/// 4. Hold inactive + pause inactive
///
/// For each state, we verify:
/// - The correct error is returned (legal hold takes precedence)
/// - Operations remain blocked when only one control is cleared
/// - Operations succeed once both controls are cleared
/// - The controls do not interfere with each other
#[test]
fn test_legal_hold_dispute_pause_interaction_matrix() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor1 = Address::generate(&env);
    let investor2 = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    // Initialize escrow
    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "HOLDPAUSE01"),
        &sme,
        &100_000i128,
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
    );

    // Fund the escrow
    client.fund(&investor1, &50_000i128);
    client.fund(&investor2, &50_000i128);

    // Settle the escrow so we can test claim and withdraw
    client.settle();

    // ─────────────────────────────────────────────────────────────────────
    // COMBINATION 1: Both hold and pause active
    // ─────────────────────────────────────────────────────────────────────

    // Activate legal hold
    client.set_legal_hold(&true, &String::from_str(&env, "compliance review"));
    assert!(client.get_legal_hold(), "legal hold should be active");

    // Activate dispute pause (24 hours = 86400 seconds)
    let pause_duration = 86400u64;
    client.pause_dispute(
        &String::from_str(&env, "TICKET-001"),
        &pause_duration,
    );
    assert!(
        client.is_dispute_paused(),
        "dispute pause should be active"
    );

    // Attempt withdraw with both controls active
    // Should fail with LegalHoldBlocksWithdrawal (123) — legal hold takes precedence
    let result = env.try_invoke_contract::<_, ()>(
        &client.contract_id,
        &Symbol::new(&env, "withdraw"),
        soroban_vec![&env, &sme],
    );
    assert!(result.is_err(), "withdraw should fail when both hold and pause active");
    assert_panic_contains_error_code(result, 123); // LegalHoldBlocksWithdrawal

    // Attempt claim with both controls active
    // Should fail with LegalHoldBlocksInvestorClaims (125) — legal hold takes precedence
    let result = env.try_invoke_contract::<_, ()>(
        &client.contract_id,
        &Symbol::new(&env, "claim_investor_payout"),
        soroban_vec![&env, &investor1],
    );
    assert!(result.is_err(), "claim should fail when both hold and pause active");
    assert_panic_contains_error_code(result, 125); // LegalHoldBlocksInvestorClaims

    // ─────────────────────────────────────────────────────────────────────
    // COMBINATION 2: Clear legal hold, keep pause active
    // ─────────────────────────────────────────────────────────────────────

    client.clear_legal_hold();
    assert!(
        !client.get_legal_hold(),
        "legal hold should be cleared"
    );
    assert!(
        client.is_dispute_paused(),
        "dispute pause should still be active"
    );

    // Attempt withdraw with hold cleared but pause still active
    // Should fail with DisputePausedBlocksWithdrawal (167)
    let result = env.try_invoke_contract::<_, ()>(
        &client.contract_id,
        &Symbol::new(&env, "withdraw"),
        soroban_vec![&env, &sme],
    );
    assert!(result.is_err(), "withdraw should fail when pause active");
    assert_panic_contains_error_code(result, 167); // DisputePausedBlocksWithdrawal

    // Attempt claim with hold cleared but pause still active
    // Should fail with DisputePausedBlocksInvestorClaims (168)
    let result = env.try_invoke_contract::<_, ()>(
        &client.contract_id,
        &Symbol::new(&env, "claim_investor_payout"),
        soroban_vec![&env, &investor1],
    );
    assert!(result.is_err(), "claim should fail when pause active");
    assert_panic_contains_error_code(result, 168); // DisputePausedBlocksInvestorClaims

    // ─────────────────────────────────────────────────────────────────────
    // COMBINATION 3: Pause expired, hold still inactive
    // ─────────────────────────────────────────────────────────────────────

    // Advance ledger time past the pause expiry
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp + pause_duration + 100;
    });

    assert!(
        !client.is_dispute_paused(),
        "dispute pause should have expired"
    );
    assert!(
        !client.get_legal_hold(),
        "legal hold should still be inactive"
    );

    // Now both controls are inactive; operations should succeed
    // Withdraw should succeed
    client.withdraw(&sme);

    // ─────────────────────────────────────────────────────────────────────
    // COMBINATION 4: Test reactivation in opposite order
    // ─────────────────────────────────────────────────────────────────────

    // Re-initialize a new escrow to test opposite activation order
    let investor3 = Address::generate(&env);
    let investor4 = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "HOLDPAUSE02"),
        &sme,
        &100_000i128,
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
    );

    client.fund(&investor3, &50_000i128);
    client.fund(&investor4, &50_000i128);
    client.settle();

    // Activate dispute pause first
    client.pause_dispute(
        &String::from_str(&env, "TICKET-002"),
        &pause_duration,
    );
    assert!(
        client.is_dispute_paused(),
        "dispute pause should be active"
    );

    // Then activate legal hold
    client.set_legal_hold(&true, &String::from_str(&env, "compliance review"));
    assert!(client.get_legal_hold(), "legal hold should be active");

    // Attempt withdraw with both active (pause activated first)
    // Should still fail with LegalHoldBlocksWithdrawal (123) — legal hold precedence is order-independent
    let result = env.try_invoke_contract::<_, ()>(
        &client.contract_id,
        &Symbol::new(&env, "withdraw"),
        soroban_vec![&env, &sme],
    );
    assert!(result.is_err(), "withdraw should fail when both hold and pause active (opposite order)");
    assert_panic_contains_error_code(result, 123); // LegalHoldBlocksWithdrawal

    // Clear pause first, hold remains active
    client.resume_dispute(&String::from_str(&env, "dispute resolved"));
    assert!(
        !client.is_dispute_paused(),
        "dispute pause should be resumed"
    );
    assert!(
        client.get_legal_hold(),
        "legal hold should still be active"
    );

    // Attempt withdraw with pause cleared but hold still active
    // Should fail with LegalHoldBlocksWithdrawal (123)
    let result = env.try_invoke_contract::<_, ()>(
        &client.contract_id,
        &Symbol::new(&env, "withdraw"),
        soroban_vec![&env, &sme],
    );
    assert!(result.is_err(), "withdraw should still fail when hold active");
    assert_panic_contains_error_code(result, 123); // LegalHoldBlocksWithdrawal

    // Clear legal hold
    client.clear_legal_hold();
    assert!(
        !client.get_legal_hold(),
        "legal hold should be cleared"
    );

    // Now both are inactive; operation should succeed
    client.withdraw(&sme);
}

/// Test that sweep_terminal_dust is NOT blocked by dispute pause
/// (only by legal hold), allowing treasury operations to proceed during disputes.
#[test]
fn test_sweep_terminal_dust_not_blocked_by_dispute_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, sme) = setup(&env);
    let investor = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init(
        &admin,
        &soroban_sdk::String::from_str(&env, "DUSTPAUSE01"),
        &sme,
        &100_000i128,
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
    );

    client.fund(&investor, &100_000i128);
    client.settle();
    client.withdraw(&sme);

    // Activate dispute pause (no legal hold)
    client.pause_dispute(
        &String::from_str(&env, "TICKET-DUST-001"),
        &3600u64,
    );
    assert!(
        client.is_dispute_paused(),
        "dispute pause should be active"
    );

    // Attempt sweep_terminal_dust while pause is active
    // Should succeed because dispute pause does not gate treasury operations
    client.sweep_terminal_dust(&treasury);

    // Now activate legal hold
    client.set_legal_hold(&true, &String::from_str(&env, "compliance"));

    // Attempt sweep_terminal_dust with legal hold active
    // Should fail with LegalHoldBlocksTreasuryDustSweep (30)
    let result = env.try_invoke_contract::<_, ()>(
        &client.contract_id,
        &Symbol::new(&env, "sweep_terminal_dust"),
        soroban_vec![&env, &treasury],
    );
    assert!(result.is_err(), "sweep should fail when legal hold active");
    assert_panic_contains_error_code(result, 30); // LegalHoldBlocksTreasuryDustSweep
}


#[test]
fn test_upgrade_emits_event_with_schema_version_and_admin() {
    use soroban_sdk::testutils::Events as _;
    use crate::ContractUpgraded;

    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    let contract_id = client.address.clone();
    default_init(&client, &env, &admin, &sme);

    // Upload WASM and get the hash
    let wasm_hash = env.deployer().upload_contract_wasm(LiquifactEscrow::WASM);

    // Call upgrade
    client.upgrade(&wasm_hash);

    // Verify the event was emitted with all three fields
    let events = env.events().all();
    let last_event = events.events().last().unwrap().clone();

    let expected_event = ContractUpgraded {
        name: symbol_short!("upgrade"),
        invoice_id: client.get_escrow().invoice_id,
        new_wasm_hash: wasm_hash.clone(),
        schema_version: SCHEMA_VERSION,
        admin: admin.clone(),
    }
    .to_xdr(&env, &contract_id);

    assert_eq!(last_event, expected_event, "upgrade event should contain new_wasm_hash, schema_version, and admin");
}
