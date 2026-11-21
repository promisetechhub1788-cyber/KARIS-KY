# Yield Claim Delegation - API Reference

## Overview

This document is the API reference for yield claim delegation. For an
investor-facing walkthrough (use cases, revocation, and CLI/SDK examples),
see the delegation section in `investor-quick-start.md`.

## Public Entrypoints

### 1. `set_yield_claim_delegate`

Sets or overwrites an investor's yield claim delegation.

```rust
pub fn set_yield_claim_delegate(env: Env, investor: Address, delegate: Address)
```

**Parameters:**
- `investor`: Address of the investor (must sign)
- `delegate`: Address to delegate claim rights to

**Authorization:**
- Requires: `investor.require_auth()`
- Caller: Investor or their authorized representative

**Validation:**
- ✅ `delegate ≠ investor` (error 165)

**Side Effects:**
- Sets `DataKey::YieldClaimDelegate(investor)` → `delegate`
- Clears `DataKey::YieldClaimDelegateRevoked(investor)` → `false`
- Emits `YieldClaimDelegationSet` event

**Errors:**
```rust
EscrowError::DelegateAddressSameAsInvestor = 165
```

**Example:**
```rust
let investor = Address::generate(&env);
let multisig = Address::generate(&env);
client.set_yield_claim_delegate(&investor, &multisig);
```

---

### 2. `revoke_yield_claim_delegate`

Revokes an investor's yield claim delegation.

```rust
pub fn revoke_yield_claim_delegate(env: Env, investor: Address)
```

**Parameters:**
- `investor`: Address of the investor (must sign)

**Authorization:**
- Requires: `investor.require_auth()`
- Caller: Investor

**Validation:**
- ✅ Active delegation exists (error 168)
- ✅ Delegation not already revoked

**Side Effects:**
- Sets `DataKey::YieldClaimDelegateRevoked(investor)` → `true`
- Preserves `DataKey::YieldClaimDelegate(investor)` (for audit)
- Emits `YieldClaimDelegationRevoked` event

**Errors:**
```rust
EscrowError::NoActiveDelegation = 168
```

**Example:**
```rust
let investor = Address::generate(&env);
client.revoke_yield_claim_delegate(&investor);
```

---

### 3. `claim_investor_payout_as_delegate`

Delegate claims an investor's payout after settlement.

```rust
pub fn claim_investor_payout_as_delegate(
    env: Env,
    investor: Address,
    delegate: Address,
)
```

**Parameters:**
- `investor`: Address of the investor who contributed
- `delegate`: Address claiming on behalf of investor (must sign)

**Authorization:**
- Requires: `delegate.require_auth()`
- Caller: The delegated address

**Validation:**
- ✅ Escrow not under legal hold
- ✅ Investor has contribution > 0 (error 126)
- ✅ Delegation exists (error 166)
- ✅ Delegation not revoked (error 167)
- ✅ Provided delegate matches stored delegate (error 166)
- ✅ Escrow is settled (status = 2, error 127)
- ✅ Claim lock expired (error 128)

**Side Effects:**
- Sets `DataKey::InvestorClaimed(investor)` → `true` (if not already)
- Emits `InvestorPayoutClaimed` event (same as `claim_investor_payout`)

**Idempotency:**
- Multiple calls from same delegate are no-op (returns silently)
- Investor can also call `claim_investor_payout` directly (both set same flag)

**Errors:**
```rust
EscrowError::LegalHoldBlocksInvestorClaims = 125
EscrowError::NoContributionToClaim = 126
EscrowError::InvestorClaimNotSettled = 127
EscrowError::InvestorCommitmentLockNotExpired = 128
EscrowError::NoDelegationSet = 166
EscrowError::DelegationRevoked = 167
```

**Example:**
```rust
let investor = Address::generate(&env);
let delegate = Address::generate(&env);

// Investor sets delegation
env.mock_all_auths();
client.set_yield_claim_delegate(&investor, &delegate);

// Investor funds
client.fund(&investor, &5000);

// SME settles after maturity
client.settle();

// Delegate claims payout
env.mock_auths(&[MockAuth {
    address: delegate.clone(),
    invoke: MockInvoke::Contract(..),
    ..Default::default()
}]);
client.claim_investor_payout_as_delegate(&investor, &delegate);
```

---

## Public Query Functions

### 4. `get_yield_claim_delegate`

Reads the currently set delegate for an investor, if any.

```rust
pub fn get_yield_claim_delegate(env: Env, investor: Address) -> Option<Address>
```

**Returns:**
- `Some(address)`: If delegation is set (whether active or revoked)
- `None`: If no delegation has ever been set

**Authorization:**
- None (read-only)

**Example:**
```rust
let investor = Address::generate(&env);
match client.get_yield_claim_delegate(&investor) {
    Some(delegate) => println!("Delegated to: {}", delegate),
    None => println!("No delegation set"),
}
```

---

### 5. `is_yield_claim_delegate_revoked`

Checks if a delegation has been revoked.

```rust
pub fn is_yield_claim_delegate_revoked(env: Env, investor: Address) -> bool
```

**Returns:**
- `true`: If delegation exists and has been revoked
- `false`: If delegation never set, or active, or not yet revoked

**Authorization:**
- None (read-only)

**Example:**
```rust
let investor = Address::generate(&env);
if client.is_yield_claim_delegate_revoked(&investor) {
    println!("Delegation is revoked");
} else {
    println!("Delegation active or not set");
}
```

---

## Related Existing Functions

### `claim_investor_payout`

Unchanged. Investor can still claim directly (authorization: `investor.require_auth()`).

```rust
pub fn claim_investor_payout(env: Env, investor: Address)
```

**Behavior:**
- Investor claims their own payout (no delegation)
- Sets same `InvestorClaimed` flag as delegate claim
- Emits same `InvestorPayoutClaimed` event

---

### `compute_investor_payout`

Unchanged. Computes gross pro-rata payout (read-only).

```rust
pub fn compute_investor_payout(env: Env, investor: Address) -> i128
```

**Returns:**
- Pro-rata payout amount (same whether claimed by investor or delegate)
- Formula: See ADR-005 or `docs/escrow-pro-rata.md`

---

## Events

### `YieldClaimDelegationSet`

Emitted when delegation is set.

```rust
pub struct YieldClaimDelegationSet {
    #[topic]
    pub name: Symbol,          // symbol_short!("del_set")
    #[topic]
    pub investor: Address,     // Who delegated
    #[topic]
    pub delegate: Address,     // Delegated to whom
}
```

**Emitted by:** `set_yield_claim_delegate`

**Indexing:** Topics = (name, investor, delegate) ⇒ can filter by investor or delegate

---

### `YieldClaimDelegationRevoked`

Emitted when delegation is revoked.

```rust
pub struct YieldClaimDelegationRevoked {
    #[topic]
    pub name: Symbol,          // symbol_short!("del_rev")
    #[topic]
    pub investor: Address,     // Who revoked
}
```

**Emitted by:** `revoke_yield_claim_delegate`

**Indexing:** Topics = (name, investor) ⇒ can filter by investor

---

## Error Codes

### Delegation-Specific Errors

| Code | Enum | Meaning | Resolution |
|------|------|---------|-----------|
| 165 | `DelegateAddressSameAsInvestor` | Delegate = investor | Use different address |
| 166 | `NoDelegationSet` | No delegation exists (or wrong delegate) | Set delegation first |
| 167 | `DelegationRevoked` | Delegation was revoked | Investor must re-set |
| 168 | `NoActiveDelegation` | No active delegation to revoke | Set delegation first |

### Related Errors (Can Occur During Delegation Claim)

| Code | Enum | Meaning | Resolution |
|------|------|---------|-----------|
| 125 | `LegalHoldBlocksInvestorClaims` | Legal hold active | Wait for admin to clear |
| 126 | `NoContributionToClaim` | Investor never contributed | Fund first |
| 127 | `InvestorClaimNotSettled` | Escrow not settled yet | Wait for settlement |
| 128 | `InvestorCommitmentLockNotExpired` | Claim lock still active | Wait for lock to expire |

---

## State Machine

### Investor Delegation States

```
┌─────────────────────────────────────────┐
│      No Delegation (Initial)            │
│  • Keys absent                          │
│  • Claim by: investor only              │
└─────────────────────────────────────────┘
            │
            │ set_yield_claim_delegate()
            ↓
┌─────────────────────────────────────────┐
│     Active Delegation                   │
│  • YieldClaimDelegate(investor) = X     │
│  • YieldClaimDelegateRevoked = false    │
│  • Claim by: investor OR delegate       │
└─────────────────────────────────────────┘
            │
            ├─→ revoke_yield_claim_delegate()
            │           ↓
            │   ┌─────────────────────────────────────────┐
            │   │     Revoked Delegation                  │
            │   │  • YieldClaimDelegate(investor) = X     │
            │   │  • YieldClaimDelegateRevoked = true     │
            │   │  • Claim by: investor only              │
            │   └─────────────────────────────────────────┘
            │
            └─→ set_yield_claim_delegate()
                        ↓
                (back to Active Delegation)
```

**Notes:**
- Revocation is not deletion: the delegate address is retained for audit.
- Re-setting a delegation clears the revoked flag and activates the new delegate.
- The investor can always claim directly regardless of delegation state.
