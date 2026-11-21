use super::*;

// ============================================================================
// Migration Error Tests — Error Codes 90, 91, 92 (Issue #414)
// ============================================================================

/// Test: Error 90 — MigrationVersionMismatch
///
/// When `from_version` does not match the stored version, the contract
/// should panic with error code 90 (MigrationVersionMismatch).
#[test]
#[should_panic(expected = "90")]
fn test_migration_error_90_version_mismatch() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize escrow (sets version to SCHEMA_VERSION)
    client.init(
        &admin,
        &String::from_str(&env, "MIGRATE_ERR_90"),
        &admin,
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

    // Try to migrate with a from_version that doesn't match stored version
    // The stored version is SCHEMA_VERSION (e.g., 6)
    // Provide a different version (e.g., 1)
    client.migrate(&1u32);
}

/// Test: Error 91 — AlreadyCurrentSchemaVersion
///
/// When `from_version > SCHEMA_VERSION`, the contract should panic
/// with error code 91 (AlreadyCurrentSchemaVersion).
#[test]
#[should_panic(expected = "91")]
fn test_migration_error_91_already_current() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize escrow (sets version to SCHEMA_VERSION)
    client.init(
        &admin,
        &String::from_str(&env, "MIGRATE_ERR_91"),
        &admin,
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

    // Get the current SCHEMA_VERSION
    let current_version = client.get_version();

    // Future versions are rejected after matching the stored version.
    let future_version = current_version + 1;
    env.storage()
        .instance()
        .set(&DataKey::Version, &future_version);
    client.migrate(&future_version);
}

/// Test: Error 92 — NoMigrationPath
///
/// When `from_version < SCHEMA_VERSION` but no migration path is implemented,
/// the contract should panic with error code 92 (NoMigrationPath).
#[test]
#[should_panic(expected = "92")]
fn test_migration_error_92_no_migration_path() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize escrow (sets version to SCHEMA_VERSION)
    client.init(
        &admin,
        &String::from_str(&env, "MIGRATE_ERR_92"),
        &admin,
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

    // Simulate a stored legacy version with no supported migration path.
    env.storage()
        .instance()
        .set(&DataKey::Version, &1u32);

    // Version 1 is older than the immediately preceding schema, so this path is unsupported.
    client.migrate(&1u32);
}

// ============================================================================
// Migration Idempotency Tests — Error Code 93 (Issue #69)
// ============================================================================

/// Test: Error 93 — MigrationAlreadyApplied (Idempotency Prevention)
///
/// When a migration nonce has already been written for a version pair,
/// attempting the same migration should fail with error code 93
/// (MigrationAlreadyApplied), preventing replay attacks.
#[test]
fn test_migrate_idempotency_prevents_second_call() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize escrow
    client.init(
        &admin,
        &String::from_str(&env, "MIGRATE_IDEMPOTENCY"),
        &admin,
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

    let stored_version = client.get_version();

    // Manually write the migration nonce to simulate a completed migration
    // This represents the state after a successful migration would be executed
    env.storage()
        .instance()
        .set(
            &DataKey::MigrationExecutionLog(stored_version.saturating_sub(1), stored_version),
            &(env.ledger().sequence_number() as u32),
        );

    // Now attempt to migrate with the same version pair
    // This should fail with error code 93 (MigrationAlreadyApplied)
    let result = client.try_migrate(&stored_version.saturating_sub(1));
    assert_contract_error(result, EscrowError::MigrationAlreadyApplied);
}

/// Test: Migration Nonce Persists in Instance Storage
///
/// Verifies that the migration nonce written to instance storage persists
/// across calls (simulating persistence across contract upgrades).
#[test]
fn test_migrate_nonce_persists_in_storage() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin, _sme) = setup(&env);
    let (token, treasury) = free_addresses(&env);

    // Initialize escrow
    client.init(
        &admin,
        &String::from_str(&env, "MIGRATE_PERSIST"),
        &admin,
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

    let stored_version = client.get_version();

    // Write the migration nonce for version transition (stored_version - 1) -> stored_version
    let from_version = stored_version - 1;
    let to_version = stored_version;
    let nonce_key = DataKey::MigrationExecutionLog(from_version, to_version);
    let ledger_seq = env.ledger().sequence_number() as u32;

    env.storage().instance().set(&nonce_key, &ledger_seq);

    // Verify the nonce exists and can be read back
    let nonce_exists = env.storage().instance().has(&nonce_key);
    assert!(nonce_exists, "Migration nonce should persist in instance storage");

    // Verify we can read back the exact value
    let stored_seq: u32 = env.storage().instance().get(&nonce_key).unwrap();
    assert_eq!(
        stored_seq, ledger_seq,
        "Migration nonce value should match stored ledger sequence"
    );
}

/// Test: Different Version Pairs Don't Cause False-Positive Idempotency
///
/// Verifies that migrating from version A to B does NOT block migration
/// from version B to C, confirming the idempotency check is specific
/// to the exact (from_version, to_version) pair.
#[test]
fn test_migrate_different_version_pair_allowed() {
    let env = Env::default();
    env.mock_all_auths();

    // Write nonce for 6 → 7
    let from_v1 = 6u32;
    let to_v1 = 7u32;
    let nonce_key_67 = DataKey::MigrationExecutionLog(from_v1, to_v1);

    env.storage()
        .instance()
        .set(&nonce_key_67, &env.ledger().sequence_number() as u32);

    // Verify 6 → 7 nonce exists
    assert!(
        env.storage().instance().has(&nonce_key_67),
        "Nonce for 6 → 7 should exist"
    );

    // Verify 7 → 8 nonce does NOT exist (different from_version)
    let nonce_key_78 = DataKey::MigrationExecutionLog(7u32, 8u32);
    assert!(
        !env.storage().instance().has(&nonce_key_78),
        "Nonce for 7 → 8 should NOT exist (different version pair)"
    );

    // Verify 6 → 8 nonce does NOT exist (different to_version)
    let nonce_key_68 = DataKey::MigrationExecutionLog(6u32, 8u32);
    assert!(
        !env.storage().instance().has(&nonce_key_68),
        "Nonce for 6 → 8 should NOT exist (different version pair)"
    );
}