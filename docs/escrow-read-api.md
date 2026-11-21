# Escrow Read API

Soroban read-only entry points on `LiquifactEscrow`. All functions take `env: Env` and are
view-only (no state mutation, no auth required).

---

## `get_funding_token() → Address`

**Storage key:** `DataKey::FundingToken`

Returns the SEP-41 token contract address bound to this escrow instance at `init`.

- **Immutable** — set once at `init`; cannot change after deploy.
- Panics with `"Funding token not set"` if called before `init`.
- This is the only token that `sweep_terminal_dust` may transfer to the treasury.

---

## `get_treasury() → Address`

**Storage key:** `DataKey::Treasury`

Returns the protocol treasury address that receives terminal dust sweeps.

- **Immutable** — set once at `init`; cannot change after deploy.
- Panics with `"Treasury not set"` if called before `init`.
- The treasury must authorize `sweep_terminal_dust`; the admin cannot sweep unless it is also the treasury.

---

## `get_registry_ref() → Option<Address>`

**Storage key:** `DataKey::RegistryRef`

Returns the optional registry contract address supplied at `init`, or `None` when absent.

### Non-authority model

`RegistryRef` is a **read-only discoverability hint** for off-chain indexers only.

- No on-chain logic in this contract reads or calls this address.
- Its presence **does not** prove registry membership; call the registry contract directly to
  verify any on-chain claim.
- `None` is a valid, fully operational state — registry integration is optional.
- The key is omitted from instance storage entirely when `registry = None` at `init`, so
  `get_registry_ref()` on an uninitialized contract also returns `None`.

---

## `get_registry_listing() → Option<RegistryListing>`

**Storage key:** `DataKey::RegistryListing`

Returns the cached registry listing metadata for this escrow, or `None` when the escrow has not
been listed in a registry. This is a read-only convenience view; it does not call the registry
contract and does not prove on-chain membership.

### Return Type: `RegistryListing`

A `#[contracttype]` struct containing:

- `registry: Address` — Registry contract address the listing belongs to.
- `listing_id: u64` — Registry-assigned listing identifier.
- `listed_at: u64` — Ledger timestamp when the listing was recorded.

### Example

```rust
match get_registry_listing() {
    Some(listing) => {
        // listing.registry, listing.listing_id, listing.listed_at
    }
    None => {
        // escrow is not listed in any registry
    }
}
```

---

## `get_escrow() → InvoiceEscrow`

**Storage key:** `DataKey::Escrow`

Returns the full escrow snapshot. Panics with `"Escrow not initialized"` before `init`.

---

## `get_escrow_summary() → EscrowSummary`

Aggregates the most commonly read escrow fields into a single host invocation, so indexers and
UIs can render a summary without issuing one call per field. View-only (no auth, no state writes).

### Return Type: `EscrowSummary`

A `#[contracttype]` struct containing:

- `status: u32` — Current escrow status (0 = open, 1 = funded, 2 = settled, etc.).
- `funded_amount: i128` — Total principal contributed so far.
- `target_amount: i128` — Funding target from the escrow snapshot.
- `unique_funder_count: u32` — Distinct addresses that have contributed.
- `maturity: u64` — Maturity timestamp (`0` means no maturity lock).
- `legal_hold: bool` — Whether a compliance hold is active.

### Behavior

- **Safe fallback** — each field matches its individual getter exactly.
- **Panics only** when the escrow itself is uninitialized, consistent with `get_escrow`.

### Example

```rust
let summary = get_escrow_summary();
// summary.status, summary.funded_amount, summary.target_amount,
// summary.unique_funder_count, summary.maturity, summary.legal_hold
```

---

## `get_version() → u32`

**Storage key:** `DataKey::Version`

Returns the current schema version (`SCHEMA_VERSION`). Returns `0` before `init`.

---

## `get_legal_hold_status() → bool`

**Storage key:** `DataKey::LegalHold`

Returns `true` when a compliance hold is active. Defaults to `false` when the key is absent.
This lightweight read-only entrypoint returns only the hold status, without loading the full
`InvoiceEscrow` snapshot. No authorization is required.

The legacy `get_legal_hold() → bool` entrypoint remains available and returns the same value.

---

## `has_maturity_lock() → bool`

Derived from `DataKey::Escrow.maturity`.

Returns `true` when `maturity > 0` and `settle()` is gated by
`Env::ledger().timestamp() >= maturity`. Returns `false` when `maturity == 0`,
which means there is no maturity time lock and a funded escrow can settle
immediately if SME auth, status, and legal-hold checks pass.

---

## `get_min_contribution_floor() → i128`

**Storage key:** `DataKey::MinContributionFloor`

Returns the per-call funding floor in token base units. `0` means no extra floor.

---

## `get_max_unique_investors_cap() → Option<u32>`

**Storage key:** `DataKey::MaxUniqueInvestorsCap`

Returns the optional cap on distinct investor addresses. `None` means unlimited.

---

## `get_max_per_investor_cap() → Option<i128>`

**Storage key:** `DataKey::MaxPerInvestorCap`

Returns the optional immutable cap on cumulative principal for a single investor. `None` means unlimited.

---

## `get_unique_funder_count() → u32`

**Storage key:** `DataKey::UniqueFunderCount`

Returns the count of distinct addresses that have contributed principal. Initialized to `0` at `init`.

---

## `get_funding_stats() → FundingStats`

Aggregates funding progress into a single host invocation so UIs and indexers can render a
progress bar without client-side arithmetic. View-only (no auth, no state writes).

### Return Type: `FundingStats`

A `#[contracttype]` struct containing:

- `funded_amount: i128` — Total principal contributed so far.
- `target_amount: i128` — Funding target from the escrow snapshot.
- `remaining_amount: i128` — `target_amount - funded_amount`, floored at `0`.
- `unique_funder_count: u32` — Distinct addresses that have contributed.
- `is_funded: bool` — `true` once `funded_amount >= target_amount`.

### Behavior

- **Safe fallback** — each field matches its individual getter exactly.
- **Panics only** when the escrow itself is uninitialized, consistent with `get_escrow`.

### Example

```rust
let stats = get_funding_stats();
// stats.funded_amount = 250_000
// stats.target_amount = 1_000_000
// stats.remaining_amount = 750_000
// stats.unique_funder_count = 12
// stats.is_funded = false
```

---

## `get_investor_cap_status() → InvestorCapStatus`

Returns a self-explanatory, single-call answer to "can more investors contribute?" without requiring
client-side arithmetic on `get_max_unique_investors_cap()` and `get_unique_funder_count()`.

### Use case

Investor-facing UIs need to display whether an escrow has room for new contributions. This entrypoint
eliminates error-prone client-side computation.

### Return Type: `InvestorCapStatus`

A `#[contracttype]` struct containing:

- `max: u32` — Maximum number of distinct investors allowed. Returns `u32::MAX` (4,294,967,295) when no cap is set (unlimited).
- `current: u32` — Current number of distinct investors that have contributed.
- `remaining: u32` — Remaining capacity for new investors, computed as `max - current`.
- `is_full: bool` — `true` when `current == max` (escrow is at capacity for new investors).

### Behavior

- **No cap set:** When `get_max_unique_investors_cap()` returns `None`, `max` is set to `u32::MAX` and `is_full` is always `false`.
- **Cap exists:** When a cap is configured, `remaining = max - current`. When `current >= max`, `is_full = true`.
- **Existing investors:** An escrow at capacity (`is_full = true`) can still accept additional principal from existing investors; the cap applies to **distinct addresses**, not total principal per address.
- **Cap changes:** If the admin calls `lower_max_unique_investors`, the status reflects the new lower cap immediately.

### Examples

```rust
// Escrow with no cap (unlimited investors)
let status = get_investor_cap_status();
// status.max = u32::MAX
// status.current = 42
// status.remaining = u32::MAX - 42
// status.is_full = false

// Escrow with cap of 100, 95 investors so far
let status = get_investor_cap_status();
// status.max = 100
// status.current = 95
// status.remaining = 5
// status.is_full = false

// Escrow with cap of 50, exactly 50 investors
let status = get_investor_cap_status();
// status.max = 50
// status.current = 50
// status.remaining = 0
// status.is_full = true
```

---

## `get_investor_position(investor: Address) → InvestorPosition`

Aggregates every per-investor read into a single host invocation, so indexers and UIs can render
an investor's full position without issuing one call per field. View-only (no auth, no state writes).

### Return Type: `InvestorPosition`

A `#[contracttype]` struct containing:

- `contribution: i128` — Cumulative principal contributed by `investor`. **Storage key:** `DataKey::InvestorContribution(investor)`. `0` when absent.
- `effective_yield_bps: i64` — Effective annualized yield (bps) locked in at the investor's first deposit. **Storage key:** `DataKey::InvestorEffectiveYield(investor)`. Falls back to `InvoiceEscrow::yield_bps` when the key is absent (legacy positions).
- `claim_not_before: u64` — Earliest ledger timestamp at which the investor may call `claim_investor_payout`. **Storage key:** `DataKey::InvestorClaimNotBefore(investor)`. `0` means no extra gate beyond settled status.
- `claimed: bool` — Whether the investor has exercised `claim_investor_payout`. **Storage key:** `DataKey::InvestorClaimed(investor)`. Defaults to `false`.

### Behavior

- **Safe fallback** — each field matches its individual getter exactly, returning the same default when the optional key is absent.
- **No panic on unknown investor** — an address that has never contributed yields an all-default position (`contribution = 0`, `claimed = false`, etc.).
- **Panics only** when the escrow itself is uninitialized, consistent with `get_escrow_summary`.

### Relationship to individual getters

The individual entrypoints remain available and are the source of truth for each field:

- `get_contribution(investor)`
- `get_investor_yield_bps(investor)`
- `get_investor_claim_not_before(investor)`
- `is_investor_claimed(investor)`

`get_investor_position` is a convenience aggregate over these; it introduces no new storage keys.

---

## `get_investor_history(investor: Address) → Vec<InvestorEvent>`

Returns the ordered contribution/claim event log for `investor`, oldest first. View-only (no auth,
no state writes).

### Return Type: `Vec<InvestorEvent>`

Each `InvestorEvent` is a `#[contracttype]` struct containing:

- `kind: u32` — Event kind (0 = contribution, 1 = claim).
- `amount: i128` — Principal contributed or payout claimed.
- `timestamp: u64` — Ledger timestamp of the event.

### Behavior

- **Empty vector** — an address that has never interacted yields `vec![]` (no panic).
- **Panics only** when the escrow itself is uninitialized, consistent with `get_escrow`.

### Example

```rust
let history = get_investor_history(investor.clone());
for event in history.iter() {
    // event.kind, event.amount, event.timestamp
}
```

---

## `get_contribution(investor: Address) → i128`

**Storage key:** `DataKey::InvestorContribution(investor)`

Returns the cumulative principal contributed by `investor`. `0` when absent.

---

## `get_funding_close_snapshot() → Option<FundingCloseSnapshot>`

**Storage key:** `DataKey::FundingCloseSnapshot`

Returns the pro-rata denominator snapshot captured when the escrow first became **funded** (status 1).
`None` until that transition. Immutable once written.

---

## `get_funding_close_merkle_root() → Option<BytesN<32>>`

**Storage key:** `DataKey::FundingCloseMerkleRoot`

Returns the Merkle root committing to the set of contributions captured at funding close, or `None`
until the escrow first becomes **funded**. Immutable once written. Off-chain verifiers can use this
root to prove an investor's inclusion without re-reading every contribution.

### Example

```rust
match get_funding_close_merkle_root() {
    Some(root) => {
        // root: BytesN<32> — verify inclusion proofs against this
    }
    None => {
        // escrow has not reached funded status yet
    }
}
```

---

## `detect_state_inconsistencies() → Vec<StateInconsistency>`

Runs a read-only consistency audit over the escrow's stored state and returns every detected
inconsistency. View-only (no auth, no state writes). Intended for monitoring and indexer health checks.

### Return Type: `Vec<StateInconsistency>`

Each `StateInconsistency` is a `#[contracttype]` struct containing:

- `code: u32` — Machine-readable inconsistency code.
- `detail: Symbol` — Short human-readable tag describing the affected field.

### Behavior

- **Empty vector** — a consistent escrow yields `vec![]`.
- **No panic** — returns an empty vector rather than panicking when the escrow is uninitialized.

### Example

```rust
let issues = detect_state_inconsistencies();
if !issues.is_empty() {
    for issue in issues.iter() {
        // issue.code, issue.detail
    }
}
```

---

## `query_attestation_logs(from: u32, limit: u32) → Vec<AttestationLogEntry>`

Returns a paginated slice of the on-chain attestation log, starting at index `from` and returning
at most `limit` entries. View-only (no auth, no state writes).

### Return Type: `Vec<AttestationLogEntry>`

Each `AttestationLogEntry` is a `#[contracttype]` struct containing:

- `index: u32` — Position of the entry in the log.
- `attestor: Address` — Address that submitted the attestation.
- `payload_hash: BytesN<32>` — Hash of the attested payload.
- `timestamp: u64` — Ledger timestamp when the attestation was recorded.

### Behavior

- **Out-of-range `from`** — returns `vec![]` rather than panicking.
- **`limit`** — clamped to the remaining entries; `0` yields `vec![]`.

### Example

```rust
// First page of 50 entries
let page = query_attestation_logs(0, 50);
for entry in page.iter() {
    // entry.index, entry.attestor, entry.payload_hash, entry.timestamp
}
```

---

## `get_investor_yield_bps(investor: Address) → i64`

**Storage key:** `DataKey::InvestorEffectiveYield(investor)`

Returns the effective annualized yield (bps) locked in at the investor's first deposit. Falls back
to `InvoiceEscrow::yield_bps` when the key is absent (legacy positions).
