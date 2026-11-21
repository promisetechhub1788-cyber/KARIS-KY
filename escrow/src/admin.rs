use super::*;

impl LiquifactEscrow {
    pub(crate) fn legal_hold_active(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::LegalHold)
            .unwrap_or(false)
    }

    /// Whether the escrow is currently paused by the admin.
    pub(crate) fn escrow_paused_active(env: &Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::EscrowPaused)
            .unwrap_or(false)
    }

    /// Compute which bucket (0..255) an investor address falls into.
    ///
    /// Uses the Soroban host's `Address` to‑bytes serialisation, then a simple
    /// DJB2‑style hash folded modulo [`INVESTOR_BUCKET_COUNT`]. The hash is
    /// deterministic within a single ledger but not guaranteed stable across
    /// network upgrades — bucket indices are soft hints for dashboards, not a
    /// consensus‑critical invariant.
    pub(crate) fn investor_bucket_index(env: &Env, addr: &Address) -> u32 {
        // Use the SDK-supplied `require_auth_for_args` / `Address` to derive
        // a stable 32-byte contract-internal representation via the host's
        // `compute_hash_sha256` function. This avoids heap-allocating a String
        // in no_std and gives us a deterministic seed.
        //
        // We take the first 8 bytes of the SHA-256 as a u64 seed and fold
        // modulo INVESTOR_BUCKET_COUNT.
        let addr_bytes = addr.to_string().to_bytes();
        let hash_bytes = env.crypto().sha256(&addr_bytes).to_array();
        let seed = u64::from_be_bytes([
            hash_bytes[0],
            hash_bytes[1],
            hash_bytes[2],
            hash_bytes[3],
            hash_bytes[4],
            hash_bytes[5],
            hash_bytes[6],
            hash_bytes[7],
        ]);
        (seed % (INVESTOR_BUCKET_COUNT as u64)) as u32
    }

    /// Emit a diagnostic event for an error with recovery guidance.
    ///
    /// # Logic
    /// - **Code 4001 (LowFundingRatio)**: `funded_ratio_bps < 5000` (< 50%) and close to/past maturity.
    /// - **Code 4002 (CloseToMaturity)**: `time_to_maturity_secs < 86400` (< 1 day) with healthy funding.
    /// - **Code 4003 (OverMaturity)**: Escrow is past maturity (`time_to_maturity_secs < 0`) and unfunded.
    /// - **Code 0**: No warning condition detected.
    ///
    /// # Non-blocking
    /// This function never causes a transaction to fail. If metrics cannot be computed
    /// (overflow, etc.), it gracefully returns code 0.
    pub(crate) fn compute_and_emit_health_warning(
        env: &Env,
        escrow: &InvoiceEscrow,
        now: u64,
    ) -> u32 {
        let funded_ratio_bps: i64 = if escrow.funding_target > 0 {
            // (funded_amount / funding_target) * 10_000
            // Use saturating arithmetic to avoid panic on overflow.
            let numerator = (escrow.funded_amount as i128).saturating_mul(10_000);
            let ratio = numerator / (escrow.funding_target as i128);
            // Clamp to i64 range.
            if ratio > i64::MAX as i128 {
                i64::MAX
            } else if ratio < 0 {
                0
            } else {
                ratio as i64
            }
        } else {
            10_000_i64 // No target set; assume fully funded.
        };

        let time_to_maturity_secs: i64 = if escrow.maturity > 0 {
            let maturity_i64 = escrow.maturity as i64;
            let now_i64 = now as i64;
            // Clamp to prevent overflow when computing the delta.
            maturity_i64.saturating_sub(now_i64)
        } else {
            i64::MAX // No maturity constraint.
        };

        // Determine warning type based on conditions.
        let warning_type = if time_to_maturity_secs < 0
            && escrow.status == 0
            && escrow.funded_amount < escrow.funding_target
        {
            4003 // OverMaturity: past maturity, still open, and unfunded.
        } else if time_to_maturity_secs >= 0 && time_to_maturity_secs < 86400 {
            // Close to maturity (< 1 day away).
            if funded_ratio_bps < 5000 {
                4001 // LowFundingRatio near maturity.
            } else {
                4002 // CloseToMaturity with healthy funding.
            }
        } else if funded_ratio_bps < 5000 && escrow.status == 0 {
            4001 // LowFundingRatio (open, no immediate time pressure).
        } else {
            0 // No warning.
        };

        // Only emit if a warning was detected.
        if warning_type != 0 {
            EscrowHealthWarning {
                name: symbol_short!("hlth_wrn"),
                invoice_id: escrow.invoice_id.clone(),
                warning_type,
                funded_amount: escrow.funded_amount,
                funding_target: escrow.funding_target,
                funded_ratio_bps,
                time_to_maturity_secs,
                recorded_at_ledger_timestamp: now,
            }
            .publish(env);
        }

        warning_type
    }

    /// Read the immutable funding token address, failing with [`EscrowError::FundingTokenNotSet`]
    /// when the escrow has not been initialized.
    pub(crate) fn funding_token_or_fail(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::FundingToken)
            .unwrap_or_else(|| fail(env, EscrowError::FundingTokenNotSet))
    }

    /// Read the immutable treasury address, failing with [`EscrowError::TreasuryNotSet`]
    /// when the escrow has not been initialized.
    pub(crate) fn treasury_or_fail(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Treasury)
            .unwrap_or_else(|| fail(env, EscrowError::TreasuryNotSet))
    }

    pub(crate) fn validate_yield_tiers_table(
        env: &Env,
        tiers: &Option<Vec<YieldTier>>,
        base_yield: i64,
    ) {
        let Some(tiers) = tiers else {
            return;
        };
        if tiers.is_empty() {
            return;
        }
        let n = tiers.len();
        for i in 0..n {
            let t = tiers.get(i).unwrap();
            ensure(
                env,
                (0..=10_000).contains(&t.yield_bps),
                EscrowError::TierYieldOutOfRange,
            );
            ensure(
                env,
                t.yield_bps >= base_yield,
                EscrowError::TierYieldBelowBase,
            );
            if i > 0 {
                let p = tiers.get(i - 1).unwrap();
                ensure(
                    env,
                    t.min_lock_secs > p.min_lock_secs,
                    EscrowError::TierLockNotIncreasing,
                );
                ensure(
                    env,
                    t.yield_bps >= p.yield_bps,
                    EscrowError::TierYieldNotNonDecreasing,
                );
            }
        }
    }

    /// Returns `(effective_yield_bps, matched_lock_secs)` for a given commitment.
    /// `matched_lock_secs` is the [`YieldTier::min_lock_secs`] of the best matching tier,
    /// or `0` when no tier was matched (base yield applies).
    pub(crate) fn effective_yield_for_commitment(
        env: &Env,
        base_yield: i64,
        committed_lock_secs: u64,
    ) -> (i64, u64) {
        if committed_lock_secs == 0 {
            return (base_yield, 0);
        }
        let Some(tiers) = env
            .storage()
            .instance()
            .get::<DataKey, Vec<YieldTier>>(&DataKey::YieldTierTable)
        else {
            return (base_yield, 0);
        };
        if tiers.is_empty() {
            return (base_yield, 0);
        }
        let mut best = base_yield;
        let mut best_lock = 0u64;
        let n = tiers.len();
        for i in 0..n {
            let t = tiers.get(i).unwrap();
            if committed_lock_secs >= t.min_lock_secs && t.yield_bps > best {
                best = t.yield_bps;
                best_lock = t.min_lock_secs;
            }
        }
        (best, best_lock)
    }

    /// Returns the optional pending admin address waiting for [`LiquifactEscrow::accept_admin`],
    /// or [`None`] when no admin handover is in progress.
    pub(crate) fn get_pending_admin_impl(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::PendingAdmin)
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
    pub(crate) fn sweep_terminal_dust_impl(env: Env, amount: i128) -> i128 {
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksTreasuryDustSweep,
        );
        ensure(
            &env,
            !Self::escrow_paused_active(&env),
            EscrowError::EscrowIsPaused,
        );
        ensure(&env, amount > 0, EscrowError::SweepAmountNotPositive);
        ensure(
            &env,
            amount <= MAX_DUST_SWEEP_AMOUNT,
            EscrowError::SweepAmountExceedsMax,
        );

        // env.clone(): env is used again after this call for treasury/token reads and publish.
        let escrow = Self::get_escrow_impl(env.clone());
        ensure(
            &env,
            escrow.status == 2 || escrow.status == 3 || escrow.status == 4 || escrow.status == 5,
            EscrowError::DustSweepNotTerminal,
        );

        let treasury = Self::treasury_or_fail(&env);
        treasury.require_auth();

        let token_addr = Self::funding_token_or_fail(&env);
        let this = env.current_contract_address();

        let token = TokenClient::new(&env, &token_addr);
        let balance = token.balance(&this);
        ensure(&env, balance > 0, EscrowError::NoFundingTokenBalanceToSweep);
        let sweep_amt = amount.min(balance);
        ensure(&env, sweep_amt > 0, EscrowError::EffectiveSweepAmountZero);

        // Liability floor (cancelled escrows only): sweep must not reduce the balance below
        // principal still owed to investors who have not yet called refund().
        //
        // In settled (2) and withdrawn (3) states, disbursement is off-chain and
        // distributed_principal stays 0, so the floor is not applicable there.
        // In cancelled (4) state, refund() is the on-chain redemption path and increments
        // distributed_principal atomically, making the invariant computable here.
        //
        // outstanding = funded_amount - distributed_principal
        // Invariant: balance - sweep_amt >= outstanding
        if escrow.status == 4 {
            let distributed: i128 = env
                .storage()
                .instance()
                .get(&DataKey::DistributedPrincipal)
                .unwrap_or(0);
            let outstanding = escrow.funded_amount.saturating_sub(distributed);
            // sweep_amt <= balance (from amount.min(balance) above), so this subtraction is safe.
            let balance_after_sweep = balance - sweep_amt;
            ensure(
                &env,
                balance_after_sweep >= outstanding,
                EscrowError::SweepExceedsLiabilityFloor,
            );
        }

        external_calls::transfer_funding_token_with_balance_checks(
            &env,
            &token_addr,
            &this,
            &treasury,
            sweep_amt,
        );

        TreasuryDustSwept {
            name: symbol_short!("dust_sw"),
            invoice_id: escrow.invoice_id.clone(),
            token: token_addr,
            amount: sweep_amt,
        }
        .publish(&env);

        sweep_amt
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
    pub(crate) fn archive_escrow_impl(env: Env) -> InvoiceEscrow {
        let mut escrow = Self::load_escrow_require_admin(&env);

        // Idempotent no-op: already archived
        if escrow.status == 5 {
            return escrow;
        }

        ensure(
            &env,
            escrow.status == 2 || escrow.status == 3 || escrow.status == 4,
            EscrowError::ArchiveNotTerminal,
        );

        let prior_status = escrow.status;
        escrow.status = 5;

        env.storage().instance().set(&DataKey::Escrow, &escrow);

        EscrowArchived {
            name: symbol_short!("esc_arch"),
            invoice_id: escrow.invoice_id.clone(),
            prior_status,
            archived_at_ledger_timestamp: env.ledger().timestamp(),
        }
        .publish(&env);

        escrow
    }

    pub(crate) fn get_escrow_impl(env: Env) -> InvoiceEscrow {
        env.storage()
            .instance()
            .get(&DataKey::Escrow)
            .unwrap_or_else(|| fail(&env, EscrowError::EscrowNotInitialized))
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
    pub(crate) fn rotate_beneficiary_impl(env: Env, new_sme_address: Address) -> InvoiceEscrow {
        // Legal-hold gate (read-only).
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksBeneficiaryRotation,
        );
        ensure(
            &env,
            !Self::escrow_paused_active(&env),
            EscrowError::EscrowIsPaused,
        );

        let mut escrow = Self::get_escrow_impl(env.clone());

        // Only permitted in pre-settlement states (open or funded).
        ensure(
            &env,
            escrow.status == 0 || escrow.status == 1,
            EscrowError::RotationNotOpen,
        );

        // Reject a no-op rotation to the current beneficiary.
        ensure(
            &env,
            new_sme_address != escrow.sme_address,
            EscrowError::NewSmeSameAsCurrent,
        );

        // Dual authorization: the outgoing SME and the admin must both sign.
        escrow.sme_address.require_auth();
        escrow.admin.require_auth();

        let prior_sme = escrow.sme_address.clone();
        escrow.sme_address = new_sme_address.clone();
        env.storage().instance().set(&DataKey::Escrow, &escrow);

        BeneficiaryRotated {
            name: symbol_short!("ben_rot"),
            invoice_id: escrow.invoice_id.clone(),
            prior_sme,
            new_sme: new_sme_address,
        }
        .publish(&env);

        escrow
    }

    /// Load the current escrow and require admin authorization in one step.
    ///
    /// Consolidates the repeated `let escrow = Self::get_escrow(env.clone()); escrow.admin.require_auth();`
    /// pattern used across multiple admin-gated entrypoints. Since #153, this also enforces
    /// [`AdminRole::Full`] via [`LiquifactEscrow::require_admin_role`]: a no-op for deployments
    /// that never configured [`DataKey::AdminRoles`] (the `admin` field is implicitly `Full`),
    /// but correctly rejects the call if an operator explicitly demoted `admin`'s own role.
    pub(crate) fn load_escrow_require_admin(env: &Env) -> InvoiceEscrow {
        let escrow: InvoiceEscrow = env
            .storage()
            .instance()
            .get(&DataKey::Escrow)
            .unwrap_or_else(|| fail(env, EscrowError::EscrowNotInitialized));
        let admin = escrow.admin.clone();
        Self::require_admin_role(env, &escrow, &admin, AdminRole::Full);
        escrow
    }

    /// Load the current escrow and require SME authorization in one step.
    ///
    /// Consolidates the repeated `let escrow = Self::get_escrow(env.clone()); escrow.sme_address.require_auth();`
    /// pattern used across multiple SME-gated entrypoints.
    pub(crate) fn load_escrow_require_sme(env: &Env) -> InvoiceEscrow {
        let escrow: InvoiceEscrow = env
            .storage()
            .instance()
            .get(&DataKey::Escrow)
            .unwrap_or_else(|| fail(env, EscrowError::EscrowNotInitialized));
        escrow.sme_address.require_auth();
        escrow
    }

    pub(crate) fn get_version_impl(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Version).unwrap_or(0)
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
    pub(crate) fn get_interface_version_impl(_env: Env) -> u32 {
        CONTRACT_INTERFACE_VERSION
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
    pub(crate) fn get_build_metadata_impl(env: Env) -> BuildMetadata {
        BuildMetadata {
            schema_version: BUILD_SCHEMA_VERSION,
            interface_version: BUILD_INTERFACE_VERSION,
            git_commit: String::from_str(&env, BUILD_GIT_COMMIT),
            git_full_commit: String::from_str(&env, BUILD_GIT_FULL_COMMIT),
            build_timestamp: String::from_str(&env, BUILD_TIMESTAMP),
            pkg_version: String::from_str(&env, BUILD_PKG_VERSION),
            rust_version: String::from_str(&env, BUILD_RUST_VERSION),
        }
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
    pub(crate) fn check_escrow_health_impl(env: Env) -> (u32, i64, i64) {
        let escrow = Self::get_escrow_impl(env.clone());
        let now = env.ledger().timestamp();

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

        let time_to_maturity_secs: i64 = if escrow.maturity > 0 {
            let maturity_i64 = escrow.maturity as i64;
            let now_i64 = now as i64;
            maturity_i64.saturating_sub(now_i64)
        } else {
            i64::MAX
        };

        let warning_type = if time_to_maturity_secs < 0
            && escrow.status == 0
            && escrow.funded_amount < escrow.funding_target
        {
            4003
        } else if time_to_maturity_secs >= 0 && time_to_maturity_secs < 86400 {
            if funded_ratio_bps < 5000 {
                4001
            } else {
                4002
            }
        } else if funded_ratio_bps < 5000 && escrow.status == 0 {
            4001
        } else {
            0
        };

        (warning_type, funded_ratio_bps, time_to_maturity_secs)
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
    pub(crate) fn get_escrow_health_metrics_impl(env: Env) -> EscrowHealthMetrics {
        let escrow = Self::get_escrow_impl(env.clone());
        let unique_investor_count = Self::get_unique_funder_count_impl(env.clone());

        // Funding progress %
        let funding_progress_percent = if escrow.funding_target > 0 {
            let pct = (escrow.funded_amount * 100) / escrow.funding_target;
            pct.min(100) as u32
        } else {
            0
        };

        // Days to maturity
        let days_to_maturity = if escrow.maturity == 0 {
            0i64
        } else {
            let now = env.ledger().timestamp();
            let delta_secs = escrow.maturity as i64 - now as i64;
            delta_secs / 86_400i64
        };

        // Average contribution
        let average_contribution_size = if unique_investor_count > 0 {
            escrow.funded_amount / (unique_investor_count as i128)
        } else {
            0i128
        };

        // Estimated yield payout
        let estimated_yield_payout = (escrow.funded_amount * escrow.yield_bps as i128) / 10_000i128;

        EscrowHealthMetrics {
            funding_progress_percent,
            days_to_maturity,
            unique_investor_count,
            average_contribution_size,
            estimated_yield_payout,
        }
    }

    /// Bundles multiple read-only values to return a comprehensive summary of the escrow state
    /// in a single host invocation.
    pub(crate) fn get_escrow_summary_impl(env: Env) -> EscrowSummary {
        let escrow = Self::get_escrow_impl(env.clone());
        let legal_hold = Self::get_legal_hold_impl(env.clone());
        let legal_hold_reason = Self::get_legal_hold_reason_impl(env.clone());
        let funding_close_snapshot_opt = Self::get_funding_close_snapshot_impl(env.clone());
        let unique_funder_count = Self::get_unique_funder_count_impl(env.clone());
        let is_allowlist_active = Self::is_allowlist_active_impl(env.clone());
        let schema_version = Self::get_version_impl(env.clone());
        let sme_collateral_commitment = Self::get_sme_collateral_commitment_impl(env.clone());
        let primary_attestation_hash = Self::get_primary_attestation_hash_impl(env.clone());
        let attestation_append_log = Self::get_attestation_append_log_impl(env.clone());
        let yield_token = Self::get_yield_token(env.clone());
        let oracle_contract = Self::get_oracle_contract(env.clone());
        let nft_contract = Self::get_nft_contract(env.clone());

        let funding_close_snapshot = match funding_close_snapshot_opt {
            Some(snap) => EscrowCloseSnapshot::Some(snap),
            None => EscrowCloseSnapshot::None,
        };

        let sme_collateral_commitment = match sme_collateral_commitment {
            Some(collateral) => CollateralCommitmentSnapshot::Some(collateral),
            None => CollateralCommitmentSnapshot::None,
        };

        EscrowSummary {
            escrow,
            has_maturity_lock: Self::has_maturity_lock_impl(env.clone()),
            legal_hold,
            funding_close_snapshot,
            unique_funder_count,
            is_allowlist_active,
            schema_version,
            sme_collateral_commitment,
            has_primary_attestation: primary_attestation_hash.is_some(),
            attestation_log_length: attestation_append_log.len(),
            yield_token,
            oracle_contract,
            nft_contract,
            settlement_nft: env.storage().instance().get(&DataKey::SettlementNft),
        }
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
    pub(crate) fn get_registry_listing_impl(env: Env) -> RegistryListing {
        let escrow = Self::get_escrow_impl(env.clone());
        let created_at = Self::get_created_at(env.clone());
        RegistryListing {
            escrow_address: env.current_contract_address(),
            invoice_id: escrow.invoice_id,
            sme_address: escrow.sme_address,
            created_at,
            status: escrow.status,
            funding_target: escrow.funding_target,
        }
    }

    /// Retrieve the configured yield slippage detection threshold (in basis points).
    /// Returns `0` if slippage detection is disabled (no threshold configured).
    ///
    /// When the absolute difference between actual and expected yield exceeds this threshold,
    /// a [`YieldSlippageWarning`] is emitted during [`LiquifactEscrow::claim_investor_payout`].
    pub(crate) fn get_yield_slippage_threshold_impl(env: Env) -> i64 {
        env.storage()
            .instance()
            .get(&DataKey::YieldSlippageThreshold)
            .unwrap_or(0)
    }

    /// Returns the configured protocol fee percentage in basis points, or `0` if
    /// no fee was configured at init. Applied to the gross yield coupon at settlement;
    /// the fee is transferred to treasury.
    pub(crate) fn get_fee_percentage_impl(env: Env) -> i64 {
        env.storage()
            .instance()
            .get(&DataKey::FeePercentage)
            .unwrap_or(0)
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
    pub(crate) fn detect_state_inconsistencies_impl(env: Env) -> StateInconsistencyReport {
        let escrow = Self::get_escrow_impl(env.clone());

        // Get optional dependent state.
        let snapshot_opt: Option<FundingCloseSnapshot> =
            env.storage().instance().get(&DataKey::FundingCloseSnapshot);
        let unique_funder_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::UniqueFunderCount)
            .unwrap_or(0);
        let ledger_timestamp = env.ledger().timestamp();

        // Check each invariant.
        let report = StateInconsistencyReport {
            funded_exceeds_target_stale: escrow.funded_amount > escrow.funding_target
                && escrow.status < 1,

            funded_positive_status_open: escrow.funded_amount > 0 && escrow.status == 0,

            funded_zero_status_advanced: escrow.funded_amount == 0 && escrow.status >= 1,

            funders_exist_status_open: unique_funder_count > 0 && escrow.status == 0,

            no_funders_advanced_status: unique_funder_count == 0 && escrow.status >= 1,

            snapshot_exists_not_funded: snapshot_opt.is_some() && escrow.status < 1,

            snapshot_missing_post_funded: snapshot_opt.is_none() && escrow.status >= 2,

            settled_before_maturity_lock: escrow.status == 2
                && escrow.maturity > 0
                && ledger_timestamp < escrow.maturity,

            invalid_funding_amounts: escrow.funding_target <= 0 || escrow.amount <= 0,

            invalid_status_value: escrow.status > 4,
        };

        // Emit event if any inconsistency is detected.
        let has_inconsistency = report.funded_exceeds_target_stale
            || report.funded_positive_status_open
            || report.funded_zero_status_advanced
            || report.funders_exist_status_open
            || report.no_funders_advanced_status
            || report.snapshot_exists_not_funded
            || report.snapshot_missing_post_funded
            || report.settled_before_maturity_lock
            || report.invalid_funding_amounts
            || report.invalid_status_value;

        if has_inconsistency {
            StateInconsistenciesDetected {
                name: symbol_short!("state_inc"),
                invoice_id: escrow.invoice_id.clone(),
                report: report.clone(),
            }
            .publish(&env);
        }

        report
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
    pub(crate) fn revalidate_token_cache_impl(env: Env) -> TokenMetadataCache {
        let escrow = Self::load_escrow_require_admin(&env);
        let token_addr = Self::funding_token_or_fail(&env);

        // Fetch fresh token metadata
        let token_client = TokenClient::new(&env, &token_addr);
        let decimals = token_client.decimals();

        // Update cache with current ledger state
        let cache = TokenMetadataCache {
            decimals,
            cached_at_ledger_timestamp: env.ledger().timestamp(),
            cached_at_ledger_sequence: env.ledger().sequence(),
        };

        env.storage()
            .instance()
            .set(&DataKey::TokenMetadataCache, &cache);

        // Publish event for auditing
        TokenCacheRevalidated {
            name: symbol_short!("tok_val"),
            invoice_id: escrow.invoice_id.clone(),
            decimals,
            revalidated_ledger_timestamp: cache.cached_at_ledger_timestamp,
        }
        .publish(&env);

        cache
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
    pub(crate) fn migrate_impl(env: Env, from_version: u32) -> u32 {
        let escrow = Self::load_escrow_require_admin(&env);

        let stored: u32 = env.storage().instance().get(&DataKey::Version).unwrap_or(0);

        // Emit diagnostic event with version information for operator insight,
        // even if the call will ultimately fail.
        MigrationDiagnosticEmitted {
            name: symbol_short!("mig_diag"),
            invoice_id: escrow.invoice_id.clone(),
            stored_version: stored,
            from_version,
            target_version: SCHEMA_VERSION,
        }
        .publish(&env);

        ensure(
            &env,
            stored == from_version,
            EscrowError::MigrationVersionMismatch,
        );

        if from_version >= SCHEMA_VERSION {
            fail(&env, EscrowError::AlreadyCurrentSchemaVersion)
        } else {
            // No migration path is implemented for any version below SCHEMA_VERSION.
            // To add one: implement the transformation here, call
            //   env.storage().instance().set(&DataKey::Version, &NEW_VERSION);
            // and return NEW_VERSION before reaching this typed error.
            //
            // When a migration path IS implemented, also record the version change:
            //   let mut vh: Vec<(u32, u64)> = env.storage().instance()
            //       .get(&DataKey::VersionHistory).unwrap_or_else(|| Vec::new(&env));
            //   vh.push_back((NEW_VERSION, env.ledger().timestamp()));
            //   env.storage().instance().set(&DataKey::VersionHistory, &vh);
            fail(&env, EscrowError::NoMigrationPath)
        }
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
    pub(crate) fn export_escrow_snapshot_impl(env: Env) -> EscrowSnapshot {
        let escrow = Self::get_escrow_impl(env.clone());
        let schema_version: u32 = env.storage().instance().get(&DataKey::Version).unwrap_or(0);
        let legal_hold: bool = env
            .storage()
            .instance()
            .get(&DataKey::LegalHold)
            .unwrap_or(false);

        let fcs_opt: Option<FundingCloseSnapshot> =
            env.storage().instance().get(&DataKey::FundingCloseSnapshot);
        let funding_close_snapshot = match fcs_opt {
            Some(s) => EscrowCloseSnapshot::Some(s),
            None => EscrowCloseSnapshot::None,
        };

        let unique_funder_count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::UniqueFunderCount)
            .unwrap_or(0);

        let is_allowlist_active: bool = env
            .storage()
            .instance()
            .get(&DataKey::AllowlistActive)
            .unwrap_or(false);

        let sme_cc_opt: Option<SmeCollateralCommitment> =
            env.storage().instance().get(&DataKey::SmeCollateralPledge);
        let sme_collateral_commitment = match sme_cc_opt {
            Some(c) => CollateralCommitmentSnapshot::Some(c),
            None => CollateralCommitmentSnapshot::None,
        };

        let primary_attestation_hash: Option<BytesN<32>> = env
            .storage()
            .instance()
            .get(&DataKey::PrimaryAttestationHash);

        let attestation_log_length: u32 = env
            .storage()
            .instance()
            .get::<DataKey, Vec<BytesN<32>>>(&DataKey::AttestationAppendLog)
            .map(|v| v.len())
            .unwrap_or(0);

        let sanctions_provider: Option<Address> =
            env.storage().instance().get(&DataKey::SanctionsProvider);

        let funding_token = Self::funding_token_or_fail(&env);
        let treasury = Self::treasury_or_fail(&env);
        let registry: Option<Address> = env.storage().instance().get(&DataKey::RegistryRef);

        let version_history: Vec<(u32, u64)> = env
            .storage()
            .instance()
            .get(&DataKey::VersionHistory)
            .unwrap_or_else(|| Vec::new(&env));

        EscrowSnapshot {
            escrow,
            schema_version,
            legal_hold,
            funding_close_snapshot,
            unique_funder_count,
            is_allowlist_active,
            sme_collateral_commitment,
            primary_attestation_hash,
            attestation_log_length,
            sanctions_provider,
            funding_token,
            treasury,
            registry,
            version_history,
        }
    }

    /// Returns the version history audit trail as a vector of `(version, ledger_timestamp)` tuples.
    ///
    /// Each tuple records when a schema version was first written to this instance — on
    /// [`LiquifactEscrow::init`] and on each successful [`LiquifactEscrow::migrate`].
    /// This helps operators track deployment lineage and audit upgrade events.
    ///
    /// Returns an empty vector if no history has been recorded (escrow not initialized).
    pub(crate) fn get_version_history_impl(env: Env) -> Vec<(u32, u64)> {
        env.storage()
            .instance()
            .get(&DataKey::VersionHistory)
            .unwrap_or_else(|| Vec::new(&env))
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
    pub(crate) fn upgrade_impl(env: Env, new_wasm_hash: BytesN<32>) {
        // Auth first — matches migrate() ordering
        let escrow = Self::load_escrow_require_admin(&env);

        // Emit event before the deployer call so the event is recorded even if
        // the deployer call somehow reverts (defensive ordering)
        ContractUpgraded {
            name: symbol_short!("upgrade"),
            invoice_id: escrow.invoice_id,
            new_wasm_hash: new_wasm_hash.clone(),
        }
        .publish(&env);

        // Replace contract WASM — no state is modified
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    pub(crate) fn update_maturity_impl(env: Env, new_maturity: u64) -> InvoiceEscrow {
        ensure(
            &env,
            !Self::escrow_paused_active(&env),
            EscrowError::EscrowIsPaused,
        );

        let mut escrow = Self::load_escrow_require_admin(&env);

        ensure(&env, escrow.status == 0, EscrowError::MaturityUpdateNotOpen);

        let old_maturity = escrow.maturity;
        escrow.maturity = new_maturity;

        env.storage().instance().set(&DataKey::Escrow, &escrow);

        MaturityUpdatedEvent {
            name: symbol_short!("maturity"),
            invoice_id: escrow.invoice_id.clone(),
            old_maturity,
            new_maturity,
        }
        .publish(&env);

        escrow
    }

    /// Propose a new admin (`PendingAdmin`) — step 1 of a two-step handover.
    ///
    /// Requires current admin authorization. The destination must differ from the current admin.
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes when the escrow is uninitialized or `new_admin` is the
    /// current admin.
    pub(crate) fn propose_admin_impl(env: Env, new_admin: Address) -> Address {
        let escrow = Self::load_escrow_require_admin(&env);

        ensure(
            &env,
            escrow.admin != new_admin,
            EscrowError::NewAdminSameAsCurrent,
        );

        env.storage()
            .instance()
            .set(&DataKey::PendingAdmin, &new_admin);

        AdminProposedEvent {
            name: symbol_short!("adm_prop"),
            invoice_id: escrow.invoice_id.clone(),
            current_admin: escrow.admin.clone(),
            pending_admin: new_admin.clone(),
        }
        .publish(&env);

        // Also emit the versioned AdminChanged with timestamp + actor.
        AdminChanged {
            name: symbol_short!("adm_chg"),
            invoice_id: escrow.invoice_id.clone(),
            actor: escrow.admin.clone(),
            timestamp: env.ledger().timestamp(),
            kind: symbol_short!("proposed"),
            old_admin: escrow.admin,
            new_admin: new_admin.clone(),
        }
        .publish(&env);

        new_admin
    }

    /// Accept a pending admin handover.
    ///
    /// The address stored in [`DataKey::PendingAdmin`] must authorize this call. On success it is
    /// promoted into [`InvoiceEscrow::admin`] and the pending key is cleared, so admin authority
    /// changes only after both the current admin and successor have explicitly authorized.
    pub(crate) fn accept_admin_impl(env: Env) -> InvoiceEscrow {
        let pending: Option<Address> = env.storage().instance().get(&DataKey::PendingAdmin);
        ensure(&env, pending.is_some(), EscrowError::NoPendingAdmin);
        let pending = pending.unwrap();
        pending.require_auth();

        let mut escrow = Self::get_escrow_impl(env.clone());
        escrow.admin = pending.clone();

        env.storage().instance().set(&DataKey::Escrow, &escrow);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        env.storage()
            .instance()
            .remove(&DataKey::LegalHoldProposalExpiresAt);

        AdminTransferredEvent {
            name: symbol_short!("admin"),
            invoice_id: escrow.invoice_id.clone(),
            new_admin: pending.clone(),
        }
        .publish(&env);

        // Also emit the versioned AdminChanged with timestamp + actor.
        AdminChanged {
            name: symbol_short!("adm_chg"),
            invoice_id: escrow.invoice_id.clone(),
            actor: pending.clone(),
            timestamp: env.ledger().timestamp(),
            kind: symbol_short!("accepted"),
            old_admin: escrow.admin.clone(),
            new_admin: pending.clone(),
        }
        .publish(&env);

        escrow
    }

    /// Deprecated shim for the former one-step admin transfer API.
    ///
    /// This function now only proposes `new_admin` by delegating to
    /// [`LiquifactEscrow::propose_admin`]. The proposed address must still call
    /// [`LiquifactEscrow::accept_admin`] before admin authority changes.
    #[deprecated(note = "use propose_admin followed by accept_admin")]
    pub(crate) fn transfer_admin_impl(env: Env, new_admin: Address) -> InvoiceEscrow {
        Self::propose_admin_impl(env.clone(), new_admin);
        Self::get_escrow_impl(env)
    }

    /// Transition an **open** escrow (status 0) to **cancelled** (status 4).
    ///
    /// Only the [`InvoiceEscrow::admin`] may call this. Blocked while a legal hold is active.
    /// After cancellation, investors may recover their principal via [`LiquifactEscrow::refund`].
    ///
    /// # Errors
    /// Emits typed [`EscrowError`] codes when legal hold is active, the escrow is uninitialized,
    /// or the escrow is not in status 0 (open).
    pub(crate) fn cancel_funding_impl(env: Env) -> InvoiceEscrow {
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksCancelFunding,
        );
        ensure(
            &env,
            !Self::escrow_paused_active(&env),
            EscrowError::EscrowIsPaused,
        );

        let mut escrow = Self::load_escrow_require_admin(&env);

        ensure(&env, escrow.status == 0, EscrowError::CancelFundingNotOpen);

        escrow.status = 4;
        env.storage().instance().set(&DataKey::Escrow, &escrow);

        FundingCancelled {
            name: symbol_short!("fund_can"),
            invoice_id: escrow.invoice_id.clone(),
            funded_amount: escrow.funded_amount,
        }
        .publish(&env);

        escrow
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
    pub(crate) fn refund_impl(env: Env, investor: Address) {
        ensure(
            &env,
            !Self::escrow_paused_active(&env),
            EscrowError::EscrowIsPaused,
        );

        investor.require_auth();

        let escrow = Self::get_escrow_impl(env.clone());
        ensure(&env, escrow.status == 4, EscrowError::RefundNotCancelled);

        let amount: i128 = Self::get_persistent_investor_contribution(&env, investor.clone());
        ensure(&env, amount > 0, EscrowError::NoContributionToRefund);

        // Zero out contribution before transfer (checks-effects-interactions).
        Self::set_persistent_investor_contribution(&env, investor.clone(), 0i128);
        env.storage()
            .instance()
            .set(&DataKey::InvestorRefunded(investor.clone()), &true);

        // Decrement the bucketed aggregate so dashboards stay accurate after refunds.
        {
            let bucket = Self::investor_bucket_index(&env, &investor);
            let prev_bucket: i128 = env
                .storage()
                .instance()
                .get(&DataKey::InvestorContributionBucket(bucket))
                .unwrap_or(0);
            env.storage().instance().set(
                &DataKey::InvestorContributionBucket(bucket),
                &prev_bucket.saturating_sub(amount),
            );
        }

        // Track distributed principal so sweep_terminal_dust can enforce the liability floor.
        let prev_distributed: i128 = env
            .storage()
            .instance()
            .get(&DataKey::DistributedPrincipal)
            .unwrap_or(0);
        env.storage().instance().set(
            &DataKey::DistributedPrincipal,
            &prev_distributed.saturating_add(amount),
        );

        let token_addr = Self::funding_token_or_fail(&env);
        let this = env.current_contract_address();

        external_calls::transfer_funding_token_with_balance_checks(
            &env,
            &token_addr,
            &this,
            &investor,
            amount,
        );

        InvestorRefundedEvt {
            name: symbol_short!("refunded"),
            investor: investor.clone(),
            invoice_id: escrow.invoice_id.clone(),
            amount,
        }
        .publish(&env);
    }

    /// Whether an investor has already received a refund in a cancelled escrow.
    pub(crate) fn is_investor_refunded_impl(env: Env, investor: Address) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::InvestorRefunded(investor))
            .unwrap_or(false)
    }

    /// Total principal already returned to investors via [`LiquifactEscrow::refund`].
    ///
    /// Used by [`LiquifactEscrow::sweep_terminal_dust`] to compute outstanding liabilities.
    /// Absent ⇒ `0` (no refunds have occurred).
    pub(crate) fn get_distributed_principal_impl(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::DistributedPrincipal)
            .unwrap_or(0)
    }
}
