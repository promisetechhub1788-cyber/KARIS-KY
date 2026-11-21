use super::*;

impl LiquifactEscrow {
    /// Whether a compliance/legal hold is active (defaults to `false` if unset).
    pub(crate) fn get_legal_hold_impl(env: Env) -> bool {
        Self::legal_hold_active(&env)
    }

    /// Whether a compliance/legal hold is active (defaults to `false` if unset).
    pub(crate) fn get_legal_hold_status_impl(env: Env) -> bool {
        Self::legal_hold_active(&env)
    }

    /// Returns the reason for the current legal hold, if any.
    /// Absent when no hold is active or the hold was set without a reason.
    pub(crate) fn get_legal_hold_reason_impl(env: Env) -> Option<String> {
        env.storage().instance().get(&DataKey::LegalHoldReason)
    }

    /// Configured minimum delay between [`LiquifactEscrow::request_clear_legal_hold`]
    /// and [`LiquifactEscrow::set_legal_hold(env, false)`]. Defaults to `0`.
    pub(crate) fn get_legal_hold_clear_delay_impl(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::LegalHoldClearDelay)
            .unwrap_or(0)
    }

    /// Reserved minimum ledger timestamp at which a pending legal-hold clear may be applied.
    /// `None` means no request has been recorded.
    pub(crate) fn get_legal_hold_clearable_at_impl(env: Env) -> Option<u64> {
        env.storage().instance().get(&DataKey::LegalHoldClearableAt)
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
    pub(crate) fn set_legal_hold_impl(env: Env, active: bool, reason: String) {
        let escrow = Self::load_escrow_require_admin(&env);

        if active && escrow.guardian.is_some() {
            fail(&env, EscrowError::LegalHoldRequiresGuardianConfirmation);
        }

        // Check if escrow is in a terminal status (2, 3, 4, or 5)
        // Terminal statuses: 2 = settled, 3 = withdrawn, 4 = cancelled, 5 = archived
        ensure(
            &env,
            escrow.status < 2,
            EscrowError::LegalHoldSetOnTerminalEscrow,
        );

        // Validate reason length (max 256 bytes)
        ensure(
            &env,
            reason.len() <= 256,
            EscrowError::LegalHoldReasonTooLong,
        );

        if !active && Self::legal_hold_active(&env) {
            let delay = Self::get_legal_hold_clear_delay_impl(env.clone());
            if delay > 0 {
                let clearable_at: Option<u64> =
                    env.storage().instance().get(&DataKey::LegalHoldClearableAt);
                ensure(
                    &env,
                    clearable_at.is_some(),
                    EscrowError::LegalHoldClearRequestMissing,
                );
                let now = env.ledger().timestamp();
                ensure(
                    &env,
                    now >= clearable_at.unwrap(),
                    EscrowError::LegalHoldClearNotReady,
                );
            }
        }

        env.storage()
            .instance()
            .remove(&DataKey::LegalHoldClearableAt);
        env.storage()
            .instance()
            .remove(&DataKey::LegalHoldProposalExpiresAt);

        env.storage().instance().set(&DataKey::LegalHold, &active);

        // Store or remove the reason
        if active && reason.len() > 0 {
            env.storage()
                .instance()
                .set(&DataKey::LegalHoldReason, &reason);
        } else {
            env.storage().instance().remove(&DataKey::LegalHoldReason);
        }

        LegalHoldChanged {
            name: symbol_short!("legalhld"),
            invoice_id: escrow.invoice_id.clone(),
            active: if active { 1 } else { 0 },
            reason,
        }
        .publish(&env);

        // Also emit the versioned LegalHoldSet with timestamp + actor.
        LegalHoldSet {
            name: symbol_short!("legal_set"),
            invoice_id: escrow.invoice_id.clone(),
            actor: escrow.admin.clone(),
            timestamp: env.ledger().timestamp(),
            active: if active { 1 } else { 0 },
        }
        .publish(&env);
    }

    /// Propose a legal hold for guardian confirmation. Proposals expire after
    /// [`LEGAL_HOLD_PROPOSAL_TTL_SECS`] seconds of ledger time.
    pub(crate) fn propose_legal_hold_impl(env: Env, admin: Address) {
        let escrow = Self::get_escrow_impl(env.clone());
        ensure(
            &env,
            admin == escrow.admin,
            EscrowError::LegalHoldAuthorityMismatch,
        );
        Self::require_admin_role(&env, &escrow, &admin, AdminRole::Full);
        ensure(
            &env,
            escrow.guardian.is_some(),
            EscrowError::LegalHoldGuardianNotConfigured,
        );
        ensure(
            &env,
            escrow.status < 2,
            EscrowError::LegalHoldSetOnTerminalEscrow,
        );
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldAlreadyActive,
        );

        let expires_at = env
            .ledger()
            .timestamp()
            .checked_add(LEGAL_HOLD_PROPOSAL_TTL_SECS)
            .unwrap_or_else(|| fail(&env, EscrowError::LedgerTimestampOverflow));
        env.storage()
            .instance()
            .set(&DataKey::LegalHoldProposalExpiresAt, &expires_at);

        LegalHoldProposed {
            name: symbol_short!("lh_prop"),
            invoice_id: escrow.invoice_id,
            admin,
            expires_at,
        }
        .publish(&env);
    }

    /// Confirm a pending legal hold using the configured guardian address.
    pub(crate) fn confirm_legal_hold_impl(env: Env, guardian: Address) {
        let escrow = Self::get_escrow_impl(env.clone());
        ensure(
            &env,
            escrow.guardian.as_ref() == Some(&guardian),
            EscrowError::LegalHoldAuthorityMismatch,
        );
        guardian.require_auth();
        ensure(
            &env,
            escrow.status < 2,
            EscrowError::LegalHoldSetOnTerminalEscrow,
        );

        let expires_at: u64 = env
            .storage()
            .instance()
            .get(&DataKey::LegalHoldProposalExpiresAt)
            .unwrap_or_else(|| fail(&env, EscrowError::LegalHoldProposalMissing));
        ensure(
            &env,
            env.ledger().timestamp() < expires_at,
            EscrowError::LegalHoldProposalExpired,
        );
        ensure(
            &env,
            !Self::legal_hold_active(&env),
            EscrowError::LegalHoldProposalMissing,
        );

        env.storage()
            .instance()
            .remove(&DataKey::LegalHoldProposalExpiresAt);
        env.storage()
            .instance()
            .remove(&DataKey::LegalHoldClearableAt);
        env.storage().instance().set(&DataKey::LegalHold, &true);

        let reason = String::from_str(&env, "");
        LegalHoldChanged {
            name: symbol_short!("legalhld"),
            invoice_id: escrow.invoice_id.clone(),
            active: 1,
            reason,
        }
        .publish(&env);
        LegalHoldSet {
            name: symbol_short!("legal_set"),
            invoice_id: escrow.invoice_id,
            actor: guardian,
            timestamp: env.ledger().timestamp(),
            active: 1,
        }
        .publish(&env);
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
    pub(crate) fn set_escrow_paused_impl(env: Env, paused: bool) {
        let escrow = Self::load_escrow_require_admin(&env);

        env.storage()
            .instance()
            .set(&DataKey::EscrowPaused, &paused);

        EscrowPaused {
            name: symbol_short!("esc_pause"),
            invoice_id: escrow.invoice_id.clone(),
            actor: escrow.admin.clone(),
            timestamp: env.ledger().timestamp(),
            paused: if paused { 1 } else { 0 },
        }
        .publish(&env);
    }

    /// Read the current paused state.
    ///
    /// `true` when the escrow is paused; no authorization required.
    pub(crate) fn is_escrow_paused_impl(env: Env) -> bool {
        Self::escrow_paused_active(&env)
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
    pub(crate) fn request_clear_legal_hold_impl(env: Env) {
        let escrow = Self::load_escrow_require_admin(&env);

        let now = env.ledger().timestamp();
        let delay = Self::get_legal_hold_clear_delay_impl(env.clone());
        let clearable_at = if delay == 0 {
            now
        } else {
            now.checked_add(delay)
                .unwrap_or_else(|| fail(&env, EscrowError::LegalHoldClearDelayOverflow))
        };

        env.storage()
            .instance()
            .set(&DataKey::LegalHoldClearableAt, &clearable_at);

        LegalHoldClearRequested {
            name: symbol_short!("lh_req"),
            invoice_id: escrow.invoice_id.clone(),
            clearable_at,
        }
        .publish(&env);
    }

    /// Convenience alias for [`LiquifactEscrow::set_legal_hold`] with `active = false`.
    pub(crate) fn clear_legal_hold_impl(env: Env) {
        let reason = String::from_str(&env, "");
        Self::set_legal_hold_impl(env, false, reason);
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
    pub(crate) fn pause_dispute_impl(env: Env, ticket_id: String, duration_secs: u64) {
        ensure(
            &env,
            !ticket_id.is_empty(),
            EscrowError::DisputeTicketIdEmpty,
        );
        ensure(
            &env,
            duration_secs > 0,
            EscrowError::DisputePauseDurationNotPositive,
        );
        ensure(
            &env,
            duration_secs <= MAX_DISPUTE_PAUSE_DURATION_SECS,
            EscrowError::DisputePauseDurationExceedsMax,
        );

        let escrow = Self::load_escrow_require_admin(&env);

        let now = env.ledger().timestamp();
        let expires_at = now
            .checked_add(duration_secs)
            .unwrap_or_else(|| fail(&env, EscrowError::LedgerTimestampOverflow));

        let pause_state = DisputePauseState {
            ticket_id: ticket_id.clone(),
            paused_at_ledger_timestamp: now,
            expires_at_ledger_timestamp: expires_at,
        };

        env.storage()
            .instance()
            .set(&DataKey::DisputePaused, &pause_state);

        DisputePausedEvt {
            name: symbol_short!("disppause"),
            invoice_id: escrow.invoice_id.clone(),
            ticket_id,
            action: 1, // 1 = paused
            paused_at: now,
            expires_at,
        }
        .publish(&env);
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
    pub(crate) fn resume_dispute_impl(env: Env) {
        let escrow = Self::load_escrow_require_admin(&env);

        let pause_state: Option<DisputePauseState> =
            env.storage().instance().get(&DataKey::DisputePaused);

        ensure(&env, pause_state.is_some(), EscrowError::NoPauseActive);

        let now = env.ledger().timestamp();

        env.storage().instance().remove(&DataKey::DisputePaused);

        DisputeResumedEvt {
            name: symbol_short!("disp_res"),
            invoice_id: escrow.invoice_id.clone(),
            admin: escrow.admin,
            resumed_by: DisputeResumedBy::Manual,
            ledger_timestamp: now,
        }
        .publish(&env);
    }

    /// Check if a dispute pause is currently active on this escrow.
    ///
    /// Includes auto-expiration logic: if the pause was configured to expire and
    /// current ledger time has reached/exceeded the expiration, the pause is
    /// considered inactive. A mutating operation later removes the stored entry
    /// and emits `DisputeResumedEvt` once; this check remains read-only.
    ///
    /// # Returns
    /// `true` if a dispute pause exists and has not auto-expired; `false` otherwise.
    pub(crate) fn is_dispute_paused(env: &Env) -> bool {
        let pause_state: Option<DisputePauseState> =
            env.storage().instance().get(&DataKey::DisputePaused);

        if let Some(state) = pause_state {
            let now = env.ledger().timestamp();
            // Pause is active if current time is before expiration
            now < state.expires_at_ledger_timestamp
        } else {
            false
        }
    }

    /// Expire a stored pause once, on the first guarded mutation after its deadline.
    /// The public read path remains side-effect free.
    pub(crate) fn is_dispute_paused_for_mutation(env: &Env) -> bool {
        let pause_state: Option<DisputePauseState> =
            env.storage().instance().get(&DataKey::DisputePaused);

        if let Some(state) = pause_state {
            let now = env.ledger().timestamp();
            if now < state.expires_at_ledger_timestamp {
                return true;
            }

            env.storage().instance().remove(&DataKey::DisputePaused);
            let escrow = Self::get_escrow_impl(env.clone());
            DisputeResumedEvt {
                name: symbol_short!("disp_res"),
                invoice_id: escrow.invoice_id,
                admin: escrow.admin,
                resumed_by: DisputeResumedBy::AutoExpiry,
                ledger_timestamp: now,
            }
            .publish(env);
        }

        false
    }

    /// Get the current dispute pause state, if active.
    ///
    /// Returns `None` if no pause is active or if the pause has auto-expired.
    pub(crate) fn get_dispute_pause_impl(env: Env) -> Option<DisputePauseState> {
        let pause_state: Option<DisputePauseState> =
            env.storage().instance().get(&DataKey::DisputePaused);

        if let Some(state) = pause_state {
            let now = env.ledger().timestamp();
            if now < state.expires_at_ledger_timestamp {
                return Some(state);
            }
        }
        None
    }
}
