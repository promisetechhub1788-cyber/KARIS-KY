use super::*;

// --- Storage keys ---

#[contracttype]
#[derive(Clone)]
/// Storage discriminator for persisted contract state.
///
/// Most variants live in **instance** storage (shared TTL with the contract instance, bounded
/// aggregate size). Per-investor variants
/// [`InvestorContribution`], [`InvestorEffectiveYield`], [`InvestorClaimNotBefore`], and
/// [`InvestorClaimed`] use **persistent** storage (independent per-address TTL; see ADR-007 and
/// `docs/escrow-gas-storage-notes.md`). [`InvestorAllowlisted`] also uses persistent storage.
///
/// Optional keys are always read with `.get(...).unwrap_or(default)` so that deployments predating
/// a key behave as “unset / default” without panicking.
///
/// ## Additive-key policy (see ADR-007)
///
/// Adding a new variant is **backward-compatible** when the new key is read with
/// `.unwrap_or(default)` and its absence does not change existing entrypoint semantics.
/// Renaming a variant, changing its XDR discriminant, or altering the stored type of an
/// existing key is **breaking** and requires a `migrate` path or a full redeploy.
///
/// Derive rationale:
/// - `Clone`: required because keys are passed by reference into storage APIs and reused
///   across lookups/sets in the same execution path.
pub enum DataKey {
    /// Full escrow snapshot ([`InvoiceEscrow`]); rewritten atomically on every state transition.
    Escrow,
    /// Stored schema version; written once by [`LiquifactEscrow::init`] to [`SCHEMA_VERSION`]
    /// and updated by [`LiquifactEscrow::migrate`] when a migration path is implemented.
    /// Read with [`LiquifactEscrow::get_version`]. Never delete or rename this variant.
    Version,
    /// Per-investor contributed principal recorded during [`LiquifactEscrow::fund`].
    /// **Persistent** storage. Absent ⇒ `0`. One entry per investor address.
    InvestorContribution(Address),
    /// When true, compliance/legal hold blocks payouts and settlement finalization.
    /// Absent ⇒ `false` (no hold). Set through legacy admin activation or guardian confirmation.
    LegalHold,
    /// Optional minimum ledger timestamp when `LegalHold` may be cleared after a
    /// [`LiquifactEscrow::request_clear_legal_hold`] call.
    /// Absent ⇒ no clear request is pending.
    LegalHoldClearableAt,
    /// Configured minimum delay between [`LiquifactEscrow::request_clear_legal_hold`] and
    /// [`LiquifactEscrow::set_legal_hold(env, false)`]. Absent ⇒ `0`.
    LegalHoldClearDelay,
    /// Optional SME collateral commitment metadata (record-only — not an on-chain asset lock).
    /// Absent when no commitment has been recorded. Replaceable by the SME.
    SmeCollateralPledge,
    /// Set to `true` when an investor has exercised a claim after settlement.
    /// **Persistent** storage. Absent ⇒ `false`. Written once; a second claim returns without re-emitting.
    InvestorClaimed(Address),
    /// SEP-41 funding asset for this invoice instance; set once in [`LiquifactEscrow::init`].
    /// Immutable after init.
    FundingToken,
    /// Protocol treasury that may receive [`LiquifactEscrow::sweep_terminal_dust`]; set once in init.
    /// Immutable after init.
    Treasury,
    /// Optional registry contract id for indexers; **hint only**, not authority (see module rustdoc).
    /// Omitted from storage when unset at init. Absent ⇒ `None`.
    RegistryRef,
    /// Immutable tier table when configured at [`LiquifactEscrow::init`]; omitted when tiering is off.
    /// Absent ⇒ no tiering (base `yield_bps` applies to all investors).
    /// **Trust:** values are protocol-supplied at deploy; the contract never mutates this key after init.
    YieldTierTable,
    /// Set once when status first becomes **funded** (1); immutable thereafter (pro-rata denominator).
    /// Absent until the escrow reaches `status == 1`. See [`FundingCloseSnapshot`].
    FundingCloseSnapshot,
    /// Effective annualized yield in bps chosen at this investor’s **first** deposit (see tiered yield).
    /// **Persistent** storage. Absent ⇒ falls back to [`InvoiceEscrow::yield_bps`]. One entry per investor address.
    InvestorEffectiveYield(Address),
    /// Minimum [`Env::ledger`] timestamp before [`LiquifactEscrow::claim_investor_payout`] (0 = no extra gate).
    /// **Persistent** storage. Absent ⇒ `0`. One entry per investor address; set on first deposit.
    InvestorClaimNotBefore(Address),
    /// Minimum [`LiquifactEscrow::fund`] / [`LiquifactEscrow::fund_with_commitment`] amount per call (0 = no floor).
    /// Written as `0` even when unconfigured so reads always succeed.
    MinContributionFloor,
    /// When set at [`LiquifactEscrow::init`], caps distinct investor addresses that may contribute.
    /// Absent ⇒ unlimited. Checked against [`DataKey::UniqueFunderCount`] on each new investor.
    MaxUniqueInvestorsCap,
    /// Optional immutable per-investor cap on total principal credited to a single address.
    /// Absent ⇒ unlimited. Checked against [`DataKey::InvestorContribution`] on every deposit.
    MaxPerInvestorCap,
    /// Proposed successor admin waiting for [`LiquifactEscrow::accept_admin`].
    /// Absent ⇒ no pending handover. Cleared after successful acceptance.
    PendingAdmin,
    /// Count of distinct investor addresses that have a non-zero [`DataKey::InvestorContribution`].
    /// Written as `0` at init; incremented once per new investor in `fund_impl`.
    UniqueFunderCount,
    /// Admin-only **single-set** off-chain attestation digest (e.g. SHA-256 of a legal/KYC bundle).
    /// Absent until [`LiquifactEscrow::bind_primary_attestation_hash`] is called; single-set thereafter.
    PrimaryAttestationHash,
    /// Append-only audit chain of digests (bounded by [`MAX_ATTESTATION_APPEND_ENTRIES`]).
    /// Absent ⇒ empty log. See [`LiquifactEscrow::append_attestation_digest`].
    AttestationAppendLog,
    /// Per-index revocation marker for [`DataKey::AttestationAppendLog`] entries.
    /// Absent ⇒ not revoked. Written as `true` by [`LiquifactEscrow::revoke_attestation_digest`].
    /// Preserves the original digest for auditability while signalling supersession.
    AttestationRevoked(u32),
    /// When true, only allowlisted addresses may call [`LiquifactEscrow::fund`] or [`LiquifactEscrow::fund_with_commitment`].
    AllowlistActive,
    /// Whether a specific address is permitted to fund when [`DataKey::AllowlistActive`] is true.
    InvestorAllowlisted(Address),
    /// Set to `true` once an investor's principal has been refunded in a cancelled escrow.
    /// Absent ⇒ `false`. Written once; prevents double-refund.
    InvestorRefunded(Address),
    /// Running total of principal already returned to investors via [`LiquifactEscrow::refund`].
    /// Absent ⇒ `0`. Incremented atomically with each successful refund transfer.
    /// Used by [`LiquifactEscrow::sweep_terminal_dust`] to compute outstanding liabilities:
    /// `outstanding = funded_amount - distributed_principal`.
    DistributedPrincipal,
    /// Optional funding deadline (ledger timestamp); after it passes, new funds are rejected.
    FundingDeadline,
    /// When active, temporarily freezes the escrow during dispute resolution (separate from legal hold).
    /// Blocks `fund`, `settle`, and `withdraw`. Stores [`DisputePauseState`].
    /// Absent ⇒ no dispute pause active.
    DisputePaused,
    /// Amount that has been settled in a partial settlement. When this equals `funded_amount`,
    /// the escrow is fully settled. Absent ⇒ 0 (no partial settlement).
    SettledAmount,
    /// Whether an investor has elected to reinvest their yield (when settling).
    /// **Persistent** storage. Absent ⇒ `false` (no reinvestment). One entry per investor address.
    InvestorReinvestElection(Address),
    /// Target escrow contract address for reinvestment (investor may reinvest in different escrow).
    /// **Persistent** storage. Absent ⇒ reinvest in same escrow. One entry per investor address.
    /// Used with [`DataKey::InvestorReinvestElection`].
    InvestorReinvestTarget(Address),
    /// Cumulative yield reinvested for this investor (becomes additional principal).
    /// **Persistent** storage. Absent ⇒ `0`. One entry per investor address.
    /// Tracking separate from [`DataKey::InvestorContribution`] to distinguish original vs reinvested.
    InvestorReinvestedAmount(Address),
    /// Append-only audit log of reinvestment events for all investors.
    /// Bounded by [`MAX_REINVESTMENT_AUDIT_ENTRIES`].
    /// Absent ⇒ empty log.
    ReinvestmentAuditLog,
    /// Immutable per-investor yield distribution snapshot captured at settlement time.
    /// **Persistent** storage. Absent ⇒ no automatic distribution. One entry per investor address.
    /// Stores pre-computed yield share for each investor at the time of settlement to enable
    /// batch auto-distribution without per-investor yield recalculation.
    /// Written by [`LiquifactEscrow::settle`] when auto-distribution is enabled.
    YieldDistributionShare(Address),
    /// Flag indicating whether automatic yield distribution snapshots are enabled for this escrow.
    /// Absent ⇒ false (default off, backwards-compatible). Set during [`LiquifactEscrow::init`].
    YieldAutoDistributionEnabled,
    /// Expiration timestamp for an admin-proposed legal hold awaiting guardian confirmation.
    /// Absent ⇒ no legal-hold proposal is pending. Appended to preserve existing key discriminants.
    LegalHoldProposalExpiresAt,
}

pub(crate) struct PerInvestorStorage<'a> {
    env: &'a Env,
}

impl<'a> PerInvestorStorage<'a> {
    fn new(env: &'a Env) -> Self {
        Self { env }
    }

    fn get_contribution(&self, investor: Address) -> i128 {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::InvestorContribution(investor))
            .unwrap_or(0)
    }

    fn set_contribution(&self, investor: Address, amount: i128) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::InvestorContribution(investor), &amount);
    }

    fn get_effective_yield(&self, investor: Address) -> Option<i64> {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::InvestorEffectiveYield(investor))
    }

    fn set_effective_yield(&self, investor: Address, value: i64) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::InvestorEffectiveYield(investor), &value);
    }

    fn get_claim_not_before(&self, investor: Address) -> u64 {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::InvestorClaimNotBefore(investor))
            .unwrap_or(0)
    }

    fn set_claim_not_before(&self, investor: Address, value: u64) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::InvestorClaimNotBefore(investor), &value);
    }

    fn get_lock_in_until(&self, investor: Address) -> u64 {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::InvestorLockInUntil(investor))
            .unwrap_or(0)
    }

    fn set_lock_in_until(&self, investor: Address, value: u64) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::InvestorLockInUntil(investor), &value);
    }

    fn get_claimed(&self, investor: Address) -> bool {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::InvestorClaimed(investor))
            .unwrap_or(false)
    }

    fn set_claimed(&self, investor: Address, value: bool) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::InvestorClaimed(investor), &value);
    }

    fn get_allowlisted(&self, investor: Address) -> bool {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::InvestorAllowlisted(investor))
            .unwrap_or(false)
    }

    fn set_allowlisted(&self, investor: Address, allowed: bool) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::InvestorAllowlisted(investor), &allowed);
    }

    fn get_yield_claim_delegate(&self, investor: Address) -> Option<Address> {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::YieldClaimDelegate(investor))
    }

    fn set_yield_claim_delegate(&self, investor: Address, delegate: Address) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::YieldClaimDelegate(investor), &delegate);
    }

    fn get_yield_claim_delegate_revoked(&self, investor: Address) -> bool {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::YieldClaimDelegateRevoked(investor))
            .unwrap_or(false)
    }

    fn set_yield_claim_delegate_revoked(&self, investor: Address, revoked: bool) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::YieldClaimDelegateRevoked(investor), &revoked);
    }
}

pub(crate) trait PerInvestorStorageExt {
    fn per_investor(&self) -> PerInvestorStorage;
}

impl PerInvestorStorageExt for Env {
    fn per_investor(&self) -> PerInvestorStorage {
        PerInvestorStorage::new(self)
    }
}

impl LiquifactEscrow {
    pub(crate) fn get_persistent_investor_contribution(env: &Env, investor: Address) -> i128 {
        PerInvestorStorage::new(env).get_contribution(investor)
    }

    pub(crate) fn set_persistent_investor_contribution(env: &Env, investor: Address, amount: i128) {
        env.per_investor().set_contribution(investor, amount);
    }

    pub(crate) fn get_persistent_investor_effective_yield(
        env: &Env,
        investor: Address,
    ) -> Option<i64> {
        env.per_investor().get_effective_yield(investor)
    }

    pub(crate) fn set_persistent_investor_effective_yield(
        env: &Env,
        investor: Address,
        value: i64,
    ) {
        env.per_investor().set_effective_yield(investor, value);
    }

    pub(crate) fn get_persistent_investor_claim_not_before(env: &Env, investor: Address) -> u64 {
        env.per_investor().get_claim_not_before(investor)
    }

    pub(crate) fn set_persistent_investor_claim_not_before(
        env: &Env,
        investor: Address,
        value: u64,
    ) {
        env.per_investor().set_claim_not_before(investor, value);
    }

    pub(crate) fn get_persistent_investor_lock_in_until(env: &Env, investor: Address) -> u64 {
        env.per_investor().get_lock_in_until(investor)
    }

    pub(crate) fn set_persistent_investor_lock_in_until(env: &Env, investor: Address, value: u64) {
        env.per_investor().set_lock_in_until(investor, value);
    }

    pub(crate) fn get_persistent_investor_claimed(env: &Env, investor: Address) -> bool {
        env.per_investor().get_claimed(investor)
    }

    pub(crate) fn set_persistent_investor_claimed(env: &Env, investor: Address, value: bool) {
        env.per_investor().set_claimed(investor, value);
    }

    pub(crate) fn get_persistent_investor_allowlisted(env: &Env, investor: Address) -> bool {
        env.per_investor().get_allowlisted(investor)
    }

    pub(crate) fn set_persistent_investor_allowlisted(env: &Env, investor: Address, allowed: bool) {
        env.per_investor().set_allowlisted(investor, allowed);
    }

    pub(crate) fn get_persistent_yield_claim_delegate(
        env: &Env,
        investor: Address,
    ) -> Option<Address> {
        env.per_investor().get_yield_claim_delegate(investor)
    }

    pub(crate) fn set_persistent_yield_claim_delegate(
        env: &Env,
        investor: Address,
        delegate: Address,
    ) {
        env.per_investor()
            .set_yield_claim_delegate(investor, delegate)
    }

    pub(crate) fn get_persistent_yield_claim_delegate_revoked(
        env: &Env,
        investor: Address,
    ) -> bool {
        env.per_investor()
            .get_yield_claim_delegate_revoked(investor)
    }

    pub(crate) fn set_persistent_yield_claim_delegate_revoked(
        env: &Env,
        investor: Address,
        revoked: bool,
    ) {
        env.per_investor()
            .set_yield_claim_delegate_revoked(investor, revoked);
    }

    pub(crate) fn delegation_is_valid(env: &Env, investor: Address) -> bool {
        Self::get_persistent_yield_claim_delegate(env, investor.clone()).is_some()
            && !Self::get_persistent_yield_claim_delegate_revoked(env, investor)
    }
}
