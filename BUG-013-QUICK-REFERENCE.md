# BUG-013 Quick Reference Guide

## What Was Fixed?
FundingCloseSnapshot did not validate that the snapshot timestamp was before the escrow's maturity date. This could cause a logically inconsistent state where funding closed after the invoice maturity.

## How Was It Fixed?
1. Added validation when FundingCloseSnapshot is created
2. If `closed_at_ledger_timestamp >= maturity`, emit warning type 4004
3. Warning includes metrics (funded ratio, time past maturity) for monitoring

## Key Code Locations

### Implementation
- **Main fund path:** `/workspaces/KARIS-KY/escrow/src/lib.rs:5485-5509`
- **Partial settle path:** `/workspaces/KARIS-KY/escrow/src/lib.rs:5674-5698`

### Tests
- **All tests:** `/workspaces/KARIS-KY/escrow/src/tests/funding.rs:3638-3874`
- 4 tests covering all scenarios

### Documentation
- **ADR-008 updated:** `docs/adr/ADR-008-escrow-health-warnings.md:51`
- Warning type 4004 now defined as "FundingClosedAfterMaturity"

## Verification
To verify the fix works:
```bash
cd /workspaces/KARIS-KY
cargo test test_funding_close_snapshot_validates_against_maturity --lib -- --nocapture
cargo test test_partial_settle_close_snapshot_validates_against_maturity --lib -- --nocapture
cargo test test_funding_close_snapshot_before_maturity_no_warning --lib -- --nocapture
cargo test test_funding_close_snapshot_no_maturity_constraint --lib -- --nocapture
```

## What Changed?
- Added ~100 lines of validation code (2 locations, both handling same logic)
- Added ~243 lines of test code (4 comprehensive test cases)
- Updated 1 line in ADR-008 documentation
- No breaking changes, no storage mutations, event-only

## Event Details
When snapshot timestamp >= maturity, `EscrowHealthWarning` is emitted with:
- `name`: "hlth_wrn"
- `warning_type`: 4004 (FundingClosedAfterMaturity)
- `funded_ratio_bps`: Funded amount as basis points (0-10000+)
- `time_to_maturity_secs`: Negative value (seconds past maturity)
- `recorded_at_ledger_timestamp`: The snapshot timestamp

## Example Scenario
```
Init escrow with maturity = now - 5000 seconds (past)
Fund to target → FundingCloseSnapshot created with current timestamp
Validation: now >= maturity? YES ✓
Emit warning 4004 with time_to_maturity_secs = -5000 seconds
Off-chain systems detect warning and alert operators
```

## Related Documentation
- **ADR-008:** Health Warning System - `docs/adr/ADR-008-escrow-health-warnings.md`
- **Main README:** Search for "FundingCloseSnapshot"
- **Init Parameters:** `docs/escrow-init-parameters.md`
