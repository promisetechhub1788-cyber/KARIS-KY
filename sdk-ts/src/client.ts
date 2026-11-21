// karis-ky Escrow SDK — Client Wrapper
//
// Provides a typed client for invoking the LiquifactEscrow Soroban contract.
// All numeric values use string representation for i128/u64/i64 precision.
//
// Schema version: 8
// Interface version: 1

import {
  EscrowStatus,
  ESCROW_STATUS_LABELS,
  EscrowErrorCode,
  ESCROW_ERROR_LABELS,
  classifyError,
  type InvoiceEscrow,
  type YieldTier,
  type FundingCloseSnapshot,
  type SmeCollateralCommitment,
  type EscrowSummary,
  type InvestorCapStatus,
  type InvestorPosition,
  type InvestorFundingRecord,
  type EscrowSnapshot,
  type ErrorDiagnostic,
  type EscrowTemplate,
  type InvestorCapStatus,
  type InitParams,
  type SorobanResult,
  type EscrowEvent,
  type SorobanEventPage,
  type EscrowEventSubscriptionOptions,
  type CheckEscrowHealth,
  type EscrowHealth,
  SCHEMA_VERSION,
  CONTRACT_INTERFACE_VERSION,
  MAX_INVOICE_ID_STRING_LEN,
  MAX_DUST_SWEEP_AMOUNT,
  MAX_FUND_BATCH,
} from "./types";

// ---------------------------------------------------------------------------
// Contract spec (loaded at runtime or bundled)
// ---------------------------------------------------------------------------

interface ContractSpec {
  contract: { name: string; schema_version: number; interface_version: number };
  entrypoints: Array<{
    name: string;
    description: string;
    auth: string;
    params: Array<{ name: string; type: string; description: string }>;
    returns: { type: string; description: string };
  }>;
  read_only_entrypoints: Array<{
    name: string;
    description: string;
    params?: Array<{ name: string; type: string }>;
    returns: { type: string };
  }>;
  error_codes: { groups: Record<string, unknown>; codes: Record<string, string> };
}

// ---------------------------------------------------------------------------
// Client configuration
// ---------------------------------------------------------------------------

export interface EscrowClientConfig {
  /** Soroban RPC endpoint URL */
  rpcUrl: string;
  /** Stellar network passphrase */
  networkPassphrase: string;
  /** Optional — contract ID (C... address); can be set after construction */
  contractId?: string;
  /** Optional — contract spec; if not provided, fetches from the default CDN location */
  spec?: ContractSpec;
  /** Optional — spec URL to fetch at construction time */
  specUrl?: string;
}

/** Raised when an SDK method receives an invalid argument. */
export class ValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ValidationError";
  }
}

function assertValidInvoiceId(invoiceId: string): void {
  if (typeof invoiceId !== "string") {
    throw new ValidationError(
      `invoice_id must be a string of 1..=${MAX_INVOICE_ID_STRING_LEN} ASCII characters [A-Za-z0-9_]`,
    );
  }

  const utf8ByteLength = new TextEncoder().encode(invoiceId).length;
  if (invoiceId.length === 0 || utf8ByteLength > MAX_INVOICE_ID_STRING_LEN) {
    throw new ValidationError(
      `invoice_id must be 1..=${MAX_INVOICE_ID_STRING_LEN} ASCII characters [A-Za-z0-9_] and cannot exceed ${MAX_INVOICE_ID_STRING_LEN} bytes`,
    );
  }

  if (!/^[A-Za-z0-9_]+$/.test(invoiceId)) {
    throw new ValidationError(
      `invoice_id must use only ASCII letters, digits, and underscores; received: ${invoiceId}`,
    );
  }
}

// ---------------------------------------------------------------------------
// SorobanClient (lightweight wrapper interface)
// ---------------------------------------------------------------------------

/**
 * Minimum interface for a Soroban RPC client.
 * Adapters for @stellar/stellar-sdk or custom clients implement this.
 */
export interface SorobanRpcClient {
  /** Invoke a contract function with the given name and args. Returns raw result. */
  invoke(contractId: string, functionName: string, args: unknown[], source?: string): Promise<unknown>;
  /** Simulate (read-only) a contract function. Returns raw result. */
  simulate(contractId: string, functionName: string, args: unknown[]): Promise<unknown>;
  /** Fetch the current ledger info (timestamp, sequence). */
  getLedger(): Promise<{ timestamp: number; sequence: number }>;
  /** Fetch contract events using Soroban RPC getEvents semantics. */
  getEvents?(filter: SorobanEventFilter, options: SorobanEventQuery): Promise<SorobanEventPage>;
}

export interface SorobanEventFilter {
  type: "contract";
  contractIds: string[];
}

export interface SorobanEventQuery {
  startLedger?: number;
  cursor?: string;
  limit?: number;
}

// ---------------------------------------------------------------------------
// EscrowClient
// ---------------------------------------------------------------------------

/** Typed client for the karis-ky LiquifactEscrow contract. */
export class EscrowClient {
  private contractId: string;
  private rpc: SorobanRpcClient;
  private spec: ContractSpec | null = null;
  private specUrl: string;
  private _rpcUrl: string;
  private _networkPassphrase: string;

  constructor(config: EscrowClientConfig, rpcClient: SorobanRpcClient) {
    this._rpcUrl = config.rpcUrl;
    this._networkPassphrase = config.networkPassphrase;
    this.contractId = config.contractId || "";
    this.rpc = rpcClient;
    this.spec = config.spec || null;
    this.specUrl = config.specUrl || "https://spec.karis-ky.com/escrow/spec.json";
  }

  // ---- Spec loading ----

  /** Fetch the contract spec from the configured URL. */
  async loadSpec(url?: string): Promise<ContractSpec> {
    const target = url || this.specUrl;
    const response = await fetch(target);
    if (!response.ok) {
      throw new Error(`Failed to fetch contract spec from ${target}: ${response.status}`);
    }
    this.spec = (await response.json()) as ContractSpec;
    return this.spec;
  }

  /** Get the loaded spec, loading it if necessary. */
  async getSpec(): Promise<ContractSpec> {
    if (!this.spec) {
      await this.loadSpec();
    }
    return this.spec!;
  }

  /** Validate that the deployed contract's interface version matches the SDK. */
  async validateInterfaceVersion(): Promise<void> {
    const deployedVersion = await this.getInterfaceVersion();
    if (deployedVersion !== CONTRACT_INTERFACE_VERSION) {
      throw new Error(
        `Interface version mismatch: SDK compiled for v${CONTRACT_INTERFACE_VERSION}, ` +
        `contract deployed with v${deployedVersion}. Update your SDK or redeploy the contract.`,
      );
    }
  }

  // ---- Contract metadata ----

  getContractId(): string {
    return this.contractId;
  }

  setContractId(id: string): void {
    this.contractId = id;
  }

  getRpcUrl(): string {
    return this._rpcUrl;
  }

  getNetworkPassphrase(): string {
    return this._networkPassphrase;
  }

  // ---- Event streaming ----

  /**
   * Stream escrow lifecycle events by polling Soroban RPC getEvents.
   * The adapter owns XDR decoding; this method handles filtering, paging, and cancellation.
   */
  async *subscribeEscrowEvents(
    options: EscrowEventSubscriptionOptions = {},
  ): AsyncGenerator<EscrowEvent, void, undefined> {
    if (!this.rpc.getEvents) {
      throw new Error("The configured Soroban RPC client does not support getEvents");
    }

    const pollInterval = options.poll_interval_ms ?? 5_000;
    const limit = options.limit ?? 100;
    if (pollInterval < 0 || !Number.isFinite(pollInterval)) {
      throw new Error("poll_interval_ms must be a finite, non-negative number");
    }
    if (!Number.isInteger(limit) || limit <= 0) {
      throw new Error("limit must be a positive integer");
    }

    let startLedger = options.start_ledger;
    let cursor = options.cursor;
    const eventNames = options.event_names ? new Set(options.event_names) : null;
    const filter: SorobanEventFilter = { type: "contract", contractIds: [this.contractId] };

    if (startLedger === undefined && cursor === undefined) {
      startLedger = (await this.rpc.getLedger()).sequence;
    }

    while (!options.signal?.aborted) {
      const query: SorobanEventQuery = { limit };
      if (cursor !== undefined) {
        query.cursor = cursor;
      } else if (startLedger !== undefined) {
        query.startLedger = startLedger;
      }
      const page = await this.rpc.getEvents(filter, query);
      if (options.signal?.aborted) {
        return;
      }
      const events = eventNames
        ? page.events.filter((event) => event.name !== undefined && eventNames.has(event.name))
        : page.events;

      for (const event of events) {
        yield event;
      }

      if (page.cursor) {
        cursor = page.cursor;
      } else {
        cursor = undefined;
        const lastLedger = page.events.at(-1)?.ledger;
        if (lastLedger !== undefined) {
          startLedger = lastLedger + 1;
        } else if (page.latest_ledger !== undefined) {
          startLedger = page.latest_ledger + 1;
        }
      }

      if (page.events.length === 0) {
        await waitForEventPoll(pollInterval, options.signal);
      }
    }
  }

  // ---- Read-only entrypoints ----

  async getEscrow(): Promise<InvoiceEscrow> {
    return this.simulate("get_escrow", []);
  }

  async getVersion(): Promise<number> {
    return this.simulate("get_version", []);
  }

  async getInterfaceVersion(): Promise<number> {
    return this.simulate("get_interface_version", []);
  }

  async getFundingToken(): Promise<string> {
    return this.simulate("get_funding_token", []);
  }

  async getTreasury(): Promise<string> {
    return this.simulate("get_treasury", []);
  }

  async getRegistryRef(): Promise<string | null> {
    return this.simulate("get_registry_ref", []);
  }

  async getPendingAdmin(): Promise<string | null> {
    return this.simulate("get_pending_admin", []);
  }

  async getLegalHold(): Promise<boolean> {
    return this.simulate("get_legal_hold", []);
  }

  async getLegalHoldStatus(): Promise<boolean> {
    return this.simulate("get_legal_hold_status", []);
  }

  async getContribution(investor: string): Promise<string> {
    return this.simulate("get_contribution", [investor]);
  }

  async computeInvestorPayout(investor: string): Promise<string> {
    return this.simulate("compute_investor_payout", [investor]);
  }

  async getDistributedPrincipal(): Promise<string> {
    return this.simulate("get_distributed_principal", []);
  }

  async getUniqueFunderCount(): Promise<number> {
    return this.simulate("get_unique_funder_count", []);
  }

  async getFundingCloseSnapshot(): Promise<FundingCloseSnapshot | null> {
    return this.simulate("get_funding_close_snapshot", []);
  }

  async getInvestorYieldBps(investor: string): Promise<string> {
    return this.simulate("get_investor_yield_bps", [investor]);
  }

  async getInvestorClaimNotBefore(investor: string): Promise<string> {
    return this.simulate("get_investor_claim_not_before", [investor]);
  }

  async isInvestorClaimed(investor: string): Promise<boolean> {
    return this.simulate("is_investor_claimed", [investor]);
  }

  async isInvestorRefunded(investor: string): Promise<boolean> {
    return this.simulate("is_investor_refunded", [investor]);
  }

  async getInvestorHistory(investor: string): Promise<InvestorFundingRecord[]> {
    return this.simulate("get_investor_history", [investor]);
  }

  async getInvestorHistoryPage(
    investor: string,
    offset: number,
    limit: number,
  ): Promise<InvestorFundingRecord[]> {
    return this.simulate("get_investor_history_page", [investor, offset, limit]);
  }

  async getMinContributionFloor(): Promise<string> {
    return this.simulate("get_min_contribution_floor", []);
  }

  async getMaxUniqueInvestorsCap(): Promise<number | null> {
    return this.simulate("get_max_unique_investors_cap", []);
  }

  async getMaxPerInvestorCap(): Promise<string | null> {
    return this.simulate("get_max_per_investor_cap", []);
  }

  async getEscrowSummary(): Promise<EscrowSummary> {
    return this.simulate("get_escrow_summary", []);
  }

  /** Export the full escrow snapshot for disaster recovery or migration. */
  async exportState(): Promise<EscrowSnapshot> {
    return this.simulate("export_state", []);
  }

  /** Import a previously exported snapshot onto a fresh, uninitialized contract instance. */
  async importState(snapshot: EscrowSnapshot, source?: string): Promise<void> {
    return this.invoke("import_state", [snapshot], source);
  }

  async getSmeCollateralCommitment(): Promise<SmeCollateralCommitment | null> {
    return this.simulate("get_sme_collateral_commitment", []);
  }

  async getPrimaryAttestationHash(): Promise<string | null> {
    return this.simulate("get_primary_attestation_hash", []);
  }

  async getAttestationAppendLog(): Promise<string[]> {
    return this.simulate("get_attestation_append_log", []);
  }

  async getAttestationLog(): Promise<string[]> {
    return this.simulate("get_attestation_log", []);
  }

  async getTemplate(name: string): Promise<EscrowTemplate | null> {
    return this.simulate("get_template", [name]);
  }

  async hasMaturityLock(): Promise<boolean> {
    return this.simulate("has_maturity_lock", []);
  }

  /**
   * Perform a quick health check on the escrow.
   *
   * Returns a lightweight summary with warning type, funded ratio, and time-to-maturity.
   * Suitable for dashboard displays and monitoring UIs.
   *
   * @returns CheckEscrowHealth with warning_type, funded_ratio_bps, and time_to_maturity_secs.
   */
  async checkEscrowHealth(): Promise<CheckEscrowHealth> {
    return this.simulate("check_escrow_health", []);
  }

  /**
   * Get comprehensive escrow health diagnostic information.
   *
   * Returns full health details including warning type, funding ratio, maturity timeline,
   * status labels, and legal hold / maturity lock status. Suitable for detailed health
   * monitoring dashboards and operator UIs.
   *
   * @returns EscrowHealth with all diagnostic fields including warning_type, funded_ratio_bps,
   *          time_to_maturity_secs, status_label, is_maturity_locked, is_legal_held, and
   *          unique_funder_count.
   */
  async getEscrowHealth(): Promise<EscrowHealth> {
    return this.simulate("get_escrow_health", []);
  }

  /**
   * Get investor capacity status for this escrow.
   *
   * Returns a single-call answer to "can more investors contribute?" without requiring
   * client-side arithmetic on max_unique_investors_cap and unique_funder_count.
   *
   * @returns InvestorCapStatus with max, current, remaining, and is_full fields.
   *          When no cap is set, max is 2^32-1 and is_full is always false.
   */
  async getInvestorCapStatus(): Promise<InvestorCapStatus> {
    return this.simulate("get_investor_cap_status", []);
  }

  async getInvestorPosition(investor: string): Promise<InvestorPosition> {
    return this.simulate("get_investor_position", [investor]);
  }

  // ---- State-mutating entrypoints ----

  /** Initialize escrow. One-shot; panics on duplicate. Auth: admin. */
  async init(params: InitParams, source?: string): Promise<InvoiceEscrow> {
    assertValidInvoiceId(params.invoice_id);

    const args = [
      params.admin,
      params.invoice_id,
      params.sme_address,
      params.amount,
      params.yield_bps,
      params.maturity,
      params.funding_token,
      params.registry,
      params.treasury,
      params.yield_tiers,
      params.min_contribution,
      params.max_unique_investors,
      params.max_per_investor,
      params.legal_hold_clear_delay,
      params.funding_deadline,
      params.max_funding_rate ?? null,
      params.yield_slippage_threshold,
      params.settlement_notifier_contract ?? null,
      params.kyc_provider_contract ?? null,
      params.admin_roles ?? null,
      params.reject_contract_admin ?? null,
    ];
    return this.invoke("init", args, source);
  }

  /** Initialize escrow with a distinct legal-hold guardian. Auth: admin. */
  async initWithGuardian(
    params: InitParams,
    guardian: string,
    source?: string,
  ): Promise<InvoiceEscrow> {
    assertValidInvoiceId(params.invoice_id);

    const args = [
      params.admin,
      guardian,
      params.invoice_id,
      params.sme_address,
      params.amount,
      params.yield_bps,
      params.maturity,
      params.funding_token,
      params.registry,
      params.treasury,
      params.yield_tiers,
      params.min_contribution,
      params.max_unique_investors,
      params.max_per_investor,
      params.legal_hold_clear_delay,
      params.funding_deadline,
      params.max_funding_rate ?? null,
      params.yield_slippage_threshold,
      params.settlement_notifier_contract ?? null,
      params.kyc_provider_contract ?? null,
      params.admin_roles ?? null,
      params.reject_contract_admin ?? null,
    ];
    return this.invoke("init_with_guardian", args, source);
  }

  /**
   * Fund escrow with base yield.
   * Auth: investor.
   * See [Fund Parameters Reference](../../../docs/escrow-fund-parameters.md#fund--simple-base-yield-deposit).
   */
  async fund(investor: string, amount: string, source?: string): Promise<InvoiceEscrow> {
    return this.invoke("fund", [investor, amount], source);
  }

  /**
   * First deposit with tiered yield commitment (single deposit per investor).
   * Auth: investor.
   * See [Fund Parameters Reference](../../../docs/escrow-fund-parameters.md#fund_with_commitment--first-deposit-tiered-yield).
   */
  async fundWithCommitment(
    investor: string,
    amount: string,
    committedLockSecs: string,
    source?: string,
  ): Promise<InvoiceEscrow> {
    return this.invoke("fund_with_commitment", [investor, amount, committedLockSecs], source);
  }

  /** Batch fund multiple investors. Auth: per-investor. Max 50 entries. */
  async fundBatch(
    entries: Array<[string, string]>,
    source?: string,
  ): Promise<InvoiceEscrow> {
    return this.invoke("fund_batch", [entries], source);
  }

  /**
   * Batch fund multiple investors in a single atomic transaction (FEAT-001).
   *
   * Identical per-entry semantics to {@link fund} — each entry requires the investor's
   * authorization and is validated against the same caps and allowlists. If any single
   * entry fails the entire transaction reverts.
   *
   * Emits one {@code BatchFundCompleted} summary event after all entries succeed, in
   * addition to the per-entry {@code EscrowFunded} / {@code FundReceived} events.
   *
   * @param contributions Array of [investor_address, amount] tuples; max 50 entries.
   * @param source Optional source account for fee payment.
   * @returns Updated {@link InvoiceEscrow} state.
   */
  async batchFund(
    contributions: Array<[string, string]>,
    source?: string,
  ): Promise<InvoiceEscrow> {
    return this.invoke("batch_fund", [contributions], source);
  }

  /** Settle a funded escrow. Auth: sme_address. */
  async settle(source?: string): Promise<InvoiceEscrow> {
    return this.invoke("settle", [], source);
  }

  /** Withdraw liquidity (alternative terminal path). Auth: sme_address. */
  async withdraw(source?: string): Promise<InvoiceEscrow> {
    return this.invoke("withdraw", [], source);
  }

  /** Record a payout claim after settlement. Auth: investor. */
  async claimInvestorPayout(investor: string, source?: string): Promise<void> {
    return this.invoke("claim_investor_payout", [investor], source);
  }

  /** Record payout claims for multiple investors. Auth: per-investor. */
  async batchClaimInvestorPayouts(investors: string[], source?: string): Promise<number> {
    if (investors.length === 0) {
      throw new Error("investors must not be empty");
    }
    return this.invoke("batch_claim_investor_payouts", [investors], source);
  }

  /** Cancel funding. Auth: admin. */
  async cancelFunding(source?: string): Promise<InvoiceEscrow> {
    return this.invoke("cancel_funding", [], source);
  }

  /** Refund principal after cancellation. Auth: investor. */
  async refund(investor: string, source?: string): Promise<void> {
    return this.invoke("refund", [investor], source);
  }

  /** Sweep terminal dust to treasury. Auth: treasury. */
  async sweepTerminalDust(amount: string, source?: string): Promise<string> {
    return this.invoke("sweep_terminal_dust", [amount], source);
  }

  /** Set or clear legal hold. Auth: admin. */
  async setLegalHold(active: boolean, reason: string, source?: string): Promise<void> {
    if (active && reason.trim().length === 0) {
      throw new Error("reason must be non-empty when legal hold is activated (active=true)");
    }
    return this.invoke("set_legal_hold", [active, reason], source);
  }

  /** Propose a legal hold for guardian confirmation. Auth: current admin. */
  async proposeLegalHold(admin: string, source?: string): Promise<void> {
    return this.invoke("propose_legal_hold", [admin], source);
  }

  /** Confirm a pending legal hold before its one-hour expiry. Auth: guardian. */
  async confirmLegalHold(guardian: string, source?: string): Promise<void> {
    return this.invoke("confirm_legal_hold", [guardian], source);
  }

  /** Clear legal hold (convenience wrapper). Auth: admin. */
  async clearLegalHold(source?: string): Promise<void> {
    return this.invoke("clear_legal_hold", [], source);
  }

  /** Request legal hold clear (starts two-phase delay). Auth: admin. */
  async requestClearLegalHold(source?: string): Promise<void> {
    return this.invoke("request_clear_legal_hold", [], source);
  }

  /** Update maturity timestamp. Auth: admin. Only in Open state. */
  async updateMaturity(newMaturity: string, source?: string): Promise<void> {
    return this.invoke("update_maturity", [newMaturity], source);
  }

  /** Update funding target. Auth: admin. Only in Open state. */
  async updateFundingTarget(newTarget: string, source?: string): Promise<void> {
    return this.invoke("update_funding_target", [newTarget], source);
  }

  /** Lower max unique investors cap. Auth: admin. Only in Open state. */
  async lowerMaxUniqueInvestors(newCap: number, source?: string): Promise<void> {
    return this.invoke("lower_max_unique_investors", [newCap], source);
  }

  /** Propose a new admin. Auth: current admin. */
  async proposeAdmin(newAdmin: string, source?: string): Promise<void> {
    return this.invoke("propose_admin", [newAdmin], source);
  }

  /** Accept admin role. Auth: pending admin. */
  async acceptAdmin(source?: string): Promise<void> {
    return this.invoke("accept_admin", [], source);
  }

  /** Migrate schema. Typed errors on all paths in current release. */
  async migrate(fromVersion: number, source?: string): Promise<void> {
    return this.invoke("migrate", [fromVersion], source);
  }

  /** Batch set investor allowlist. Auth: admin. Max 32 per call. */
  async setInvestorsAllowlisted(investors: string[], allowed: boolean, source?: string): Promise<void> {
    return this.invoke("set_investors_allowlisted", [investors, allowed], source);
  }

  /** Toggle allowlist gate. Auth: admin. */
  async setAllowlistActive(active: boolean, source?: string): Promise<void> {
    return this.invoke("set_allowlist_active", [active], source);
  }

  /** Bind primary attestation hash. Auth: admin. Single-set. */
  async bindPrimaryAttestationHash(digest: string, source?: string): Promise<void> {
    return this.invoke("bind_primary_attestation_hash", [digest], source);
  }

  /** Append a 32-byte digest to the attestation log. Auth: admin. Max 32 entries. */
  async appendAttestationDigest(digest: Uint8Array, source?: string): Promise<void> {
    if (!(digest instanceof Uint8Array) || digest.byteLength !== 32) {
      throw new ValidationError("Attestation digest must be exactly 32 bytes");
    }
    return this.invoke("append_attestation_digest", [digest], source);
  }

  /** Revoke attestation at index. Auth: admin. */
  async revokeAttestationDigest(index: number, source?: string): Promise<void> {
    return this.invoke("revoke_attestation_digest", [index], source);
  }

  /** Record SME collateral metadata. Auth: sme_address. ⚠️ Metadata only — not proof of custody. */
  async recordSmeCollateralCommitment(asset: string, amount: string, source?: string): Promise<void> {
    return this.invoke("record_sme_collateral_commitment", [asset, amount], source);
  }

  /** Rotate beneficiary (SME) address. Auth: sme_address + admin. */
  async rotateBeneficiary(newSme: string, source?: string): Promise<void> {
    return this.invoke("rotate_beneficiary", [newSme], source);
  }

  /** Verify asset custody. Auth: admin. Returns discrepancy. */
  async verifyAssetCustody(source?: string): Promise<string> {
    return this.invoke("verify_asset_custody", [], source);
  }

  /** Initialize from a named template. Auth: admin. */
  async initFromTemplate(
    admin: string,
    templateName: string,
    invoiceId: string,
    smeAddress: string,
    amount: string,
    fundingToken: string,
    treasury: string,
    registry: string | null,
    source?: string,
  ): Promise<InvoiceEscrow> {
    assertValidInvoiceId(invoiceId);
    return this.invoke("init_from_template", [
      admin, templateName, invoiceId, smeAddress,
      amount, fundingToken, treasury, registry,
    ], source);
  }

  /** Register a custom template. Auth: admin. */
  async registerTemplate(name: string, template: EscrowTemplate, source?: string): Promise<void> {
    return this.invoke("register_template", [name, template], source);
  }

  // ---- Private helpers ----

  private async simulate<T>(fnName: string, args: unknown[]): Promise<T> {
    const raw = await this.rpc.simulate(this.contractId, fnName, args);
    return raw as T;
  }

  private async invoke<T>(fnName: string, args: unknown[], source?: string): Promise<T> {
    const raw = await this.rpc.invoke(this.contractId, fnName, args, source);
    return raw as T;
  }
}

function waitForEventPoll(delayMs: number, signal?: AbortSignal): Promise<void> {
  if (signal?.aborted || delayMs === 0) {
    return Promise.resolve();
  }

  return new Promise((resolve) => {
    const timer = setTimeout(resolve, delayMs);
    signal?.addEventListener("abort", () => {
      clearTimeout(timer);
      resolve();
    }, { once: true });
  });
}
