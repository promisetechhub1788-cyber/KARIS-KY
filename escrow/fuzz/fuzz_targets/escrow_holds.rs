#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::*, Address, Env, String};

use karis_ky_escrow::{DataKey, InvoiceEscrow, LiquifactEscrow, LiquifactEscrowClient};

#[derive(Arbitrary, Debug)]
struct HoldSequence {
    actions: Vec<u8>,
    use_guardian: bool,
}

fn assert_hold_invariants(client: &LiquifactEscrowClient<'_>) {
    let escrow = client.get_escrow();
    assert!(
        escrow.status <= 5,
        "escrow status must remain in its valid range"
    );
}

fuzz_target!(|input: HoldSequence| {
    let env = Env::default();
    env.budget().reset_unlimited();
    env.mock_all_auths();

    let mut ledger = env.ledger().get();
    ledger.timestamp = 1_000_000;
    ledger.sequence_number = 1;
    env.ledger().set(ledger);

    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let sme = Address::generate(&env);
    let investor = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let treasury = Address::generate(&env);
    let contract_id = env.register(LiquifactEscrow, ());
    let client = LiquifactEscrowClient::new(&env, &contract_id);

    let initialized = client.try_init(
        &admin,
        &String::from_str(&env, "FUZZ-HOLD"),
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
        &Some(10u64),
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );
    if initialized.is_err() {
        return;
    }

    if input.use_guardian {
        env.as_contract(&contract_id, || {
            let mut escrow: InvoiceEscrow = env
                .storage()
                .instance()
                .get(&DataKey::Escrow)
                .expect("initialized escrow state");
            escrow.guardian = Some(guardian.clone());
            env.storage().instance().set(&DataKey::Escrow, &escrow);
        });
    }

    if client.try_fund(&investor, &1_000i128).is_err() {
        return;
    }
    assert_hold_invariants(&client);

    for action in input.actions.iter().take(64) {
        let before = client.get_escrow();
        let legal_hold_before = client.get_legal_hold_status();
        let settlement_blocked =
            client.get_legal_hold_status() || client.get_dispute_pause().is_some();
        let clearable_at = client.get_legal_hold_clearable_at();

        match action % 8 {
            0 if input.use_guardian => {
                let _ = client.try_propose_legal_hold(&admin);
            }
            1 if input.use_guardian => {
                let _ = client.try_confirm_legal_hold(&guardian);
            }
            2 if !input.use_guardian => {
                let _ = client.try_set_legal_hold(&true, &String::from_str(&env, "fuzz hold"));
            }
            3 => {
                let _ = client.try_request_clear_legal_hold();
            }
            4 => {
                let _ = client.try_clear_legal_hold();
            }
            5 => {
                let _ = client.try_pause_dispute(&String::from_str(&env, "FUZZ"), &5u64);
            }
            6 => {
                let _ = client.try_resume_dispute();
            }
            _ => {
                let _ = client.try_settle();
            }
        }

        if *action % 4 == 0 {
            let mut ledger = env.ledger().get();
            ledger.timestamp = ledger.timestamp.saturating_add(5);
            env.ledger().set(ledger);
        }

        assert_hold_invariants(&client);
        let after = client.get_escrow();
        if action % 8 == 7 && settlement_blocked {
            assert_eq!(
                after.status, before.status,
                "settlement must not advance while a legal hold or dispute pause is active"
            );
        }
        if legal_hold_before
            && before.status < 2
            && client.get_legal_hold_clear_delay() > 0
            && clearable_at.is_some_and(|timestamp| env.ledger().timestamp() < timestamp)
            && action % 8 == 4
        {
            assert!(
                client.get_legal_hold_status(),
                "a delayed legal hold clear cannot take effect before its deadline"
            );
        }
    }
});