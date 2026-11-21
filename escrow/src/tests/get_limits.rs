//! Tests for the contract limits entrypoint.
//!
//! Verifies that the values returned by [`LiquifactEscrow::get_limits`]
//! match the source-of-truth constants defined in `escrow/src/lib.rs`.
//! This ensures SDKs and integrations can query runtime limits without
//! hard-coding them.

use soroban_sdk::Env;

use crate::{
    LiquifactEscrow, LiquifactEscrowClient, ContractLimits,
    MAX_ATTESTATION_APPEND_ENTRIES, MAX_REINVESTMENT_AUDIT_ENTRIES,
    MAX_INVESTOR_HISTORY_ENTRIES, MAX_ADMIN_ROLES, MAX_MULTISIG_SIGNERS,
    MAX_MULTISIG_OPERATIONS, MAX_INVESTOR_ALLOWLIST_BATCH, MAX_FUND_BATCH,
    MAX_BATCH_CLAIM, INVESTOR_BUCKET_COUNT, MAX_DUST_SWEEP_AMOUNT,
    MAX_DISPUTE_PAUSE_DURATION_SECS, MAX_INVOICE_ID_STRING_LEN,
    INSTANCE_TTL_MIN_EXTENSION_LEDGERS, PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
    MAX_TTL_BUMP_BATCH, MAX_SNAPSHOT_NAME_LEN, MAX_STATE_SNAPSHOTS,
    LEGAL_HOLD_PROPOSAL_TTL_SECS,
};

#[test]
fn get_limits_reports_attestation_append_entries() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_attestation_append_entries, MAX_ATTESTATION_APPEND_ENTRIES,
        "max_attestation_append_entries must match constant"
    );
}

#[test]
fn get_limits_reports_reinvestment_audit_entries() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_reinvestment_audit_entries, MAX_REINVESTMENT_AUDIT_ENTRIES,
        "max_reinvestment_audit_entries must match constant"
    );
}

#[test]
fn get_limits_reports_investor_history_entries() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_investor_history_entries, MAX_INVESTOR_HISTORY_ENTRIES,
        "max_investor_history_entries must match constant"
    );
}

#[test]
fn get_limits_reports_admin_roles() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_admin_roles, MAX_ADMIN_ROLES,
        "max_admin_roles must match constant"
    );
}

#[test]
fn get_limits_reports_multisig_signers() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_multisig_signers, MAX_MULTISIG_SIGNERS,
        "max_multisig_signers must match constant"
    );
}

#[test]
fn get_limits_reports_multisig_operations() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_multisig_operations, MAX_MULTISIG_OPERATIONS,
        "max_multisig_operations must match constant"
    );
}

#[test]
fn get_limits_reports_investor_allowlist_batch() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_investor_allowlist_batch, MAX_INVESTOR_ALLOWLIST_BATCH,
        "max_investor_allowlist_batch must match constant"
    );
}

#[test]
fn get_limits_reports_fund_batch() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_fund_batch, MAX_FUND_BATCH,
        "max_fund_batch must match constant — clients use this for transaction planning"
    );
}

#[test]
fn get_limits_reports_batch_claim() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_batch_claim, MAX_BATCH_CLAIM,
        "max_batch_claim must match constant"
    );
}

#[test]
fn get_limits_reports_investor_bucket_count() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.investor_bucket_count, INVESTOR_BUCKET_COUNT,
        "investor_bucket_count must match constant"
    );
}

#[test]
fn get_limits_reports_dust_sweep_amount() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_dust_sweep_amount, MAX_DUST_SWEEP_AMOUNT,
        "max_dust_sweep_amount must match constant — operatorsuse this to set sweep rates"
    );
}

#[test]
fn get_limits_reports_dispute_pause_duration() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_dispute_pause_duration_secs, MAX_DISPUTE_PAUSE_DURATION_SECS,
        "max_dispute_pause_duration_secs must match constant"
    );
}

#[test]
fn get_limits_reports_invoice_id_string_len() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_invoice_id_string_len, MAX_INVOICE_ID_STRING_LEN,
        "max_invoice_id_string_len must match constant"
    );
}

#[test]
fn get_limits_reports_instance_ttl_min_extension_ledgers() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.instance_ttl_min_extension_ledgers, INSTANCE_TTL_MIN_EXTENSION_LEDGERS,
        "instance_ttl_min_extension_ledgers must match constant"
    );
}

#[test]
fn get_limits_reports_persistent_ttl_min_extension_ledgers() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.persistent_ttl_min_extension_ledgers, PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
        "persistent_ttl_min_extension_ledgers must match constant"
    );
}

#[test]
fn get_limits_reports_ttl_bump_batch() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_ttl_bump_batch, MAX_TTL_BUMP_BATCH,
        "max_ttl_bump_batch must match constant"
    );
}

#[test]
fn get_limits_reports_snapshot_name_len() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_snapshot_name_len, MAX_SNAPSHOT_NAME_LEN,
        "max_snapshot_name_len must match constant"
    );
}

#[test]
fn get_limits_reports_state_snapshots() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.max_state_snapshots, MAX_STATE_SNAPSHOTS,
        "max_state_snapshots must match constant"
    );
}

#[test]
fn get_limits_reports_legal_hold_proposal_ttl() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    assert_eq!(
        limits.legal_hold_proposal_ttl_secs, LEGAL_HOLD_PROPOSAL_TTL_SECS,
        "legal_hold_proposal_ttl_secs must match constant"
    );
}

#[test]
fn get_limits_available_without_init() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    // No init() called — limits should still be available because they are
    // compile-time constants baked into the WASM, not stored state.
    let limits = client.get_limits();

    assert!(
        limits.max_fund_batch > 0,
        "all limits must be positive"
    );
    assert!(
        limits.max_batch_claim > 0,
        "all limits must be positive"
    );
    assert!(
        limits.investor_bucket_count > 0,
        "all limits must be positive"
    );
}

#[test]
fn get_limits_is_deterministic() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let first = client.get_limits();
    let second = client.get_limits();
    let third = client.get_limits();

    assert_eq!(first, second, "limits must be deterministic");
    assert_eq!(second, third, "limits must be deterministic");
}

#[test]
fn get_limits_all_values_positive_or_reasonable() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    // All limits should have reasonable values (positive for counts/sizes, reasonable for durations)
    assert!(limits.max_attestation_append_entries > 0);
    assert!(limits.max_reinvestment_audit_entries > 0);
    assert!(limits.max_investor_history_entries > 0);
    assert!(limits.max_admin_roles > 0);
    assert!(limits.max_multisig_signers > 0);
    assert!(limits.max_multisig_operations > 0);
    assert!(limits.max_investor_allowlist_batch > 0);
    assert!(limits.max_fund_batch > 0);
    assert!(limits.max_batch_claim > 0);
    assert!(limits.investor_bucket_count > 0);
    assert!(limits.max_dust_sweep_amount > 0);
    assert!(limits.max_dispute_pause_duration_secs > 0);
    assert!(limits.max_invoice_id_string_len > 0);
    assert!(limits.instance_ttl_min_extension_ledgers > 0);
    assert!(limits.persistent_ttl_min_extension_ledgers > 0);
    assert!(limits.max_ttl_bump_batch > 0);
    assert!(limits.max_snapshot_name_len > 0);
    assert!(limits.max_state_snapshots > 0);
    assert!(limits.legal_hold_proposal_ttl_secs > 0);
}

#[test]
fn get_limits_fund_batch_reasonable_for_clients() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    // MAX_FUND_BATCH should be reasonable for transaction planning
    // (not too small to be useless, not so large it's obviously wrong)
    assert!(limits.max_fund_batch >= 10, "MAX_FUND_BATCH too small");
    assert!(limits.max_fund_batch <= 1000, "MAX_FUND_BATCH too large");
}

#[test]
fn get_limits_batch_claim_reasonable_for_clients() {
    let env = Env::default();
    let contract_id = env.register(LiquifactEscrow {}, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let limits = client.get_limits();

    // MAX_BATCH_CLAIM should be reasonable for payout batching
    assert!(limits.max_batch_claim >= 10, "MAX_BATCH_CLAIM too small");
    assert!(limits.max_batch_claim <= 1000, "MAX_BATCH_CLAIM too large");
}
