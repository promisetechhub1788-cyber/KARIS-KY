# Escrow Security Checklist

> Issue #167 — Soroban security research.
> Scope: `escrow/src/lib.rs` and `escrow/src/external_calls.rs`.
> Schema version: [`SCHEMA_VERSION`] = 6.

---

## 1. Authentication Matrix

Every state-mutating entrypoint and the identity required to authorize it.

| Entrypoint | Required signer | Auth call site | Notes |
|---|---|---|---|
| `init` | `admin` (caller-supplied) | `admin.require_auth()` | One-time; panics if escrow exists |
| `propose_admin` | current `escrow.admin` | `escrow.admin.require_auth()` | `new_admin` must differ; writes `DataKey::PendingAdmin` only |
| `accept_admin` | `DataKey::PendingAdmin` | `pending.require_auth()` | Promotes pending address into `escrow.admin`; clears pending key |
| `update_maturity` | `escrow.admin` | `escrow.admin.require_auth()` | Only in `status == 0`; **extensions** (new maturity > current) require a propose/confirm delay or guardian confirmation (issue #241) |
| `update_funding_target` | `escrow.admin` | `escrow.admin.require_auth()` | Only in `status == 0`; `new_target >= funded_amount` |
| `set_legal_hold` / `clear_legal_hold` | `escrow.admin` | `escrow.admin.require_auth()` | No timelock; no multisig enforced on-chain |
| `set_allowlist_active` | `escrow.admin` | `escrow.admin.require_auth()` | Enables/disables `AllowlistActive` gate |
| `set_investor_allowlisted` | `escrow.admin` | `escrow.admin.require_auth()` | Writes to **persistent** storage (see §5.4) |
| `bind_primary_attestation_hash` | `escrow.admin` | `escrow.admin.require_auth()` | Single-set; second call panics |
| `append_attestation_digest` | `escrow.admin` | `escrow.admin.require_auth()` | Bounded at `MAX_ATTESTATION_APPEND_ENTRIES` = 32 |
| `fund` | `investor` (caller-supplied) | `investor.require_auth()` | `status == 0`; allowlist-gated when active |
| `fund_with_commitment` | `investor` (caller-supplied) | `investor.require_auth()` | First deposit only (`prev == 0`); sets claim lock |
| `record_sme_collateral_commitment` | `escrow.sme_address` | `escrow.sme_address.require_auth()` | Ledger record only; no token transfer |
| `settle` | `escrow.sme_address` | `escrow.sme_address.require_auth()` | `status == 1`; optional maturity gate |
| `withdraw` | `escrow.sme_address` | `escrow.sme_address.require_auth()` | `status == 1`; sets `status = 3` |
| `claim_investor_payout` | `investor` (caller-supplied) | `investor.require_auth()` | `status == 2`; contribution > 0; claim-lock gate |
| `sweep_terminal_dust` | `treasury` | `treasury.require_auth()` | `status == 2 or 3`; amount ≤ `MAX_DUST_SWEEP_AMOUNT` |
| `verify_asset_custody` | `escrow.admin` | `escrow.admin.require_auth()` | Admin-triggered reconciliation check; suitable for manual audits or external schedulers |
| `migrate` | `escrow.admin` (via `load_escrow_require_admin`) | `Self::load_escrow_require_admin(&env)` before version checks | Pre-emptive guard; all current paths panic; idempotency nonce prevents replay |
| `propose_upgrade` | `escrow.admin` | `escrow.admin.require_auth()` | Records a pending wasm hash and proposal ledger; no code swap (issue #242) |
| `execute_upgrade` | `escrow.admin` | `escrow.admin.require_auth()` | Only after the minimum delay elapses; performs the actual code swap (issue #242) |

### Read-only entrypoints

All `get_*` and `is_*` functions carry no `require_auth`. They expose full escrow state to any caller. This is intentional for indexers and off-chain tooling; treat all stored data as public.

---

## 2. Trusted Addresses

### 2.1 Admin (`InvoiceEscrow::admin`)

- Set at `init`; mutable only via `propose_admin` plus `accept_admin` (current admin and successor signatures).
- Controls: hold activation, allowlist, attestation binding, maturity, funding target, schema migration (future).
- **Risk**: a single EOA admin can indefinitely freeze funds via `set_legal_hold`. Production deployments **must** use a governed contract or multisig at this address. There is no on-chain escape hatch if the admin key is lost or malicious.

### 2.2 SME (`InvoiceEscrow::sme_address`)

- Immutable after `init`; cannot be rotated.
- Controls: settlement finalization (`settle`), liquidity pull (`withdraw`), collateral record (`record_sme_collateral_commitment`).
- **Risk**: if `sme_address` is compromised while the escrow is funded, the SME can call `withdraw` (status → 3) before maturity, pulling all principal. No admin veto exists on `withdraw`.

### 2.3 Treasury (`DataKey::Treasury`)

- Set once at `init`; immutable.
- Only permitted actor for `sweep_terminal_dust`.
- Receives at most `MAX_DUST_SWEEP_AMOUNT` = 100,000,000 base units per call, only in terminal states.
- **Risk**: if treasury address is a contract, confirm it cannot re-enter or redirect the transfer during the SEP-41 call. Soroban host-function atomicity makes interleaved re-entry impossible, but the treasury contract receiving the transfer could call back into unrelated contracts after the balance check.

### 2.4 Funding Token (`DataKey::FundingToken`)

- Set once at `init`; immutable.
- Treated as a compliant SEP-41 token for all balance-delta checks.
- See §4 for threat model.

### 2.5 Registry (`DataKey::RegistryRef`)

- Optional; written as a hint at `init`.
- **No on-chain authority.** The contract never reads or verifies this address after storage. Off-chain indexers must not treat its presence as proof of current registry membership.

---

## 3. Invariants

These conditions must hold at every ledger boundary. A violation indicates either a contract bug or an adversarial token.

### I-1: Contribution accounting

```
funded_amount == Σ InvestorContribution(addr) for all addr with a non-zero entry
```

Maintained in `fund_impl`: `escrow.funded_amount` is incremented by `amount` atomically with `InvestorContribution(investor) += amount` in the same host function. No other entrypoint modifies `funded_amount` or individual contributions.

**Caveat**: `fund_impl` does **not** pull tokens from the investor. The contract records the commitment but does not enforce that the caller's token balance decreased. Actual token custody is the responsibility of the integration layer. See §5.2.

### I-2: Status monotonicity

```
status ∈ {0, 1, 2, 3}
0 → 1 → 2  (settle path)
0 → 1 → 3  (withdraw path)
```

No entrypoint decrements or resets `status`. Once `status == 2` or `status == 3`, no further state transitions are possible.

### I-3: FundingCloseSnapshot is write-once

Written inside `fund_impl` the first time `funded_amount >= funding_target` triggers status → 1. Guarded by `if !env.storage().instance().has(&DataKey::FundingCloseSnapshot)`. Never overwritten. Immutable after first set.

### I-4: InvestorClaimed is write-once

`InvestorClaimed(addr)` is set to `true` in `claim_investor_payout`. The function returns early on a second call (idempotent). Never reset to `false`.

### I-5: PrimaryAttestationHash is single-set

`bind_primary_attestation_hash` panics if `DataKey::PrimaryAttestationHash` already exists. Cannot be replaced or cleared.

### I-6: UniqueFunderCount monotonically increases

Incremented once per new investor (`prev == 0`) in `fund_impl`. Never decremented. Bounded above by `MaxUniqueInvestorsCap` when set.

### I-7: AttestationAppendLog bounded

`log.len() < MAX_ATTESTATION_APPEND_ENTRIES` (32) is enforced before each append. Panics at capacity.

### I-8: Immutable init keys

`FundingToken`, `Treasury`, and `YieldTierTable` are written once at `init` and never overwritten by any entrypoint. Confirmed by absence of any `.set(&DataKey::FundingToken, ...)` after the init block.

### I-9: funded_amount ≥ 0

`fund_impl` only accepts positive `amount` and uses `checked_add` (panics on overflow). No entrypoint subtracts from `funded_amount`. The stored value can only grow.

### I-10: Yield tier ladder non-decreasing

Enforced in `validate_yield_tiers_table` at `init`: each tier must have `yield_bps >= previous.yield_bps` and `min_lock_secs > previous.min_lock_secs`. Base `yield_bps` is a lower bound. Immutable after init.

---

## 4. SEP-41 Trust Boundaries and Breakpoints

The only on-chain token interaction is in `external_calls::transfer_funding_token_with_balance_checks`, called exclusively from `sweep_terminal_dust`.

### 4.1 Fee-on-transfer tokens

`transfer_funding_token_with_balance_checks` measures `from_before`, `treasury_before`, calls `token.transfer(...)`, then asserts:

```rust
spent == amount    // sender delta
received == amount // recipient delta
```

A fee-on-transfer token delivers `amount - fee` to the recipient. The recipient-delta assertion panics. **Effect**: `sweep_terminal_dust` is permanently broken for this escrow instance. Tokens remain locked. Governance must redeploy.

### 4.2 Rebasing tokens

If the token contract modifies balances between the pre-transfer read and the post-transfer read (e.g. yield accrual mid-call), either delta assertion may fail. Same outcome as §4.1.

### 4.3 Balance-query manipulation

A malicious token could return a fabricated value from `balance()`. If the token returns `from_before < amount`, the insufficient-balance assert fires before the transfer. If it manipulates `from_after` to fake a smaller spend or `treasury_after` to fake a larger receipt, the delta assertions catch it.

The checks are necessary but assume the token does not lie about both the pre- and post-transfer balances in a coordinated way. A token that lies consistently (same fake delta on both sides) can pass the checks. Governance allowlists are the correct defense; the contract cannot eliminate this assumption.

### 4.4 Soroban execution model and reentrancy

Soroban executes cross-contract calls synchronously to completion within the host function. A token contract cannot interleave execution mid-transfer into the escrow's own entrypoints. Classic EVM reentrancy is not applicable.

A token contract can, however, call **other** contracts during `transfer`. If those contracts interact with this escrow in a separate transaction, no in-flight state is exposed. The pre/post balance pattern remains the correct defense for correctness.

### 4.5 Token contract upgrade post-init

`FundingToken` is an `Address`. If the token contract at that address is upgraded to a malicious implementation after escrow `init`, all subsequent `balance()` and `transfer()` calls go to the upgraded code. The escrow cannot detect this. Governance must monitor the token contract's upgrade authority and treat any upgrade as a potential invalidation of this escrow's token assumptions.

### 4.6 fund() and withdraw() do not transfer tokens

`fund_impl` records investor contributions and updates `funded_amount`. **It does not call the token contract.** Actual tokens must arrive at the escrow contract address via a separate SEP-41 `transfer` call by the investor. If an investor calls `fund()` without first transferring tokens, the accounting diverges from the actual token balance. The contract treats the caller's signed commitment as the source of truth.

`withdraw()` and `settle()` change status but do not move tokens. `claim_investor_payout()` sets a claimed flag and emits an event. The actual token disbursement to the SME or investors is off-chain or via a separate orchestrating contract. **The contract is an accounting ledger, not a token custodian for payouts.**

This creates a window where `funded_amount` > actual token balance (unfunded commitments) or actual token balance > `funded_amount` (stray transfers, airdrops). `sweep_terminal_dust` addresses the latter in terminal states, capped at `MAX_DUST_SWEEP_AMOUNT`.

---

## 5. Assumptions and Risks

### 5.1 `migrate()` Authorization and Idempotency Guards

**Starting from schema version 7:**

- **Authorization guard:** `migrate()` calls `Self::load_escrow_require_admin(&env)` as the first statement, before any version checks or storage reads.
- **Idempotency nonce:** After a successful migration, a [`DataKey::MigrationExecutionLog`] entry is written to prevent replaying the same version transition.

The migration entrypoint is now guarded against both unauthorized access and replay attacks. This is critical when migration logic adds state transformations (yield calculations, per-investor records, collateral audits). Replaying these transformations twice would corrupt accounting.

**For developers implementing migration paths:** Ensure the nonce is written **after** state transformation but **before** returning. See `docs/OPERATOR_RUNBOOK.md` §2 for the template.

**For operators:** If you call `migrate(from_version)` and receive error code 93 (`MigrationAlreadyApplied`), verify the on-chain version with `get_version()`. If it matches your target, the migration succeeded — do not retry.

### 5.2 Accounting-custody decoupling

`funded_amount` is a commitment record, not a token balance assertion. Integrations that custody principal on-chain must enforce that token transfers and `fund()` calls are atomic (e.g., via an outer orchestrator contract or a Soroban transaction with both operations). Without this, `funded_amount` can overstate actual holdings, and `sweep_terminal_dust` could drain funds that were never genuinely deposited.

### 5.2a Reconciliation procedure for custody audits

Operators should reconcile the escrow's recorded funding with its actual on-chain balance on a cadence that matches the custody workflow. The admin can call `verify_asset_custody()` to compare the contract's current funding-token balance with the stored `funded_amount`; the call returns a signed discrepancy (`contract_balance - recorded_funded_amount`) and emits an `AssetCustodyVerified` event for off-chain indexing.

- Positive discrepancy: the contract holds more than the recorded funded amount (for example, a stray transfer or airdrop).
- Negative discrepancy: the contract holds less than the recorded funded amount (for example, a missed transfer or partial withdrawal).
- Use the result with custody statements, bridge receipts, or bank reconciliations before settlement, withdrawal, or a dust sweep.
- External schedulers or trigger systems can invoke the same entrypoint automatically so reconciliation becomes part of the operating runbook.

### 5.3 Legal hold has no on-chain expiry

`LegalHold` is a boolean toggled exclusively by the **current** `escrow.admin`.
There is no timelock, no council override, and no programmatic expiry. A
compromised or malicious admin can freeze `settle`, `withdraw`,
`claim_investor_payout`, and `sweep_terminal_dust` indefinitely. **Recovery:**
governance executes `propose_admin` and the successor executes `accept_admin`
(both not blocked by the hold), then the new admin calls `clear_legal_hold`.
See `docs/escrow-legal-hold.md` and ADR-004. Production deployments **must**
use a governed admin (multisig or DAO) so a single lost key cannot strand funds
without a documented rotation playbook.

### 5.4 Storage type mismatch: AllowlistActive vs. InvestorAllowlisted

`AllowlistActive` is stored in **instance** storage. `InvestorAllowlisted(addr)` entries are stored in **persistent** storage. These have different TTL semantics under Soroban's rent model. If instance storage expires and is not extended, `AllowlistActive` returns `false` (default via `unwrap_or`), silently disabling the allowlist gate even if persistent allowlist entries remain. Operators must extend instance storage TTL together with persistent storage TTL.

### 5.5 Ledger timestamp trust

`settle`, `claim_investor_payout`, and `fund_with_commitment` (claim lock calculation) rely on `env.ledger().timestamp()`. This is validator-observed ledger close time, not a wall-clock oracle. Boundaries are `>=` / `<` comparisons on integer seconds. There is measurable skew between simulated environments (Futurenet, local) and mainnet. Do not assume sub-minute maturity precision.

### 5.6 Admin key loss is unrecoverable without admin authority

There is no guardian, recovery address, or protocol DAO escape hatch beyond the
two-step admin handover. If **all** current-admin signers are lost while a legal
hold is active, funds remain blocked until signing capability is restored
off-chain. If at least one signer remains, rotate via `propose_admin` and
`accept_admin`, then clear the hold.
See `docs/escrow-legal-hold.md` § "Failure mode: hold + lost admin key".

### 5.7 Over-funding is intentional and unbound above the target

`fund_impl` permits `funded_amount` to exceed `funding_target`. The `FundingCloseSnapshot` records the actual `funded_amount` at close (including overflow). Off-chain pro-rata calculations must use `snapshot.total_principal`, not `snapshot.funding_target`. Misuse of the funding target as the denominator underestimates each investor's share.

### 5.8 Collateral pledge is non-custodial

`record_sme_collateral_commitment` writes a `SmeCollateralCommitment` struct. It does not transfer, lock, or encumber any on-chain asset. The record cannot trigger automated liquidation. Do not use the presence of a collateral record as on-chain proof of asset lock without a separate enforcement contract.

### 5.9 InvestorClaimed is an event marker, not a payment proof

`claim_investor_payout` marks an investor as claimed and emits `InvestorPayoutClaimed`. It does not transfer tokens. Off-chain systems that release principal or yield based on this event must implement their own idempotency and replay guards. A re-emitted or replayed event must not trigger a second disbursement.

---

## 7. Timing and side-channel threat model

### 7.1 Threat model

- This escrow contract does not process confidential inputs. All on-chain entrypoint parameters, investor contributions, yield tiers, funding amounts, and ledger timestamps are either publicly observable on the ledger or controlled by the transaction sender.
- Soroban contract storage and transaction inputs are public; there is no private secret within this escrow logic that requires constant-time protection.
- The primary security goal is correctness and invariant enforcement, not side-channel secrecy.

### 7.2 Value-dependent execution

- `effective_yield_for_commitment()` iterates the immutable `YieldTierTable` and selects the best tier based on `committed_lock_secs`. The tier table and commitment value are public in the transaction context.
- `compute_investor_payout()`, `claim_investor_payout()`, and other investor-facing entrypoints branch on public snapshot values and stored contribution records. There is no hidden secret being protected by these branches.
- `fund_impl()` performs public input validation (`amount`, caps, allowlist membership) and recording. Any timing differences are on data that is already observable or attacker-controlled.
- `withdraw()` and `sweep_terminal_dust()` perform SEP-41 balance-delta checks. These operations are not constant-time, but their security goal is to detect non-standard token behavior, not to protect confidential data.

### 7.3 Constant-time assessment

- No constant-time alternatives are required for the current escrow implementation because there is no secret-dependent branching or private state within this contract.
- The current design already fails safely on non-standard token behavior through strict pre/post balance invariants in `external_calls::transfer_funding_token_with_balance_checks`.
- If future features introduce confidential or privacy-sensitive inputs, add a focused side-channel review at that time.

### 7.4 Risk statement

- Current side-channel risk is low: contract state and transaction inputs are public and visible to ledger observers.
- The main remaining security risk class is unsupported token economics and governance allowlist failure, not execution-timing leaks.
- This conclusion is intentional: the contract does not need a constant-time rewrite for the current threat model.

---

## 6. Authorization guard ordering (issue #265)

Per [Stellar contract authorization](https://developers.stellar.org/docs/build/guides/auth/contract-authorization),
`Address::require_auth()` is the contract's security policy boundary: the Soroban
host validates signatures, replay protection, and authorization entries before the
call proceeds. **Invariant:** no storage write (`instance` or `persistent`) and no
SEP-41 token transfer occurs until the relevant `require_auth` succeeds.

### Canonical sequence

```
1. Read-only preconditions (legal hold, status, input asserts)
2. Address::require_auth() for the bound role
3. Storage writes and token transfers (external_calls only)
```

Reading `DataKey::Escrow` before step 2 is **intentional** — it is read-only
and does not weaken the auth boundary. Refactors must not move step 3 above step 2.

### Entrypoint checklist

| Entrypoint | Signer | Pre-auth reads (no writes) | `require_auth` | First mutation |
|---|---|---|---|---|
| `init` | `admin` | — | line ~549 (`admin`) | `DataKey::Escrow` set |
| `propose_admin` | current `escrow.admin` | `get_escrow` | line ~1888 | `DataKey::PendingAdmin` set |
| `accept_admin` | `DataKey::PendingAdmin` | pending read, `get_escrow` after auth | line ~1917 | `DataKey::Escrow` set |
| `update_maturity` (shorten) | `escrow.admin` | `get_escrow` | line ~1400 | `DataKey::Escrow` set |
| `update_maturity` (extend) | `escrow.admin` | `get_escrow`, pending-extension read | line ~1400 | `DataKey::Escrow` set only after delay/guardian confirmation (issue #241) |
| `update_funding_target` | `escrow.admin` | `get_escrow` | line ~1007 | `DataKey::Escrow` set |
| `set_legal_hold` / `clear_legal_hold` | current `escrow.admin` | `get_escrow` | line ~940 | `DataKey::LegalHold` set |
| `set_allowlist_active` | `escrow.admin` | `get_escrow` | line ~956 | `DataKey::AllowlistActive` set |
| `set_investor_allowlisted` | `escrow.admin` | `get_escrow` | line ~978 | persistent allowlist set |
| `bind_primary_attestation_hash` | `escrow.admin` | `get_escrow`, `has` check | line ~791 | `PrimaryAttestationHash` set |
| `append_attestation_digest` | `escrow.admin` | `get_escrow`, log read | line ~820 | log append + set |
| `fund` / `fund_with_commitment` | `investor` | floor read | line ~1119 (`investor`) | per-investor keys |
| `record_sme_collateral_commitment` | `escrow.sme_address` | `get_escrow` | line ~911 | collateral set |
| `settle` | `escrow.sme_address` | legal hold, `get_escrow` | line ~1282 | `DataKey::Escrow` set |
| `withdraw` | `escrow.sme_address` | legal hold, `get_escrow` | line ~1321 | `DataKey::Escrow` set |
| `claim_investor_payout` | `investor` | legal hold, contribution read | line ~1350 | `InvestorClaimed` set |
| `sweep_terminal_dust` | `treasury` | legal hold, `get_escrow`, treasury read | line ~702 | SEP-41 transfer |
| `migrate` | **none** (panics) | version read only | — | none (all paths panic) |
| `propose_upgrade` | `escrow.admin` | `get_escrow` | line ~TBD | `DataKey::PendingUpgrade` set |
| `execute_upgrade` | `escrow.admin` | `get_escrow`, pending-upgrade read, delay check | line ~TBD | `env.deployer().update_current_contract_wasm` |

Line numbers refer to `escrow/src/lib.rs` at schema version 6; re-audit after refactors.

### Admin-power threat model

The full admin-power threat-model table (admin power, worst-case impact,
current mitigation, recommended mitigation) lives in
[`docs/DEPLOYER_SECURITY.md`](./DEPLOYER_SECURITY.md) § "Admin-power threat
model" (issue #240). Consult it before granting admin authority to any address.

### Negative-auth test coverage

| Entrypoint | Test location |
|---|---|
| `init` | `escrow/src/tests/init.rs` |
| `propose_admin`, `accept_admin`, `fund`, `fund_with_commitment`, `settle`, `withdraw`, `claim_investor_payout`, attestation, allowlist, sweep | `escrow/src/tests/admin.rs` § auth audit |
| `update_maturity`, `update_funding_target`, collateral | `escrow/src/tests/admin.rs` |
| `update_maturity` extension delay/guardian | `escrow/src/tests/admin.rs` (issue #241) |
| `propose_upgrade`, `execute_upgrade` | `escrow/src/tests/admin.rs` (issue #242) |
| `set_legal_hold`, `clear_legal_hold` | `escrow/src/tests/legal_hold.rs` |
| `bind_primary_attestation_hash`, `append_attestation_digest` | `escrow/src/tests/attestations.rs` |
| `set_allowlist_active`, `set_investor_allowlisted` | `escrow/src/test_allowlist_tests.rs` |
