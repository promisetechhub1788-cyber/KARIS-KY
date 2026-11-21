use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env, String as SorobanString,
};

// Use the escrow contract from the lib crate
use karis_ky_escrow::{LiquifactEscrow, LiquifactEscrowClient};

fn deploy_contract(env: &Env) -> (LiquifactEscrowClient, Address) {
    let id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(env, &id);
    (client, id)
}

fn setup_ledger(env: &Env) {
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 12345;
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);
    env.mock_all_auths();
}

fn generate_addresses(env: &Env) -> (Address, Address, Address, Address) {
    (
        Address::generate(env),
        Address::generate(env),
        Address::generate(env),
        Address::generate(env),
    )
}

/// Install a standard Stellar asset token contract for testing.
fn install_stellar_asset_token(env: &Env) -> (Address, TokenClient, StellarAssetClient) {
    let sac = env.register_stellar_asset_contract_v2(Address::generate(env));
    let id = sac.address();
    (
        id.clone(),
        TokenClient::new(env, &id),
        StellarAssetClient::new(env, &id),
    )
}

#[test]
fn snapshot_test_init_state() {
    let env = Env::new();
    setup_ledger(&env);

    let (client, _contract_id) = deploy_contract(&env);
    let (admin, sme, investor, treasury) = generate_addresses(&env);
    let (token_id, _token_client, _sac) = install_stellar_asset_token(&env);

    let invoice_id = SorobanString::from_str(&env, "INV-2024-001");
    let funding_target = 100_000_000_000i128;
    let yield_bps = 500i64;

    client.init(
        &admin,
        &invoice_id,
        &sme,
        &funding_target,
        &yield_bps,
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
    );

    // Read the escrow state and snapshot it
    let escrow = client.get_escrow();
    insta::assert_json_snapshot!(escrow, @r###"
    {
      "invoice_id": "INV-2024-001",
      "admin": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
      "sme_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
      "amount": 100000000000,
      "funding_target": 100000000000,
      "funded_amount": 0,
      "yield_bps": 500,
      "maturity": 0,
      "status": 0
    }
    "###);
}

#[test]
fn snapshot_test_post_funding() {
    let env = Env::new();
    setup_ledger(&env);

    let (client, contract_id) = deploy_contract(&env);
    let (admin, sme, investor, treasury) = generate_addresses(&env);
    let (token_id, token_client, sac) = install_stellar_asset_token(&env);

    let invoice_id = SorobanString::from_str(&env, "INV-2024-002");
    let funding_target = 100_000_000_000i128;
    let yield_bps = 500i64;

    client.init(
        &admin,
        &invoice_id,
        &sme,
        &funding_target,
        &yield_bps,
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
    );

    // Fund the escrow to the target
    client.fund(&investor, &funding_target);

    // Mint tokens into escrow for later withdrawal
    sac.mint(&contract_id, &funding_target);

    // Read the escrow state and snapshot it
    let escrow = client.get_escrow();
    insta::assert_json_snapshot!(escrow, @r###"
    {
      "invoice_id": "INV-2024-002",
      "admin": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
      "sme_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
      "amount": 100000000000,
      "funding_target": 100000000000,
      "funded_amount": 100000000000,
      "yield_bps": 500,
      "maturity": 0,
      "status": 1
    }
    "###);
}

#[test]
fn snapshot_test_post_settlement() {
    let env = Env::new();
    setup_ledger(&env);

    let (client, contract_id) = deploy_contract(&env);
    let (admin, sme, investor, treasury) = generate_addresses(&env);
    let (token_id, token_client, sac) = install_stellar_asset_token(&env);

    let invoice_id = SorobanString::from_str(&env, "INV-2024-003");
    let funding_target = 100_000_000_000i128;
    let yield_bps = 500i64;

    client.init(
        &admin,
        &invoice_id,
        &sme,
        &funding_target,
        &yield_bps,
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
    );

    // Fund the escrow
    client.fund(&investor, &funding_target);

    // Mint tokens into escrow
    sac.mint(&contract_id, &funding_target);

    // Settle the escrow
    client.settle(&sme);

    // Read the escrow state and snapshot it
    let escrow = client.get_escrow();
    insta::assert_json_snapshot!(escrow, @r###"
    {
      "invoice_id": "INV-2024-003",
      "admin": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
      "sme_address": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
      "amount": 100000000000,
      "funding_target": 100000000000,
      "funded_amount": 100000000000,
      "yield_bps": 500,
      "maturity": 0,
      "status": 2
    }
    "###);
}

#[test]
fn snapshot_test_version() {
    let env = Env::new();
    setup_ledger(&env);

    let (client, _contract_id) = deploy_contract(&env);

    // Read the version
    let version = client.get_version();
    insta::assert_json_snapshot!(version, @"6");
}

#[test]
fn snapshot_test_export_escrow_snapshot() {
    let env = Env::new();
    setup_ledger(&env);
    let (client, _contract_id) = deploy_contract(&env);
    let (admin, sme, investor, treasury) = generate_addresses(&env);
    let (token_id, _token_client, _sac) = install_stellar_asset_token(&env);

    client.init(
        &admin,
        &SorobanString::from_str(&env, "INV-EXPORT-001"),
        &sme,
        &100_000i128,
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
    );
    client.fund(&investor, &50_000i128);

    insta::assert_json_snapshot!(client.export_escrow_snapshot(), {
        ".escrow.admin" => "[admin]",
        ".escrow.sme_address" => "[sme]",
        ".funding_token" => "[funding token]",
        ".treasury" => "[treasury]"
    }, @r###"
    {
      "escrow": {
        "invoice_id": "INV-EXPORT-001",
        "admin": "[admin]",
        "sme_address": "[sme]",
        "amount": 100000,
        "funding_target": 100000,
        "funded_amount": 50000,
        "yield_bps": 800,
        "maturity": 0,
        "status": 0,
        "guardian": null
      },
      "schema_version": 6,
      "legal_hold": false,
      "funding_close_snapshot": "None",
      "unique_funder_count": 1,
      "is_allowlist_active": false,
      "sme_collateral_commitment": "None",
      "primary_attestation_hash": null,
      "attestation_log_length": 0,
      "sanctions_provider": null,
      "funding_token": "[funding token]",
      "treasury": "[treasury]",
      "registry": null,
      "version_history": [[6, 12345]]
    }
    "###);
}
