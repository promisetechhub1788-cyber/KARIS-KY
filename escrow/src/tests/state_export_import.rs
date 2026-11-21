//! Tests for `export_state` and `import_state` entrypoints.
//!
//! Each test creates its own fresh `Env` and does not depend on shared state.

use super::*;
use crate::tests::{deploy_id, free_addresses, install_stellar_asset_token, setup};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String,
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Initialise a minimal escrow and return the client, admin, sme, token addr, treasury addr.
fn init_minimal(
    env: &Env,
) -> (LiquifactEscrowClient<'_>, Address, Address, Address, Address) {
    env.mock_all_auths();
    let mut li = env.ledger().get();
    li.timestamp = 1_000;
    li.sequence_number = 50;
    env.ledger().set(li);

    let escrow_id = deploy_id(env);
    let client = LiquifactEscrowClient::new(env, &escrow_id);

    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let (token, treasury) = free_addresses(env);

    client.init(
        &admin,
        &String::from_str(env, "INV_EXP001"),
        &sme,
        &50_000i128,
        &500i64,  // 5% yield
        &0u64,    // no maturity lock
        &token,
        &None,
        &treasury,
        &None, // no yield tiers
        &None, // no min contribution
        &None, // no investor cap
        &None, // no per-investor cap
        &None, // no clear delay
        &None, // no funding deadline
    );

    (client, admin, sme, token, treasury)
}

// ---------------------------------------------------------------------------
// export_state happy path
// ---------------------------------------------------------------------------

#[test]
fn test_export_state_returns_correct_fields() {
    let env = Env::default();
    let (client, admin, sme, token, treasury) = init_minimal(&env);

    let export = client.export_state();

    assert_eq!(export.schema_version, SCHEMA_VERSION);
    assert_eq!(export.escrow.status, 0); // open
    assert_eq!(export.escrow.yield_bps, 500);
    assert_eq!(export.funding_token, token);
    assert_eq!(export.treasury, treasury);
    assert!(export.registry.is_none());
    assert!(export.yield_tiers.is_none());
    assert!(export.funding_close_snapshot.is_none());
    assert_eq!(export.min_contribution_floor, 0);
    assert!(export.max_unique_investors_cap.is_none());
    assert!(export.max_per_investor_cap.is_none());
    assert_eq!(export.unique_funder_count, 0);
    assert!(!export.legal_hold);
    assert_eq!(export.legal_hold_clear_delay, 0);
    assert!(export.legal_hold_clearable_at.is_none());
    assert!(!export.allowlist_active);
    assert!(export.primary_attestation_hash.is_none());
    assert_eq!(export.attestation_log.len(), 0);
    assert!(export.collateral.is_none());
    assert_eq!(export.distributed_principal, 0);
    assert!(export.funding_deadline.is_none());
    assert!(export.pending_admin.is_none());
    // Checksum must be 32 bytes (BytesN<32>), just confirm it was set
    let checksum_bytes: [u8; 32] = export.checksum.into();
    // A SHA-256 of non-zero input should not be all zeros
    assert_ne!(checksum_bytes, [0u8; 32]);
}

#[test]
fn test_export_state_captures_funded_snapshot() {
    let env = Env::default();
    env.mock_all_auths();

    let mut li = env.ledger().get();
    li.timestamp = 5_000;
    li.sequence_number = 200;
    env.ledger().set(li);

    let token_setup = install_stellar_asset_token(&env);
    let escrow_id = deploy_id(&env);
    let client = LiquifactEscrowClient::new(&env, &escrow_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let target = 20_000i128;

    client.init(
        &admin,
        &String::from_str(&env, "INV_SNAP"),
        &sme,
        &target,
        &300i64,
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
    );

    let investor = Address::generate(&env);
    token_setup.stellar.mint(&investor, &target);
    token_setup.token.approve(&investor, &escrow_id, &target, &999_999);
    client.fund(&investor, &target);

    let export = client.export_state();

    assert_eq!(export.escrow.status, 1); // funded
    assert!(export.funding_close_snapshot.is_some());
    let snap = export.funding_close_snapshot.unwrap();
    assert_eq!(snap.total_principal, target);
    assert_eq!(snap.funding_target, target);
    assert_eq!(snap.closed_at_ledger_timestamp, 5_000);
}

#[test]
fn test_export_state_captures_legal_hold() {
    let env = Env::default();
    let (client, _admin, _sme, _token, _treasury) = init_minimal(&env);

    client.set_legal_hold(&true);

    let export = client.export_state();
    assert!(export.legal_hold);
}

#[test]
fn test_export_state_captures_attestation_log() {
    let env = Env::default();
    let (client, _admin, _sme, _token, _treasury) = init_minimal(&env);

    let digest = soroban_sdk::BytesN::from_array(&env, &[0xABu8; 32]);
    client.append_attestation_digest(&digest, &Symbol::new(&env, ""));

    let export = client.export_state();
    assert_eq!(export.attestation_log.len(), 1);
    assert_eq!(export.attestation_log.get(0).unwrap(), digest);
}

// ---------------------------------------------------------------------------
// import_state happy path
// ---------------------------------------------------------------------------

#[test]
fn test_import_state_round_trip() {
    let env = Env::default();
    env.mock_all_auths();

    // --- source contract ---
    let (src, admin, sme, token, treasury) = init_minimal(&env);

    let export = src.export_state();

    // --- target contract (fresh, uninitialized) ---
    let target_id = deploy_id(&env);
    let target = LiquifactEscrowClient::new(&env, &target_id);

    target.import_state(&export);

    // Verify the restored state matches
    let restored = target.get_escrow();
    assert_eq!(restored.yield_bps, 500);
    assert_eq!(restored.status, 0);
    assert_eq!(target.get_funding_token(), token);
    assert_eq!(target.get_treasury(), treasury);
    assert_eq!(target.get_version(), SCHEMA_VERSION);
    assert_eq!(target.get_unique_funder_count(), 0);
    assert!(!target.get_legal_hold());
}

#[test]
fn test_import_state_preserves_attestation_hash_and_log() {
    let env = Env::default();
    let (src, _admin, _sme, _token, _treasury) = init_minimal(&env);

    let primary_hash = soroban_sdk::BytesN::from_array(&env, &[0xA1u8; 32]);
    src.bind_primary_attestation_hash(&soroban_sdk::Bytes::from_array(
        &env,
        &[0xA1u8; 32],
    ));
    let digests = [
        soroban_sdk::BytesN::from_array(&env, &[0xB1u8; 32]),
        soroban_sdk::BytesN::from_array(&env, &[0xB2u8; 32]),
        soroban_sdk::BytesN::from_array(&env, &[0xB3u8; 32]),
    ];
    for digest in &digests {
        src.append_attestation_digest(digest, &Symbol::new(&env, ""));
    }

    let export = src.export_state();
    assert_eq!(export.primary_attestation_hash, Some(primary_hash.clone()));
    assert_eq!(export.attestation_log.len(), 3);

    let target_id = deploy_id(&env);
    let target = LiquifactEscrowClient::new(&env, &target_id);
    target.import_state(&export);

    assert_eq!(target.get_primary_attestation_hash(), Some(primary_hash));
    let restored_log = target.get_attestation_log();
    assert_eq!(restored_log.len(), digests.len() as u32);
    for (index, digest) in digests.iter().enumerate() {
        assert_eq!(restored_log.get(index as u32).unwrap(), *digest);
    }
}

#[test]
fn test_import_state_preserves_funded_snapshot() {
    let env = Env::default();
    env.mock_all_auths();

    let token_setup = install_stellar_asset_token(&env);
    let src_id = deploy_id(&env);
    let src = LiquifactEscrowClient::new(&env, &src_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let target_amount = 30_000i128;

    src.init(
        &admin,
        &String::from_str(&env, "INV_RT"),
        &sme,
        &target_amount,
        &400i64,
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
    );

    let investor = Address::generate(&env);
    token_setup.stellar.mint(&investor, &target_amount);
    token_setup.token.approve(&investor, &src_id, &target_amount, &999_999);
    src.fund(&investor, &target_amount);

    let export = src.export_state();
    assert_eq!(export.escrow.status, 1);

    let tgt_id = deploy_id(&env);
    let tgt = LiquifactEscrowClient::new(&env, &tgt_id);
    tgt.import_state(&export);

    let snap = tgt.get_funding_close_snapshot();
    assert!(snap.is_some());
    let snap = snap.unwrap();
    assert_eq!(snap.total_principal, target_amount);
}

// ---------------------------------------------------------------------------
// import_state error paths
// ---------------------------------------------------------------------------

#[test]
fn test_import_state_fails_if_already_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let (src, _, _, _, _) = init_minimal(&env);
    let export = src.export_state();

    // src is already initialized — import onto it should fail
    let result = src.try_import_state(&export);
    crate::tests::assert_contract_error(result, EscrowError::ImportAlreadyInitialized);
}

#[test]
fn test_import_state_fails_on_schema_version_mismatch() {
    let env = Env::default();
    env.mock_all_auths();
    let (src, _, _, _, _) = init_minimal(&env);
    let mut export = src.export_state();

    // Tamper with schema version
    export.schema_version = SCHEMA_VERSION.wrapping_add(1);

    let tgt_id = deploy_id(&env);
    let tgt = LiquifactEscrowClient::new(&env, &tgt_id);
    let result = tgt.try_import_state(&export);
    crate::tests::assert_contract_error(result, EscrowError::ImportSchemaMismatch);
}

#[test]
fn test_import_state_fails_on_checksum_mismatch() {
    let env = Env::default();
    env.mock_all_auths();
    let (src, _, _, _, _) = init_minimal(&env);
    let mut export = src.export_state();

    // Corrupt the checksum
    export.checksum = soroban_sdk::BytesN::from_array(&env, &[0xFFu8; 32]);

    let tgt_id = deploy_id(&env);
    let tgt = LiquifactEscrowClient::new(&env, &tgt_id);
    let result = tgt.try_import_state(&export);
    crate::tests::assert_contract_error(result, EscrowError::ImportChecksumMismatch);
}

#[test]
fn test_import_state_fails_if_funded_amount_tampered() {
    let env = Env::default();
    env.mock_all_auths();
    let (src, _, _, _, _) = init_minimal(&env);
    let mut export = src.export_state();

    // Tamper with funded_amount without updating checksum → checksum mismatch
    export.escrow.funded_amount = 999_999_999;

    let tgt_id = deploy_id(&env);
    let tgt = LiquifactEscrowClient::new(&env, &tgt_id);
    let result = tgt.try_import_state(&export);
    crate::tests::assert_contract_error(result, EscrowError::ImportChecksumMismatch);
}

// ---------------------------------------------------------------------------
// export_state auth guard
// ---------------------------------------------------------------------------

#[test]
fn test_export_state_requires_admin_auth() {
    let env = Env::default();
    // Do NOT mock_all_auths — only the admin can export
    let escrow_id = deploy_id(&env);
    let client = LiquifactEscrowClient::new(&env, &escrow_id);

    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    env.mock_all_auths(); // needed only for init
    client.init(
        &admin,
        &String::from_str(&env, "INV_AUTH"),
        &sme,
        &10_000i128,
        &200i64,
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
    );
    // Reset: only authorise as sme (not admin)
    env.set_auths(&[]);
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &sme,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &escrow_id,
            fn_name: "export_state",
            args: soroban_sdk::vec![&env].into(),
            sub_invokes: &[],
        },
    }]);

    let result = client.try_export_state();
    // Should fail because sme is not the admin
    assert!(result.is_err(), "export_state should require admin auth");
}
