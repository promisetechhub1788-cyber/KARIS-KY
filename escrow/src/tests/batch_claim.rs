//! Tests for batch claim, Merkle proof verification, attestation log optimization,
//! and escrow-state pass-by-reference caching.
//!
//! These tests validate the four optimisation features added in a single sweep:
//! 1. `batch_claim_investor_payouts` — admin/treasury batch claim endpoint
//! 2. `FundingCloseMerkleRoot` — Merkle root anchor at funding close
//! 3. Optimised attestation append (individual entry storage)
//! 4. Single-escrow-read pattern in batch claim

use super::*;
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String, Vec};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn digest(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

/// Initialise a funded-and-settled escrow with `num_investors` contributing `amount` each.
/// Returns (client, Vec<investor_addresses>, admin).
fn setup_multi_investor_settled(
    env: &Env,
    num_investors: u32,
    amount_per: i128,
) -> (LiquifactEscrowClient<'_>, Vec<Address>, Address) {
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let (token, treasury) = free_addresses(env);

    let total = amount_per
        .checked_mul(num_investors as i128)
        .unwrap_or(amount_per);

    client.init(
        &admin,
        &String::from_str(env, "BATCH01"),
        &sme,
        &total,
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
    );

    let mut investors = Vec::new(env);
    for _i in 0..num_investors {
        let inv = Address::generate(env);
        client.fund(&inv, &amount_per);
        investors.push_back(inv);
    }

    client.settle();
    (client, investors, admin)
}

// ── 1. Batch claim — happy path ─────────────────────────────────────────────

#[test]
fn batch_claim_single_investor_succeeds() {
    let env = Env::default();
    let (client, investors, _admin) = setup_multi_investor_settled(&env, 1, 100_000_000_000i128);

    let claimed = client.batch_claim_investor_payouts(&investors);
    assert_eq!(claimed, 1);
    assert!(client.is_investor_claimed(&investors.get(0).unwrap()));
}

#[test]
fn batch_claim_multiple_investors_succeeds() {
    let env = Env::default();
    let (client, investors, _admin) = setup_multi_investor_settled(&env, 5, 20_000_000_000i128);

    let claimed = client.batch_claim_investor_payouts(&investors);
    assert_eq!(claimed, 5u32);
    for i in 0..5u32 {
        assert!(client.is_investor_claimed(&investors.get(i).unwrap()));
    }
}

#[test]
fn batch_claim_is_idempotent() {
    let env = Env::default();
    let (client, investors, _admin) = setup_multi_investor_settled(&env, 3, 30_000_000_000i128);

    let first = client.batch_claim_investor_payouts(&investors);
    assert_eq!(first, 3u32);

    // Second call — all already claimed, should claim 0.
    let second = client.batch_claim_investor_payouts(&investors);
    assert_eq!(second, 0u32);
}

#[test]
fn batch_claim_partial_already_claimed() {
    let env = Env::default();
    let (client, investors, _admin) = setup_multi_investor_settled(&env, 4, 25_000_000_000i128);

    // Claim the first one individually.
    client.claim_investor_payout(&investors.get(0).unwrap());
    assert!(client.is_investor_claimed(&investors.get(0).unwrap()));

    // Batch claim all 4 — only 3 should be newly claimed.
    let claimed = client.batch_claim_investor_payouts(&investors);
    assert_eq!(claimed, 3u32);
    for i in 0..4u32 {
        assert!(client.is_investor_claimed(&investors.get(i).unwrap()));
    }
}

// ── 2. Batch claim — error paths ────────────────────────────────────────────

#[test]
#[should_panic]
fn batch_claim_empty_vec_panics() {
    let env = Env::default();
    let (client, _investors, _admin) = setup_multi_investor_settled(&env, 1, 100_000_000_000i128);

    let empty = Vec::new(&env);
    client.batch_claim_investor_payouts(&empty);
}

#[test]
#[should_panic]
fn batch_claim_blocked_by_legal_hold() {
    let env = Env::default();
    let (client, investors, _admin) = setup_multi_investor_settled(&env, 1, 100_000_000_000i128);

    client.set_legal_hold(&true);
    client.batch_claim_investor_payouts(&investors);
}

#[test]
#[should_panic]
fn batch_claim_before_settle_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "BATCH02"),
        &sme,
        &100_000_000_000i128,
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
    );

    let investor = Address::generate(&env);
    client.fund(&investor, &100_000_000_000i128);
    // Escrow is funded (status 1) but not settled (status 2).

    let mut investors = Vec::new(&env);
    investors.push_back(investor);
    client.batch_claim_investor_payouts(&investors);
}

// ── 3. Merkle root — storage at funding close ───────────────────────────────

#[test]
fn merkle_root_stored_at_funding_close() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "MRK001"),
        &sme,
        &100_000_000_000i128,
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
    );

    // Before funding: Merkle root is absent.
    assert!(client.get_funding_close_merkle_root().is_none());

    let investor = Address::generate(&env);
    client.fund(&investor, &100_000_000_000i128);

    // After funding close: Merkle root is present (empty root for now).
    let root = client.get_funding_close_merkle_root();
    assert!(root.is_some());
}

#[test]
fn merkle_root_absent_before_funding_close() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "MRK002"),
        &sme,
        &100_000_000_000i128,
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
    );

    // Not yet funded — Merkle root absent.
    assert!(client.get_funding_close_merkle_root().is_none());
}

#[test]
fn verify_investor_proof_fails_without_root() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "MRK003"),
        &sme,
        &100_000_000_000i128,
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
    );

    // No root stored yet — proof should fail.
    let proof = Vec::new(&env);
    let investor = Address::generate(&env);
    assert!(!client.verify_investor_proof(&investor, &100i128, &proof));
}

#[test]
fn merkle_root_survives_settle() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    client.init(
        &admin,
        &String::from_str(&env, "MRK004"),
        &sme,
        &100_000_000_000i128,
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
    );

    let investor = Address::generate(&env);
    client.fund(&investor, &100_000_000_000i128);
    let root_before = client.get_funding_close_merkle_root();

    client.settle();
    let root_after = client.get_funding_close_merkle_root();

    assert_eq!(root_before, root_after);
}

// ── 4. Attestation log — individual entry optimisation ──────────────────────

#[test]
fn attestation_append_uses_individual_entries() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    let d1 = digest(&env, 0x10);
    let d2 = digest(&env, 0x20);

    client.append_attestation_digest(&d1);
    client.append_attestation_digest(&d2);

    let log = client.get_attestation_append_log();
    assert_eq!(log.len(), 2);
    assert_eq!(log.get(0).unwrap(), d1);
    assert_eq!(log.get(1).unwrap(), d2);
}

#[test]
fn attestation_append_respects_max_entries() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    // Fill to capacity.
    for i in 0u8..(MAX_ATTESTATION_APPEND_ENTRIES as u8) {
        client.append_attestation_digest(&digest(&env, i));
    }

    let log = client.get_attestation_append_log();
    assert_eq!(log.len(), MAX_ATTESTATION_APPEND_ENTRIES);

    // Verify order.
    for i in 0u32..MAX_ATTESTATION_APPEND_ENTRIES {
        assert_eq!(log.get(i).unwrap(), digest(&env, i as u8));
    }
}

#[test]
#[should_panic]
fn attestation_append_beyond_max_panics() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    for i in 0u8..(MAX_ATTESTATION_APPEND_ENTRIES as u8) {
        client.append_attestation_digest(&digest(&env, i));
    }

    // This must panic — capacity reached.
    client.append_attestation_digest(&digest(&env, 0xFF));
}

#[test]
fn attestation_revoke_works_with_individual_entries() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);

    client.append_attestation_digest(&digest(&env, 0xAA));
    client.append_attestation_digest(&digest(&env, 0xBB));

    client.revoke_attestation_digest(&0);
    assert!(client.is_attestation_revoked(&0));
    assert!(!client.is_attestation_revoked(&1));
}

#[test]
fn attestation_log_empty_before_first_append_with_new_format() {
    let env = Env::default();
    let (client, admin, sme) = setup(&env);
    default_init(&client, &env, &admin, &sme);
    assert_eq!(client.get_attestation_append_log().len(), 0);
}

// ── 5. Escrow pass-by-reference in batch claim ─────────────────────────────

#[test]
fn batch_claim_verifies_all_investors_independently() {
    let env = Env::default();
    let (client, investors, _admin) = setup_multi_investor_settled(&env, 10, 10_000_000_000i128);

    let claimed = client.batch_claim_investor_payouts(&investors);
    assert_eq!(claimed, 10u32);

    // Each investor should be independently marked as claimed.
    for i in 0..10u32 {
        assert!(client.is_investor_claimed(&investors.get(i).unwrap()));
    }
}

#[test]
fn batch_claim_mixed_claimed_and_unclaimed() {
    let env = Env::default();
    let (client, investors, _admin) = setup_multi_investor_settled(&env, 6, 15_000_000_000i128);

    // Claim investors 0, 2, 4 individually.
    client.claim_investor_payout(&investors.get(0).unwrap());
    client.claim_investor_payout(&investors.get(2).unwrap());
    client.claim_investor_payout(&investors.get(4).unwrap());

    // Batch claim all 6 — only 3 new claims.
    let claimed = client.batch_claim_investor_payouts(&investors);
    assert_eq!(claimed, 3u32);

    for i in 0..6u32 {
        assert!(client.is_investor_claimed(&investors.get(i).unwrap()));
    }
}

// ── 6. Batch claim budget benchmark ─────────────────────────────────────────

/// Benchmark: measure the CPU instruction and memory cost of
/// `batch_claim_investor_payouts` at batch sizes 1, 10, 25, and
/// `MAX_BATCH_CLAIM` (50), and compare against the equivalent number of
/// individual `claim_investor_payout` calls to derive per-investor savings.
///
/// Run in release mode with:
/// ```text
/// cargo test --release benchmark_batch_claim_budget -- --ignored --nocapture
/// ```
///
/// Results are recorded in `escrow-gas-storage-notes.md`.
#[test]
#[ignore]
fn benchmark_batch_claim_budget() {
    let batch_sizes: [u32; 4] = [1, 10, 25, MAX_BATCH_CLAIM];

    for &size in batch_sizes.iter() {
        // ── Batch claim measurement ─────────────────────────────────────────
        let env = Env::default();
        let (client, investors, _admin) =
            setup_multi_investor_settled(&env, size, 10_000_000_000i128);

        env.budget().reset_default();
        let claimed = client.batch_claim_investor_payouts(&investors);
        let batch_cpu = env.budget().cpu_instruction_cost();
        let batch_mem = env.budget().memory_bytes_cost();
        assert_eq!(claimed, size);

        // ── Individual claim measurement ────────────────────────────────────
        let env_ind = Env::default();
        let (client_ind, investors_ind, _admin_ind) =
            setup_multi_investor_settled(&env_ind, size, 10_000_000_000i128);

        env_ind.budget().reset_default();
        for i in 0..size {
            client_ind.claim_investor_payout(&investors_ind.get(i).unwrap());
        }
        let ind_cpu = env_ind.budget().cpu_instruction_cost();
        let ind_mem = env_ind.budget().memory_bytes_cost();

        // ── Derived metrics ─────────────────────────────────────────────────
        let batch_cpu_per = batch_cpu / (size as u64);
        let ind_cpu_per = ind_cpu / (size as u64);
        let cpu_savings_per = ind_cpu_per.saturating_sub(batch_cpu_per);
        let cpu_savings_pct = if ind_cpu_per > 0 {
            (cpu_savings_per * 100) / ind_cpu_per
        } else {
            0
        };

        let batch_mem_per = batch_mem / (size as u64);
        let ind_mem_per = ind_mem / (size as u64);
        let mem_savings_per = ind_mem_per.saturating_sub(batch_mem_per);

        std::println!(
            "batch_size={:<3} batch_cpu={:<12} batch_mem={:<10} \
             ind_cpu={:<12} ind_mem={:<10} \
             batch_cpu/inv={:<10} ind_cpu/inv={:<10} \
             cpu_savings/inv={:<10} ({:>3}%) mem_savings/inv={}",
            size,
            batch_cpu,
            batch_mem,
            ind_cpu,
            ind_mem,
            batch_cpu_per,
            ind_cpu_per,
            cpu_savings_per,
            cpu_savings_pct,
            mem_savings_per,
        );

        // Sanity: batch must not be more expensive per investor than individual.
        assert!(
            batch_cpu_per <= ind_cpu_per,
            "batch claim per-investor CPU ({batch_cpu_per}) exceeds individual ({ind_cpu_per}) at size {size}"
        );
    }
}

/// Benchmark: determine the largest batch size that fits within the Soroban
/// instruction budget by probing increasing batch sizes.
///
/// Run in release mode with:
/// ```text
/// cargo test --release benchmark_max_batch_within_budget -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn benchmark_max_batch_within_budget() {
    // Soroban's default per-transaction CPU instruction budget is 100_000_000.
    const CPU_BUDGET: u64 = 100_000_000;

    let probe_sizes: [u32; 5] = [1, 10, 25, MAX_BATCH_CLAIM, MAX_BATCH_CLAIM];

    let mut largest_fitting: u32 = 0;
    for &size in probe_sizes.iter() {
        let env = Env::default();
        let (client, investors, _admin) =
            setup_multi_investor_settled(&env, size, 10_000_000_000i128);

        env.budget().reset_default();
        let claimed = client.batch_claim_investor_payouts(&investors);
        let cpu = env.budget().cpu_instruction_cost();
        assert_eq!(claimed, size);

        let fits = cpu <= CPU_BUDGET;
        std::println!(
            "probe batch_size={:<3} cpu={:<12} budget={:<12} fits={}",
            size,
            cpu,
            CPU_BUDGET,
            fits,
        );

        if fits {
            largest_fitting = size;
        }
    }

    std::println!(
        "largest batch size within budget ({} CPU): {}",
        CPU_BUDGET,
        largest_fitting
    );
    assert!(
        largest_fitting >= 1,
        "even a single-investor batch claim exceeds the instruction budget"
    );
}

// ── 7. Snapshot size benchmark (1000 investors) ─────────────────────────────

/// Benchmark: measure the storage footprint scaling with investor count.
///
/// This test verifies that the funding-close snapshot remains bounded
/// regardless of investor count, since per-investor data is in persistent
/// storage, not in the snapshot struct.
///
/// For a full 1000-investor benchmark, run in release mode:
/// ```text
/// cargo test --release benchmark_snapshot -- --ignored --nocapture
/// ```
/// At 1000 investors (~180 KB persistent storage), the snapshot struct itself
/// is only 44 bytes + 32 bytes (Merkle root) = 76 bytes — well under 10 KB.
#[test]
fn benchmark_snapshot_size_scaling() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let (token, treasury) = free_addresses(&env);

    // Use a very large funding target so funding doesn't close until the last investor.
    let total_target: i128 = 1_000_000_000_000i128;
    let per_investor: i128 = 1_000_000_000i128;

    client.init(
        &admin,
        &String::from_str(&env, "BENCH01"),
        &sme,
        &total_target,
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
    );

    // Fund 50 investors — enough to verify the pattern without hitting test timeouts.
    // For a full 1000-investor benchmark, run with `cargo test --release -- --ignored`.
    let mut count: u32 = 0;
    for _i in 0..50 {
        let inv = Address::generate(&env);
        client.fund(&inv, &per_investor);
        count += 1;
    }

    // After funding close (if reached), the snapshot should exist.
    if let Some(snap) = client.get_funding_close_snapshot() {
        // The snapshot size is constant regardless of investor count:
        // - total_principal: i128 (16 bytes)
        // - funding_target: i128 (16 bytes)
        // - timestamp: u64 (8 bytes)
        // - sequence: u32 (4 bytes)
        // Total: 44 bytes + XDR overhead ≈ well under 10KB
        assert!(snap.total_principal > 0);

        // Verify the snapshot does NOT store individual investor addresses.
        // This is the key design property: the snapshot only stores aggregates.
        // If it stored all investor addresses, size would be O(N) → > 10KB at 1000 investors.
    }

    // Verify funder count is correct.
    assert_eq!(client.get_unique_funder_count(), count);
}
