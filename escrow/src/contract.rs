use super::*;

#[contractimpl]
impl LiquifactEscrow {
    /// Initialize escrow. `funding_target` defaults to `amount`.
    ///
    /// Binds **`funding_token`**, **`treasury`**, and optional **`registry`** for this instance only.
    /// The funding token and treasury addresses are **immutable** after this call; the registry id is
    /// optional metadata for off-chain indexers (not an on-chain authority).
    ///
    /// `maturity == 0` is an explicit "no maturity lock" configuration: once funded, the SME may
    /// call [`LiquifactEscrow::settle`] immediately. Positive maturity values are validator-observed
    /// ledger timestamps and are enforced with an inclusive `ledger.timestamp() >= maturity` check.
    ///
    /// `invoice_id` must satisfy [`MAX_INVOICE_ID_STRING_LEN`] and charset rules (see
    /// [`validate_invoice_id_string`]).
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes for invalid amounts, yield bounds, invoice id validation,
    /// duplicate initialization, malformed optional caps, and invalid tier configuration.
    pub fn init(
        env: Env,
        admin: Address,
        invoice_id: String,
        sme_address: Address,
        amount: i128,
        yield_bps: i64,
        maturity: u64,
        funding_token: Address,
        registry: Option<Address>,
        treasury: Address,
        yield_tiers: Option<Vec<YieldTier>>,
        min_contribution: Option<i128>,
        max_unique_investors: Option<u32>,
        max_per_investor: Option<i128>,
        legal_hold_clear_delay: Option<u64>,
        funding_deadline: Option<u64>,
        max_funding_rate: Option<u64>,
        yield_slippage_threshold: Option<i64>,
        settlement_notifier_contract: Option<Address>,
        kyc_provider_contract: Option<Address>,
        admin_roles: Option<Vec<(Address, AdminRole)>>,
        reject_contract_admin: Option<bool>,
    ) -> InvoiceEscrow {
        Self::init_impl(
            env,
            admin,
            invoice_id,
            sme_address,
            amount,
            yield_bps,
            maturity,
            funding_token,
            registry,
            treasury,
            yield_tiers,
            min_contribution,
            max_unique_investors,
            max_per_investor,
            legal_hold_clear_delay,
            funding_deadline,
            max_funding_rate,
            yield_slippage_threshold,
            settlement_notifier_contract,
            kyc_provider_contract,
            admin_roles,
            reject_contract_admin,
        )
    }

    /// Initialize an escrow with a separate legal-hold guardian. Existing deployments and
    /// callers using [`LiquifactEscrow::init`] remain guardian-free.
    pub fn init_with_guardian(
        env: Env,
        admin: Address,
        guardian: Address,
        invoice_id: String,
        sme_address: Address,
        amount: i128,
        yield_bps: i64,
        maturity: u64,
        funding_token: Address,
        registry: Option<Address>,
        treasury: Address,
        yield_tiers: Option<Vec<YieldTier>>,
        min_contribution: Option<i128>,
        max_unique_investors: Option<u32>,
        max_per_investor: Option<i128>,
        legal_hold_clear_delay: Option<u64>,
        funding_deadline: Option<u64>,
        max_funding_rate: Option<u64>,
        yield_slippage_threshold: Option<i64>,
        settlement_notifier_contract: Option<Address>,
        kyc_provider_contract: Option<Address>,
        admin_roles: Option<Vec<(Address, AdminRole)>>,
        reject_contract_admin: Option<bool>,
    ) -> InvoiceEscrow {
        Self::init_with_guardian_impl(
            env,
            admin,
            guardian,
            invoice_id,
            sme_address,
            amount,
            yield_bps,
            maturity,
            funding_token,
            registry,
            treasury,
            yield_tiers,
            min_contribution,
            max_unique_investors,
            max_per_investor,
            legal_hold_clear_delay,
            funding_deadline,
            max_funding_rate,
            yield_slippage_threshold,
            settlement_notifier_contract,
            kyc_provider_contract,
            admin_roles,
            reject_contract_admin,
        )
    }

    /// Returns the SEP-41 funding token bound at [`LiquifactEscrow::init`] ([`DataKey::FundingToken`]).
    ///
    /// **Immutable:** set once at init; cannot change after deploy. Emits
    /// [`EscrowError::FundingTokenNotSet`] if called before init.
    pub fn get_funding_token(env: Env) -> Address {
        Self::get_funding_token_impl(env)
    }

    /// Returns the protocol treasury address bound at [`LiquifactEscrow::init`] ([`DataKey::Treasury`]).
    ///
    /// **Immutable:** set once at init; cannot change after deploy. The treasury is the only
    /// recipient of [`LiquifactEscrow::sweep_terminal_dust`]. Emits
    /// [`EscrowError::TreasuryNotSet`] if called before init.
    pub fn get_treasury(env: Env) -> Address {
        Self::get_treasury_impl(env)
    }

    /// Returns the optional off-chain registry hint stored at [`DataKey::RegistryRef`], or [`None`]
    /// when no registry was supplied at [`LiquifactEscrow::init`].
    ///
    /// **Non-authority:** this address is a read-only discoverability hint for off-chain indexers.
    /// No on-chain logic in this contract consults it. Callers must **not** treat its presence as
    /// proof of registry membership — query the registry contract directly to verify on-chain state.
    pub fn get_registry_ref(env: Env) -> Option<Address> {
        Self::get_registry_ref_impl(env)
    }

    /// Best-effort registration with the immutable registry configured at init.
    ///
    /// Only the escrow admin may invoke this entrypoint. Registry failures are recorded in an
    /// event and return `false`; they never revert the escrow transaction.
    pub fn register_with_registry(env: Env) -> bool {
        Self::register_with_registry_impl(env)
    }

    /// Returns the cached token metadata (decimals, cache timestamp, cache sequence).
    ///
    /// This metadata is captured at [`LiquifactEscrow::init`] to reduce external token contract
    /// calls during fund operations. Returns [`None`] if not yet cached (should not happen if init succeeded).
    ///
    /// For staleness detection or re-caching, use [`LiquifactEscrow::revalidate_token_cache`].
    pub fn get_token_metadata_cache(env: Env) -> Option<TokenMetadataCache> {
        Self::get_token_metadata_cache_impl(env)
    }

    /// Returns the optional pending admin address waiting for [`LiquifactEscrow::accept_admin`],
    /// or [`None`] when no admin handover is in progress.
    pub fn get_pending_admin(env: Env) -> Option<Address> {
        Self::get_pending_admin_impl(env)
    }

    /// Compare the escrow contract's current funding-token balance to the recorded `funded_amount`.
    ///
    /// Returns a signed discrepancy equal to `contract_balance - recorded_funded_amount`.
    /// Positive values indicate the contract holds more than the recorded funded amount;
    /// negative values indicate a shortfall.
    ///
    /// # Authorization
    /// Requires the current admin to authorize the call. This makes the entrypoint suitable for
    /// manual audits and for off-chain schedulers or external triggers that reconcile custody.
    pub fn verify_asset_custody(env: Env) -> i128 {
        Self::verify_asset_custody_impl(env)
    }

    /// Return whether this escrow has a configured maturity time lock.
    ///
    /// `true` means [`InvoiceEscrow::maturity`] is positive and [`LiquifactEscrow::settle`] requires
    /// `Env::ledger().timestamp() >= maturity`. `false` means `maturity == 0`: there is no maturity
    /// gate, so a funded escrow can be settled immediately by the SME, subject to legal-hold and
    /// status guards.
    pub fn has_maturity_lock(env: Env) -> bool {
        Self::has_maturity_lock_impl(env)
    }

    /// Move up to `amount` (capped by balance and [`MAX_DUST_SWEEP_AMOUNT`]) of the **funding token**
    /// from this contract to [`DataKey::Treasury`].
    ///
    /// # Terminal state requirement
    /// Only permitted when [`InvoiceEscrow::status`] is **2 (settled)**, **3 (withdrawn)**, or
    /// **4 (cancelled)**. Open (0) or funded (1) states reject the call so live principal cannot
    /// be swept as dust.
    ///
    /// # Liability floor invariant
    /// In **cancelled** (status 4) escrows, the sweep is rejected if it would reduce the
    /// contract's token balance below the amount still owed to investors who have not yet
    /// called [`LiquifactEscrow::refund`]:
    ///
    /// ```text
    /// outstanding = funded_amount - distributed_principal
    /// assert balance - sweep_amt >= outstanding
    /// ```
    ///
    /// `distributed_principal` ([`DataKey::DistributedPrincipal`]) is incremented atomically
    /// by [`LiquifactEscrow::refund`] each time an investor's principal is returned. This makes
    /// the invariant computable on-chain without iterating over all investor addresses.
    ///
    /// In **settled** (2) and **withdrawn** (3) states, disbursement is off-chain and this
    /// floor does not apply.
    ///
    /// # Authorization
    /// The configured **treasury** account must authorize this call; the admin cannot sweep unless
    /// it is also the treasury.
    ///
    /// Blocked while [`DataKey::LegalHold`] is active.
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes for legal hold, invalid sweep amount, non-terminal state,
    /// missing initialized addresses, empty balances, liability floor violation, and token
    /// transfer invariant failures.
    pub fn sweep_terminal_dust(env: Env, amount: i128) -> i128 {
        Self::sweep_terminal_dust_impl(env, amount)
    }

    /// Archive a terminal escrow to mark it as closed and reduce active monitoring.
    ///
    /// Transitions the escrow to status `5` (archived). Only permitted from terminal states:
    /// `2` (settled), `3` (withdrawn), `4` (cancelled), or `5` (already archived — no-op).
    /// Open (`0`) or funded (`1`) states are rejected.
    ///
    /// # Authorization
    /// Requires admin authorization. Read-only operations continue to work on archived escrows.
    /// Archived escrows remain accessible but should be excluded from active-monitoring queries
    /// by off-chain indexers.
    ///
    /// # Errors
    /// Emits [`EscrowError::ArchiveNotTerminal`] if the escrow is not in a terminal state.
    /// Emits [`EscrowError::EscrowAlreadyArchived`] if already status `5`.
    pub fn archive_escrow(env: Env) -> InvoiceEscrow {
        Self::archive_escrow_impl(env)
    }

    pub fn get_escrow(env: Env) -> InvoiceEscrow {
        Self::get_escrow_impl(env)
    }

    /// Rotate the beneficiary (SME) address that receives liquidity on
    /// settlement / `withdraw`.
    ///
    /// Permitted only before settlement (`status` 0 = open or 1 = funded) and
    /// while no legal hold is active. Requires authorization from **both** the
    /// current SME and the admin, so the payout destination can never be changed
    /// unilaterally. A no-op rotation to the current address is rejected. Emits
    /// [`BeneficiaryRotated`] with the prior and new addresses and returns the
    /// updated escrow snapshot.
    ///
    /// # Errors
    ///
    /// | Condition | Typed error |
    /// |-----------|-------------|
    /// | Legal hold active | [`EscrowError::LegalHoldBlocksBeneficiaryRotation`] |
    /// | Escrow not open or funded | [`EscrowError::RotationNotOpen`] |
    /// | `new_sme_address == current SME` | [`EscrowError::NewSmeSameAsCurrent`] |
    pub fn rotate_beneficiary(env: Env, new_sme_address: Address) -> InvoiceEscrow {
        Self::rotate_beneficiary_impl(env, new_sme_address)
    }

    pub fn get_version(env: Env) -> u32 {
        Self::get_version_impl(env)
    }

    /// Returns the contract interface version ([`CONTRACT_INTERFACE_VERSION`]).
    ///
    /// Use this to detect caller/contract ABI mismatches **before** invoking state-mutating
    /// entrypoints. This value is a compile-time constant baked into the deployed WASM and
    /// does **not** require the escrow to be initialized (no [`DataKey::Escrow`] read).
    ///
    /// # Interface version vs schema version
    ///
    /// | Getter | Constant | Tracks |
    /// |--------|----------|--------|
    /// | `get_interface_version` | [`CONTRACT_INTERFACE_VERSION`] | Entrypoint signatures, parameter lists, return types |
    /// | `get_version` | [`SCHEMA_VERSION`] | On-chain storage layout (XDR structs, `DataKey` variants) |
    ///
    /// # Caller guidance
    ///
    /// SDKs and integration adapters should call `get_interface_version` at startup and
    /// compare the returned value against the version they were compiled against. A mismatch
    /// means the deployed contract has a different ABI than expected; the caller should
    /// refuse further calls and surface a diagnostic error rather than silently mis-parse
    /// arguments or return values.
    ///
    /// See `docs/escrow-interface-versioning.md` for the full versioning policy.
    ///
    /// # Authorization
    ///
    /// None — pure read; no auth required. Safe to call before `init`.
    pub fn get_interface_version(_env: Env) -> u32 {
        Self::get_interface_version_impl(_env)
    }

    /// Return the build metadata baked into this WASM artifact.
    ///
    /// This is the canonical way for off-chain tooling and CI to verify that a
    /// deployed contract was built from the expected source commit, with the
    /// expected [`SCHEMA_VERSION`] and [`CONTRACT_INTERFACE_VERSION`].
    ///
    /// The values are compile-time constants produced by `escrow/build.rs`
    /// (git commit hash, build timestamp, package version, rustc version,
    /// and the source-of-truth schema/interface versions). They are part of
    /// the WASM binary, not on-chain storage, so:
    ///
    /// - The values do not change after deploy (a WASM upgrade would produce
    ///   a new instance with a new `CONTRACT_ID` and a new build metadata).
    /// - The entrypoint is available **without** prior [`LiquifactEscrow::init`].
    /// - No authentication is required.
    ///
    /// The `schema_version` and `interface_version` returned here are
    /// **extracted from `src/lib.rs` at compile time** by `build.rs`, so they
    /// are guaranteed to match the source constants — `scripts/validate_build_metadata.py`
    /// enforces this in CI.
    pub fn get_build_metadata(env: Env) -> BuildMetadata {
        Self::get_build_metadata_impl(env)
    }

    /// Get the optional funding deadline (ledger timestamp), returns None if not set.
    pub fn get_funding_deadline(env: Env) -> Option<u64> {
        Self::get_funding_deadline_impl(env)
    }

    /// Check if funding has expired (deadline set and now > deadline).
    pub fn is_funding_expired(env: Env) -> bool {
        Self::is_funding_expired_impl(env)
    }

    /// Read-only health check: compute and return the current health metrics without state mutation.
    /// Returns `(warning_type, funded_ratio_bps, time_to_maturity_secs)`.
    ///
    /// # Returns
    /// - `warning_type`: 0 = no warning, 4001–4004 = specific warning condition.
    /// - `funded_ratio_bps`: basis points ratio (0–10_000+, clamped to i64::MAX on overflow).
    /// - `time_to_maturity_secs`: seconds until maturity (negative if past, i64::MAX if no maturity).
    ///
    /// # Authorization
    /// None — pure read; no auth required.
    pub fn check_escrow_health(env: Env) -> (u32, i64, i64) {
        Self::check_escrow_health_impl(env)
    }

    /// Whether a compliance/legal hold is active (defaults to `false` if unset).
    pub fn get_legal_hold(env: Env) -> bool {
        Self::get_legal_hold_impl(env)
    }

    /// Whether a compliance/legal hold is active (defaults to `false` if unset).
    pub fn get_legal_hold_status(env: Env) -> bool {
        Self::get_legal_hold_status_impl(env)
    }

    /// Returns the reason for the current legal hold, if any.
    /// Absent when no hold is active or the hold was set without a reason.
    pub fn get_legal_hold_reason(env: Env) -> Option<String> {
        Self::get_legal_hold_reason_impl(env)
    }

    /// Configured minimum delay between [`LiquifactEscrow::request_clear_legal_hold`]
    /// and [`LiquifactEscrow::set_legal_hold(env, false)`]. Defaults to `0`.
    pub fn get_legal_hold_clear_delay(env: Env) -> u64 {
        Self::get_legal_hold_clear_delay_impl(env)
    }

    /// Reserved minimum ledger timestamp at which a pending legal-hold clear may be applied.
    /// `None` means no request has been recorded.
    pub fn get_legal_hold_clearable_at(env: Env) -> Option<u64> {
        Self::get_legal_hold_clearable_at_impl(env)
    }

    /// Minimum principal per [`LiquifactEscrow::fund`] or [`LiquifactEscrow::fund_with_commitment`] call
    /// in token base units; `0` means no extra floor beyond “amount must be positive”.
    ///
    /// **Ceilings:** [`InvoiceEscrow::funding_target`] and over-funding behavior are unchanged; the floor
    /// applies to **each** call, so follow-on deposits from the same investor must also meet the floor.
    pub fn get_min_contribution_floor(env: Env) -> i128 {
        Self::get_min_contribution_floor_impl(env)
    }

    /// Optional cap on **distinct** investor addresses (`prev == 0` at fund time); [`None`] if unlimited.
    ///
    /// Reflects the current stored cap, including any admin reduction via
    /// [`LiquifactEscrow::lower_max_unique_investors`].
    pub fn get_max_unique_investors_cap(env: Env) -> Option<u32> {
        Self::get_max_unique_investors_cap_impl(env)
    }

    /// Optional cap on total principal for a single investor address.
    /// Absent ⇒ unlimited. Enforced on every deposit.
    pub fn get_max_per_investor_cap(env: Env) -> Option<i128> {
        Self::get_max_per_investor_cap_impl(env)
    }

    /// The configured [`YieldTier`] ladder, if any. Absent ⇒ flat `yield_bps` for all investors.
    pub fn get_yield_tier_table(env: Env) -> Option<Vec<YieldTier>> {
        Self::get_yield_tier_table_impl(env)
    }

    /// Distinct funders counted so far (each address counted once when it first receives principal).
    ///
    /// **Sybil:** this limits distinct **chain accounts**, not real-world persons; Sybil resistance is
    /// not a goal of this counter.
    pub fn get_unique_funder_count(env: Env) -> u32 {
        Self::get_unique_funder_count_impl(env)
    }

    /// Compute and return dashboard‑ready health metrics for the escrow.
    ///
    /// Read‑only — **no authorization required**. Intended for dashboards
    /// and monitoring tools that surface at‑a‑glance escrow health.
    ///
    /// # Design notes
    ///
    /// - `funding_progress_percent` is computed as `(funded_amount × 100) / funding_target`
    ///   (floor), capped at 100 when overfunded.
    /// - `days_to_maturity` uses integer seconds → days conversion (86_400 s/day).
    ///   Returns 0 when `maturity == 0` (no maturity lock). Negative when maturity
    ///   has passed (i.e., settlement is already possible).
    /// - `estimated_yield_payout` is the flat coupon on the whole `funded_amount` —
    ///   pro‑rata per‑investor breakdown requires the funding‑close snapshot.
    pub fn get_escrow_health_metrics(env: Env) -> EscrowHealthMetrics {
        Self::get_escrow_health_metrics_impl(env)
    }

    /// Bundles multiple read-only values to return a comprehensive summary of the escrow state
    /// in a single host invocation.
    pub fn get_escrow_summary(env: Env) -> EscrowSummary {
        Self::get_escrow_summary_impl(env)
    }

    /// Returns discovery metadata for this escrow, suitable for registry contracts
    /// and off-chain indexers.
    ///
    /// Bundles the escrow's own address, invoice identifier, SME, creation timestamp,
    /// current status, and funding target into a single read call. This is the canonical
    /// on-chain source for registry listings.
    ///
    /// # Returns
    /// [`RegistryListing`] with escrow metadata. `created_at` is `0` for escrows that
    /// predate the [`DataKey::CreatedAt`] storage key.
    pub fn get_registry_listing(env: Env) -> RegistryListing {
        Self::get_registry_listing_impl(env)
    }

    /// Returns investor capacity status for this escrow.
    ///
    /// Provides a self-explanatory, single-call answer to "can more investors contribute?"
    /// without requiring client-side arithmetic on `max_unique_investors_cap` and `unique_funder_count`.
    ///
    /// **Read-only** — no authorization required, no state mutation.
    ///
    /// # Returns
    /// [`InvestorCapStatus`] containing:
    /// - `max`: Maximum number of distinct investors allowed. Returns `u32::MAX` when no cap is set.
    /// - `current`: Current number of distinct investors that have contributed.
    /// - `remaining`: Remaining capacity for new investors (always `max - current`).
    /// - `is_full`: `true` when `current == max` (escrow is at capacity).
    ///
    /// # Notes
    /// - When no cap is configured (`get_max_unique_investors_cap()` returns `None`),
    ///   `max` is set to `u32::MAX` and `is_full` is always `false`.
    /// - When a cap exists, `remaining = max - current`.
    /// - This is a read-only convenience entrypoint; cap enforcement logic is unchanged.
    pub fn get_investor_cap_status(env: Env) -> InvestorCapStatus {
        Self::get_investor_cap_status_impl(env)
    }

    /// Bind a **primary** 32-byte digest (e.g. SHA-256 of an IPFS CID or document bundle). **Single-set:**
    /// the call succeeds only while no primary hash exists; use [`LiquifactEscrow::append_attestation_digest`]
    /// for an append-only audit trail.
    ///
    /// **Authorization:** [`InvoiceEscrow::admin`]. **Frontrunning:** whichever binding transaction lands
    /// first wins; observers must read on-chain state (or parse events) after finality—there is no replay lock.
    ///
    /// # Validation
    /// The `digest` must be exactly 32 bytes. Shorter or longer inputs are rejected with a typed error
    /// rather than an opaque panic, giving callers a recoverable signal.
    ///
    /// # Errors
    /// - [`EscrowError::AttestationHashAlreadyBound`] if a primary hash already exists.
    /// - [`EscrowError::InvalidAttestationHashLength`] if `digest.len() != 32`.
    /// - [`EscrowError::EscrowNotInitialized`] if called before `init`.
    pub fn bind_primary_attestation_hash(env: Env, digest: Bytes) {
        Self::bind_primary_attestation_hash_impl(env, digest)
    }

    pub fn get_primary_attestation_hash(env: Env) -> Option<BytesN<32>> {
        Self::get_primary_attestation_hash_impl(env)
    }

    /// Append a digest to a bounded on-chain log (see [`MAX_ATTESTATION_APPEND_ENTRIES`]) for **versioned**
    /// or incremental attestation updates. Does not replace [`LiquifactEscrow::bind_primary_attestation_hash`].
    ///
    /// An optional `tag` (Soroban [`Symbol`]) may be provided for filtering via
    /// [`LiquifactEscrow::query_attestation_logs`]; pass an empty symbol for no tag.
    /// The ledger timestamp is automatically recorded for time-based queries.
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes when the escrow is uninitialized or the append log is full.
    pub fn append_attestation_digest(env: Env, digest: BytesN<32>, tag: Symbol) {
        Self::append_attestation_digest_impl(env, digest, tag)
    }

    pub fn get_attestation_append_log(env: Env) -> Vec<BytesN<32>> {
        Self::get_attestation_append_log_impl(env)
    }

    /// Public API: Check the current delegate for an investor (if any).
    pub fn get_yield_claim_delegate(env: Env, investor: Address) -> Option<Address> {
        Self::get_yield_claim_delegate_impl(env, investor)
    }

    /// Public API: Check whether an investor's delegation is revoked.
    pub fn is_yield_claim_delegate_revoked(env: Env, investor: Address) -> bool {
        Self::is_yield_claim_delegate_revoked_impl(env, investor)
    }

    /// Public API: contribution recorded for `investor` (persistent storage).
    pub fn get_contribution(env: Env, investor: Address) -> i128 {
        Self::get_contribution_impl(env, investor)
    }

    /// Returns `investor`'s funding checkpoint history ([`DataKey::InvestorFundingHistory`]).
    ///
    /// Each entry is a snapshot recorded at a checkpoint: a deposit via
    /// [`LiquifactEscrow::fund`] / [`LiquifactEscrow::fund_with_commitment`] (`"deposit"`,
    /// or `"funding_close"` for the deposit that closes funding), or a settlement claim via
    /// [`LiquifactEscrow::claim_investor_payout`] (`"settle"`). Bounded by
    /// [`MAX_INVESTOR_HISTORY_ENTRIES`]; returns an empty vector if `investor` never funded.
    pub fn get_investor_history(env: Env, investor: Address) -> Vec<InvestorFundingRecord> {
        Self::get_investor_history_impl(env, investor)
    }

    /// Pro-rata denominator captured when the escrow first became **funded**; [`None`] until then.
    ///
    /// The snapshot is write-once. It records the full `funded_amount` at the threshold-crossing
    /// funding call, including any over-funding past `funding_target`, plus the close ledger time
    /// and sequence used by off-chain auditors.
    pub fn get_funding_close_snapshot(env: Env) -> Option<FundingCloseSnapshot> {
        Self::get_funding_close_snapshot_impl(env)
    }

    /// Returns the Merkle root bound at funding close, or [`None`] when not yet
    /// computed (escrow has not reached funded status).
    ///
    /// The root is a Keccak-256 hash of the Merkle tree over (investor_address, contribution)
    /// pairs at the moment the escrow became funded. Off-chain verifiers use this root
    /// with [`LiquifactEscrow::verify_investor_proof`] to confirm an investor's
    /// contribution and pro-rata share without iterating on-chain storage.
    pub fn get_funding_close_merkle_root(env: Env) -> Option<BytesN<32>> {
        Self::get_funding_close_merkle_root_impl(env)
    }

    /// Effective yield (bps) for this investor after their **first** deposit; later [`LiquifactEscrow::fund`]
    /// calls add principal at this rate. Defaults to [`InvoiceEscrow::yield_bps`] when unset (legacy positions).
    ///
    /// Note: reads `DataKey::Escrow` for the base yield fallback; callers that already hold the
    /// escrow should prefer reading `DataKey::InvestorEffectiveYield` directly.
    pub fn get_investor_yield_bps(env: Env, investor: Address) -> i64 {
        Self::get_investor_yield_bps_impl(env, investor)
    }

    /// Earliest ledger timestamp for [`LiquifactEscrow::claim_investor_payout`]; `0` if not gated.
    pub fn get_investor_claim_not_before(env: Env, investor: Address) -> u64 {
        Self::get_investor_claim_not_before_impl(env, investor)
    }

    /// Retrieve the configured yield slippage detection threshold (in basis points).
    /// Returns `0` if slippage detection is disabled (no threshold configured).
    ///
    /// When the absolute difference between actual and expected yield exceeds this threshold,
    /// a [`YieldSlippageWarning`] is emitted during [`LiquifactEscrow::claim_investor_payout`].
    pub fn get_yield_slippage_threshold(env: Env) -> i64 {
        Self::get_yield_slippage_threshold_impl(env)
    }

    /// Returns the configured protocol fee percentage in basis points, or `0` if
    /// no fee was configured at init. Applied to the gross yield coupon at settlement;
    /// the fee is transferred to treasury.
    pub fn get_fee_percentage(env: Env) -> i64 {
        Self::get_fee_percentage_impl(env)
    }

    /// Returns the configured per-ledger maximum funding rate, or `0` if unlimited.
    pub fn get_max_funding_rate(env: Env) -> u64 {
        Self::get_max_funding_rate_impl(env)
    }

    /// Returns `true` when funding is paused (auto-paused by velocity breach or
    /// manual admin action). Admin may resume via [`LiquifactEscrow::resume_funding`].
    pub fn is_funding_paused(env: Env) -> bool {
        Self::is_funding_paused_impl(env)
    }

    /// Compute expected and actual yield for an investor, and the resulting slippage deviation.
    ///
    /// Returns:
    /// - `expected_yield_bps`: escrow base yield (from [`InvoiceEscrow::yield_bps`])
    /// - `actual_yield_bps`: investor's effective yield (from [`DataKey::InvestorEffectiveYield`] or base)
    /// - `deviation_bps`: absolute difference in basis points
    ///
    /// Useful for off-chain tools to preview whether a claim would trigger a slippage warning.
    pub fn get_investor_yield_slippage(env: Env, investor: Address) -> (i64, i64, i64) {
        Self::get_investor_yield_slippage_impl(env, investor)
    }

    /// Retrieve the currently recorded SME collateral commitment metadata from storage.
    /// Returns `None` if no commitment has been recorded yet.
    pub fn get_sme_collateral_commitment(env: Env) -> Option<SmeCollateralCommitment> {
        Self::get_sme_collateral_commitment_impl(env)
    }

    /// Detect and report state inconsistencies in the escrow contract.
    ///
    /// This is a **read-only diagnostic entrypoint** that scans the escrow state for logical
    /// invariant violations without modifying storage. Returns a [`StateInconsistencyReport`]
    /// with flags indicating detected issues. An empty report (all fields `false`) signals valid state.
    ///
    /// # Detectable inconsistencies
    ///
    /// - **funded_exceeds_target_stale**: `funded_amount > funding_target` but `status < 1`.
    ///   Funding has surpassed the target yet the escrow was not advanced to funded.
    /// - **funded_positive_status_open**: `funded_amount > 0` but `status == 0`.
    ///   Principal was received but escrow remains in open state.
    /// - **funded_zero_status_advanced**: `funded_amount == 0` but `status >= 1`.
    ///   A funded/settled/withdrawn escrow has no principal.
    /// - **funders_exist_status_open**: `unique_funder_count > 0` but `status == 0`.
    ///   Investors have contributed but escrow is still open.
    /// - **no_funders_advanced_status**: `unique_funder_count == 0` but `status >= 1`.
    ///   A funded/settled escrow has no recorded investors.
    /// - **snapshot_exists_not_funded**: Funding close snapshot set before `status == 1`.
    ///   Snapshot should only exist once the escrow reaches funded status.
    /// - **snapshot_missing_post_funded**: `status >= 2` but no funding close snapshot.
    ///   Past the funded point, the snapshot is immutable and must exist.
    /// - **settled_before_maturity_lock**: `status == 2` but current ledger time < `maturity`
    ///   (when `maturity > 0`). Settlement occurred before the configured lock expired.
    ///   *Note: Returns `false` if maturity is not set (no time lock configured).*
    /// - **invalid_funding_amounts**: `funding_target <= 0` or `amount <= 0`.
    /// - **invalid_status_value**: `status < 0` or `status > 4`.
    ///   Valid range is 0 (open) through 4 (cancelled).
    ///
    /// # Emitted event
    ///
    /// If any inconsistency is detected (i.e., any report flag is `true`), a
    /// [`StateInconsistenciesDetected`] event is emitted for auditing and monitoring.
    ///
    /// # Authorization
    ///
    /// None — pure read; no auth required. Safe to call at any time.
    ///
    /// # Returns
    ///
    /// A [`StateInconsistencyReport`] capturing all detected violations.
    pub fn detect_state_inconsistencies(env: Env) -> StateInconsistencyReport {
        Self::detect_state_inconsistencies_impl(env)
    }

    pub fn revoke_attestation_digest(env: Env, index: u32) {
        Self::revoke_attestation_digest_impl(env, index)
    }

    pub fn is_attestation_revoked(env: Env, index: u32) -> bool {
        Self::is_attestation_revoked_impl(env, index)
    }

    pub fn is_investor_claimed(env: Env, investor: Address) -> bool {
        Self::is_investor_claimed_impl(env, investor)
    }

    /// Returns the ledger timestamp until which this investor is locked from claiming
    /// after funding. Returns 0 if no lock-in was configured.
    pub fn get_investor_lock_in_until(env: Env, investor: Address) -> u64 {
        Self::get_investor_lock_in_until_impl(env, investor)
    }

    /// Record or replace the optional SME collateral commitment metadata.
    ///
    /// **Metadata-only:** this writes [`DataKey::SmeCollateralPledge`] and emits
    /// [`CollateralRecordedEvt`]. It does not transfer tokens, reserve balances, verify custody,
    /// create an on-chain encumbrance, or block any contract flows (such as settlement, withdrawals,
    /// or claims).
    ///
    /// # Authorization
    /// - Requires the signature of the configured SME (`InvoiceEscrow::sme_address`). Enforced via
    ///   `sme_address.require_auth()` during execution.
    ///
    /// # Validation Rules
    /// - **Positive Amount:** The `amount` parameter must be strictly positive (`amount > 0`).
    /// - **Non-empty Asset Symbol:** The `asset` parameter must be a non-empty Symbol (not equal to `Symbol::new(&env, "")`).
    /// - **Monotonic Timestamp:** When replacing an existing commitment, the current ledger timestamp must not
    ///   be earlier than the prior `recorded_at` value (`now >= prior.recorded_at`).
    ///
    /// # Errors
    /// - [`EscrowError::CollateralAmountNotPositive`] if `amount <= 0`.
    /// - [`EscrowError::CollateralAssetEmpty`] if `asset` is empty.
    /// - [`EscrowError::CollateralTimestampBackwards`] if the replacement timestamp is in the past.
    /// - [`EscrowError::CollateralUpdateAfterSettlement`] if the escrow has already settled,
    ///   been withdrawn, or been cancelled (status >= 2). Initial records on open or funded
    ///   escrows are always permitted.
    /// - Standard uninitialized check via `load_escrow_require_sme`.
    pub fn record_sme_collateral_commitment(
        env: Env,
        asset: Symbol,
        amount: i128,
    ) -> SmeCollateralCommitment {
        Self::record_sme_collateral_commitment_impl(env, asset, amount)
    }

    /// Set or clear compliance hold. Clearing requires the current [`InvoiceEscrow::admin`].
    /// Activation also requires only the admin when no guardian is configured; when a guardian
    /// is configured, use [`LiquifactEscrow::propose_legal_hold`] and
    /// [`LiquifactEscrow::confirm_legal_hold`] instead.
    ///
    /// **Clearing:** always requires the current admin's authorization — there is no timelock,
    /// council override, or break-glass entrypoint. After
    /// [`LiquifactEscrow::propose_admin`] and [`LiquifactEscrow::accept_admin`], only the **new**
    /// admin can clear a persisted hold.
    ///
    /// **Governance posture:** production `admin` must be a multisig or governed contract so
    /// hold + key loss cannot strand funds without an off-chain recovery vote that executes
    /// `propose_admin`, `accept_admin`, then `clear_legal_hold`. See
    /// `docs/escrow-legal-hold.md`.
    pub fn set_legal_hold(env: Env, active: bool, reason: String) {
        Self::set_legal_hold_impl(env, active, reason)
    }

    /// Propose a legal hold for guardian confirmation. Proposals expire after
    /// [`LEGAL_HOLD_PROPOSAL_TTL_SECS`] seconds of ledger time.
    pub fn propose_legal_hold(env: Env, admin: Address) {
        Self::propose_legal_hold_impl(env, admin)
    }

    /// Confirm a pending legal hold using the configured guardian address.
    pub fn confirm_legal_hold(env: Env, guardian: Address) {
        Self::confirm_legal_hold_impl(env, guardian)
    }

    /// Pause or unpause the escrow.
    ///
    /// While paused, **all** state‑mutating entrypoints are blocked:
    /// `fund`, `fund_with_commitment`, `settle`, `withdraw`,
    /// `claim_investor_payout`, `cancel_funding`, `refund`, `sweep_terminal_dust`,
    /// `rotate_beneficiary`, `record_sme_collateral_commitment`, and
    /// `lower_max_unique_investors` / `update_funding_target` / `update_maturity`.
    ///
    /// Read‑only entrypoints continue to work. The admin can always unpause.
    ///
    /// # Authorization
    /// Requires the current [`InvoiceEscrow::admin`].
    pub fn set_escrow_paused(env: Env, paused: bool) {
        Self::set_escrow_paused_impl(env, paused)
    }

    /// Read the current paused state.
    ///
    /// `true` when the escrow is paused; no authorization required.
    pub fn is_escrow_paused(env: Env) -> bool {
        Self::is_escrow_paused_impl(env)
    }

    /// Schedule a compliance hold clear window. The current admin must authorize.
    ///
    /// If a non-zero clear delay is configured, the hold may not be lifted until the
    /// returned ledger timestamp is reached.
    ///
    /// # Errors
    ///
    /// | Condition | Typed error |
    /// |-----------|-------------|
    /// | `timestamp + delay` overflows | [`EscrowError::LegalHoldClearDelayOverflow`] |
    pub fn request_clear_legal_hold(env: Env) {
        Self::request_clear_legal_hold_impl(env)
    }

    /// Revalidate and update the cached token metadata by fetching fresh decimals from the token contract.
    ///
    /// This is an **admin-only** entrypoint that explicitly updates the token metadata cache
    /// (normally written only at [`LiquifactEscrow::init`]). Used when:
    /// - The token contract is upgraded and decimals change
    /// - The cached metadata is suspected stale
    /// - Governance decides to refresh the cache for other reasons
    ///
    /// # Returns
    ///
    /// The updated [`TokenMetadataCache`] with fresh decimals and current ledger time/sequence.
    ///
    /// # Authorization
    ///
    /// Requires admin authorization. This prevents accidental cache pollution by non-admins.
    pub fn enable_yield_auto_distribution(env: Env) {
        Self::enable_yield_auto_distribution_impl(env)
    }

    /// Disable automatic yield distribution for this escrow.
    ///
    /// When disabled, yields are computed on-demand per `claim_investor_payout` call.
    /// Has no effect on already-claimed investors (claim markers persist).
    /// Has no effect if the escrow is already settled and snapshot was captured.
    ///
    /// # Authorization
    ///
    /// Requires the current [`InvoiceEscrow::admin`].
    pub fn disable_yield_auto_distribution(env: Env) {
        Self::disable_yield_auto_distribution_impl(env)
    }

    /// Check if automatic yield distribution is enabled for this escrow.
    ///
    /// When enabled, [`LiquifactEscrow::settle`] pre-computes each investor's yield
    /// and stores it for batch claims. When disabled or not set, yields are computed
    /// on-demand during each `claim_investor_payout` call.
    pub fn is_yield_auto_dist_enabled(env: Env) -> bool {
        Self::is_yield_auto_dist_enabled_impl(env)
    }

    /// Revalidate and update the cached token metadata by fetching fresh decimals from the token contract.
    ///
    /// This is an **admin-only** entrypoint that explicitly updates the token metadata cache
    /// (normally written only at [`LiquifactEscrow::init`]). Used when:
    /// - The token contract is upgraded and decimals change
    /// - The cached metadata is suspected stale
    /// - Governance decides to refresh the cache for other reasons
    ///
    /// # Returns
    ///
    /// The updated [`TokenMetadataCache`] with fresh decimals and current ledger time/sequence.
    ///
    /// # Authorization
    ///
    /// Requires admin authorization. This prevents accidental cache pollution by non-admins.
    pub fn revalidate_token_cache(env: Env) -> TokenMetadataCache {
        Self::revalidate_token_cache_impl(env)
    }

    /// Enable or disable the investor allowlist. When enabled, only addresses with
    /// [`DataKey::InvestorAllowlisted`] set to true may fund the escrow.
    pub fn set_allowlist_active(env: Env, active: bool) {
        Self::set_allowlist_active_impl(env, active)
    }

    pub fn is_allowlist_active(env: Env) -> bool {
        Self::is_allowlist_active_impl(env)
    }

    /// Resume funding after an automatic velocity pause or manual admin pause.
    ///
    /// Clears the [`DataKey::FundingPaused`] flag. Does not alter the configured
    /// [`DataKey::MaxFundingRate`] — use [`LiquifactEscrow::set_max_funding_rate`]
    /// to adjust the rate after a breach.
    ///
    /// # Authorization
    /// The configured **admin** must authorize this call.
    pub fn resume_funding(env: Env) {
        Self::resume_funding_impl(env)
    }

    /// Adjust the per-ledger maximum funding rate.
    ///
    /// Useful after a velocity breach: admin may raise the rate before resuming
    /// funding.
    ///
    /// # Authorization
    /// The configured **admin** must authorize this call.
    pub fn set_max_funding_rate(env: Env, new_rate: u64) {
        Self::set_max_funding_rate_impl(env, new_rate)
    }

    /// Add or remove an investor from the allowlist.
    pub fn set_investor_allowlisted(env: Env, investor: Address, allowed: bool) {
        Self::set_investor_allowlisted_impl(env, investor, allowed)
    }

    /// Batch add or remove investors from the allowlist.
    ///
    /// Accepts a `Vec<Address>` and a single `allowed` flag. Requires admin authorization
    /// once. The call is rejected for empty vectors or vectors longer than
    /// `MAX_INVESTOR_ALLOWLIST_BATCH` to keep storage and CPU bounded.
    ///
    /// Invariant: the end state and emitted events are identical to calling
    /// `set_investor_allowlisted` individually for each element in `investors`.
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes when the escrow is uninitialized, the batch is empty, or
    /// the batch exceeds [`MAX_INVESTOR_ALLOWLIST_BATCH`].
    pub fn set_investors_allowlisted(env: Env, investors: Vec<Address>, allowed: bool) {
        Self::set_investors_allowlisted_impl(env, investors, allowed)
    }

    pub fn is_investor_allowlisted(env: Env, investor: Address) -> bool {
        Self::is_investor_allowlisted_impl(env, investor)
    }

    /// Convenience alias for [`LiquifactEscrow::set_legal_hold`] with `active = false`.
    pub fn clear_legal_hold(env: Env) {
        Self::clear_legal_hold_impl(env)
    }

    pub fn update_funding_target(env: Env, new_target: i128) -> InvoiceEscrow {
        Self::update_funding_target_impl(env, new_target)
    }

    /// Lower the configured distinct-investor cap while the escrow is still open.
    ///
    /// This is admin-only and intentionally cannot raise a cap or impose one on an unlimited
    /// escrow. Existing investors remain able to add principal after the cap is lowered; only new
    /// investor addresses are blocked once `UniqueFunderCount >= new_cap`.
    ///
    /// # Panics
    /// - If the escrow is not open.
    /// - If no unique-investor cap was configured at initialization.
    /// - If `new_cap` is not strictly lower than the current cap.
    /// - If `new_cap` is below the current unique funder count.
    pub fn lower_max_unique_investors(env: Env, new_cap: u32) -> u32 {
        Self::lower_max_unique_investors_impl(env, new_cap)
    }

    /// Validate the stored schema version and apply a migration if one is implemented.
    ///
    /// # Behavior - **typed error on all current paths**
    ///
    /// This entrypoint currently contains **no implemented migration logic**. Every call
    /// terminates with a typed contract error (aborts the Soroban transaction). This is intentional:
    /// it makes the "no migration" guarantee explicit rather than silently returning success.
    ///
    /// **Execution order:** the function first requires current admin authorization, then reads
    /// [`DataKey::Version`] from instance storage, validates the supplied `from_version`, and emits
    /// a typed error. No storage writes ever occur in the current release. The authorization guard
    /// is intentionally placed before version checks so future migration logic remains admin-gated
    /// by construction.
    ///
    /// Do **not** call `migrate` expecting it to perform bookkeeping work in the current
    /// release. To add a real migration path (e.g. rewriting a stored struct after a field
    /// addition), implement the transformation above the final error branch, update
    /// [`DataKey::Version`], and bump [`SCHEMA_VERSION`].
    ///
    /// # When to call
    ///
    /// - **Only** when you have extended `migrate` with a concrete transformation for the
    ///   `from_version → SCHEMA_VERSION` path you need.
    /// - Additive new [`DataKey`] variants read with `.get(...).unwrap_or(default)` do **not**
    ///   require a `migrate` call; old instances simply return the default.
    /// - If `InvoiceEscrow` struct layout changed, `migrate` cannot help — redeploy instead.
    ///
    /// # Errors
    ///
    /// Requires current admin authorization before any version checks or future storage rewrites.
    ///
    /// | Condition | Typed error |
    /// |-----------|--------|
    /// | `stored_version != from_version` | [`EscrowError::MigrationVersionMismatch`] |
    /// | `from_version >= SCHEMA_VERSION` | [`EscrowError::AlreadyCurrentSchemaVersion`] |
    /// | Any `from_version < SCHEMA_VERSION` (all paths) | [`EscrowError::NoMigrationPath`] |
    ///
    /// See `docs/OPERATOR_RUNBOOK.md` §2 for step-by-step instructions on implementing
    /// a concrete migration path.
    pub fn migrate(env: Env, from_version: u32) -> u32 {
        Self::migrate_impl(env, from_version)
    }

    /// Query the attestation append log with optional filters and pagination.
    ///
    /// Returns a paginated page of [`AttestationLogEntry`] entries matching the given
    /// [`AuditLogFilter`]. Pass an empty filter to retrieve all entries unfiltered.
    ///
    /// # Filtering
    /// - `after_timestamp`: Only entries with `timestamp >= after_timestamp` (inclusive).
    /// - `tag_prefix`: Only entries whose tag exactly equals this value. Named `tag_prefix` for
    ///   the field's original intent, but implemented as an exact [`Symbol`] match — `Symbol`
    ///   exposes no substring API that is safe to call from a `wasm32` contract build.
    ///
    /// # Pagination
    /// - `limit`: Maximum entries per page (clamped to the available matching entries).
    /// - `offset`: Number of matching entries to skip before returning results.
    ///
    /// # Performance
    /// Optimized for recent-logs queries: entries are scanned in append order (oldest first)
    /// and filtered in Wasm memory. Callers should use `after_timestamp` to prune scans.
    pub fn query_attestation_logs(
        env: Env,
        filters: AuditLogFilter,
        limit: u32,
        offset: u32,
    ) -> AuditLogPage {
        Self::query_attestation_logs_impl(env, filters, limit, offset)
    }

    /// Export the complete escrow state as a structured snapshot for archival, analysis,
    /// compliance, and audit reporting.
    ///
    /// Returns an [`EscrowSnapshot`] containing all instance-storage state: escrow, version,
    /// legal hold, funding close snapshot, funder count, allowlist status, SME collateral,
    /// attestation metadata, sanctions provider, funding token, treasury, registry, and
    /// version history.
    ///
    /// The snapshot **does not** include per-investor records (contributions, claimed flags,
    /// allowlist entries) because Soroban cannot enumerate persistent storage keys on-chain.
    /// Combine this snapshot with off-chain indexer data for a full investor list.
    pub fn export_escrow_snapshot(env: Env) -> EscrowSnapshot {
        Self::export_escrow_snapshot_impl(env)
    }

    /// Returns the version history audit trail as a vector of `(version, ledger_timestamp)` tuples.
    ///
    /// Each tuple records when a schema version was first written to this instance — on
    /// [`LiquifactEscrow::init`] and on each successful [`LiquifactEscrow::migrate`].
    /// This helps operators track deployment lineage and audit upgrade events.
    ///
    /// Returns an empty vector if no history has been recorded (escrow not initialized).
    pub fn get_version_history(env: Env) -> Vec<(u32, u64)> {
        Self::get_version_history_impl(env)
    }

    /// Replaces the deployed contract WASM with the binary identified by `new_wasm_hash`.
    ///
    /// # Authorization
    /// Only the current escrow admin may call this function. Authorization is verified
    /// before any deployer call. Unauthorised callers will cause the transaction to revert.
    ///
    /// # State
    /// No persistent storage keys, escrow records, or balances are modified.
    /// Only the contract code is replaced. `SCHEMA_VERSION` is not incremented here;
    /// run `migrate()` after upgrading if schema changes accompany the new WASM.
    ///
    /// # Interaction with ADR-007
    /// This function does not add, remove, or rename any `DataKey` variants.
    /// It is safe to upgrade to a WASM that adds new `DataKey` variants (additive-key
    /// policy) without calling `migrate()`, but removing or reordering variants in the
    /// new WASM would corrupt stored data. Operators must verify additive-only changes
    /// before upgrading.
    ///
    /// # Risks
    /// Deploying an incompatible WASM will corrupt stored state. Test thoroughly on
    /// testnet before upgrading production contracts.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) {
        Self::upgrade_impl(env, new_wasm_hash)
    }

    /// Record investor principal while the invoice is **open**. First deposit sets base
    /// [`InvoiceEscrow::yield_bps`] for this investor; further amounts must use this method (not
    /// [`LiquifactEscrow::fund_with_commitment`]) so tier selection stays immutable after the first leg.
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes for invalid amount, legal hold, closed funding state,
    /// allowlist rejection, cap violations, and checked-arithmetic overflow.
    pub fn fund(env: Env, investor: Address, amount: i128) -> InvoiceEscrow {
        Self::contract_fund(env, investor, amount)
    }

    /// First deposit only (per investor): optional longer lock and tier ladder from [`DataKey::YieldTierTable`].
    /// Sets [`DataKey::InvestorClaimNotBefore`] when `committed_lock_secs > 0`. Additional principal
    /// from the same investor must use [`LiquifactEscrow::fund`].
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes for the same funding guards as [`LiquifactEscrow::fund`],
    /// plus tiered follow-on deposit misuse and claim-lock timestamp overflow.
    pub fn fund_with_commitment(
        env: Env,
        investor: Address,
        amount: i128,
        committed_lock_secs: u64,
    ) -> InvoiceEscrow {
        Self::fund_with_commitment_impl(env, investor, amount, committed_lock_secs)
    }

    /// Batch funding entrypoint: record multiple investor principals in a single call.
    ///
    /// Each entry is processed sequentially with per-investor [`Address::require_auth()`].
    /// All existing [`LiquifactEscrow::fund`] invariants (allowlist, caps, min contribution,
    /// overflow guards) are enforced per entry. If an entry fails its invariants,
    /// the call returns an error without corrupting prior entries.
    ///
    /// # Parameters
    /// - `entries`: `Vec<(Address, i128)>` of (investor address, funding amount) tuples.
    ///
    /// # Errors
    /// - [`EscrowError::FundingBatchEmpty`] if entries is empty
    /// - [`EscrowError::FundingBatchTooLarge`] if entries.len() > [`MAX_FUND_BATCH`]
    /// - Per-entry: all errors from [`LiquifactEscrow::fund`] for that investor/amount pair
    ///
    /// # Events
    /// One [`EscrowFunded`] event per entry (identical to single [`LiquifactEscrow::fund`] semantics).
    ///
    /// # Funded-target snapshot
    /// If any entry causes the escrow to transition to **funded** (status 0 → 1),
    /// [`DataKey::FundingCloseSnapshot`] is recorded exactly once. Remaining entries are
    /// processed even after transition.
    pub fn fund_batch(env: Env, entries: Vec<(Address, i128)>) -> InvoiceEscrow {
        Self::fund_batch_impl(env, entries)
    }

    /// Batch funding entrypoint: record multiple investor principals in a single atomic call
    /// and emit a [`BatchFundCompleted`] summary event when all entries succeed.
    ///
    /// This is the canonical FEAT-001 entrypoint. It differs from [`LiquifactEscrow::fund_batch`]
    /// in one key way: after every entry is processed it emits a single `BatchFundCompleted` event
    /// carrying the aggregate totals (`total_funded_amount` and `investor_count`), which lets
    /// indexers reconcile a batch submission with a single event lookup rather than scanning all
    /// individual per-entry [`EscrowFunded`] events.
    ///
    /// Each entry is processed sequentially with per-investor [`Address::require_auth()`].
    /// All existing [`LiquifactEscrow::fund`] invariants (allowlist, caps, min contribution,
    /// overflow guards) are enforced per entry. If any single entry fails its invariants the
    /// entire transaction reverts atomically — no partial state is persisted.
    ///
    /// # Parameters
    /// - `contributions`: `Vec<(Address, i128)>` of `(investor_address, amount)` tuples.
    ///   Maximum [`MAX_FUND_BATCH`] (50) entries per call.
    ///
    /// # Returns
    /// The updated [`InvoiceEscrow`] after all entries are applied.
    ///
    /// # Events
    /// - Per entry: [`EscrowFunded`] + [`FundReceived`] (identical to single
    ///   [`LiquifactEscrow::fund`] semantics, emitted inside [`fund_impl`]).
    /// - Once, at end: [`BatchFundCompleted`] with aggregate totals.
    ///
    /// # Funded-target snapshot
    /// If any entry causes the escrow to transition to **funded** (`status 0 → 1`),
    /// [`DataKey::FundingCloseSnapshot`] is recorded exactly once. Remaining entries are
    /// still processed after the transition (over-funding is allowed up to `funding_target`
    /// per entry validation; exact semantics follow [`fund_impl`]).
    ///
    /// # Errors
    /// - [`EscrowError::FundingBatchEmpty`] if `contributions` is empty.
    /// - [`EscrowError::FundingBatchTooLarge`] if `contributions.len() > MAX_FUND_BATCH`.
    /// - Per-entry: all errors from [`LiquifactEscrow::fund`] for that investor/amount pair.
    pub fn batch_fund(env: Env, contributions: Vec<(Address, i128)>) -> InvoiceEscrow {
        Self::batch_fund_impl(env, contributions)
    }

    /// Closes funding early for an under-funded invoice, transitioning the escrow to a settleable state.
    ///
    /// # Authorization
    /// The configured **SME** address must authorize this call.
    ///
    /// Blocked while [`DataKey::LegalHold`] is active.
    /// Closes funding early for an under-funded invoice, transitioning the escrow to a settleable state.
    ///
    /// # Authorization
    /// The configured **SME** or **Admin** address must authorize this call.
    ///
    /// Blocked while [`DataKey::LegalHold`] is active.
    pub fn partial_settle(env: Env, caller: Address) -> InvoiceEscrow {
        Self::partial_settle_impl(env, caller)
    }

    /// Retry / standalone settlement notification. Invokes the configured
    /// [`DataKey::SettlementNotifierContract`] with the current settlement details.
    ///
    /// Use this entrypoint when:
    /// - The inline notifier call in [`LiquifactEscrow::settle`] reverted (and settlement
    ///   was retried without a notifier).
    /// - An off-chain relayer or indexer wants to push settlement data to an external system.
    ///
    /// # Authorization
    /// Open to any caller — the notifier itself enforces its own access control.
    ///
    /// # Errors
    /// Emits [`EscrowError::SettlementNotifierNotSet`] when no notifier was configured at init.
    /// Emits [`EscrowError::InvestorClaimNotSettled`] (127) when escrow status is not `2` (settled).
    pub fn notify_settlement(env: Env) {
        Self::notify_settlement_impl(env)
    }

    pub fn settle(env: Env, partial_amount: Option<i128>) -> InvoiceEscrow {
        Self::settle_impl(env, partial_amount)
    }

    /// Clone a settled escrow template to create a new independent escrow instance.
    ///
    /// Creates a fresh escrow for a **new invoice** (supplied by caller) with the same
    /// configuration parameters as the template escrow. The template must be in **settled** status
    /// (status == 2). All immutable configuration (guardian, yield_bps, maturity, yield tiers, min
    /// contribution, investor caps, legal hold clear delay, registry) is copied; per-investor state,
    /// funding state, pending proposals, and active legal holds are **reset** for the new instance.
    ///
    /// # Parameters
    ///
    /// - `template_contract`: The contract address of the **template** escrow (settled).
    ///   Clone internally cross-contract-calls the template's read-only entrypoints
    ///   (`get_escrow`, `get_funding_token`, etc.) to verify status and read immutable config —
    ///   a contract can never directly access another contract instance's storage.
    ///
    /// - `new_invoice_id` / `new_amount`: Caller must supply the new invoice identifier and funding target.
    ///
    /// The new escrow will have:
    /// - `invoice_id = new_invoice_id` (caller-supplied).
    /// - `admin, sme_address, yield_bps, maturity` = copied from template.
    /// - `amount, funding_target = new_amount` (caller-supplied).
    /// - `funded_amount = 0, status = 0` (open).
    /// - All per-investor state: cleared (new instance).
    /// - Immutable configuration (guardian, registry, token, treasury, yield tiers, caps, delays) = copied from template.
    ///
    /// # Errors
    ///
    /// - [`EscrowError::CloneNotSettled`] — template escrow is not in settled status.
    /// - [`EscrowError::CloneAmountNotPositive`] — `new_amount` is not positive.
    /// - [`EscrowError::InvoiceIdInvalidLength`] — `new_invoice_id` violates length constraints.
    /// - [`EscrowError::InvoiceIdInvalidCharset`] — `new_invoice_id` contains invalid characters.
    /// - Other [`EscrowError`] codes from `init` validation.
    ///
    /// # Authorization
    ///
    /// Only the **admin** (from the template escrow) may call this method.
    pub fn clone_settled_escrow(
        env: Env,
        template_contract: Address,
        new_invoice_id: String,
        new_amount: i128,
    ) -> InvoiceEscrow {
        Self::clone_settled_escrow_impl(env, template_contract, new_invoice_id, new_amount)
    }

    /// SME pulls funded liquidity. Transfers `funded_amount` of the bound funding token
    /// from this contract to `sme_address`, then transitions status to 3 (withdrawn).
    /// Blocked when a legal hold is active.
    ///
    /// # Guard ordering
    ///
    /// 1. Legal-hold gate (read-only).
    /// 2. `sme_address.require_auth()` and verify caller == sme_address.
    /// 3. Status == 1 (funded) check.
    /// 4. Contract balance sufficiency check ([`EscrowError::InsufficientContractBalance`]).
    /// 5. Status transition to 3, `DistributedPrincipal` update, storage write.
    /// 6. SEP-41 token transfer with balance-delta verification.
    /// 7. Event emission.
    ///
    /// # Errors
    /// - [`EscrowError::LegalHoldBlocksWithdrawal`] — hold is active.
    /// - [`EscrowError::WithdrawalNotFunded`] — escrow not in funded state.
    /// - [`EscrowError::InsufficientContractBalance`] — contract holds less than `funded_amount`.

    pub fn withdraw(env: Env, caller: Address) -> InvoiceEscrow {
        Self::withdraw_impl(env, caller)
    }

    /// Investor records a payout claim after settlement. Idempotent marker per investor.
    ///
    /// # Idempotency
    ///
    /// A second call for the same investor is a silent no-op: the `InvestorClaimed` marker is
    /// written **before** `InvestorPayoutClaimed` is emitted, so re-entrant or replayed calls
    /// return early without re-emitting the event.
    ///
    /// # Guard ordering (ADR-002)
    ///
    /// 1. Legal-hold gate (read-only).
    /// 2. `investor.require_auth()`.
    /// 3. Single contribution fetch — eliminates the previous duplicate `get_contribution` call;
    ///    the value is reused for the participation guard.
    /// 4. Settled-status gate (escrow read).
    /// 5. `not_before` ledger-time gate (see `docs/escrow-ledger-time.md`).
    /// 6. Idempotent early-return on `InvestorClaimed`.
    /// 7. Storage write + event emit.
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes for legal hold, missing contribution, unsettled escrow,
    /// or an unexpired commitment lock.
    pub fn claim_investor_payout(env: Env, investor: Address) {
        Self::claim_investor_payout_impl(env, investor)
    }

    /// Batch-claim investor payouts in a single invocation.
    ///
    /// Admin-only or treasury-only entrypoint that iterates a list of investors,
    /// performs all the usual claim checks (legal hold, settlement status, contribution,
    /// commitment locks, idempotency, yield slippage), and marks each as claimed.
    ///
    /// # Motivation
    ///
    /// Large escrows with many investors previously required one contract invocation
    /// per `claim_investor_payout`. This batch endpoint reduces the transaction count
    /// from O(N) to O(1) for the operator.
    ///
    /// # Authorization
    ///
    /// The current **admin** or **treasury** must authorize this call. The treasury
    /// is included so that protocol-operated settlement bots can batch-claim on behalf
    /// of investors without holding the admin key.
    ///
    /// # Bounds
    ///
    /// At most [`MAX_BATCH_CLAIM`] investors per call. Each investor's claim is
    /// processed independently — a failure for one does not roll back the batch.
    /// Investors that are already claimed, have zero contribution, or are still
    /// in their commitment lock period are silently skipped.
    ///
    /// # Events
    ///
    /// Emits one [`InvestorPayoutClaimed`] per successfully-claimed investor and a
    /// final [`BatchInvestorPayoutsClaimed`] with the total count processed.
    pub fn batch_claim_investor_payouts(env: Env, investors: Vec<Address>) -> u32 {
        Self::batch_claim_investor_payouts_impl(env, investors)
    }

    /// Delegate claims an investor's payout on behalf of the investor after settlement.
    ///
    /// # Authorization
    /// - **Delegate's signature** is required (the delegate authorized via `require_auth()`).
    /// - **Delegation must exist** and **not be revoked** for the investor.
    ///
    /// # Idempotency
    ///
    /// A second call (whether by the investor directly or by the delegate) is a silent no-op:
    /// the `InvestorClaimed` marker is checked first, and if set, the function returns early.
    ///
    /// # Guard ordering
    ///
    /// 1. Legal-hold gate (read-only).
    /// 2. Investor validation (contribution check).
    /// 3. Delegation validation (exists and not revoked).
    /// 4. Delegate authorization (`delegate.require_auth()`).
    /// 5. Settled-status gate (escrow read).
    /// 6. `not_before` ledger-time gate.
    /// 7. Idempotent early-return on `InvestorClaimed`.
    /// 8. Storage write + event emit.
    ///
    /// # Errors
    /// - [`EscrowError::LegalHoldBlocksInvestorClaims`] if a legal hold is active.
    /// - [`EscrowError::NoContributionToClaim`] if investor has no contribution.
    /// - [`EscrowError::NoDelegationSet`] if investor has not delegated.
    /// - [`EscrowError::DelegationRevoked`] if the delegation was revoked.
    /// - [`EscrowError::InvestorClaimNotSettled`] if escrow is not settled.
    /// - [`EscrowError::InvestorCommitmentLockNotExpired`] if claim lock has not expired.
    pub fn claim_payout_as_delegate(env: Env, investor: Address, delegate: Address) {
        Self::claim_payout_as_delegate_impl(env, investor, delegate)
    }

    /// Investor delegates their yield claim rights to another address.
    ///
    /// # Authorization
    /// - Requires the signature of the investor (via `investor.require_auth()`).
    ///
    /// # Behavior
    /// - Sets [`DataKey::YieldClaimDelegate`] for the investor to the specified `delegate` address.
    /// - Clears the [`DataKey::YieldClaimDelegateRevoked`] marker (if previously revoked, resets it).
    /// - Overwrites any previous delegation.
    ///
    /// # Validation
    /// - `delegate` must not be the same address as the investor.
    ///
    /// # Errors
    /// - [`EscrowError::DelegateAddressSameAsInvestor`] if `delegate == investor`.
    pub fn set_yield_claim_delegate(env: Env, investor: Address, delegate: Address) {
        Self::set_yield_claim_delegate_impl(env, investor, delegate)
    }

    /// Investor revokes their yield claim delegation.
    ///
    /// # Authorization
    /// - Requires the signature of the investor (via `investor.require_auth()`).
    ///
    /// # Behavior
    /// - Sets [`DataKey::YieldClaimDelegateRevoked`] for the investor to `true`.
    /// - Preserves the original delegate address in [`DataKey::YieldClaimDelegate`] for auditability.
    /// - After revocation, only the investor can call [`LiquifactEscrow::claim_investor_payout`] directly.
    ///
    /// # Errors
    /// - [`EscrowError::NoActiveDelegation`] if the investor has no active delegation to revoke.
    pub fn revoke_yield_claim_delegate(env: Env, investor: Address) {
        Self::revoke_yield_claim_delegate_impl(env, investor)
    }

    /// Investor rolls accrued yield from a settled source escrow into a live funding target.
    ///
    /// # Authorization
    /// - Requires the signature of the investor (via `investor.require_auth()`).
    ///
    /// # Parameters
    /// - `investor`: The investor address authorizing the reinvestment.
    /// - `target_escrow`: The escrow receiving the rolled yield.
    /// - `amount`: Positive yield amount to transfer from the settled source escrow into the target.
    ///
    /// # Behavior
    /// - Validates that this source escrow has reached `status == 2` (settled).
    /// - Validates that the target escrow is still in a funding flow (`status == 0 || status == 1`).
    /// - Uses the investor's accrued yield in the source escrow as the source of funds.
    /// - Records the reinvestment as additional principal for the target investor and emits
    ///   a `YieldReinvested` event.
    ///
    /// # Errors
    /// - [`EscrowError::ReinvestAmountNotPositive`] if `amount <= 0`
    /// - [`EscrowError::ReinvestSourceEscrowNotSettled`] if the source escrow is not settled.
    /// - [`EscrowError::ReinvestTargetEscrowNotFunding`] if the target is not in funding state.
    /// - [`EscrowError::ReinvestYieldInsufficient`] if `amount` exceeds the investor's accrued yield.
    pub fn reinvest_yield(env: Env, investor: Address, target_escrow: Address, amount: i128) {
        Self::reinvest_yield_impl(env, investor, target_escrow, amount)
    }

    /// On-chain read-only pro-rata gross payout for `investor`.
    ///
    /// Derives the **gross payout** (principal share plus `InvestorEffectiveYield`-adjusted
    /// coupon) from [`FundingCloseSnapshot`], providing an authoritative on-chain implementation
    /// of the math specified in `docs/escrow-pro-rata.md`. Off-chain tooling should call this
    /// view rather than re-implementing the formula to guarantee identical rounding.
    ///
    /// # Formula (floor / truncating integer division)
    ///
    /// ```text
    /// coupon       = total_principal × effective_yield_bps / 10_000  (floor)
    /// settle_pool  = total_principal + coupon
    /// gross_payout = contribution × settle_pool / total_principal     (floor)
    /// ```
    ///
    /// # Returns
    ///
    /// - `0` when [`DataKey::FundingCloseSnapshot`] does not exist (escrow not yet funded).
    /// - `0` when `investor` has no contribution (`DataKey::InvestorContribution` absent or zero).
    /// - Computed floor payout otherwise.
    ///
    /// # Invariant
    ///
    /// The sum of `compute_investor_payout` over all investors is ≤ `total_principal + coupon`;
    /// any rounding residual is swept by [`LiquifactEscrow::sweep_terminal_dust`].
    ///
    /// # Overflow safety
    ///
    /// All multiplications use [`i128::checked_mul`] and divisions use [`i128::checked_div`].
    /// Emits [`EscrowError::ComputePayoutArithmeticOverflow`] rather than silently producing a
    /// wrong value.
    ///
    /// # Authorization
    ///
    /// None — pure read; no auth required.
    pub fn compute_investor_payout(env: Env, investor: Address) -> i128 {
        Self::compute_investor_payout_impl(env, investor)
    }

    /// Verify a Merkle proof that `investor` contributed `contribution` at funding close.
    ///
    /// This entrypoint allows off-chain verifiers to confirm an investor's pro-rata share
    /// against the stored [`DataKey::FundingCloseMerkleRoot`] without iterating on-chain
    /// storage. The proof is a list of sibling hashes from the leaf to the root.
    ///
    /// # Proof format
    ///
    /// The proof is a Vec<BytesN<32>> where each element is a sibling hash. The verifier
    /// computes the leaf as `Keccak256(investor || contribution)` and walks up the tree,
    /// hashing with each sibling. If the resulting root matches the stored root, the proof
    /// is valid.
    ///
    /// # Returns
    ///
    /// `true` when the proof is valid; `false` when the root is absent (not yet funded)
    /// or the proof does not validate.
    ///
    /// # Security
    ///
    /// Uses Keccak-256 with domain separation. The leaf encoding is:
    /// `keccak256(address_bytes || contribution_be_bytes)`.
    /// This prevents second-preimage attacks (see `docs/escrow-merkle-snapshot.md`).
    pub fn verify_investor_proof(
        env: Env,
        investor: Address,
        contribution: i128,
        proof: Vec<BytesN<32>>,
    ) -> bool {
        Self::verify_investor_proof_impl(env, investor, contribution, proof)
    }

    pub fn update_maturity(env: Env, new_maturity: u64) -> InvoiceEscrow {
        Self::update_maturity_impl(env, new_maturity)
    }

    /// Extend storage TTLs for the contract instance and, optionally, a batch of per-investor
    /// persistent keys, in a single call.
    ///
    /// # Bounds
    ///
    /// `allowlisted` may be empty (the instance-storage TTL extension always runs regardless),
    /// but is capped at [`MAX_TTL_BUMP_BATCH`] entries; larger investor sets must be split
    /// across multiple calls.
    pub fn bump_ttl(env: Env, allowlisted: Vec<Address>) {
        Self::bump_ttl_impl(env, allowlisted)
    }

    /// Propose a new admin (`PendingAdmin`) — step 1 of a two-step handover.
    ///
    /// Requires current admin authorization. The destination must differ from the current admin.
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes when the escrow is uninitialized or `new_admin` is the
    /// current admin.
    pub fn propose_admin(env: Env, new_admin: Address) -> Address {
        Self::propose_admin_impl(env, new_admin)
    }

    /// Accept a pending admin handover.
    ///
    /// The address stored in [`DataKey::PendingAdmin`] must authorize this call. On success it is
    /// promoted into [`InvoiceEscrow::admin`] and the pending key is cleared, so admin authority
    /// changes only after both the current admin and successor have explicitly authorized.
    pub fn accept_admin(env: Env) -> InvoiceEscrow {
        Self::accept_admin_impl(env)
    }

    /// Deprecated shim for the former one-step admin transfer API.
    ///
    /// This function now only proposes `new_admin` by delegating to
    /// [`LiquifactEscrow::propose_admin`]. The proposed address must still call
    /// [`LiquifactEscrow::accept_admin`] before admin authority changes.
    #[deprecated(note = "use propose_admin followed by accept_admin")]
    pub fn transfer_admin(env: Env, new_admin: Address) -> InvoiceEscrow {
        Self::transfer_admin_impl(env, new_admin)
    }

    /// Transition an **open** escrow (status 0) to **cancelled** (status 4).
    ///
    /// Only the [`InvoiceEscrow::admin`] may call this. Blocked while a legal hold is active.
    /// After cancellation, investors may recover their principal via [`LiquifactEscrow::refund`].
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes when legal hold is active, the escrow is uninitialized,
    /// or the escrow is not in status 0 (open).
    pub fn cancel_funding(env: Env) -> InvoiceEscrow {
        Self::cancel_funding_impl(env)
    }

    /// Return an investor's recorded principal when the escrow is **cancelled** (status 4).
    ///
    /// Requires `investor` auth. Zeroes [`DataKey::InvestorContribution`] after transfer so a
    /// second call fails with [`EscrowError::NoContributionToRefund`].
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes when the escrow is not cancelled, the investor has no
    /// refundable contribution, initialized token data is missing, or the refund transfer fails
    /// token-balance invariants.
    pub fn refund(env: Env, investor: Address) {
        Self::refund_impl(env, investor)
    }

    /// Whether an investor has already received a refund in a cancelled escrow.
    pub fn is_investor_refunded(env: Env, investor: Address) -> bool {
        Self::is_investor_refunded_impl(env, investor)
    }

    /// Total principal already returned to investors via [`LiquifactEscrow::refund`].
    ///
    /// Used by [`LiquifactEscrow::sweep_terminal_dust`] to compute outstanding liabilities.
    /// Absent ⇒ `0` (no refunds have occurred).
    pub fn get_distributed_principal(env: Env) -> i128 {
        Self::get_distributed_principal_impl(env)
    }

    /// Pause the escrow due to a dispute (e.g., invoice validity challenge).
    ///
    /// Freezes the escrow independently of legal hold: blocks `fund`, `settle`, and `withdraw`
    /// until either auto-expiration (after `duration_secs`) or manual resumption.
    ///
    /// # Parameters
    /// - `ticket_id`: Support/dispute ticket reference (non-empty String for audit trail).
    /// - `duration_secs`: How long the pause lasts (must be positive).
    ///
    /// # Guard ordering
    ///
    /// 1. Ticket ID validation (non-empty).
    /// 2. Duration validation (positive).
    /// 3. `admin.require_auth()` (via `load_escrow_require_admin`).
    /// 4. Compute expiration timestamp and check for overflow.
    /// 5. Storage write and event emission.
    ///
    /// # Errors
    /// - [`EscrowError::DisputeTicketIdEmpty`] — `ticket_id` is empty.
    /// - [`EscrowError::DisputePauseDurationNotPositive`] — `duration_secs <= 0`.
    /// - [`EscrowError::DisputePauseDurationExceedsMax`] — `duration_secs` exceeds the maximum.
    /// - [`EscrowError::LedgerTimestampOverflow`] — computing expiration timestamp overflows.
    pub fn pause_dispute(env: Env, ticket_id: String, duration_secs: u64) {
        Self::pause_dispute_impl(env, ticket_id, duration_secs)
    }

    /// Resume (unpause) a dispute pause on the escrow.
    ///
    /// Clears the dispute pause, allowing normal escrow operations to resume.
    ///
    /// # Guard ordering
    ///
    /// 1. Dispute pause existence check.
    /// 2. `admin.require_auth()` (via `load_escrow_require_admin`).
    /// 3. Remove the pause state and emit event.
    ///
    /// # Errors
    /// - [`EscrowError::NoPauseActive`] — no active dispute pause exists.
    pub fn resume_dispute(env: Env) {
        Self::resume_dispute_impl(env)
    }

    /// Get the current dispute pause state, if active.
    ///
    /// Returns `None` if no pause is active or if the pause has auto-expired.
    pub fn get_dispute_pause(env: Env) -> Option<DisputePauseState> {
        Self::get_dispute_pause_impl(env)
    }
}
