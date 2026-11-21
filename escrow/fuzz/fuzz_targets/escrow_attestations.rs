#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::*, Address, BytesN, Env, String, Symbol};

use karis_ky_escrow::{
    AuditLogFilter, LiquifactEscrow, MAX_ATTESTATION_APPEND_ENTRIES,
};

#[derive(Arbitrary, Debug)]
enum AttestationOperation {
    Append {
        digest: [u8; 32],
        tagged: bool,
    },
    Revoke {
        index: u32,
    },
    Query {
        after_timestamp: Option<u64>,
        tagged: bool,
        limit: u32,
        offset: u32,
    },
}

#[derive(Arbitrary, Debug)]
struct AttestationInput {
    operations: Vec<AttestationOperation>,
}

fn initialized_env() -> Env {
    let env = Env::default();
    env.budget().reset_unlimited();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let token = Address::generate(&env);
    let treasury = Address::generate(&env);

    LiquifactEscrow::init(
        env.clone(),
        admin,
        String::from_str(&env, "ATTEST_FUZZ"),
        sme,
        1,
        0,
        0,
        token,
        None,
        treasury,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    env
}

fuzz_target!(|input: AttestationInput| {
    let env = initialized_env();
    let fuzz_tag = Symbol::new(&env, "fuzz");

    for operation in input.operations.iter().take(512) {
        match operation {
            AttestationOperation::Append { digest, tagged } => {
                let tag = if *tagged {
                    fuzz_tag.clone()
                } else {
                    Symbol::new(&env, "")
                };
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    LiquifactEscrow::append_attestation_digest(
                        env.clone(),
                        BytesN::from_array(&env, digest),
                        tag,
                    );
                }));
            }
            AttestationOperation::Revoke { index } => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    LiquifactEscrow::revoke_attestation_digest(env.clone(), *index);
                }));
            }
            AttestationOperation::Query {
                after_timestamp,
                tagged,
                limit,
                offset,
            } => {
                let filters = AuditLogFilter {
                    after_timestamp: *after_timestamp,
                    tag_prefix: if *tagged { Some(fuzz_tag.clone()) } else { None },
                };
                let page = LiquifactEscrow::query_attestation_logs(
                    env.clone(),
                    filters,
                    *limit,
                    *offset,
                );
                assert!(page.entries.len() <= MAX_ATTESTATION_APPEND_ENTRIES);
                assert!(page.total_count <= MAX_ATTESTATION_APPEND_ENTRIES);
            }
        }

        let log = LiquifactEscrow::get_attestation_append_log(env.clone());
        assert!(
            log.len() <= MAX_ATTESTATION_APPEND_ENTRIES,
            "attestation append log exceeded configured capacity"
        );
    }
});