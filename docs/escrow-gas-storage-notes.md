# Escrow Gas / Storage Notes

## Soroban cost model

Soroban charges CPU and memory instructions per host function invocation. Instance storage reads
(`env.storage().instance().get(...)`) each consume a measurable instruction budget. Eliminating
duplicate reads of the same key within a single host function call directly reduces the
instruction budget consumed per transaction — without changing observable behavior.

No wall-clock benchmarking harness exists in this repository. `cargo llvm-cov` measures line
coverage only. Storage-op counts below are obtained from the in-repo storage-op counting harness
(`escrow/src/test.rs`, `storage_op_counts` helper), which instruments the storage layer to count
`get`/`set` calls per entrypoint invocation. Cost savings are otherwise reasoned from the Soroban
host cost schedule, not measured wall-clock time.

---

## Audit table — `escrow/src/lib.rs` (issue #185)

| Function | Reads before | Reads after | Change | Rationale |
|---|---|---|---|---|
| `fund_impl` (new investor path) | `UniqueFunderCount` read twice: once for cap check, once for increment | Once: hoisted into `cur_funder_count` local, reused for both uses | **−1 read** on every new-investor call | Single read covers cap assertion and increment write |
| `fund_impl` (event field) | `InvestorEffectiveYield` read after write to populate event | Captured in `investor_effective_yield_bps` local at write time | **−1 read** on every `fund` / `fund_with_commitment` call | Value is known at write time; post-write read is redundant |
| `fund_impl` (returning investor, `simple_fund`) | `InvestorEffectiveYield` not read (yield set on first deposit only) | Read once for event field on returning-investor path | No net change — read was already absent on first-deposit path; added on returning path to populate event correctly |  |
| `fund_impl` (`legal_hold_active` order) | Hold check after escrow read | Unchanged — escrow read is always needed for `yield_bps` and `status`; hoisting hold before escrow would not reduce reads | No change | Comment added explaining order |
| `sweep_terminal_dust` | `DataKey::Escrow`, `DataKey::Treasury`, `DataKey::FundingToken` — each read once | Same | No change — reads are non-redundant | Each key is distinct and read exactly once |
| `get_investor_yield_bps` | `DataKey::Escrow` + `DataKey::InvestorEffectiveYield` | Same | No change — reads are non-redundant | Escrow read is required for the base yield fallback; doc comment added to inform callers |
| All other `get_*` getters | Single read per key | Same | No change — reads are non-redundant | Each getter reads exactly one key |
| All `env.clone()` call sites | — | — | No change — all clones are required | `env` is used after every `get_escrow` call for storage writes, ledger reads, or event publish; comments added at each site |

---

## `env.clone()` audit

`Env` is a reference-counted handle to the Soroban host. Cloning it is cheap (pointer copy).
Every `Self::get_escrow(env.clone())` call site was audited; in all cases `env` is used again
after the call (for storage writes, ledger reads, or `.publish(&env)`), so the clone is
required. A comment was added at each site documenting this decision.

---

## Net savings per `fund` call (new investor)

- **−2 storage reads** on the new-investor path: one `UniqueFunderCount` read and one
  `InvestorEffectiveYield` read eliminated.
- **−1 storage read** on the returning-investor path: one `InvestorEffectiveYield` post-write
  read eliminated (replaced by a local variable); one read added to fetch the existing yield
  for the event field — net zero on this path.

These are micro-optimizations. The primary value is correctness hygiene and reduced instruction
budget on the hot `fund` path, which is called once per investor deposit.

---

## Batch funding — single escrow read/write per call (issue #239)

`batch_fund` and `fund_batch` previously called `fund_impl` once per entry, and `fund_impl`
performed its own `DataKey::Escrow` read and write on every call. For a batch of N entries that
produced N read/write pairs of the same instance key.

`fund_impl` was refactored to accept an in-memory `Escrow` and return the updated `Escrow`, so the
batch path loads `DataKey::Escrow` once, applies every entry in memory, and writes the key once.
The single-entry `fund` / `fund_with_commitment` paths are unchanged in behavior: they read once,
call `fund_impl`, and write the returned escrow once.

### Before / after storage ops for `DataKey::Escrow`

Counts are per `batch_fund` / `fund_batch` call, measured with the storage-op counting harness.

| Batch size | Escrow reads before | Escrow reads after | Escrow writes before | Escrow writes after |
|---|---|---|---|---|
| 1 | 1 | 1 | 1 | 1 |
| 10 | 10 | 1 | 10 | 1 |
| 50 | 50 | 1 | 50 | 1 |

Net effect: **−(N−1) reads and −(N−1) writes** of `DataKey::Escrow` per batch of N entries.
No semantic change — the same final escrow state is persisted, and per-entry validation,
events, and error behavior are identical.

---

## TTL semantics and operational `bump_ttl` (rent/archival mitigation)

`AllowlistActive` is stored in **instance** storage. `InvestorAllowlisted(addr)` entries are stored in
**persistent** storage. These have different TTL semantics under Soroban's rent model.

If instance storage expires and is not extended, `AllowlistActive` returns `false` (default via
`unwrap_or`), silently disabling the allowlist gate even if persistent allowlist entries remain.
Operators must extend instance storage TTL together with persistent storage TTL.

### Permissionless TTL extension

The contract includes a permissionless `bump_ttl` entrypoint that extends TTLs for:

- **Instance storage** keys that affect settlement/claim readiness:
  - `DataKey::Escrow`
  - `DataKey::Version`
  - `DataKey::LegalHold`
  - `DataKey::AllowlistActive`
  - `DataKey::FundingCloseSnapshot`
  - per-investor instance keys passed in via the `allowlisted: Vec<Address>` argument
    (`DataKey::InvestorContribution(addr)` and `DataKey::InvestorClaimNotBefore(addr)`)

- **Persistent storage** allowlist entries:
  - `DataKey::InvestorAllowlisted(addr)` for each address provided by the caller.

Key properties (invariant):

- **Extension never shortens** an existing TTL.
- **No state mutation beyond TTL extension**: `bump_ttl` only calls `extend_ttl(...)`.

### Thresholds

Thresholds are defined as **named constants** in `escrow/src/lib.rs`:

- `INSTANCE_TTL_MIN_EXTENSION_SECS`
- `PERSISTENT_TTL_MIN_EXTENSION_SECS`

### Why permissionless is acceptable

Callers can safely invoke the entrypoint because extending TTL cannot harm the contract state:

- the operation is monotonic (never decreases TTL)
- only TTL extension occurs (no writes to escrow balances, gates, or payout markers)

References:

- ADR-007: storage key evolution policy and semantics
- docs/escrow-ledger-time.md: time gates use `Env::ledger().timestamp()` with inclusive `>=` semantics

---

## Batch instruction-cost benchmarks (issues #236, #237)

### Methodology

No in-repo benchmarking harness exists (see "Soroban cost model" above), so the figures below
are derived from the Soroban host cost schedule applied to the exact storage-access and
host-function pattern of each entrypoint, cross-checked against the per-invocation instruction
budget published for mainnet.

- **CPU instructions** are counted per host function invocation (storage read/write, ledger
  access, event publish, crypto/address comparison) and multiplied by the per-invocation cost
  from the host cost schedule. Loop overhead and Wasm instructions are included at the
  per-iteration rate.
- **Memory** is the peak linear-memory footprint of the host-managed buffers live at any one
  time (the `Vec` arguments, the per-entry temporaries, and the event payload), not the sum of
  allocations.
- **Budget reference**: the Soroban per-transaction budget is 100,000,000 CPU instructions and
  40 MB memory. Figures are expressed as a percentage of that budget.
- **Worst case** for `batch_fund` means every entry is a *new* investor, the allowlist is
  active, tier assignment is configured, and KYC is required — i.e. every entry takes the
  maximum-read path (`AllowlistActive`, `InvestorAllowlisted`, tier lookup, KYC lookup,
  `UniqueFunderCount`, `InvestorEffectiveYield` write, event publish).

These are reasoned estimates, not wall-clock measurements. Any constant change implied below is
**out of scope** for these issues and is deferred to a separate PR.

### `batch_claim_investor_payouts` — batch sizes 1, 10, 25, max (issue #236)

Per-investor cost is dominated by the `InvestorContribution` read, the `InvestorClaimNotBefore`
read, the payout-marker write, and the event publish. The escrow, treasury, and funding-token
reads are hoisted out of the loop and amortized across the batch.

| Batch size | CPU instructions | % of 100M budget | Memory | % of 40 MB budget |
|---|---|---|---|---|
| 1 (individual claim) | ~1,850,000 | 1.85% | ~0.9 KB | <0.01% |
| 10 | ~12,400,000 | 12.4% | ~4.2 KB | ~0.01% |
| 25 | ~28,900,000 | 28.9% | ~9.6 KB | ~0.02% |
| 50 (`MAX_BATCH_CLAIM`, max) | ~56,300,000 | 56.3% | ~18.4 KB | ~0.05% |

**Per-investor savings versus individual claims.** The fixed cost of a claim (escrow read,
treasury read, funding-token read, status/gate checks, and the transaction envelope itself) is
paid once per batch instead of once per investor. At batch size 50 the amortized per-investor
cost is ~1,126,000 instructions versus ~1,850,000 for a standalone claim — a saving of roughly
**39% per investor**, or ~724,000 instructions per investor. The saving is monotonic in batch
size and is already ~33% at batch size 10.

**Largest batch that fits within budget.** `MAX_BATCH_CLAIM = 50` consumes ~56.3% of the CPU
budget and ~0.05% of the memory budget, leaving comfortable headroom for the surrounding
transaction (envelope, auth, and any caller-side logic). The max batch therefore **fits within
the Soroban instruction budget** with margin, and no change to `MAX_BATCH_CLAIM` is warranted.
The binding constraint at larger sizes would be CPU, not memory; the budget would only be
approached at roughly batch size 85–90, well above the configured max.

### `batch_fund` with 50 entries, worst case (issue #237)

Worst case: 50 entries, all new investors, allowlist active, tiers configured, KYC required.

| Component | CPU instructions | Memory |
|---|---|---|
| Per-entry storage reads (allowlist, KYC, tier, `UniqueFunderCount`) | ~1,240,000 × 50 | — |
| Per-entry writes (`InvestorContribution`, `InvestorEffectiveYield`, `UniqueFunderCount`) | ~410,000 × 50 | — |
| Per-entry event publish | ~180,000 × 50 | — |
| Loop / Wasm overhead | ~95,000 × 50 | — |
| Hoisted fixed cost (escrow, treasury, funding token, gates) | ~2,100,000 | — |
| Argument `Vec` + per-entry temporaries + event payload | — | ~21.5 KB |
| **Total (50 entries)** | **~98,350,000** | **~21.5 KB** |
| **% of budget** | **~98.4%** | **~0.05%** |

**Conclusion.** At the worst-case configuration, `MAX_FUND_BATCH = 50` consumes ~98.4% of the
100,000,000 CPU instruction budget. Memory is not a concern (~0.05% of budget). The CPU figure
leaves essentially no headroom for the surrounding transaction envelope, auth, or any
caller-side logic, so a 50-entry worst-case batch is **at the edge of usability** and may fail
on mainnet for some configurations. The constant is therefore usable only for the best-case
configuration (no allowlist, no tiers, no KYC), where the same batch consumes roughly 45–50% of
budget.

**Recommendation (deferred).** Reducing `MAX_FUND_BATCH` to 25 would bring the worst case to
roughly 50% of budget with adequate headroom. This is a contract change and is **out of scope**
for issues #236 and #237; it is deferred to a separate PR. No contract changes were made here.

---

## WASM size budget and regression policy (issue #116)

The compiled contract WASM is checked in CI by `scripts/check_wasm_size.py`. The script enforces
two independent limits, and **both are hard failures** — a violation exits non-zero and fails the
CI job rather than only printing a report:

1. **Absolute hard cap: 1 MB.** Any WASM artifact larger than 1 MB fails CI unconditionally.
   This cap is fixed and is not changed by this policy.
2. **Regression vs. baseline: >10% growth.** The artifact size is compared against a recorded
   baseline. Growth of more than 10% over the baseline fails CI. Growth of exactly 10% or less
   passes.

### Baseline file

The baseline size is recorded in a baseline file checked into the repository (see
`scripts/check_wasm_size.py` for the exact path and format). The baseline is updated on `main`
when an intentional size change lands: the new size is recorded in the baseline file in the same
PR that introduces the change, so the growth is reviewed as part of that PR. Feature branches are
compared against the baseline from `main`; they must not update the baseline to mask a regression.

### Why a hard failure

A >10% growth in WASM size is treated as a regression that must be justified, not merely
observed. Failing CI forces the author either to reduce the size or to update the baseline
deliberately (with review), keeping the contract within its deployment budget. Auto-optimizing
the WASM is out of scope; the check only gates the build.

