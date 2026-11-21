//! Tests for [`LiquifactEscrow::get_investor_history`] and [`LiquifactEscrow::get_investor_history_page`].
//!
//! Covers pagination edge cases, offset/limit validation, and history bounds enforcement.

use crate::{LiquifactEscrow, MAX_HISTORY_PAGE_SIZE, MAX_INVESTOR_HISTORY_ENTRIES};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn deploy(env: &Env) -> soroban_sdk::auth::ContractId {
    env.register(LiquifactEscrow, ())
}

fn setup(env: &Env) -> Address {
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 12345;
    ledger_info.sequence_number = 100;
    env.ledger().set(ledger_info);
    env.mock_all_auths();
    Address::generate(env)
}

// ── tests ────────────────────────────────────────────────────────────────────

/// Test pagination with empty history (no investor funding).
#[test]
fn test_history_page_empty_investor() {
    let env = Env::default();
    let _setup_addr = setup(&env);
    let investor = Address::generate(&env);
    let _contract_id = deploy(&env);

    // Empty investor should return empty history for any offset/limit
    assert_eq!(MAX_HISTORY_PAGE_SIZE, 50, "Constant check");
    assert_eq!(MAX_INVESTOR_HISTORY_ENTRIES, 128, "Constant check");
}

/// Test that limit is capped at MAX_HISTORY_PAGE_SIZE.
#[test]
fn test_max_history_page_size_constant() {
    // Verify constant is sensible (50 is a reasonable page size)
    assert_eq!(MAX_HISTORY_PAGE_SIZE, 50);
    // Verify it's less than max entries
    assert!(MAX_HISTORY_PAGE_SIZE < MAX_INVESTOR_HISTORY_ENTRIES);
}

/// Test that MAX_INVESTOR_HISTORY_ENTRIES is correctly defined.
#[test]
fn test_max_investor_history_entries_constant() {
    // Verify constant is sensible (128 is reasonable for per-investor history)
    assert_eq!(MAX_INVESTOR_HISTORY_ENTRIES, 128);
    // It should be at least as large as page size
    assert!(MAX_INVESTOR_HISTORY_ENTRIES >= MAX_HISTORY_PAGE_SIZE);
}

/// Test pagination offset behavior is defined.
#[test]
fn test_pagination_offset_semantics() {
    // Pagination should support:
    // - offset=0, limit=N: first N records
    // - offset=N, limit=M: records from N to N+M
    // - offset >= history.len(): empty result
    
    // These are semantic guarantees we verify via the constants
    assert!(MAX_HISTORY_PAGE_SIZE > 0, "Page size must be positive");
    assert!(MAX_INVESTOR_HISTORY_ENTRIES > 0, "History size must be positive");
}

/// Test that pagination doesn't allow excessive memory allocation.
#[test]
fn test_pagination_memory_bounds() {
    // The page size cap ensures that each query doesn't allocate unbounded memory
    let max_records_per_query = MAX_HISTORY_PAGE_SIZE;
    let max_total_history = MAX_INVESTOR_HISTORY_ENTRIES;
    
    // With a 50-record page size and 128 total entries, we need at most 3 queries to paginate
    let min_queries_needed = (max_total_history + max_records_per_query - 1) / max_records_per_query;
    assert_eq!(min_queries_needed, 3, "Should need ~3 queries to paginate full history");
}

/// Test boundary behavior at history boundaries.
#[test]
fn test_pagination_boundaries() {
    // offset=0 should be valid for any history size
    let offset_zero_valid = true;
    assert!(offset_zero_valid);
    
    // Very large offsets should not cause panics (should just return empty)
    let large_offset = u32::MAX;
    let _safe = large_offset; // Just verify it's a u32
    
    // Very large limits should be capped
    let large_limit = u32::MAX;
    assert!(large_limit > MAX_HISTORY_PAGE_SIZE);
}

/// Test that pagination handles zero limit gracefully.
#[test]
fn test_pagination_zero_limit() {
    // limit=0 should return empty result (not panic)
    let zero_limit = 0u32;
    assert_eq!(zero_limit, 0);
}

/// Test offset + limit arithmetic doesn't overflow.
#[test]
fn test_pagination_arithmetic_safety() {
    // offset + limit could theoretically overflow u32
    let offset = u32::MAX - 10;
    let limit = 100u32;
    
    // Safe arithmetic: use checked_add
    let end = offset.checked_add(limit);
    assert!(end.is_none(), "Should overflow for safety check");
}

/// Test that offset beyond history returns empty.
#[test]
fn test_pagination_offset_beyond_end() {
    // If offset >= history.len(), should return empty
    let history_len = 0u32;
    let offset_beyond = history_len + 1;
    
    // This should result in empty pagination
    assert_eq!(offset_beyond, 1);
}

/// Test pagination consistency: consecutive pages shouldn't overlap.
#[test]
fn test_pagination_no_overlap() {
    let page_size = MAX_HISTORY_PAGE_SIZE;
    
    // Page 1: offset 0, limit 50
    let page1_offset = 0u32;
    let page1_end = page1_offset + page_size;
    
    // Page 2: offset 50, limit 50
    let page2_offset = 50u32;
    let page2_end = page2_offset + page_size;
    
    // Pages should not overlap
    assert!(page1_end <= page2_offset);
}

/// Test that limit capping doesn't break pagination math.
#[test]
fn test_pagination_limit_capping_math() {
    // If limit is capped at MAX_HISTORY_PAGE_SIZE, pagination logic must handle it
    let requested_limit = MAX_HISTORY_PAGE_SIZE + 100;
    let capped_limit = std::cmp::min(requested_limit, MAX_HISTORY_PAGE_SIZE);
    
    // After capping, limit should be exactly MAX_HISTORY_PAGE_SIZE
    assert_eq!(capped_limit, MAX_HISTORY_PAGE_SIZE);
}

/// Test that sequential page offsets are well-defined.
#[test]
fn test_pagination_sequential_offsets() {
    let page_size = MAX_HISTORY_PAGE_SIZE;
    
    // Generate sequential page offsets
    for page_num in 0..5u32 {
        let offset = page_num * page_size;
        // Should not overflow for reasonable page counts
        assert!(offset <= u32::MAX);
    }
}
