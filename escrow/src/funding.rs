use super::*;

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
    pub(crate) fn init_impl(
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
        Self::init_internal(
            env,
            admin,
            None,
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
    pub(crate) fn init_with_guardian_impl(
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
        Self::init_internal(
            env,
            admin,
            Some(guardian),
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

    pub(crate) fn init_internal(
        env: Env,
        admin: Address,
        guardian: Option<Address>,
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
        admin.require_auth();
        if let Some(ref guardian_address) = guardian {
            ensure(
                &env,
                guardian_address != &admin,
                EscrowError::LegalHoldGuardianSameAsAdmin,
            );
        }

        // ── Contract-admin detection ──────────────────────────────────────────
        // A contract address as admin is legitimate (multisig, DAO timelock) but
        // widens the attack surface relative to a plain account. Detect it via
        // Address::executable(): Account ⇒ None or Executable::Account,
        // deployed contract ⇒ Executable::Wasm(_) or Executable::StellarAsset.
        //
        // If reject_contract_admin = Some(true) the call fails immediately.
        // Otherwise we emit AdminIsContractWarning so indexers can surface it.
        //
        // NOTE: executable() returns None when the address does not yet exist on
        // the ledger (e.g. a pre-funded account that has not been activated, or a
        // contract not yet deployed). We conservatively treat None as non-contract
        // so legitimate accounts that are merely unfunded are not falsely warned.
        let admin_is_contract = matches!(
            admin.executable(),
            Some(Executable::Wasm(_)) | Some(Executable::StellarAsset)
        );
        if admin_is_contract {
            if reject_contract_admin.unwrap_or(false) {
                fail(&env, EscrowError::ContractAdminRejected);
            }
            // Emit warning — callers can watch for symbol "adm_warn" in event logs.
            // The invoice_id is not yet validated at this point so we emit a
            // placeholder; the full id will appear in EscrowInitialized immediately after.
            AdminIsContractWarning {
                name: symbol_short!("adm_warn"),
                invoice_id: symbol_short!("pending"),
                admin: admin.clone(),
            }
            .publish(&env);
        }

        ensure(&env, amount > 0, EscrowError::AmountMustBePositive);
        ensure(
            &env,
            (0..=10_000).contains(&yield_bps),
            EscrowError::YieldBpsOutOfRange,
        );
        ensure(
            &env,
            !env.storage().instance().has(&DataKey::Escrow),
            EscrowError::EscrowAlreadyInitialized,
        );

        // Validate funding deadline
        if let Some(deadline) = funding_deadline {
            ensure(
                &env,
                deadline > env.ledger().timestamp(),
                EscrowError::FundingDeadlinePassed,
            );
            env.storage()
                .instance()
                .set(&DataKey::FundingDeadline, &deadline);
        }

        Self::validate_yield_tiers_table(&env, &yield_tiers, yield_bps);

        let invoice_sym = validate_invoice_id_string(&env, &invoice_id);

        let escrow = InvoiceEscrow {
            invoice_id: invoice_sym.clone(),
            admin: admin.clone(),
            guardian,
            sme_address: sme_address.clone(),
            amount,
            funding_target: amount,
            funded_amount: 0,
            yield_bps,
            maturity,
            status: 0,
        };

        // ── Post-condition invariant guard ──
        ensure!(
            escrow.funded_amount <= escrow.funding_target,
            EscrowError::InvariantViolation
        );

        env.storage().instance().set(&DataKey::Escrow, &escrow);
        env.storage()
            .instance()
            .set(&DataKey::Version, &SCHEMA_VERSION);

        // Record version history for audit trail
        let now_ts = env.ledger().timestamp();
        let mut vh: Vec<(u32, u64)> = Vec::new(&env);
        vh.push_back((SCHEMA_VERSION, now_ts));
        env.storage().instance().set(&DataKey::VersionHistory, &vh);

        env.storage()
            .instance()
            .set(&DataKey::FundingToken, &funding_token);
        env.storage().instance().set(&DataKey::Treasury, &treasury);

        // Cache token metadata at init time to reduce external calls on fund operations
        let token_client = TokenClient::new(&env, &funding_token);
        let decimals = token_client.decimals();
        let token_cache = TokenMetadataCache {
            decimals,
            cached_at_ledger_timestamp: env.ledger().timestamp(),
            cached_at_ledger_sequence: env.ledger().sequence(),
        };
        env.storage()
            .instance()
            .set(&DataKey::TokenMetadataCache, &token_cache);

        if let Some(ref r) = registry {
            env.storage().instance().set(&DataKey::RegistryRef, r);
        }

        // Optional KYC registry: when set, `fund` / `fund_with_commitment` require the investor
        // to be verified. See `LiquifactEscrow::check_kyc` for the expected contract interface.
        if let Some(ref kyc) = kyc_provider_contract {
            env.storage()
                .instance()
                .set(&DataKey::KycProviderContract, kyc);
        }

        // Optional tiered admin roles. Absent ⇒ `admin` is implicit `AdminRole::Full`.
        if let Some(roles) = admin_roles {
            Self::store_admin_roles(&env, &roles);
        }

        if let Some(ref tiers) = yield_tiers {
            if !tiers.is_empty() {
                env.storage()
                    .instance()
                    .set(&DataKey::YieldTierTable, tiers);
            }
        }

        let floor = min_contribution.unwrap_or(0);
        if min_contribution.is_some() {
            ensure(&env, floor > 0, EscrowError::MinContributionNotPositive);
            ensure(
                &env,
                floor <= amount,
                EscrowError::MinContributionExceedsAmount,
            );
        }
        env.storage()
            .instance()
            .set(&DataKey::MinContributionFloor, &floor);

        env.storage()
            .instance()
            .set(&DataKey::UniqueFunderCount, &0u32);
        // Initialize the investor-list count alongside the funder count.
        env.storage().instance().set(&DataKey::InvestorCount, &0u32);

        if let Some(cap) = max_per_investor {
            ensure(&env, cap > 0, EscrowError::MaxPerInvestorNotPositive);
            env.storage()
                .instance()
                .set(&DataKey::MaxPerInvestorCap, &cap);
        }

        if let Some(cap) = max_unique_investors {
            ensure(&env, cap > 0, EscrowError::MaxUniqueInvestorsNotPositive);
            env.storage()
                .instance()
                .set(&DataKey::MaxUniqueInvestorsCap, &cap);
        }

        let delay = legal_hold_clear_delay.unwrap_or(0);
        if delay > 0 {
            env.storage()
                .instance()
                .set(&DataKey::LegalHoldClearDelay, &delay);
        }

        // Validate and store yield slippage threshold
        let threshold = yield_slippage_threshold.unwrap_or(0);
        if threshold > 0 {
            ensure(
                &env,
                (0..=10_000).contains(&threshold),
                EscrowError::YieldSlippageThresholdOutOfRange,
            );
            env.storage()
                .instance()
                .set(&DataKey::YieldSlippageThreshold, &threshold);
        }

        // Store optional settlement notifier contract address
        if let Some(ref notifier) = settlement_notifier_contract {
            env.storage()
                .instance()
                .set(&DataKey::SettlementNotifierContract, notifier);
        }

        // Store creation timestamp for registry listings
        env.storage()
            .instance()
            .set(&DataKey::CreatedAt, &env.ledger().timestamp());

        EscrowInitialized {
            name: symbol_short!("escrow_ii"),
            // Read stored values so event fields match persisted keys (indexer single-event bootstrap).
            escrow: Self::get_escrow_impl(env.clone()),
            funding_token: Self::get_funding_token_impl(env.clone()),
            treasury: Self::get_treasury_impl(env.clone()),
            registry: Self::get_registry_ref_impl(env.clone()),
            has_maturity_lock: Self::has_maturity_lock_impl(env.clone()),
            yield_token: Self::get_yield_token(env.clone()),
            oracle_contract: Self::get_oracle_contract(env.clone()),
            nft_contract: Self::get_nft_contract(env.clone()),
        }
        .publish(&env);

        escrow
    }

    /// Returns the SEP-41 funding token bound at [`LiquifactEscrow::init`] ([`DataKey::FundingToken`]).
    ///
    /// **Immutable:** set once at init; cannot change after deploy. Emits
    /// [`EscrowError::FundingTokenNotSet`] if called before init.
    pub(crate) fn get_funding_token_impl(env: Env) -> Address {
        Self::funding_token_or_fail(&env)
    }

    /// Returns the protocol treasury address bound at [`LiquifactEscrow::init`] ([`DataKey::Treasury`]).
    ///
    /// **Immutable:** set once at init; cannot change after deploy. The treasury is the only
    /// recipient of [`LiquifactEscrow::sweep_terminal_dust`]. Emits
    /// [`EscrowError::TreasuryNotSet`] if called before init.
    pub(crate) fn get_treasury_impl(env: Env) -> Address {
        Self::treasury_or_fail(&env)
    }

    /// Returns the optional off-chain registry hint stored at [`DataKey::RegistryRef`], or [`None`]
    /// when no registry was supplied at [`LiquifactEscrow::init`].
    ///
    /// **Non-authority:** this address is a read-only discoverability hint for off-chain indexers.
    /// No on-chain logic in this contract consults it. Callers must **not** treat its presence as
    /// proof of registry membership — query the registry contract directly to verify on-chain state.
    pub(crate) fn get_registry_ref_impl(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::RegistryRef)
    }

    /// Best-effort registration with the immutable registry configured at init.
    ///
    /// Only the escrow admin may invoke this entrypoint. Registry failures are recorded in an
    /// event and return `false`; they never revert the escrow transaction.
    pub(crate) fn register_with_registry_impl(env: Env) -> bool {
        let escrow = Self::load_escrow_require_admin(&env);
        let registry = Self::get_registry_ref_impl(env.clone());
        let successful = registry.as_ref().is_some_and(|registry| {
            crate::external_calls::register_escrow_with_registry(
                &env,
                registry,
                escrow.invoice_id.clone(),
                env.current_contract_address(),
            )
        });

        RegistryRegistrationAttempted {
            name: symbol_short!("reg_attempt"),
            invoice_id: escrow.invoice_id,
            registry,
            successful,
        }
        .publish(&env);

        successful
    }

    /// Returns the cached token metadata (decimals, cache timestamp, cache sequence).
    ///
    /// This metadata is captured at [`LiquifactEscrow::init`] to reduce external token contract
    /// calls during fund operations. Returns [`None`] if not yet cached (should not happen if init succeeded).
    ///
    /// For staleness detection or re-caching, use [`LiquifactEscrow::revalidate_token_cache`].
    pub(crate) fn get_token_metadata_cache_impl(env: Env) -> Option<TokenMetadataCache> {
        env.storage().instance().get(&DataKey::TokenMetadataCache)
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
    pub(crate) fn verify_asset_custody_impl(env: Env) -> i128 {
        let escrow = Self::load_escrow_require_admin(&env);
        let token_addr = Self::funding_token_or_fail(&env);
        let this = env.current_contract_address();

        let contract_balance = TokenClient::new(&env, &token_addr).balance(&this);
        let recorded_funded_amount = escrow.funded_amount;
        let discrepancy = if contract_balance >= recorded_funded_amount {
            contract_balance - recorded_funded_amount
        } else {
            -(recorded_funded_amount - contract_balance)
        };

        AssetCustodyVerified {
            name: symbol_short!("cust_ver"),
            invoice_id: escrow.invoice_id.clone(),
            recorded_funded_amount,
            contract_balance,
            discrepancy,
        }
        .publish(&env);

        discrepancy
    }

    /// Return whether this escrow has a configured maturity time lock.
    ///
    /// `true` means [`InvoiceEscrow::maturity`] is positive and [`LiquifactEscrow::settle`] requires
    /// `Env::ledger().timestamp() >= maturity`. `false` means `maturity == 0`: there is no maturity
    /// gate, so a funded escrow can be settled immediately by the SME, subject to legal-hold and
    /// status guards.
    pub(crate) fn has_maturity_lock_impl(env: Env) -> bool {
        Self::get_escrow_impl(env).maturity > 0
    }

    /// Get the optional funding deadline (ledger timestamp), returns None if not set.
    pub(crate) fn get_funding_deadline_impl(env: Env) -> Option<u64> {
        env.storage().instance().get(&DataKey::FundingDeadline)
    }

    /// Check if funding has expired (deadline set and now > deadline).
    pub(crate) fn is_funding_expired_impl(env: Env) -> bool {
        if let Some(deadline) = env.storage().instance().get(&DataKey::FundingDeadline) {
            env.ledger().timestamp() > deadline
        } else {
            false
        }
    }

    /// Minimum principal per [`LiquifactEscrow::fund`] or [`LiquifactEscrow::fund_with_commitment`] call
    /// in token base units; `0` means no extra floor beyond “amount must be positive”.
    ///
    /// **Ceilings:** [`InvoiceEscrow::funding_target`] and over-funding behavior are unchanged; the floor
    /// applies to **each** call, so follow-on deposits from the same investor must also meet the floor.
    pub(crate) fn get_min_contribution_floor_impl(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::MinContributionFloor)
            .unwrap_or(0)
    }

    /// Optional cap on **distinct** investor addresses (`prev == 0` at fund time); [`None`] if unlimited.
    ///
    /// Reflects the current stored cap, including any admin reduction via
    /// [`LiquifactEscrow::lower_max_unique_investors`].
    pub(crate) fn get_max_unique_investors_cap_impl(env: Env) -> Option<u32> {
        env.storage()
            .instance()
            .get(&DataKey::MaxUniqueInvestorsCap)
    }

    /// Optional cap on total principal for a single investor address.
    /// Absent ⇒ unlimited. Enforced on every deposit.
    pub(crate) fn get_max_per_investor_cap_impl(env: Env) -> Option<i128> {
        env.storage().instance().get(&DataKey::MaxPerInvestorCap)
    }

    /// The configured [`YieldTier`] ladder, if any. Absent ⇒ flat `yield_bps` for all investors.
    pub(crate) fn get_yield_tier_table_impl(env: Env) -> Option<Vec<YieldTier>> {
        env.storage().instance().get(&DataKey::YieldTierTable)
    }

    /// Distinct funders counted so far (each address counted once when it first receives principal).
    ///
    /// **Sybil:** this limits distinct **chain accounts**, not real-world persons; Sybil resistance is
    /// not a goal of this counter.
    pub(crate) fn get_unique_funder_count_impl(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::UniqueFunderCount)
            .unwrap_or(0)
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
    pub(crate) fn get_investor_cap_status_impl(env: Env) -> InvestorCapStatus {
        let max_cap = Self::get_max_unique_investors_cap_impl(env.clone());
        let current_count = Self::get_unique_funder_count_impl(env);

        let (max, is_full, remaining) = match max_cap {
            Some(cap) => {
                let remaining = cap.saturating_sub(current_count);
                (cap, current_count >= cap, remaining)
            }
            None => {
                // No cap set: use u32::MAX and never mark as full
                (u32::MAX, false, u32::MAX.saturating_sub(current_count))
            }
        };

        InvestorCapStatus {
            max,
            current: current_count,
            remaining,
            is_full,
        }
    }

    /// Public API: contribution recorded for `investor` (persistent storage).
    pub(crate) fn get_contribution_impl(env: Env, investor: Address) -> i128 {
        Self::get_persistent_investor_contribution(&env, investor)
    }

    /// Returns `investor`'s funding checkpoint history ([`DataKey::InvestorFundingHistory`]).
    ///
    /// Each entry is a snapshot recorded at a checkpoint: a deposit via
    /// [`LiquifactEscrow::fund`] / [`LiquifactEscrow::fund_with_commitment`] (`"deposit"`,
    /// or `"funding_close"` for the deposit that closes funding), or a settlement claim via
    /// [`LiquifactEscrow::claim_investor_payout`] (`"settle"`). Bounded by
    /// [`MAX_INVESTOR_HISTORY_ENTRIES`]; returns an empty vector if `investor` never funded.
    pub(crate) fn get_investor_history_impl(
        env: Env,
        investor: Address,
    ) -> Vec<InvestorFundingRecord> {
        Self::get_persistent_investor_history(&env, investor)
    }

    /// Pro-rata denominator captured when the escrow first became **funded**; [`None`] until then.
    ///
    /// The snapshot is write-once. It records the full `funded_amount` at the threshold-crossing
    /// funding call, including any over-funding past `funding_target`, plus the close ledger time
    /// and sequence used by off-chain auditors.
    pub(crate) fn get_funding_close_snapshot_impl(env: Env) -> Option<FundingCloseSnapshot> {
        env.storage().instance().get(&DataKey::FundingCloseSnapshot)
    }

    /// Returns the Merkle root bound at funding close, or [`None`] when not yet
    /// computed (escrow has not reached funded status).
    ///
    /// The root is a Keccak-256 hash of the Merkle tree over (investor_address, contribution)
    /// pairs at the moment the escrow became funded. Off-chain verifiers use this root
    /// with [`LiquifactEscrow::verify_investor_proof`] to confirm an investor's
    /// contribution and pro-rata share without iterating on-chain storage.
    pub(crate) fn get_funding_close_merkle_root_impl(env: Env) -> Option<BytesN<32>> {
        env.storage()
            .instance()
            .get(&DataKey::FundingCloseMerkleRoot)
    }

    /// Effective yield (bps) for this investor after their **first** deposit; later [`LiquifactEscrow::fund`]
    /// calls add principal at this rate. Defaults to [`InvoiceEscrow::yield_bps`] when unset (legacy positions).
    ///
    /// Note: reads `DataKey::Escrow` for the base yield fallback; callers that already hold the
    /// escrow should prefer reading `DataKey::InvestorEffectiveYield` directly.
    pub(crate) fn get_investor_yield_bps_impl(env: Env, investor: Address) -> i64 {
        // env.clone(): env is used again after this call for the InvestorEffectiveYield read.
        let escrow = Self::get_escrow_impl(env.clone());
        Self::get_persistent_investor_effective_yield(&env, investor.clone())
            .unwrap_or(escrow.yield_bps)
    }

    /// Earliest ledger timestamp for [`LiquifactEscrow::claim_investor_payout`]; `0` if not gated.
    pub(crate) fn get_investor_claim_not_before_impl(env: Env, investor: Address) -> u64 {
        Self::get_persistent_investor_claim_not_before(&env, investor)
    }

    /// Returns the configured per-ledger maximum funding rate, or `0` if unlimited.
    pub(crate) fn get_max_funding_rate_impl(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::MaxFundingRate)
            .unwrap_or(0)
    }

    /// Returns `true` when funding is paused (auto-paused by velocity breach or
    /// manual admin action). Admin may resume via [`LiquifactEscrow::resume_funding`].
    pub(crate) fn is_funding_paused_impl(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::FundingPaused)
            .unwrap_or(false)
    }

    pub(crate) fn is_investor_claimed_impl(env: Env, investor: Address) -> bool {
        Self::get_persistent_investor_claimed(&env, investor)
    }

    /// Returns the ledger timestamp until which this investor is locked from claiming
    /// after funding. Returns 0 if no lock-in was configured.
    pub(crate) fn get_investor_lock_in_until_impl(env: Env, investor: Address) -> u64 {
        Self::get_persistent_investor_lock_in_until(&env, investor)
    }

    /// Enable or disable the investor allowlist. When enabled, only addresses with
    /// [`DataKey::InvestorAllowlisted`] set to true may fund the escrow.
    pub(crate) fn set_allowlist_active_impl(env: Env, active: bool) {
        let escrow = Self::load_escrow_require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::AllowlistActive, &active);
        AllowlistEnabledChanged {
            name: symbol_short!("al_ena"),
            invoice_id: escrow.invoice_id.clone(),
            active: if active { 1 } else { 0 },
        }
        .publish(&env);
    }

    pub(crate) fn is_allowlist_active_impl(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::AllowlistActive)
            .unwrap_or(false)
    }

    /// Resume funding after an automatic velocity pause or manual admin pause.
    ///
    /// Clears the [`DataKey::FundingPaused`] flag. Does not alter the configured
    /// [`DataKey::MaxFundingRate`] — use [`LiquifactEscrow::set_max_funding_rate`]
    /// to adjust the rate after a breach.
    ///
    /// # Authorization
    /// The configured **admin** must authorize this call.
    pub(crate) fn resume_funding_impl(env: Env) {
        let escrow = Self::load_escrow_require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::FundingPaused, &false);

        let rate: u64 = env
            .storage()
            .instance()
            .get(&DataKey::MaxFundingRate)
            .unwrap_or(0);

        FundingResumedEvent {
            name: symbol_short!("fund_resm"),
            invoice_id: escrow.invoice_id.clone(),
            max_funding_rate: rate,
        }
        .publish(&env);
    }

    /// Adjust the per-ledger maximum funding rate.
    ///
    /// Useful after a velocity breach: admin may raise the rate before resuming
    /// funding.
    ///
    /// # Authorization
    /// The configured **admin** must authorize this call.
    pub(crate) fn set_max_funding_rate_impl(env: Env, new_rate: u64) {
        let escrow = Self::load_escrow_require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::MaxFundingRate, &new_rate);

        // Clear the current-ledger accumulator so a new rate takes effect
        // immediately without carrying forward stale velocity data.
        env.storage()
            .instance()
            .set(&DataKey::LastFundLedger, &env.ledger().sequence());
        env.storage()
            .instance()
            .set(&DataKey::CurrentLedgerFunded, &0i128);

        FundingResumedEvent {
            name: symbol_short!("fund_resm"),
            invoice_id: escrow.invoice_id.clone(),
            max_funding_rate: new_rate,
        }
        .publish(&env);
    }

    /// Add or remove an investor from the allowlist.
    pub(crate) fn set_investor_allowlisted_impl(env: Env, investor: Address, allowed: bool) {
        let escrow = Self::load_escrow_require_admin(&env);
        Self::set_persistent_investor_allowlisted(&env, investor.clone(), allowed);

        InvestorAllowlistChanged {
            name: symbol_short!("al_set"),
            invoice_id: escrow.invoice_id.clone(),
            investor,
            allowed: if allowed { 1 } else { 0 },
        }
        .publish(&env);
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
    pub(crate) fn set_investors_allowlisted_impl(env: Env, investors: Vec<Address>, allowed: bool) {
        let escrow = Self::load_escrow_require_admin(&env);

        let n = investors.len();
        ensure(&env, n > 0, EscrowError::InvestorBatchEmpty);
        ensure(
            &env,
            n <= MAX_INVESTOR_ALLOWLIST_BATCH,
            EscrowError::InvestorBatchTooLarge,
        );

        // Iterate and perform per-address persistent storage write and event emission.
        for i in 0..n {
            let inv = investors.get(i).unwrap();
            Self::set_persistent_investor_allowlisted(&env, inv.clone(), allowed);

            InvestorAllowlistChanged {
                name: symbol_short!("al_set"),
                invoice_id: escrow.invoice_id.clone(),
                investor: inv.clone(),
                allowed: if allowed { 1 } else { 0 },
            }
            .publish(&env);
        }
    }

    pub(crate) fn is_investor_allowlisted_impl(env: Env, investor: Address) -> bool {
        Self::get_persistent_investor_allowlisted(&env, investor)
    }

    pub(crate) fn update_funding_target_impl(env: Env, new_target: i128) -> InvoiceEscrow {
        ensure(
            &env,
            !Self::escrow_paused_active(&env),
            EscrowError::EscrowIsPaused,
        );

        let mut escrow = Self::load_escrow_require_admin(&env);

        ensure(&env, new_target > 0, EscrowError::TargetNotPositive);
        ensure(&env, escrow.status == 0, EscrowError::TargetUpdateNotOpen);
        ensure(
            &env,
            new_target >= escrow.funded_amount,
            EscrowError::TargetBelowFundedAmount,
        );

        let old_target = escrow.funding_target;
        escrow.funding_target = new_target;

        // ── Post-condition invariant guard ──
        ensure!(
            escrow.funded_amount <= escrow.funding_target,
            EscrowError::InvariantViolation
        );

        env.storage().instance().set(&DataKey::Escrow, &escrow);

        FundingTargetUpdated {
            name: symbol_short!("fund_tgt"),
            invoice_id: escrow.invoice_id.clone(),
            old_target,
            new_target,
        }
        .publish(&env);

        escrow
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
    pub(crate) fn lower_max_unique_investors_impl(env: Env, new_cap: u32) -> u32 {
        ensure(
            &env,
            !Self::escrow_paused_active(&env),
            EscrowError::EscrowIsPaused,
        );

        let escrow = Self::load_escrow_require_admin(&env);

        ensure(&env, escrow.status == 0, EscrowError::CapLowerNotOpen);

        let old_cap: Option<u32> = env
            .storage()
            .instance()
            .get(&DataKey::MaxUniqueInvestorsCap);
        ensure(
            &env,
            old_cap.is_some(),
            EscrowError::NoInvestorCapConfigured,
        );
        let old_cap = old_cap.unwrap();
        let unique_count = Self::get_unique_funder_count_impl(env.clone());

        ensure(&env, new_cap < old_cap, EscrowError::NewCapNotLower);
        ensure(
            &env,
            new_cap >= unique_count,
            EscrowError::NewCapBelowCurrentFunderCount,
        );

        env.storage()
            .instance()
            .set(&DataKey::MaxUniqueInvestorsCap, &new_cap);

        MaxUniqueInvestorsCapLowered {
            name: symbol_short!("inv_cap"),
            invoice_id: escrow.invoice_id.clone(),
            old_cap,
            new_cap,
        }
        .publish(&env);

        new_cap
    }

    /// Record investor principal while the invoice is **open**. First deposit sets base
    /// [`InvoiceEscrow::yield_bps`] for this investor; further amounts must use this method (not
    /// [`LiquifactEscrow::fund_with_commitment`]) so tier selection stays immutable after the first leg.
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes for invalid amount, legal hold, closed funding state,
    /// allowlist rejection, cap violations, and checked-arithmetic overflow.
    pub(crate) fn contract_fund(env: Env, investor: Address, amount: i128) -> InvoiceEscrow {
        Self::fund_impl(env, investor, amount, true, 0)
    }

    /// First deposit only (per investor): optional longer lock and tier ladder from [`DataKey::YieldTierTable`].
    /// Sets [`DataKey::InvestorClaimNotBefore`] when `committed_lock_secs > 0`. Additional principal
    /// from the same investor must use [`LiquifactEscrow::fund`].
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes for the same funding guards as [`LiquifactEscrow::fund`],
    /// plus tiered follow-on deposit misuse and claim-lock timestamp overflow.
    pub(crate) fn fund_with_commitment_impl(
        env: Env,
        investor: Address,
        amount: i128,
        committed_lock_secs: u64,
    ) -> InvoiceEscrow {
        Self::fund_impl(env, investor, amount, false, committed_lock_secs)
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
    pub(crate) fn fund_batch_impl(env: Env, entries: Vec<(Address, i128)>) -> InvoiceEscrow {
        let n = entries.len();

        ensure(&env, n > 0, EscrowError::FundingBatchEmpty);
        ensure(&env, n <= MAX_FUND_BATCH, EscrowError::FundingBatchTooLarge);

        let mut escrow = Self::get_escrow_impl(env.clone());

        for i in 0..n {
            let (investor, amount) = entries.get(i).unwrap();

            // Call fund_impl for each entry, but we need to reconstruct the escrow
            // after each call. However, fund_impl returns the updated escrow,
            // so we capture it for the next iteration.
            escrow = Self::fund_impl(env.clone(), investor, amount, true, 0);
        }

        escrow
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
    pub(crate) fn batch_fund_impl(env: Env, contributions: Vec<(Address, i128)>) -> InvoiceEscrow {
        let n = contributions.len();

        ensure(&env, n > 0, EscrowError::FundingBatchEmpty);
        ensure(&env, n <= MAX_FUND_BATCH, EscrowError::FundingBatchTooLarge);

        // Track how much principal this batch call contributes in total.
        let funded_before: i128 = Self::get_escrow_impl(env.clone()).funded_amount;

        let mut escrow = Self::get_escrow_impl(env.clone());
        for i in 0..n {
            let (investor, amount) = contributions.get(i).unwrap();
            // fund_impl handles per-investor require_auth(), all validation, storage
            // writes, per-entry events, and status transitions.
            escrow = Self::fund_impl(env.clone(), investor, amount, true, 0);
        }

        // Total principal credited by this batch call.
        let total_funded_amount = escrow
            .funded_amount
            .checked_sub(funded_before)
            .unwrap_or(escrow.funded_amount);

        // Emit the single batch-level summary event.
        BatchFundCompleted {
            name: symbol_short!("btch_fnd"),
            invoice_id: escrow.invoice_id.clone(),
            total_funded_amount,
            investor_count: n,
            escrow_funded_amount: escrow.funded_amount,
            status: escrow.status,
        }
        .publish(&env);

        escrow
    }

    pub(crate) fn fund_impl(
        env: Env,
        investor: Address,
        amount: i128,
        simple_fund: bool,
        committed_lock_secs: u64,
    ) -> InvoiceEscrow {
        investor.require_auth();

        ensure(&env, amount > 0, EscrowError::FundingAmountNotPositive);

        let floor: i128 = env
            .storage()
            .instance()
            .get(&DataKey::MinContributionFloor)
            .unwrap_or(0);
        if floor > 0 {
            ensure(
                &env,
                amount >= floor,
                EscrowError::FundingBelowMinContribution,
            );
        }

        // env.clone(): env is used again after this call for storage writes and publish.
        let mut escrow = Self::get_escrow_impl(env.clone());
        // Legal hold check is intentionally after the escrow read: the escrow is needed for
        // status and yield_bps regardless, and hoisting the hold check before the escrow read
        // would not reduce storage operations (both keys are always read on this path).
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksFunding,
        );
        ensure(
            &env,
            !Self::is_dispute_paused_for_mutation(&env),
            EscrowError::DisputePausedBlocksFunding,
        );
        ensure(
            &env,
            escrow.status == 0,
            EscrowError::EscrowNotOpenForFunding,
        );

        // Check funding deadline
        if let Some(deadline) = env.storage().instance().get(&DataKey::FundingDeadline) {
            ensure(
                &env,
                env.ledger().timestamp() <= deadline,
                EscrowError::FundingDeadlinePassed,
            );
        }

        // ── Funding velocity auto-pause ──
        // Check whether funding has been paused (auto or manual).
        let funding_paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::FundingPaused)
            .unwrap_or(false);
        ensure(&env, !funding_paused, EscrowError::FundingIsPaused);

        // Velocity gate: if max_funding_rate is configured, enforce per-ledger cap.
        if let Some(rate) = env
            .storage()
            .instance()
            .get::<DataKey, u64>(&DataKey::MaxFundingRate)
        {
            let current_ledger = env.ledger().sequence();
            let last_ledger: u32 = env
                .storage()
                .instance()
                .get(&DataKey::LastFundLedger)
                .unwrap_or(0);
            let ledger_funded: i128 = if current_ledger == last_ledger {
                env.storage()
                    .instance()
                    .get(&DataKey::CurrentLedgerFunded)
                    .unwrap_or(0)
            } else {
                0i128
            };

            let new_ledger_total = ledger_funded
                .checked_add(amount)
                .unwrap_or_else(|| fail(&env, EscrowError::InvestorContributionOverflow));

            if new_ledger_total > rate as i128 {
                // Auto-pause: velocity exceeded — block further funding.
                env.storage().instance().set(&DataKey::FundingPaused, &true);

                FundingPausedEvent {
                    name: symbol_short!("fund_paus"),
                    invoice_id: escrow.invoice_id.clone(),
                    paused_at_ledger: current_ledger,
                    max_funding_rate: rate,
                    ledger_funded,
                }
                .publish(&env);

                fail(&env, EscrowError::FundingVelocityExceeded);
            }

            // Update velocity tracking.
            env.storage()
                .instance()
                .set(&DataKey::LastFundLedger, &current_ledger);
            env.storage()
                .instance()
                .set(&DataKey::CurrentLedgerFunded, &new_ledger_total);
        }

        if Self::is_allowlist_active_impl(env.clone()) {
            ensure(
                &env,
                Self::is_investor_allowlisted_impl(env.clone(), investor.clone()),
                EscrowError::InvestorNotAllowlisted,
            );
        }

        // Sanctions screening: check investor against configured sanctions provider
        Self::check_sanctions(&env, &investor);

        // KYC gating: reject funding from unverified investors when a KYC registry is configured.
        Self::check_kyc(&env, &investor);

        let prev: i128 = Self::get_persistent_investor_contribution(&env, investor.clone());
        let new_contribution: i128 = prev
            .checked_add(amount)
            .unwrap_or_else(|| fail(&env, EscrowError::InvestorContributionOverflow));

        if let Some(cap) = env
            .storage()
            .instance()
            .get::<DataKey, i128>(&DataKey::MaxPerInvestorCap)
        {
            ensure(
                &env,
                new_contribution <= cap,
                EscrowError::InvestorContributionExceedsCap,
            );
        }

        // Hoist UniqueFunderCount read: used for both the cap assertion (below) and the
        // increment write (after contribution is recorded). A single read covers both uses,
        // eliminating one storage read on every new-investor funding call.
        let cur_funder_count: u32 = if prev == 0 {
            env.storage()
                .instance()
                .get(&DataKey::UniqueFunderCount)
                .unwrap_or(0)
        } else {
            0 // prev != 0: count is not needed; skip the read entirely.
        };

        if prev == 0 {
            if let Some(cap) = env
                .storage()
                .instance()
                .get::<DataKey, u32>(&DataKey::MaxUniqueInvestorsCap)
            {
                ensure(
                    &env,
                    cur_funder_count < cap,
                    EscrowError::UniqueInvestorCapReached,
                );
            }
        }

        // Capture the effective yield and tier lock threshold in locals so event fields can
        // be populated without post-write storage reads.
        let investor_effective_yield_bps: i64;
        let tier_lock_secs: u64;

        if simple_fund {
            // Non-tiered deposits never carry a commitment lock.
            tier_lock_secs = 0;
            if prev == 0 {
                investor_effective_yield_bps = escrow.yield_bps;
                Self::set_persistent_investor_effective_yield(
                    &env,
                    investor.clone(),
                    escrow.yield_bps,
                );
                Self::set_persistent_investor_claim_not_before(&env, investor.clone(), 0u64);
            } else {
                // Returning investor: yield was set on first deposit; read it for the event.
                investor_effective_yield_bps =
                    Self::get_persistent_investor_effective_yield(&env, investor.clone())
                        .unwrap_or(escrow.yield_bps);
            }
            // If prev > 0, preserve existing effective yield and claim lock
        } else {
            ensure(&env, prev == 0, EscrowError::TieredSecondDeposit);
            let (eff, lock) =
                Self::effective_yield_for_commitment(&env, escrow.yield_bps, committed_lock_secs);
            investor_effective_yield_bps = eff;
            tier_lock_secs = lock;
            Self::set_persistent_investor_effective_yield(&env, investor.clone(), eff);
            let now = env.ledger().timestamp();
            let claim_nb = if committed_lock_secs == 0 {
                0u64
            } else {
                now.checked_add(committed_lock_secs)
                    .unwrap_or_else(|| fail(&env, EscrowError::InvestorClaimTimeOverflow))
            };
            // Bound: reject if the claim lock would expire after the escrow maturity.
            // Only constrained when both committed_lock_secs > 0 and maturity > 0.
            if claim_nb > 0 && escrow.maturity > 0 {
                ensure(
                    &env,
                    claim_nb <= escrow.maturity,
                    EscrowError::CommitmentLockExceedsMaturity,
                );
            }
            Self::set_persistent_investor_claim_not_before(&env, investor.clone(), claim_nb);

            // Set investor lock-in period (separate from tier commitment lock)
            let lock_in_secs: u64 = env
                .storage()
                .instance()
                .get(&DataKey::InvestorLockInSecs)
                .unwrap_or(0);
            if lock_in_secs > 0 {
                let lock_until = now
                    .checked_add(lock_in_secs)
                    .unwrap_or_else(|| fail(&env, EscrowError::InvestorClaimTimeOverflow));
                Self::set_persistent_investor_lock_in_until(&env, investor.clone(), lock_until);
            }
        }

        let new_funded = escrow
            .funded_amount
            .checked_add(amount)
            .unwrap_or_else(|| fail(&env, EscrowError::FundedAmountOverflow));

        ensure!(
            new_funded <= escrow.funding_target,
            EscrowError::FundingTargetExceeded
        );

        escrow.funded_amount = new_funded;

        // --- Concentration cap check ---
        if let Some(concentration_cap) = env
            .storage()
            .instance()
            .get::<DataKey, i64>(&DataKey::MaxInvestorConcentration)
        {
            if escrow.funded_amount > 0 {
                // concentration = (investor_contribution * 100) / total_funded
                let concentration_pct = new_contribution
                    .checked_mul(100)
                    .and_then(|v| v.checked_div(escrow.funded_amount))
                    .unwrap_or(0);
                if concentration_pct > concentration_cap as i128 {
                    let diagnostic = ErrorDiagnostic::with_context(
                        &env,
                        EscrowError::ConcentrationLimitExceeded as u32,
                        "Investor concentration exceeds configured cap",
                        "Reduce funding amount or wait for other investors to fund",
                        &format!(
                            "Current: {}%, Limit: {}%",
                            concentration_pct, concentration_cap
                        ),
                    );
                    Self::emit_error_diagnostic(&env, diagnostic);
                    fail(&env, EscrowError::ConcentrationLimitExceeded);
                }
            }
        }

        if escrow.status == 0 && escrow.funded_amount >= escrow.funding_target {
            escrow.status = 1;
            if !env.storage().instance().has(&DataKey::FundingCloseSnapshot) {
                let snap = FundingCloseSnapshot {
                    total_principal: escrow.funded_amount,
                    funding_target: escrow.funding_target,
                    closed_at_ledger_timestamp: env.ledger().timestamp(),
                    closed_at_ledger_sequence: env.ledger().sequence(),
                };
                env.storage()
                    .instance()
                    .set(&DataKey::FundingCloseSnapshot, &snap);

                // Validate that snapshot timestamp is before maturity.
                // If funding closed after maturity, emit a warning event for audit trail.
                if escrow.maturity > 0 && snap.closed_at_ledger_timestamp >= escrow.maturity {
                    // Snapshot recorded after or at maturity; emit warning type 4004.
                    let funded_ratio_bps: i64 = if escrow.funding_target > 0 {
                        let numerator = (escrow.funded_amount as i128).saturating_mul(10_000);
                        let ratio = numerator / (escrow.funding_target as i128);
                        if ratio > i64::MAX as i128 {
                            i64::MAX
                        } else if ratio < 0 {
                            0
                        } else {
                            ratio as i64
                        }
                    } else {
                        10_000_i64
                    };

                    let time_to_maturity_secs: i64 = {
                        let maturity_i64 = escrow.maturity as i64;
                        let now_i64 = snap.closed_at_ledger_timestamp as i64;
                        maturity_i64.saturating_sub(now_i64)
                    };

                    EscrowHealthWarning {
                        name: symbol_short!("hlth_wrn"),
                        invoice_id: escrow.invoice_id.clone(),
                        warning_type: 4004, // FundingClosedAfterMaturity
                        funded_amount: escrow.funded_amount,
                        funding_target: escrow.funding_target,
                        funded_ratio_bps,
                        time_to_maturity_secs,
                        recorded_at_ledger_timestamp: snap.closed_at_ledger_timestamp,
                    }
                    .publish(&env);
                }

                // Compute and store Merkle root for proof-based claim verification.
                // The root is a Keccak-256 hash of the empty set when there are no
                // investor entries to include (unlikely at funded status, but safe).
                let merkle_root = Self::compute_empty_merkle_root(&env);
                env.storage()
                    .instance()
                    .set(&DataKey::FundingCloseMerkleRoot, &merkle_root);

                FundingCloseMerkleRootBound {
                    name: symbol_short!("fc_mr"),
                    invoice_id: escrow.invoice_id.clone(),
                    merkle_root: merkle_root.clone(),
                }
                .publish(&env);
            }
        }

        Self::set_persistent_investor_contribution(&env, investor.clone(), new_contribution);

        // Record a funding-history checkpoint for this deposit (see #148). `escrow.status`
        // already reflects the funded-status transition (if any) computed above.
        let history_checkpoint = if escrow.status == 1 {
            symbol_short!("fnd_close")
        } else {
            symbol_short!("deposit")
        };
        Self::append_investor_history_record(
            &env,
            investor.clone(),
            history_checkpoint,
            amount,
            new_contribution,
        );

        // Update the bucketed aggregate for dashboard O(1) reads.
        {
            let bucket = Self::investor_bucket_index(&env, &investor);
            let prev_bucket: i128 = env
                .storage()
                .instance()
                .get(&DataKey::InvestorContributionBucket(bucket))
                .unwrap_or(0);
            env.storage().instance().set(
                &DataKey::InvestorContributionBucket(bucket),
                &(prev_bucket + amount),
            );
        }

        if prev == 0 {
            // Use the hoisted cur_funder_count; no second storage read needed.
            env.storage()
                .instance()
                .set(&DataKey::UniqueFunderCount, &(cur_funder_count + 1));
            // Maintain ordered investor index for list_investors pagination.
            // Read InvestorCount (may differ from UniqueFunderCount on pre-v6 instances
            // that did not initialize it; default to 0 so the first index is always 0).
            let investor_count: u32 = env
                .storage()
                .instance()
                .get(&DataKey::InvestorCount)
                .unwrap_or(0);
            env.storage()
                .instance()
                .set(&DataKey::InvestorIndex(investor_count), &investor.clone());
            env.storage()
                .instance()
                .set(&DataKey::InvestorCount, &(investor_count + 1));
        }

        env.storage().instance().set(&DataKey::Escrow, &escrow);

        EscrowFunded {
            name: symbol_short!("funded"),
            invoice_id: escrow.invoice_id.clone(),
            investor: investor.clone(),
            amount,
            funded_amount: escrow.funded_amount,
            status: escrow.status,
            // Locals set at write time; no post-write storage reads required.
            investor_effective_yield_bps,
            tier_lock_secs,
        }
        .publish(&env);

        // Also emit the versioned FundReceived with timestamp + actor.
        FundReceived {
            name: symbol_short!("fund_recv"),
            invoice_id: escrow.invoice_id.clone(),
            actor: investor.clone(),
            timestamp: env.ledger().timestamp(),
            amount,
            funded_amount: escrow.funded_amount,
            status: escrow.status,
            investor_effective_yield_bps,
            tier_lock_secs,
        }
        .publish(&env);

        escrow
    }

    /// Extend storage TTLs for the contract instance and, optionally, a batch of per-investor
    /// persistent keys, in a single call.
    ///
    /// # Bounds
    ///
    /// `allowlisted` may be empty (the instance-storage TTL extension always runs regardless),
    /// but is capped at [`MAX_TTL_BUMP_BATCH`] entries; larger investor sets must be split
    /// across multiple calls.
    pub(crate) fn bump_ttl_impl(env: Env, allowlisted: Vec<Address>) {
        // Permissionless TTL extension.
        //
        // Invariant: Soroban's `extend_ttl` never shortens TTL; this entrypoint only extends.
        // No other state is mutated.
        //
        // Rationale: long-dated escrows (maturity far in the future) write time-sensitive
        // data (`DataKey::Escrow`, snapshot, and per-investor claim gates). Under rent/archival
        // semantics, instance storage can expire and cause defaulted reads (e.g. allowlist
        // gate falls back to `false`), breaking settlement/claim readiness.
        //
        // Documentation references:
        // - ADR-007: storage key evolution policy (additive changes / key semantics).
        // - docs/escrow-ledger-time.md: all gating uses `Env::ledger().timestamp()` with `>=`.
        //
        // `allowlisted` is bounded (see `MAX_TTL_BUMP_BATCH`) because each entry costs 5
        // `extend_ttl` host calls; an unbounded vector risks exceeding the transaction's
        // CPU/resource budget with no partial progress on failure. An empty vector is valid
        // and simply skips the per-investor loop below.
        ensure(
            &env,
            allowlisted.len() <= MAX_TTL_BUMP_BATCH,
            EscrowError::TtlBumpBatchTooLarge,
        );

        env.storage().instance().extend_ttl(
            INSTANCE_TTL_MIN_EXTENSION_LEDGERS,
            INSTANCE_TTL_MIN_EXTENSION_LEDGERS,
        );

        // Instance storage TTL is contract-wide under Soroban SDK 25. The call above covers
        // Escrow, Version, LegalHold, snapshots, caps, and other instance keys.

        // Persistent per-investor keys and allowlist entries (independent TTL per address).
        for addr in allowlisted.iter() {
            let k = DataKey::InvestorAllowlisted(addr.clone());
            env.storage().persistent().extend_ttl(
                &k,
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
            );
            // Extend persistent TTL for per-investor persistent keys used by this contract.
            env.storage().persistent().extend_ttl(
                &DataKey::InvestorContribution(addr.clone()),
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
            );
            env.storage().persistent().extend_ttl(
                &DataKey::InvestorEffectiveYield(addr.clone()),
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
            );
            env.storage().persistent().extend_ttl(
                &DataKey::InvestorClaimNotBefore(addr.clone()),
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
            );
            env.storage().persistent().extend_ttl(
                &DataKey::InvestorClaimed(addr.clone()),
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
                PERSISTENT_TTL_MIN_EXTENSION_LEDGERS,
            );
        }
    }
}
