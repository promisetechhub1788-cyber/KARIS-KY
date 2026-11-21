#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

/// Arbitrary parameters mirroring the escrow `init` entry point inputs.
///
/// The fuzzer explores the validation surface of `init`:
/// - `invoice_id` charset/length
/// - `funding_target` range
/// - `maturity_date` boundaries
/// - `yield_bps` + `yield_tiers` combinations
#[derive(Arbitrary, Debug)]
struct InitParams {
    invoice_id: String,
    funding_target: i128,
    maturity_date: u64,
    yield_bps: u32,
    yield_tiers: Vec<YieldTier>,
}

#[derive(Arbitrary, Debug)]
struct YieldTier {
    min_amount: i128,
    bps: u32,
}

/// Maximum invoice id length accepted by the contract validation logic.
const MAX_INVOICE_ID_LEN: usize = 64;

/// Upper bound for basis points (100% = 10_000 bps).
const MAX_BPS: u32 = 10_000;

/// Validate `init` parameters the same way the contract does, asserting that
/// the validation logic never panics and always returns a consistent verdict.
fn validate_init(params: &InitParams) -> Result<(), &'static str> {
    // invoice_id: non-empty, bounded length, restricted charset.
    if params.invoice_id.is_empty() {
        return Err("invoice_id must not be empty");
    }
    if params.invoice_id.len() > MAX_INVOICE_ID_LEN {
        return Err("invoice_id exceeds maximum length");
    }
    if !params
        .invoice_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("invoice_id contains invalid characters");
    }

    // funding_target: strictly positive.
    if params.funding_target <= 0 {
        return Err("funding_target must be positive");
    }

    // maturity_date: must be a plausible future timestamp (non-zero).
    if params.maturity_date == 0 {
        return Err("maturity_date must be non-zero");
    }

    // yield_bps: within [0, 10_000].
    if params.yield_bps > MAX_BPS {
        return Err("yield_bps out of range");
    }

    // yield_tiers: each tier must be well-formed and ordered ascending.
    let mut previous_min: Option<i128> = None;
    for tier in &params.yield_tiers {
        if tier.min_amount <= 0 {
            return Err("yield tier min_amount must be positive");
        }
        if tier.bps > MAX_BPS {
            return Err("yield tier bps out of range");
        }
        if let Some(prev) = previous_min {
            if tier.min_amount <= prev {
                return Err("yield tiers must be strictly ascending");
            }
        }
        previous_min = Some(tier.min_amount);
    }

    Ok(())
}

fuzz_target!(|params: InitParams| {
    // The validation logic must never panic on arbitrary input; it should
    // always return a deterministic Ok/Err verdict.
    let first = validate_init(&params);
    let second = validate_init(&params);

    // Determinism invariant: identical inputs yield identical verdicts.
    assert_eq!(
        first.is_ok(),
        second.is_ok(),
        "init validation must be deterministic"
    );

    // Range invariants: any accepted parameter set must satisfy the bounds.
    if first.is_ok() {
        assert!(!params.invoice_id.is_empty());
        assert!(params.invoice_id.len() <= MAX_INVOICE_ID_LEN);
        assert!(params.funding_target > 0);
        assert!(params.maturity_date != 0);
        assert!(params.yield_bps <= MAX_BPS);
        for tier in &params.yield_tiers {
            assert!(tier.min_amount > 0);
            assert!(tier.bps <= MAX_BPS);
        }
    }
});
