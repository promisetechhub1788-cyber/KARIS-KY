# get_investor_cap_status() - Quick Reference

## What Was Built

A new read-only entrypoint that returns the investor capacity status of an escrow in a single call, eliminating error-prone client-side arithmetic.

## Key Files

| File | Purpose | Lines |
|------|---------|-------|
| `escrow/src/lib.rs:831-840` | InvestorCapStatus struct definition | 10 |
| `escrow/src/lib.rs:3492-3513` | get_investor_cap_status() function | 22 |
| `sdk-ts/src/types.ts:83-91` | TypeScript interface definition | 9 |
| `sdk-ts/src/client.ts:351-354` | SDK method wrapper | 4 |
| `escrow/src/tests/cap_validation.rs` | 6 comprehensive tests | 337 |
| `docs/escrow-read-api.md:118-167` | Documentation section | 50 |

## API Overview

### Rust (Contract)
```rust
pub fn get_investor_cap_status(env: Env) -> InvestorCapStatus
```

### TypeScript (SDK)
```typescript
async getInvestorCapStatus(): Promise<InvestorCapStatus>
```

### Return Type
```typescript
interface InvestorCapStatus {
  max: number;          // u32::MAX if unlimited, else configured cap
  current: number;      // current distinct investor count
  remaining: number;    // max - current
  is_full: boolean;     // true when current >= max
}
```

## Examples

### No Cap (Unlimited Investors)
```rust
let status = get_investor_cap_status();
// status.max = 4,294,967,295 (u32::MAX)
// status.current = 42
// status.remaining = 4,294,967,253
// status.is_full = false
```

### With Cap (100 investors, 95 currently)
```rust
let status = get_investor_cap_status();
// status.max = 100
// status.current = 95
// status.remaining = 5
// status.is_full = false
```

### At Capacity (50 cap, 50 current)
```rust
let status = get_investor_cap_status();
// status.max = 50
// status.current = 50
// status.remaining = 0
// status.is_full = true
```

## Key Behaviors

1. **Unlimited Investors:** When no cap is set, returns `max = u32::MAX` and `is_full = false` always
2. **Capped Scenario:** Returns exact cap and remaining capacity
3. **After Cap Lowering:** Immediately reflects new cap value
4. **Distinct Addresses:** Cap applies to unique addresses, not total principal per address
5. **Pure Read-Only:** No auth required, no state mutation, no external calls

## Tests Included

1. `test_investor_cap_status_no_cap_set` - Unlimited investors scenario
2. `test_investor_cap_status_with_cap_and_room` - Capacity with available space
3. `test_investor_cap_status_at_capacity` - Escrow full scenario
4. `test_investor_cap_status_cap_of_one` - Edge case: single investor cap
5. `test_investor_cap_status_large_cap` - Large cap (1000) with 50 investors
6. `test_investor_cap_status_after_cap_lowering` - Admin reduces cap

## Use Cases

✅ **Investor-facing UIs:** Show "X slots remaining" or "at capacity"  
✅ **Integration dashboards:** Monitor investor capacity trends  
✅ **Funding triggers:** Determine if more investors can join  
✅ **Risk monitoring:** Alert when cap is nearing full  

## Not Included (Out of Scope)

❌ Cap enforcement logic (unchanged in fund/fund_with_commitment)  
❌ Cap update entrypoints (read-only only)  
❌ Per-investor caps (different entrypoint)  
❌ Funding target related capacity (different concept)

## Implementation Status

✅ Rust contract code complete  
✅ TypeScript SDK wrapper complete  
✅ Comprehensive test suite complete (6 tests)  
✅ Documentation complete  
✅ Code review ready  
⏳ Build verification (awaiting Rust toolchain)

---

For full details, see:
- Implementation summary: `INVESTOR_CAP_STATUS_IMPLEMENTATION.md`
- Verification report: `INVESTOR_CAP_STATUS_VERIFICATION.md`
- API documentation: `docs/escrow-read-api.md`
