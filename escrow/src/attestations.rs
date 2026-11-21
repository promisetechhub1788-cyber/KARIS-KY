use super::*;

impl LiquifactEscrow {
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
    pub(crate) fn bind_primary_attestation_hash_impl(env: Env, digest: Bytes) {
        ensure(
            &env,
            !env.storage()
                .instance()
                .has(&DataKey::PrimaryAttestationHash),
            EscrowError::AttestationHashAlreadyBound,
        );

        // Length validation: must be exactly 32 bytes (e.g. SHA-256).
        // Emits a typed error so callers receive error code 52 instead of an opaque panic.
        ensure(
            &env,
            digest.len() == 32,
            EscrowError::InvalidAttestationHashLength,
        );
        let digest: BytesN<32> = digest
            .try_into()
            .unwrap_or_else(|_| fail(&env, EscrowError::InvalidAttestationHashLength));

        let escrow = Self::load_escrow_require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::PrimaryAttestationHash, &digest);
        PrimaryAttestationBound {
            name: symbol_short!("att_bind"),
            invoice_id: escrow.invoice_id.clone(),
            digest: digest.clone(),
        }
        .publish(&env);
        AttestationBoundEvt {
            name: symbol_short!("att_bound"),
            hash: digest,
            ledger_timestamp: env.ledger().timestamp(),
        }
        .publish(&env);
    }

    pub(crate) fn get_primary_attestation_hash_impl(env: Env) -> Option<BytesN<32>> {
        env.storage()
            .instance()
            .get(&DataKey::PrimaryAttestationHash)
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
    pub(crate) fn append_attestation_digest_impl(env: Env, digest: BytesN<32>, tag: Symbol) {
        let escrow = Self::load_escrow_require_admin(&env);

        // Auto-migrate: if an old Vec-based log exists but no count has been set,
        // migrate each old entry to the new individual-entry format first.
        let has_old_log = env.storage().instance().has(&DataKey::AttestationAppendLog);
        let has_count = env
            .storage()
            .instance()
            .has(&DataKey::AttestationAppendLogCount);

        if has_old_log && !has_count {
            // Read the legacy Vec once, write each entry individually, then delete it.
            let legacy: Vec<BytesN<32>> = env
                .storage()
                .instance()
                .get(&DataKey::AttestationAppendLog)
                .unwrap_or_else(|| Vec::new(&env));
            let legacy_len = legacy.len();
            for i in 0..legacy_len {
                env.storage()
                    .instance()
                    .set(&DataKey::AttestationLogEntry(i), &legacy.get(i).unwrap());
            }
            env.storage()
                .instance()
                .set(&DataKey::AttestationAppendLogCount, &legacy_len);
            // Remove the legacy Vec so future reads use the new format.
            env.storage()
                .instance()
                .remove(&DataKey::AttestationAppendLog);
        }

        // Optimized: read only the count instead of the full Vec<BytesN<32>>.
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AttestationAppendLogCount)
            .unwrap_or(0);

        ensure(
            &env,
            count < MAX_ATTESTATION_APPEND_ENTRIES,
            EscrowError::AttestationAppendLogCapacityReached,
        );

        let idx = count;

        // Store the new entry individually — avoids deserializing all 32 entries.
        env.storage()
            .instance()
            .set(&DataKey::AttestationLogEntry(idx), &digest);

        // Bump the count atomically.
        env.storage()
            .instance()
            .set(&DataKey::AttestationAppendLogCount, &(count + 1));

        // Store the ledger timestamp for this entry (enables time-based queries)
        let ts = env.ledger().timestamp();
        env.storage()
            .instance()
            .set(&DataKey::AttestationTimestamp(idx), &ts);

        // Store the tag if non-empty (enables tag-based queries)
        if tag != Symbol::new(&env, "") {
            env.storage()
                .instance()
                .set(&DataKey::AttestationTag(idx), &tag);
        }

        AttestationDigestAppended {
            name: symbol_short!("att_app"),
            invoice_id: escrow.invoice_id.clone(),
            index: idx,
            digest,
        }
        .publish(&env);
    }

    pub(crate) fn get_attestation_append_log_impl(env: Env) -> Vec<BytesN<32>> {
        // Backward compat: if the old Vec-based key exists, return it (existing escrows).
        // New deployments use individual AttestationLogEntry keys gated by AttestationAppendLogCount.
        if let Some(legacy_log) = env
            .storage()
            .instance()
            .get::<DataKey, Vec<BytesN<32>>>(&DataKey::AttestationAppendLog)
        {
            return legacy_log;
        }

        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AttestationAppendLogCount)
            .unwrap_or(0);

        if count == 0 {
            return Vec::new(&env);
        }

        let mut log = Vec::new(&env);
        for i in 0..count {
            if let Some(entry) = env
                .storage()
                .instance()
                .get::<DataKey, BytesN<32>>(&DataKey::AttestationLogEntry(i))
            {
                log.push_back(entry);
            }
        }
        log
    }

    /// Retrieve the currently recorded SME collateral commitment metadata from storage.
    /// Returns `None` if no commitment has been recorded yet.
    pub(crate) fn get_sme_collateral_commitment_impl(env: Env) -> Option<SmeCollateralCommitment> {
        env.storage().instance().get(&DataKey::SmeCollateralPledge)
    }

    pub(crate) fn revoke_attestation_digest_impl(env: Env, index: u32) {
        let escrow = Self::get_escrow_impl(env.clone());
        escrow.admin.require_auth();

        // Determine log length — prefer count, fall back to legacy Vec length.
        let log_len: u32 = env
            .storage()
            .instance()
            .get(&DataKey::AttestationAppendLogCount)
            .unwrap_or_else(|| {
                // Legacy path: read old Vec to get length.
                let legacy: Vec<BytesN<32>> = env
                    .storage()
                    .instance()
                    .get(&DataKey::AttestationAppendLog)
                    .unwrap_or_else(|| Vec::new(&env));
                legacy.len()
            });

        assert!(index < log_len, "attestation index out of range");
        assert!(
            !env.storage()
                .instance()
                .has(&DataKey::AttestationRevoked(index)),
            "attestation already revoked at index"
        );

        env.storage()
            .instance()
            .set(&DataKey::AttestationRevoked(index), &true);

        AttestationDigestRevoked {
            name: symbol_short!("att_rev"),
            invoice_id: escrow.invoice_id.clone(),
            index,
        }
        .publish(&env);
    }

    pub(crate) fn is_attestation_revoked_impl(env: Env, index: u32) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::AttestationRevoked(index))
            .unwrap_or(false)
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
    pub(crate) fn record_sme_collateral_commitment_impl(
        env: Env,
        asset: Symbol,
        amount: i128,
    ) -> SmeCollateralCommitment {
        ensure(&env, amount > 0, EscrowError::CollateralAmountNotPositive);
        ensure(
            &env,
            asset != Symbol::new(&env, ""),
            EscrowError::CollateralAssetEmpty,
        );

        // env.clone(): env is used again after this call for storage read/write, timestamp, and publish.
        let escrow = Self::load_escrow_require_sme(&env);

        let now = env.ledger().timestamp();
        let prior: Option<SmeCollateralCommitment> =
            env.storage().instance().get(&DataKey::SmeCollateralPledge);
        let prior_amount = prior.as_ref().map(|c| c.amount).unwrap_or(0);

        if let Some(ref existing) = prior {
            // Updates are not permitted after settlement (status >= 2: settled, withdrawn, cancelled).
            ensure(
                &env,
                escrow.status < 2,
                EscrowError::CollateralUpdateAfterSettlement,
            );
            ensure(
                &env,
                now >= existing.updated_at,
                EscrowError::CollateralTimestampBackwards,
            );
        }

        // On first write: recorded_at = updated_at = now (both timestamps are the same).
        // On update: recorded_at is preserved from the original write; updated_at advances.
        let recorded_at = prior.as_ref().map(|c| c.recorded_at).unwrap_or(now);

        let commitment = SmeCollateralCommitment {
            asset,
            amount,
            recorded_at,
            updated_at: now,
        };
        env.storage()
            .instance()
            .set(&DataKey::SmeCollateralPledge, &commitment);

        CollateralRecordedEvt {
            name: symbol_short!("coll_rec"),
            invoice_id: escrow.invoice_id.clone(),
            amount,
            prior_amount,
        }
        .publish(&env);

        commitment
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
    pub(crate) fn query_attestation_logs_impl(
        env: Env,
        filters: AuditLogFilter,
        limit: u32,
        offset: u32,
    ) -> AuditLogPage {
        let log: Vec<BytesN<32>> = env
            .storage()
            .instance()
            .get(&DataKey::AttestationAppendLog)
            .unwrap_or_else(|| Vec::new(&env));

        let log_len = log.len();
        let mut all_matches: Vec<AttestationLogEntry> = Vec::new(&env);

        for i in 0..log_len {
            let digest = log.get(i).unwrap();

            // Read timestamp (default 0)
            let ts: u64 = env
                .storage()
                .instance()
                .get(&DataKey::AttestationTimestamp(i))
                .unwrap_or(0);

            // Read tag (default empty symbol)
            let tag: Symbol = env
                .storage()
                .instance()
                .get(&DataKey::AttestationTag(i))
                .unwrap_or_else(|| symbol_short!(""));

            // Read revocation status
            let revoked: bool = env
                .storage()
                .instance()
                .get(&DataKey::AttestationRevoked(i))
                .unwrap_or(false);

            // Apply filter: after_timestamp
            if let Some(after) = filters.after_timestamp {
                if ts < after {
                    continue;
                }
            }

            // Apply filter: tag_prefix (exact match — `Symbol` has no wasm-safe substring API,
            // so this is not a true prefix match; see the field's rustdoc).
            if let Some(ref prefix) = filters.tag_prefix {
                if &tag != prefix {
                    continue;
                }
            }

            all_matches.push_back(AttestationLogEntry {
                digest,
                index: i,
                timestamp: ts,
                tag,
                revoked,
            });
        }

        let total_count = all_matches.len();

        // Apply pagination: skip `offset` entries, take up to `limit`
        let mut page: Vec<AttestationLogEntry> = Vec::new(&env);
        let start = offset.min(total_count);
        let end = (offset.saturating_add(limit)).min(total_count);
        for j in start..end {
            page.push_back(all_matches.get(j).unwrap());
        }

        AuditLogPage {
            entries: page,
            total_count,
        }
    }
}
