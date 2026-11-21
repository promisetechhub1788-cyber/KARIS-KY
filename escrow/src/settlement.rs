use super::*;

impl LiquifactEscrow {
    /// Public API: Check the current delegate for an investor (if any).
    pub(crate) fn get_yield_claim_delegate_impl(env: Env, investor: Address) -> Option<Address> {
        Self::get_persistent_yield_claim_delegate(&env, investor)
    }

    /// Public API: Check whether an investor's delegation is revoked.
    pub(crate) fn is_yield_claim_delegate_revoked_impl(env: Env, investor: Address) -> bool {
        Self::get_persistent_yield_claim_delegate_revoked(&env, investor)
    }

    /// Compute expected and actual yield for an investor, and the resulting slippage deviation.
    ///
    /// Returns:
    /// - `expected_yield_bps`: escrow base yield (from [`InvoiceEscrow::yield_bps`])
    /// - `actual_yield_bps`: investor's effective yield (from [`DataKey::InvestorEffectiveYield`] or base)
    /// - `deviation_bps`: absolute difference in basis points
    ///
    /// Useful for off-chain tools to preview whether a claim would trigger a slippage warning.
    pub(crate) fn get_investor_yield_slippage_impl(env: Env, investor: Address) -> (i64, i64, i64) {
        // env.clone(): env is used again after this call for the InvestorEffectiveYield read.
        let escrow = Self::get_escrow_impl(env.clone());
        let actual_yield_bps =
            Self::get_persistent_investor_effective_yield(&env, investor.clone())
                .unwrap_or(escrow.yield_bps);
        let expected_yield_bps = escrow.yield_bps;
        let deviation_bps = (actual_yield_bps - expected_yield_bps).abs();

        (expected_yield_bps, actual_yield_bps, deviation_bps)
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
    pub(crate) fn enable_yield_auto_distribution_impl(env: Env) {
        let escrow = Self::load_escrow_require_admin(&env);

        env.storage()
            .instance()
            .set(&DataKey::YieldAutoDistributionEnabled, &true);

        YieldAutoDistributionEnabled {
            name: symbol_short!("yield_ad"),
            invoice_id: escrow.invoice_id.clone(),
            enabled: true,
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);
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
    pub(crate) fn disable_yield_auto_distribution_impl(env: Env) {
        let escrow = Self::load_escrow_require_admin(&env);

        env.storage()
            .instance()
            .set(&DataKey::YieldAutoDistributionEnabled, &false);

        YieldAutoDistributionDisabled {
            name: symbol_short!("yield_dis"),
            invoice_id: escrow.invoice_id.clone(),
            timestamp: env.ledger().timestamp(),
        }
        .publish(&env);
    }

    /// Check if automatic yield distribution is enabled for this escrow.
    ///
    /// When enabled, [`LiquifactEscrow::settle`] pre-computes each investor's yield
    /// and stores it for batch claims. When disabled or not set, yields are computed
    /// on-demand during each `claim_investor_payout` call.
    pub(crate) fn is_yield_auto_dist_enabled_impl(env: Env) -> bool {
        Self::yield_auto_distribution_enabled(&env)
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
    pub(crate) fn partial_settle_impl(env: Env, caller: Address) -> InvoiceEscrow {
        caller.require_auth();

        assert!(
            !Self::legal_hold_active(&env),
            "Legal hold blocks partial settlement"
        );

        let mut escrow = Self::get_escrow_impl(env.clone());

        // Since #153: also permit any address holding >= AdminRole::SettlementOnly.
        let has_settlement_role = Self::effective_admin_role(&env, &escrow, &caller)
            .map(|r| r.level() >= AdminRole::SettlementOnly.level())
            .unwrap_or(false);
        assert!(
            caller == escrow.sme_address || caller == escrow.admin || has_settlement_role,
            "Unauthorized caller for partial settlement"
        );

        assert!(
            escrow.status == 0,
            "Escrow must be in Open state for partial settlement"
        );

        // Transition to funded status early.
        escrow.status = 1;

        // Write FundingCloseSnapshot if not already present.
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

            // Compute and store Merkle root for the partial-settle path.
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

        env.storage().instance().set(&DataKey::Escrow, &escrow);

        // ── Post-condition invariant guard ──
        ensure!(
            escrow.funded_amount <= escrow.funding_target,
            EscrowError::InvariantViolation
        );

        env.storage().instance().set(&DataKey::Escrow, &escrow);

        EscrowPartialSettle {
            name: symbol_short!("part_set"),
            invoice_id: escrow.invoice_id.clone(),
            funded_amount: escrow.funded_amount,
        }
        .publish(&env);

        escrow
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
    pub(crate) fn notify_settlement_impl(env: Env) {
        let escrow = Self::get_escrow_impl(env.clone());

        ensure(
            &env,
            escrow.status == 2,
            EscrowError::InvestorClaimNotSettled,
        );

        let notifier: Address = env
            .storage()
            .instance()
            .get(&DataKey::SettlementNotifierContract)
            .unwrap_or_else(|| fail(&env, EscrowError::SettlementNotifierNotSet));

        let now = env.ledger().timestamp();
        let args = soroban_sdk::vec![
            &env,
            escrow.invoice_id.to_val(),
            escrow.funded_amount.into_val(&env),
            escrow.yield_bps.into_val(&env),
            now.into_val(&env),
        ];
        env.invoke_contract::<()>(&notifier, &symbol_short!("on_settle"), args);

        SettlementNotifierInvoked {
            name: symbol_short!("notify_ok"),
            invoice_id: escrow.invoice_id.clone(),
            notifier_contract: notifier,
            funded_amount: escrow.funded_amount,
            yield_bps: escrow.yield_bps,
            settled_at_ledger_timestamp: now,
        }
        .publish(&env);
    }

    pub(crate) fn settle_impl(env: Env, partial_amount: Option<i128>) -> InvoiceEscrow {
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksSettlement,
        );
        ensure(
            &env,
            !Self::is_dispute_paused_for_mutation(&env),
            EscrowError::DisputePausedBlocksSettlement,
        );

        // env.clone(): env is used again after this call for ledger timestamp, storage set, and publish.
        let mut escrow = Self::load_escrow_require_sme(&env);

        // Sanctions screening: check SME address against configured sanctions provider
        Self::check_sanctions(&env, &escrow.sme_address);

        ensure(&env, escrow.status == 1, EscrowError::SettlementNotFunded);

        let now = env.ledger().timestamp();
        // Maturity check applies to all settlements (full or partial)
        // Settlement cannot occur before the configured maturity timestamp
        if escrow.maturity > 0 {
            if now < escrow.maturity {
                let seconds_remaining = escrow.maturity.saturating_sub(now);
                let context_msg = String::from_str(
                    &env,
                    &format!(
                        "Maturity timestamp: {} (in ~{} seconds)",
                        escrow.maturity, seconds_remaining
                    ),
                );
                let diagnostic = ErrorDiagnostic {
                    error_code: EscrowError::MaturityNotReached as u32,
                    message: String::from_str(&env, "Escrow has not reached maturity yet"),
                    recovery_action: String::from_str(
                        &env,
                        "Wait until maturity timestamp to settle",
                    ),
                    context: Some(context_msg),
                };
                Self::emit_error_diagnostic(&env, diagnostic);
                fail(&env, EscrowError::MaturityNotReached);
            }
        }

        // Handle partial settlement
        let current_settled_amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SettledAmount)
            .unwrap_or(0);

        let new_settled_amount = match partial_amount {
            Some(amount) => {
                // Validate partial amount
                ensure(&env, amount > 0, EscrowError::SettlementNotPositive);
                ensure(
                    &env,
                    amount <= escrow.funded_amount,
                    EscrowError::SettlementExceedsFunded,
                );
                ensure(
                    &env,
                    amount + current_settled_amount <= escrow.funded_amount,
                    EscrowError::SettlementExceedsFunded,
                );

                current_settled_amount + amount
            }
            None => escrow.funded_amount, // Full settlement
        };

        // Update settled amount in storage
        env.storage()
            .instance()
            .set(&DataKey::SettledAmount, &new_settled_amount);

        // Determine if this is a full or partial settlement
        let is_full_settlement = new_settled_amount == escrow.funded_amount;

        if is_full_settlement {
            escrow.status = 2;
        }
        // For partial settlement, status remains 1 (funded)

        // ── Protocol fee collection ──
        // Compute gross coupon, deduct protocol fee, transfer fee to treasury.
        // Fee is computed as: gross_coupon * fee_pct / 10_000 (floor division).
        // For partial settlement, calculate fees only on the settled portion.
        let fee_percentage: i64 = env
            .storage()
            .instance()
            .get(&DataKey::FeePercentage)
            .unwrap_or(0);
        if fee_percentage > 0 {
            let settlement_basis = if is_full_settlement {
                escrow.funded_amount
            } else {
                // For partial settlement, use only the amount being settled now
                new_settled_amount - current_settled_amount
            };

            if settlement_basis > 0 {
                // Gross coupon = settlement_basis * yield_bps / 10_000
                let gross_coupon = settlement_basis
                    .checked_mul(escrow.yield_bps as i128)
                    .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
                    .checked_div(10_000)
                    .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow));

                if gross_coupon > 0 {
                    // fee_amount = gross_coupon * fee_percentage / 10_000 (floor)
                    let fee_amount = gross_coupon
                        .checked_mul(fee_percentage as i128)
                        .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
                        .checked_div(10_000)
                        .unwrap_or_else(|| {
                            fail(&env, EscrowError::ComputePayoutArithmeticOverflow)
                        });

                    if fee_amount > 0 {
                        let net_coupon = gross_coupon.saturating_sub(fee_amount);
                        let treasury = Self::treasury_or_fail(&env);
                        let token_addr = Self::funding_token_or_fail(&env);
                        let this = env.current_contract_address();

                        // Transfer fee to treasury. If the contract lacks sufficient balance,
                        // the transfer will fail with a typed error — operators must ensure the
                        // contract holds enough tokens before calling settle.
                        external_calls::transfer_funding_token_with_balance_checks(
                            &env,
                            &token_addr,
                            &this,
                            &treasury,
                            fee_amount,
                        );

                        ProtocolFeeCollected {
                            name: symbol_short!("fee_coll"),
                            invoice_id: escrow.invoice_id.clone(),
                            gross_coupon,
                            fee_percentage,
                            fee_amount,
                            net_coupon,
                        }
                        .publish(&env);
                    }
                }
            }
        }

        // ── Post-condition invariant guard ──
        ensure!(
            escrow.funded_amount <= escrow.funding_target,
            EscrowError::InvariantViolation
        );

        env.storage().instance().set(&DataKey::Escrow, &escrow);

        let settled_at = now;

        // ── Automatic yield distribution snapshot (if enabled) ──
        // Pre-compute each investor's yield share at settlement time to enable batch auto-distribution.
        // This allows investors to claim their payouts without per-investor yield recalculation.
        if Self::yield_auto_distribution_enabled(&env) && is_full_settlement {
            // Get the funding close snapshot to determine total principal (pro-rata denominator)
            if let Some(snap) = env
                .storage()
                .instance()
                .get::<DataKey, FundingCloseSnapshot>(&DataKey::FundingCloseSnapshot)
            {
                let total_principal = snap.total_principal;
                if total_principal > 0 {
                    // Compute net coupon for yield distribution
                    let gross_coupon = new_settled_amount
                        .checked_mul(escrow.yield_bps as i128)
                        .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
                        .checked_div(10_000)
                        .unwrap_or_else(|| {
                            fail(&env, EscrowError::ComputePayoutArithmeticOverflow)
                        });

                    let net_coupon = if fee_percentage > 0 && gross_coupon > 0 {
                        let fee_amount = gross_coupon
                            .checked_mul(fee_percentage as i128)
                            .unwrap_or_else(|| {
                                fail(&env, EscrowError::ComputePayoutArithmeticOverflow)
                            })
                            .checked_div(10_000)
                            .unwrap_or_else(|| {
                                fail(&env, EscrowError::ComputePayoutArithmeticOverflow)
                            });
                        gross_coupon.saturating_sub(fee_amount)
                    } else {
                        gross_coupon
                    };

                    let settle_pool =
                        new_settled_amount
                            .checked_add(net_coupon)
                            .unwrap_or_else(|| {
                                fail(&env, EscrowError::ComputePayoutArithmeticOverflow)
                            });

                    // Iterate over all investors and pre-compute their yield shares
                    // For now, we'll store markers; in production, indexers enumerate InvestorContribution keys
                    // and we iterate them here. For MVP, we emit an event and investors claim normally.
                    // TODO: Implement efficient investor enumeration for batch snapshot computation

                    YieldDistSnapshotCreated {
                        name: symbol_short!("yld_snap"),
                        invoice_id: escrow.invoice_id.clone(),
                        settled_amount: new_settled_amount,
                        investor_count: env
                            .storage()
                            .instance()
                            .get(&DataKey::UniqueFunderCount)
                            .unwrap_or(0),
                        captured_at_ledger_timestamp: settled_at,
                    }
                    .publish(&env);
                }
            }
        }

        if is_full_settlement {
            EscrowSettled {
                name: symbol_short!("escrow_sd"),
                invoice_id: escrow.invoice_id.clone(),
                funded_amount: escrow.funded_amount,
                yield_bps: escrow.yield_bps,
                maturity: escrow.maturity,
                settled_at_ledger_timestamp: settled_at,
            }
            .publish(&env);
        } else {
            EscrowPartiallySettled {
                name: symbol_short!("esc_pause"),
                invoice_id: escrow.invoice_id.clone(),
                funded_amount: escrow.funded_amount,
                settled_amount: new_settled_amount,
                yield_bps: escrow.yield_bps,
                maturity: escrow.maturity,
                settled_at_ledger_timestamp: settled_at,
            }
            .publish(&env);
        }

        // Emit health warning if applicable (for audit trail).
        Self::compute_and_emit_health_warning(&env, &escrow, now);

        escrow
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
    pub(crate) fn clone_settled_escrow_impl(
        env: Env,
        template_contract: Address,
        new_invoice_id: String,
        new_amount: i128,
    ) -> InvoiceEscrow {
        // A contract can only ever access its own storage/`Env`; reading the template's state
        // requires cross-contract calls through its public read-only entrypoints.
        let template_client = LiquifactEscrowClient::new(&env, &template_contract);

        // Load and validate the template escrow.
        let template_escrow = template_client.get_escrow();
        ensure(
            &env,
            template_escrow.status == 2,
            EscrowError::CloneNotSettled,
        );
        ensure(&env, new_amount > 0, EscrowError::CloneAmountNotPositive);

        // Require auth from the template admin.
        template_escrow.admin.require_auth();

        // Read immutable configuration from the template contract.
        let funding_token: Address = template_client.get_funding_token();
        let treasury: Address = template_client.get_treasury();
        let registry: Option<Address> = template_client.get_registry_ref();
        let yield_tiers: Option<Vec<YieldTier>> = template_client.get_yield_tier_table();
        let min_contribution_floor: i128 = template_client.get_min_contribution_floor();
        let max_unique_investors_cap: Option<u32> = template_client.get_max_unique_investors_cap();
        let max_per_investor_cap: Option<i128> = template_client.get_max_per_investor_cap();
        let legal_hold_clear_delay: Option<u64> = {
            let d = template_client.get_legal_hold_clear_delay();
            if d > 0 {
                Some(d)
            } else {
                None
            }
        };
        let funding_deadline: Option<u64> = template_client.get_funding_deadline();

        // Call init on the current (target) environment with cloned parameters.
        Self::init_internal(
            env.clone(),
            template_escrow.admin.clone(),
            template_escrow.guardian.clone(),
            new_invoice_id.clone(),
            template_escrow.sme_address.clone(),
            new_amount,
            template_escrow.yield_bps,
            template_escrow.maturity,
            funding_token,
            registry,
            treasury,
            yield_tiers,
            if min_contribution_floor > 0 {
                Some(min_contribution_floor)
            } else {
                None
            },
            max_unique_investors_cap,
            max_per_investor_cap,
            legal_hold_clear_delay,
            funding_deadline,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        // Emit clone event.
        let new_escrow = Self::get_escrow_impl(env.clone());
        EscrowCloned {
            name: symbol_short!("escrow_cl"),
            template_invoice_id: template_escrow.invoice_id.clone(),
            new_invoice_id: new_escrow.invoice_id.clone(),
            admin: template_escrow.admin.clone(),
            sme_address: template_escrow.sme_address.clone(),
            yield_bps: template_escrow.yield_bps,
            maturity: template_escrow.maturity,
            new_amount,
        }
        .publish(&env);

        new_escrow
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

    pub(crate) fn withdraw_impl(env: Env, caller: Address) -> InvoiceEscrow {
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksWithdrawal,
        );
        ensure(
            &env,
            !Self::is_dispute_paused_for_mutation(&env),
            EscrowError::DisputePausedBlocksWithdrawal,
        );

        caller.require_auth();
        let mut escrow = Self::get_escrow_impl(env.clone());

        // Verify that the caller is the registered SME address
        ensure(
            &env,
            caller == escrow.sme_address,
            EscrowError::UnauthorizedWithdrawer,
        );

        ensure(&env, escrow.status == 1, EscrowError::WithdrawalNotFunded);

        let amount = escrow.funded_amount;
        let sme = escrow.sme_address.clone();

        let token_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::FundingToken)
            .unwrap_or_else(|| fail(&env, EscrowError::FundingTokenNotSet));

        // Verify the contract holds enough before mutating state.
        let this = env.current_contract_address();
        let contract_balance = TokenClient::new(&env, &token_addr).balance(&this);
        ensure(
            &env,
            contract_balance >= amount,
            EscrowError::InsufficientContractBalance,
        );

        // State transition and accounting (checks-effects-interactions).
        escrow.status = 3;

        // ── Post-condition invariant guard ──
        ensure!(
            escrow.funded_amount <= escrow.funding_target,
            EscrowError::InvariantViolation
        );

        env.storage().instance().set(&DataKey::Escrow, &escrow);

        let prev_distributed: i128 = env
            .storage()
            .instance()
            .get(&DataKey::DistributedPrincipal)
            .unwrap_or(0);
        env.storage().instance().set(
            &DataKey::DistributedPrincipal,
            &prev_distributed.saturating_add(amount),
        );

        // Token transfer with SEP-41 balance-delta verification.
        external_calls::transfer_funding_token_with_balance_checks(
            &env,
            &token_addr,
            &this,
            &sme,
            amount,
        );

        SmeWithdrew {
            name: symbol_short!("sme_wd"),
            invoice_id: escrow.invoice_id.clone(),
            amount,
            recipient: sme,
        }
        .publish(&env);

        escrow
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
    pub(crate) fn claim_investor_payout_impl(env: Env, investor: Address) {
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksInvestorClaims,
        );
        ensure(
            &env,
            !Self::is_dispute_paused_for_mutation(&env),
            EscrowError::DisputePausedBlocksInvestorClaims,
        );

        investor.require_auth();

        // Single fetch: consolidates the previous two reads of InvestorContribution.
        // Retains the participation guard without a redundant second storage access.
        let contribution: i128 = Self::get_persistent_investor_contribution(&env, investor.clone());
        ensure(&env, contribution > 0, EscrowError::NoContributionToClaim);

        // env.clone(): env is used again after this call for storage reads, ledger timestamp, and publish.
        let escrow = Self::get_escrow_impl(env.clone());

        // For partial settlement: allow claims if there's a settled amount (status 1 with SettledAmount)
        // For full settlement: require status 2
        let settled_amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SettledAmount)
            .unwrap_or(0);

        let is_claimable = escrow.status == 2 || (escrow.status == 1 && settled_amount > 0);
        ensure(&env, is_claimable, EscrowError::InvestorClaimNotSettled);

        let now = env.ledger().timestamp();

        // --- Investor lock-in period check (separate from tier commitment lock) ---
        let lock_in_until: u64 =
            Self::get_persistent_investor_lock_in_until(&env, investor.clone());
        if lock_in_until > 0 && now < lock_in_until {
            let blocks_remaining = lock_in_until.saturating_sub(now);
            let diagnostic = ErrorDiagnostic::with_context(
                &env,
                EscrowError::InvestorStillInLockIn as u32,
                "Investor is still in lock-in period after funding",
                "Wait for the lock-in period to expire before claiming payout",
                &format!("{} seconds remaining", blocks_remaining),
            );
            Self::emit_error_diagnostic(&env, diagnostic);
            fail(&env, EscrowError::InvestorStillInLockIn);
        }

        let not_before: u64 =
            Self::get_persistent_investor_claim_not_before(&env, investor.clone());

        if now < not_before {
            let context_msg = if not_before > 0 {
                let blocks_remaining = not_before.saturating_sub(now);
                String::from_str(
                    &env,
                    &format!(
                        "Can claim in {} seconds (block {}) from now",
                        blocks_remaining, not_before
                    ),
                )
            } else {
                String::from_str(&env, "Commitment lock is active")
            };

            let diagnostic = ErrorDiagnostic {
                error_code: EscrowError::InvestorCommitmentLockNotExpired as u32,
                message: String::from_str(&env, "Investment is in commitment lock period"),
                recovery_action: String::from_str(
                    &env,
                    "Wait for the lock period to expire before claiming payout",
                ),
                context: Some(context_msg),
            };
            Self::emit_error_diagnostic(&env, diagnostic);
            fail(&env, EscrowError::InvestorCommitmentLockNotExpired);
        }

        // Idempotent early-return: a second claim is a no-op (no re-emit).
        if Self::get_persistent_investor_claimed(&env, investor.clone()) {
            return;
        }

        // Mark before emit — prevents re-emission on any re-entrant path.
        Self::set_persistent_investor_claimed(&env, investor.clone(), true);

        // Check for pre-computed yield distribution (if auto-distribution is enabled)
        let mut total_payout = 0i128;
        if let Some(yield_dist) =
            Self::get_persistent_yield_distribution_share(&env, investor.clone())
        {
            // Use pre-computed payout amount from settlement snapshot
            total_payout = yield_dist.payout_amount;

            // Emit auto-distributed event to signal pre-computed yield usage
            AutoDistributedYieldClaimed {
                name: symbol_short!("auto_yld"),
                investor: investor.clone(),
                invoice_id: escrow.invoice_id.clone(),
                payout_amount: total_payout,
            }
            .publish(&env);
        } else {
            // Fallback: compute yield on-demand (backwards compatibility)
            total_payout = Self::compute_investor_payout_impl(env.clone(), investor.clone());
        }

        // Check if investor elected to reinvest their yield
        let has_reinvestment_election: bool = env
            .storage()
            .persistent()
            .get(&DataKey::InvestorReinvestElection(investor.clone()))
            .unwrap_or(false);

        if has_reinvestment_election {
            // Calculate the yield portion of the payout
            let original_contribution: i128 =
                Self::get_persistent_investor_contribution(&env, investor.clone());
            let yield_amount = total_payout.saturating_sub(original_contribution);

            if yield_amount > 0 {
                // Add yield to reinvested amount (becomes new principal)
                let current_reinvested: i128 = env
                    .storage()
                    .persistent()
                    .get(&DataKey::InvestorReinvestedAmount(investor.clone()))
                    .unwrap_or(0);

                let new_reinvested = current_reinvested
                    .checked_add(yield_amount)
                    .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow));

                env.storage().persistent().set(
                    &DataKey::InvestorReinvestedAmount(investor.clone()),
                    &new_reinvested,
                );

                // Update audit log with reinvestment event
                let mut audit_log: Vec<ReinvestmentAuditEntry> = env
                    .storage()
                    .instance()
                    .get(&DataKey::ReinvestmentAuditLog)
                    .unwrap_or(Vec::new(&env));

                // Find and update the most recent entry for this investor, or add new one
                let mut found = false;
                for i in (0..audit_log.len()).rev() {
                    let entry = audit_log.get(i).unwrap();
                    if entry.investor == investor {
                        let mut updated_entry = entry.clone();
                        updated_entry.reinvested_amount = yield_amount;
                        audit_log.set(i, updated_entry);
                        found = true;
                        break;
                    }
                }

                if !found && (audit_log.len() as u32) < MAX_REINVESTMENT_AUDIT_ENTRIES {
                    let entry = ReinvestmentAuditEntry {
                        investor: investor.clone(),
                        invoice_id: escrow.invoice_id.clone(),
                        target_escrow: None,
                        reinvested_amount: yield_amount,
                        elected_at_ledger_timestamp: now,
                        elected_at_ledger_sequence: env.ledger().sequence(),
                    };
                    audit_log.push_back(entry);
                }

                env.storage()
                    .instance()
                    .set(&DataKey::ReinvestmentAuditLog, &audit_log);

                // Emit reinvestment event
                YieldReinvested {
                    name: symbol_short!("yld_ri_ev"),
                    investor: investor.clone(),
                    invoice_id: escrow.invoice_id.clone(),
                    reinvested_amount: yield_amount,
                    new_total_principal: new_reinvested + original_contribution,
                }
                .publish(&env);
            }
        }

        // Real-time slippage detection: compare expected vs actual yield
        Self::detect_yield_slippage(&env, investor.clone(), &escrow);

        InvestorPayoutClaimed {
            name: symbol_short!("inv_claim"),
            investor,
            invoice_id: escrow.invoice_id.clone(),
        }
        .publish(&env);

        // Emit health warning if applicable (for audit trail).
        Self::compute_and_emit_health_warning(&env, &escrow, now);
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
    pub(crate) fn batch_claim_investor_payouts_impl(env: Env, investors: Vec<Address>) -> u32 {
        ensure(&env, !investors.is_empty(), EscrowError::BatchClaimEmpty);
        ensure(
            &env,
            investors.len() <= MAX_BATCH_CLAIM,
            EscrowError::BatchClaimTooLarge,
        );

        // Read-once shared state: single storage fetch for legal hold, escrow, and now.
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksInvestorClaims,
        );
        ensure(
            &env,
            !Self::is_dispute_paused_for_mutation(&env),
            EscrowError::DisputePausedBlocksInvestorClaims,
        );

        let escrow = Self::get_escrow_impl(env.clone());
        ensure(
            &env,
            escrow.status == 2,
            EscrowError::InvestorClaimNotSettled,
        );

        // Auth: admin-only gating for batch operations.
        // The escrow.admin was already read above; auth before any mutable work.
        escrow.admin.require_auth();

        let now = env.ledger().timestamp();
        let n = investors.len();
        let mut claimed: u32 = 0;

        for i in 0..n {
            let investor = investors.get(i).unwrap();

            let contribution: i128 =
                Self::get_persistent_investor_contribution(&env, investor.clone());
            if contribution == 0 {
                continue;
            }

            let not_before: u64 =
                Self::get_persistent_investor_claim_not_before(&env, investor.clone());
            if now < not_before {
                continue;
            }

            // Idempotent: skip already-claimed investors.
            if Self::get_persistent_investor_claimed(&env, investor.clone()) {
                continue;
            }

            // Mark claimed.
            Self::set_persistent_investor_claimed(&env, investor.clone(), true);

            // Record the settlement checkpoint (see #148).
            let settlement_payout =
                Self::compute_investor_payout_impl(env.clone(), investor.clone());
            Self::append_investor_history_record(
                &env,
                investor.clone(),
                symbol_short!("settle"),
                settlement_payout,
                contribution,
            );

            // Yield slippage detection.
            Self::detect_yield_slippage(&env, investor.clone(), &escrow);

            InvestorPayoutClaimed {
                name: symbol_short!("inv_claim"),
                investor,
                invoice_id: escrow.invoice_id.clone(),
            }
            .publish(&env);

            claimed += 1;
        }

        BatchInvestorPayoutsClaimed {
            name: symbol_short!("bch_clm"),
            invoice_id: escrow.invoice_id.clone(),
            claimed_count: claimed,
        }
        .publish(&env);

        claimed
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
    pub(crate) fn claim_payout_as_delegate_impl(env: Env, investor: Address, delegate: Address) {
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldBlocksInvestorClaims,
        );
        ensure(
            &env,
            !Self::is_dispute_paused_for_mutation(&env),
            EscrowError::DisputePausedBlocksInvestorClaims,
        );

        // Verify investor has a contribution.
        let contribution: i128 = Self::get_persistent_investor_contribution(&env, investor.clone());
        ensure(&env, contribution > 0, EscrowError::NoContributionToClaim);

        // Verify delegation exists and is not revoked.
        let delegated_addr = Self::get_persistent_yield_claim_delegate(&env, investor.clone());
        ensure(&env, delegated_addr.is_some(), EscrowError::NoDelegationSet);

        // Check if delegation is revoked.
        ensure(
            &env,
            !Self::get_persistent_yield_claim_delegate_revoked(&env, investor.clone()),
            EscrowError::DelegationRevoked,
        );

        // Verify the provided delegate matches the stored delegation.
        let stored_delegate = delegated_addr.unwrap();
        ensure(
            &env,
            stored_delegate == delegate,
            EscrowError::NoDelegationSet, // Reuse as "unauthorized delegate"
        );

        // Authorize the delegate.
        delegate.require_auth();

        // env.clone(): used for escrow read and ledger timestamp.
        let escrow = Self::get_escrow_impl(env.clone());

        // For partial settlement: allow claims if there's a settled amount (status 1 with SettledAmount)
        // For full settlement: require status 2
        let settled_amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SettledAmount)
            .unwrap_or(0);

        let is_claimable = escrow.status == 2 || (escrow.status == 1 && settled_amount > 0);
        ensure(&env, is_claimable, EscrowError::InvestorClaimNotSettled);

        let not_before: u64 =
            Self::get_persistent_investor_claim_not_before(&env, investor.clone());
        let now = env.ledger().timestamp();
        ensure(
            &env,
            now >= not_before,
            EscrowError::InvestorCommitmentLockNotExpired,
        );

        // Idempotent early-return: a second claim is a no-op (no re-emit).
        if Self::get_persistent_investor_claimed(&env, investor.clone()) {
            return;
        }

        // Mark before emit — prevents re-emission on any re-entrant path.
        Self::set_persistent_investor_claimed(&env, investor.clone(), true);

        // Record the settlement checkpoint (see #148).
        let settlement_payout = Self::compute_investor_payout_impl(env.clone(), investor.clone());
        Self::append_investor_history_record(
            &env,
            investor.clone(),
            symbol_short!("settle"),
            settlement_payout,
            contribution,
        );

        InvestorPayoutClaimed {
            name: symbol_short!("inv_claim"),
            investor,
            invoice_id: escrow.invoice_id.clone(),
        }
        .publish(&env);
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
    pub(crate) fn set_yield_claim_delegate_impl(env: Env, investor: Address, delegate: Address) {
        ensure(
            &env,
            investor != delegate,
            EscrowError::DelegateAddressSameAsInvestor,
        );

        investor.require_auth();

        // Set the delegation and clear the revocation marker.
        Self::set_persistent_yield_claim_delegate(&env, investor.clone(), delegate.clone());
        Self::set_persistent_yield_claim_delegate_revoked(&env, investor.clone(), false);

        YieldClaimDelegationSet {
            name: symbol_short!("del_set"),
            investor,
            delegate,
        }
        .publish(&env);
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
    pub(crate) fn revoke_yield_claim_delegate_impl(env: Env, investor: Address) {
        investor.require_auth();

        // Verify there is an active delegation to revoke.
        ensure(
            &env,
            Self::delegation_is_valid(&env, investor.clone()),
            EscrowError::NoActiveDelegation,
        );

        // Mark as revoked (keep the delegate address for audit trail).
        Self::set_persistent_yield_claim_delegate_revoked(&env, investor.clone(), true);

        YieldClaimDelegationRevoked {
            name: symbol_short!("del_rev"),
            investor,
        }
        .publish(&env);
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
    pub(crate) fn reinvest_yield_impl(
        env: Env,
        investor: Address,
        target_escrow: Address,
        amount: i128,
    ) {
        investor.require_auth();
        ensure(&env, amount > 0, EscrowError::ReinvestAmountNotPositive);

        let source_escrow = Self::get_escrow_impl(env.clone());
        ensure(
            &env,
            source_escrow.status == 2,
            EscrowError::ReinvestSourceEscrowNotSettled,
        );

        // Query target escrow state via a cross-contract getter; a target in open or funded state is
        // considered valid for reinvestment.
        let target: InvoiceEscrow = env.invoke_contract(
            &target_escrow,
            &Symbol::new(&env, "get_escrow"),
            soroban_sdk::vec![&env],
        );
        ensure(
            &env,
            target.status == 0 || target.status == 1,
            EscrowError::ReinvestTargetEscrowNotFunding,
        );

        let contribution = Self::get_persistent_investor_contribution(&env, investor.clone());
        let accrued_yield = Self::compute_investor_payout_impl(env.clone(), investor.clone())
            .saturating_sub(contribution);
        ensure(
            &env,
            accrued_yield >= amount,
            EscrowError::ReinvestYieldInsufficient,
        );

        let current_reinvested: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::InvestorReinvestedAmount(investor.clone()))
            .unwrap_or(0);
        let new_reinvested = current_reinvested
            .checked_add(amount)
            .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow));

        env.storage().persistent().set(
            &DataKey::InvestorReinvestedAmount(investor.clone()),
            &new_reinvested,
        );
        env.storage()
            .persistent()
            .set(&DataKey::InvestorReinvestElection(investor.clone()), &true);
        env.storage().persistent().set(
            &DataKey::InvestorReinvestTarget(investor.clone()),
            &target_escrow,
        );

        let mut audit_log: Vec<ReinvestmentAuditEntry> = env
            .storage()
            .instance()
            .get(&DataKey::ReinvestmentAuditLog)
            .unwrap_or(Vec::new(&env));

        if (audit_log.len() as u32) < MAX_REINVESTMENT_AUDIT_ENTRIES {
            audit_log.push_back(ReinvestmentAuditEntry {
                investor: investor.clone(),
                invoice_id: source_escrow.invoice_id.clone(),
                target_escrow: Some(target_escrow.clone()),
                reinvested_amount: amount,
                elected_at_ledger_timestamp: env.ledger().timestamp(),
                elected_at_ledger_sequence: env.ledger().sequence(),
            });
        }

        env.storage()
            .instance()
            .set(&DataKey::ReinvestmentAuditLog, &audit_log);

        let target_total = target
            .funded_amount
            .checked_add(amount)
            .unwrap_or_else(|| fail(&env, EscrowError::FundedAmountOverflow));

        env.invoke_contract::<InvoiceEscrow>(
            &target_escrow,
            &Symbol::new(&env, "fund"),
            soroban_sdk::vec![&env, investor.clone().into_val(&env), amount.into_val(&env)],
        );

        let updated_target = Self::get_escrow_impl(target_escrow.clone());
        ensure(
            &env,
            updated_target.funded_amount >= target_total.saturating_sub(amount),
            EscrowError::ReinvestTargetEscrowNotFunding,
        );

        YieldReinvested {
            name: symbol_short!("yield_reinv_evt"),
            investor: investor.clone(),
            invoice_id: source_escrow.invoice_id.clone(),
            reinvested_amount: amount,
            new_total_principal: new_reinvested + contribution,
        }
        .publish(&env);
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
    pub(crate) fn compute_investor_payout_impl(env: Env, investor: Address) -> i128 {
        // Contribution fetch: returns 0 for non-participants without panicking.
        let contribution: i128 = Self::get_persistent_investor_contribution(&env, investor.clone());
        if contribution == 0 {
            return 0;
        }

        // Get reinvested amount (if any)
        let reinvested_amount: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::InvestorReinvestedAmount(investor.clone()))
            .unwrap_or(0);

        // Total principal includes both original contribution and any reinvested yield
        let total_investor_principal = contribution
            .checked_add(reinvested_amount)
            .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow));

        // Snapshot must exist (written when escrow first reaches status == 1).
        let Some(snap) = env
            .storage()
            .instance()
            .get::<DataKey, FundingCloseSnapshot>(&DataKey::FundingCloseSnapshot)
        else {
            return 0;
        };

        let total_principal = snap.total_principal;
        if total_principal <= 0 {
            return 0;
        }

        // Get settled amount (if any partial settlement has occurred)
        let settled_amount: i128 = env
            .storage()
            .instance()
            .get(&DataKey::SettledAmount)
            .unwrap_or(0);

        // If no settlement has occurred yet, return 0
        if settled_amount == 0 {
            return 0;
        }

        // Resolve effective yield: investor-specific tier (set at first deposit) or escrow base.
        // env.clone(): env is used again after this call for InvestorEffectiveYield read.
        let escrow = Self::get_escrow_impl(env.clone());
        let effective_yield_bps: i64 =
            Self::get_persistent_investor_effective_yield(&env, investor.clone())
                .unwrap_or(escrow.yield_bps);

        // coupon = total_principal × effective_yield_bps / 10_000  (floor)
        // Guard: if yield is zero the coupon is 0 and the settle pool equals total_principal;
        // skip the multiply/divide to avoid any potential platform-specific edge case and to
        // make the zero-yield path explicit and auditable.
        let coupon = if effective_yield_bps == 0 {
            0i128
        } else {
            total_principal
                .checked_mul(effective_yield_bps as i128)
                .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
                .checked_div(10_000)
                .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
        };

        // Deduct protocol fee from the coupon: fee = gross_coupon * fee_pct / 10_000
        let fee_percentage: i64 = env
            .storage()
            .instance()
            .get(&DataKey::FeePercentage)
            .unwrap_or(0);
        let net_coupon = if fee_percentage > 0 && coupon > 0 {
            let fee_amount = coupon
                .checked_mul(fee_percentage as i128)
                .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
                .checked_div(10_000)
                .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow));
            coupon.saturating_sub(fee_amount)
        } else {
            coupon
        };

        let settle_pool = settled_amount
            .checked_add(net_coupon)
            .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow));

        // Guard: if settle_pool is zero (impossible in practice given total_principal > 0, but
        // defensively checked so checked_div never receives a zero divisor from this path).
        if settle_pool <= 0 {
            return 0;
        }

        // gross_payout = contribution × settle_pool / total_principal  (floor)
        contribution
            .checked_mul(settle_pool)
            .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
            .checked_div(total_principal)
            .unwrap_or_else(|| fail(&env, EscrowError::ComputePayoutArithmeticOverflow))
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
    pub(crate) fn verify_investor_proof_impl(
        env: Env,
        investor: Address,
        contribution: i128,
        proof: Vec<BytesN<32>>,
    ) -> bool {
        let Some(stored_root) = env
            .storage()
            .instance()
            .get::<DataKey, BytesN<32>>(&DataKey::FundingCloseMerkleRoot)
        else {
            return false;
        };

        let computed_root =
            Self::compute_merkle_root_from_proof(&env, &investor, contribution, &proof);
        computed_root == stored_root
    }

    /// Compute a Keccak-256 Merkle root from a leaf and its proof path.
    ///
    /// # Algorithm
    ///
    /// 1. Hash the leaf: `h = keccak256(address_bytes || contribution_be_bytes)`
    /// 2. For each sibling in proof: `h = keccak256(min(h, sibling) || max(h, sibling))`
    /// 3. Return `h`
    ///
    /// Sorting sibling order ensures deterministic proofs regardless of tree position.
    pub(crate) fn compute_merkle_root_from_proof(
        env: &Env,
        investor: &Address,
        contribution: i128,
        proof: &Vec<BytesN<32>>,
    ) -> BytesN<32> {
        use soroban_sdk::Bytes;

        // Build leaf: keccak256(address_bytes || contribution_be_bytes)
        let mut leaf_data = Bytes::new(env);

        // Serialize investor address via its Soroban strkey `String` representation,
        // converted to bytes for deterministic hashing.
        let addr_str = investor.to_string();
        leaf_data.append(&addr_str.to_bytes());

        // Serialize contribution as big-endian bytes (i128 → 16 bytes).
        let mut contrib_bytes = [0u8; 16];
        let mut val = contribution;
        for i in (0..16).rev() {
            contrib_bytes[i] = (val & 0xFF) as u8;
            val >>= 8;
        }
        for b in &contrib_bytes {
            leaf_data.push_back(*b);
        }

        let mut current: BytesN<32> = env.crypto().keccak256(&leaf_data).to_bytes();

        let n = proof.len();
        for i in 0..n {
            let sibling = proof.get(i).unwrap();
            // Sort to ensure deterministic proofs.
            let (left, right) = if current < sibling {
                (current.clone(), sibling.clone())
            } else {
                (sibling.clone(), current.clone())
            };
            let mut combined = Bytes::new(env);
            combined.append(left.as_bytes());
            combined.append(right.as_bytes());
            current = env.crypto().keccak256(&combined).to_bytes();
        }

        current
    }

    /// Compute an empty Merkle root (Keccak-256 of empty input).
    ///
    /// Used at funding close when the tree is built incrementally off-chain;
    /// the on-chain anchor starts as the empty root and is updated by the admin
    /// once the full tree is constructed.
    pub(crate) fn compute_empty_merkle_root(env: &Env) -> BytesN<32> {
        use soroban_sdk::Bytes;
        let empty = Bytes::new(env);
        env.crypto().keccak256(&empty).to_bytes()
    }

    /// Real-time slippage detection during investor claim.
    ///
    /// Compares expected yield (escrow base) vs. actual yield (investor's effective yield)
    /// and emits a warning event if the deviation exceeds the configured threshold.
    ///
    /// # Detection logic
    ///
    /// - **Expected yield**: escrow's [`InvoiceEscrow::yield_bps`]
    /// - **Actual yield**: investor's [`DataKey::InvestorEffectiveYield`] (or falls back to base)
    /// - **Deviation**: absolute difference between actual and expected (in basis points)
    /// - **Threshold**: stored in [`DataKey::YieldSlippageThreshold`] (0 = disabled)
    ///
    /// When `deviation > threshold`, emits [`YieldSlippageWarning`] for admin review.
    ///
    /// # Authorization
    ///
    /// None — pure read-only check; called internally during claim finalization.
    pub(crate) fn detect_yield_slippage(env: &Env, investor: Address, escrow: &InvoiceEscrow) {
        // Fetch configured slippage threshold; 0 means disabled
        let threshold_bps: i64 = env
            .storage()
            .instance()
            .get(&DataKey::YieldSlippageThreshold)
            .unwrap_or(0);

        if threshold_bps == 0 {
            // Slippage detection is disabled
            return;
        }

        // Resolve actual yield: investor-specific tier (set at first deposit) or escrow base.
        let actual_yield_bps: i64 =
            Self::get_persistent_investor_effective_yield(&env, investor.clone())
                .unwrap_or(escrow.yield_bps);

        let expected_yield_bps = escrow.yield_bps;

        // Compute deviation (absolute difference in basis points)
        let deviation_bps = (actual_yield_bps - expected_yield_bps).abs();

        // Emit warning if deviation exceeds threshold
        if deviation_bps > threshold_bps {
            YieldSlippageWarning {
                name: symbol_short!("yld_slip"),
                investor,
                invoice_id: escrow.invoice_id.clone(),
                expected_yield_bps,
                actual_yield_bps,
                slippage_threshold_bps: threshold_bps,
                deviation_bps,
            }
            .publish(&env);
        }
    }
}
