# Contract Interface Versioning

## Purpose

`CONTRACT_INTERFACE_VERSION` identifies the **public ABI surface** of the escrow contract—the set of callable entrypoints, their parameter lists, return types, and event schemas.

Callers (SDKs, integrations, indexers) must verify interface version compatibility **before** invoking state-mutating operations to detect signature mismatches early and avoid silent failures or data corruption.

### Distinction: Interface vs. Schema

| Aspect | Interface Version | Schema Version |
|--------|-------------------|-----------------|
| **Tracks** | Entrypoint signatures, parameters, return types, event shapes | On-chain storage layout (XDR structs, `DataKey` variants) |
| **Read by** | Callers before invoke (governance, SDK, indexers) | Smart contract upgrade logic via `migrate()` |
| **Stored** | Compile-time constant (not on-chain) | On-chain at `DataKey::Version` |
| **Getter** | `get_interface_version()` | `get_version()` |
| **Increment rule** | When ABI surface changes | When storage layout changes |

**Example:** Adding an optional parameter to `fund()` via `Option<T>` may not change schema version (storage is identical) but does change interface version (caller must know new parameter exists).

---

## Versioning Policy

### When to Increment `CONTRACT_INTERFACE_VERSION`

Increment the constant when:

1. **Entrypoint renamed or removed**
   - `withdraw()` → `withdraw_principal()`: **BUMP**
   - Removing `legacy_fund()`: **BUMP**

2. **Parameter added, removed, or retyped**
   - `fund(investor, amount)` → `fund(investor, amount, tier)`: **BUMP**
   - Changing `amount: i128` → `amount: u64`: **BUMP**
   - Adding optional `reason: Option<String>`: **BUMP** (caller SDKs must know to pass it)

3. **Return type changes in a non-backward-compatible way**
   - `fund() -> InvoiceEscrow` → `fund() -> (InvoiceEscrow, u64)`: **BUMP**
   - Removing a field from returned struct: **BUMP**

4. **Event `#[topic]` or shape changes**
   - Adding a required field to an event: **BUMP**
   - Removing an event: **BUMP**
   - Changing event topic coverage (e.g., moving from indexed to non-indexed): **BUMP**

### When NOT to Increment

Do NOT increment for:

- **Adding a new entrypoint** (callers that don't invoke it are unaffected)
- **Adding optional fields to response structs** (must be guarded with `Option<T>`)
- **Adding new `DataKey` variants** (storage-layer change; managed by `SCHEMA_VERSION`)
- **Internal refactors** (no caller-facing change)
- **Documentation or comment updates**

---

## Two-Step Upgrade Flow (`propose_upgrade` / `execute_upgrade`)

### Motivation

The legacy `upgrade(new_wasm_hash)` entrypoint takes effect **immediately** upon admin
authorization. A single compromised admin key can therefore swap the contract code in one
transaction, with no opportunity for monitoring or intervention.

To mitigate this, upgrades are gated behind a **two-step propose/execute flow with a minimum
delay**.

### Entrypoints

| Entrypoint | Auth | Description |
|------------|------|-------------|
| `propose_upgrade(new_wasm_hash: BytesN<32>)` | Admin | Records a pending upgrade proposal and the earliest execution timestamp. Emits an event. |
| `execute_upgrade()` | Admin | Executes the pending proposal, but only once the minimum delay has elapsed. Emits an event. |
| `upgrade(new_wasm_hash: BytesN<32>)` | Admin | **Legacy, gated.** Retained for backward compatibility until the next interface version bump; it now routes through the propose/execute flow rather than applying immediately. |

### Minimum-Delay Semantics

- `propose_upgrade` stores the proposed hash together with `execute_after = now + MIN_UPGRADE_DELAY`.
- `execute_upgrade` may only be called at or after `execute_after`; calling earlier fails with a
  delay-not-elapsed error.
- Proposing again before execution replaces the pending proposal and resets the delay clock.
- The delay is a compile-time constant (`MIN_UPGRADE_DELAY`); it is not caller-configurable.

### Events

Both steps emit events so off-chain monitors and indexers can observe the upgrade lifecycle:

- **Proposal event** — emitted by `propose_upgrade`, carrying the proposed `new_wasm_hash` and the
  `execute_after` timestamp.
- **Execution event** — emitted by `execute_upgrade`, carrying the executed `new_wasm_hash`.

Indexers should alert on proposals and verify that execution only occurs after the delay.

### Interface Version Impact

Adding `propose_upgrade` and `execute_upgrade` is an **additive** change: callers that do not use
the new entrypoints are unaffected, so `CONTRACT_INTERFACE_VERSION` does **not** need to be bumped
for their introduction.

The legacy `upgrade` entrypoint is **not removed** in this change; it is gated behind the new flow.
Removing it (or otherwise changing its signature) is a breaking change and **must** be accompanied
by a `CONTRACT_INTERFACE_VERSION` bump at that time.

---

## Append-Only Policy

- **Never reuse or decrement** a numeric value once published in production.
- Example: If `v1` was deployed to mainnet, the next version must be `v2`, never `v1` again.
- This ensures clients can reliably identify deployed versions across upgrades.

---

## SDK Integration Guidance

### Caller-Side Version Check

SDKs and integration adapters should call `get_interface_version()` at startup:

```rust
// Example: Rust SDK
let deployed_version = client.get_interface_version().await?;
let sdk_version = 1;

if deployed_version != sdk_version {
    return Err(format!(
        "Interface mismatch: SDK compiled for v{}, contract is v{}",
        sdk_version, deployed_version
    ));
}
```

### Error Handling

When version mismatch is detected, SDKs must:

1. **Refuse further calls** to the contract
2. **Surface a diagnostic error** (not panic or silent failure)
3. **Suggest remediation**:
   - "Update your SDK version"
   - "Upgrade contract to matching interface"
   - "Link to migration guide"

### Deployment Verification

Before deploying a new contract version to production:

1. Test that `get_interface_version()` returns the correct constant
2. Verify that all clients have been updated to handle the new version
3. Coordinate with SDK teams on release timing
4. Include version bump in release notes

---

## Examples

### Example 1: Adding a New Optional Parameter (BUMP)

**Before (v1):**
```rust
pub fn fund(env: Env, investor: Address, amount: i128) -> InvoiceEscrow {
    // ...
}
```

**After (v2):**
```rust
pub fn fund(env: Env, investor: Address, amount: i128, memo: Option<String>) -> InvoiceEscrow {
    // ...
}
```

**Decision:** BUMP to v2. Even though `memo` is optional, the signature has changed; callers must know it exists.

### Example 2: Adding a New Entrypoint (NO BUMP)

**Before (v1):**
```rust
// No emergency_pause entrypoint
```

**After (v1 maintained):**
```rust
pub fn emergency_pause(env: Env) {
    // Admin-only
}
```

**Decision:** Keep v1. Existing callers that don't use `emergency_pause` are unaffected.

### Example 5: Adding the Two-Step Upgrade Entrypoints (NO BUMP)

**Before (v1):**
```rust
pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
    // Applies immediately on admin auth
}
```

**After (v1 maintained):**
```rust
pub fn propose_upgrade(env: Env, new_wasm_hash: BytesN<32>) {
    // Records proposal + execute_after; emits proposal event
}

pub fn execute_upgrade(env: Env) {
    // Applies pending proposal after MIN_UPGRADE_DELAY; emits execution event
}

pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
    // Legacy, gated behind the propose/execute flow
}
```

**Decision:** Keep v1. The new entrypoints are additive and the legacy `upgrade` signature is
unchanged. A bump is only required when `upgrade` is eventually removed.

### Example 3: Changing a Return Type (BUMP)

**Before (v1):**
```rust
pub fn settle(env: Env) -> InvoiceEscrow {
    // Returns updated escrow state
}
```

**After (v2):**
```rust
pub fn settle(env: Env) -> (InvoiceEscrow, SettlementReceipt) {
    // Returns escrow + new receipt details
}
```

**Decision:** BUMP to v2. Return type shape changed; callers must update parsing logic.

### Example 4: Renaming an Entrypoint (BUMP)

**Before (v1):**
```rust
pub fn claim_investor_payout(env: Env, investor: Address) -> i128 {
    // ...
}
```

**After (v2):**
```rust
pub fn claim_payout(env: Env, investor: Address) -> i128 {
    // Old name removed; new name is canonical
}
```

**Decision:** BUMP to v2. Entrypoint renamed; SDKs must update invocation site.

---

## Versioning Checklist

Before releasing a new contract version:

- [ ] Confirm all entrypoint signatures remain stable OR version was bumped
- [ ] Confirm no entrypoints were removed OR version was bumped
- [ ] Confirm the legacy `upgrade` entrypoint is still present (or a bump was made when removing it)
- [ ] Confirm return types are stable OR version was bumped
- [ ] Confirm event schemas are stable OR version was bumped
- [ ] Confirm `propose_upgrade`/`execute_upgrade` events are documented for indexers
- [ ] Update `CONTRACT_INTERFACE_VERSION` in code if needed
- [ ] Test `get_interface_version()` returns the correct value
- [ ] Document interface changes in PR description
- [ ] Notify SDK teams of version change (if applicable)
- [ ] Include migration guide if version was bumped

---

## Troubleshooting

### "Interface version mismatch" error

**Symptom:** SDK or caller reports version mismatch.

**Cause:** Contract interface version does not match SDK's compiled version.

**Solution:**
1. Verify deployed contract version: `get_interface_version()`
2. Update SDK to match version or redeploy contract to earlier version
3. Review release notes for breaking changes

### Unknown entrypoint

**Symptom:** Caller invokes entrypoint that doesn't exist.

**Cause:** SDK was compiled against a different interface version than deployed contract.

**Solution:**
1. Call `get_interface_version()` to verify contract version
2. Update SDK to match contract version
3. Review ADRs and migration guides for entrypoint changes

---

## References

- [ADR-002: Authorization Boundaries](adr/ADR-002-auth-boundaries.md) — entrypoint roles
- [docs/OPERATOR_RUNBOOK.md](OPERATOR_RUNBOOK.md) — deployment procedures
- [docs/escrow-error-messages.md](escrow-error-messages.md) — stable error codes
- [Issue #242](https://github.com/example/repo/issues/242) — two-step propose/execute upgrade guard
