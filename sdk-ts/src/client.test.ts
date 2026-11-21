/**
 * Integration tests for EscrowClient
 * 
 * Tests the full request-response cycle with a mocked Soroban RPC client.
 * Covers: init, fund, settle, claim_investor_payout, get_escrow, get_escrow_summary
 * Validates XDR parameter encoding for each method.
 * 
 * All tests use a stub Stellar SDK to avoid live network connections.
 */

import { EscrowClient, type SorobanRpcClient } from "./client";
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
  type EscrowSnapshot,
  type ErrorDiagnostic,
  type EscrowTemplate,
  type InitParams,
  type SorobanResult,
  type EscrowEvent,
  type SorobanEventPage,
  type EscrowEventSubscriptionOptions,
  SCHEMA_VERSION,
  CONTRACT_INTERFACE_VERSION,
  MAX_INVOICE_ID_STRING_LEN,
  MAX_DUST_SWEEP_AMOUNT,
  MAX_FUND_BATCH,
  ValidationError,
  toBaseUnits,
} from "./index";

const event = (name: string, ledger: number): EscrowEvent => ({
  id: `${name}-${ledger}`,
  type: "contract",
  contract_id: "CESCROW",
  ledger,
  ledger_closed_at: "2026-01-01T00:00:00Z",
  paging_token: `${ledger}-token`,
  topics: [name],
  value: {},
  name,
});

// ---------------------------------------------------------------------------
// Mock Soroban RPC Client
// ---------------------------------------------------------------------------

/**
 * Stub SorobanRpcClient that records invocations and returns controlled responses.
 * Tracks method names, arguments, and enables assertions on XDR encoding.
 */
class StubSorobanClient implements SorobanRpcClient {
  // Track all invocations for assertion
  public invocationLog: Array<{
    method: "invoke" | "simulate";
    functionName: string;
    args: unknown[];
    source?: string;
  }> = [];

  // Controlled responses per function
  private responses: Map<string, unknown> = new Map();

  constructor() {
    this.setupDefaultResponses();
  }

  /**
   * Set up default mock responses for common queries.
   */
  private setupDefaultResponses(): void {
    // Metadata queries
    this.responses.set("get_version", SCHEMA_VERSION);
    this.responses.set("get_interface_version", CONTRACT_INTERFACE_VERSION);
    this.responses.set("get_funding_token", "C" + "A".repeat(55)); // Mock token address
    this.responses.set("get_treasury", "C" + "B".repeat(55)); // Mock treasury address
    this.responses.set("get_legal_hold", false);
    this.responses.set("get_legal_hold_status", false);
    this.responses.set("get_unique_funder_count", 1);
    this.responses.set("get_min_contribution_floor", "100000000"); // 1 unit in base units
  }

  /**
   * Register a mock response for a specific function.
   */
  setResponse(functionName: string, response: unknown): void {
    this.responses.set(functionName, response);
  }

  /**
   * Simulate a contract read-only call. Records invocation and returns mock response.
   */
  async simulate(contractId: string, functionName: string, args: unknown[]): Promise<unknown> {
    this.invocationLog.push({ method: "simulate", functionName, args });

    // Return configured response or throw
    if (this.responses.has(functionName)) {
      return this.responses.get(functionName);
    }

    // Default fallback responses for common patterns
    if (functionName === "get_escrow") {
      return this.createMockEscrow();
    }
    if (functionName === "get_escrow_summary") {
      return this.createMockEscrowSummary();
    }
    if (functionName === "get_contribution" && args.length > 0) {
      return "500000000"; // Mock contribution
    }
    if (functionName === "get_investor_yield_bps" && args.length > 0) {
      return "800"; // 8%
    }
    if (functionName === "get_investor_claim_not_before" && args.length > 0) {
      return "1700000000"; // Unix timestamp
    }
    if (functionName === "is_investor_claimed" && args.length > 0) {
      return false;
    }
    if (functionName === "is_investor_refunded" && args.length > 0) {
      return false;
    }
    if (functionName === "get_funding_close_snapshot") {
      return this.createMockFundingCloseSnapshot();
    }
    if (functionName === "get_investor_position") {
      return this.createMockInvestorPosition();
    }

    throw new Error(`Unexpected simulate call: ${functionName}`);
  }

  /**
   * Invoke a state-changing contract method. Records invocation and returns mock response.
   */
  async invoke(
    contractId: string,
    functionName: string,
    args: unknown[],
    source?: string,
  ): Promise<unknown> {
    this.invocationLog.push({ method: "invoke", functionName, args, source });

    // Return configured response or fallback
    if (this.responses.has(functionName)) {
      return this.responses.get(functionName);
    }

    // Default fallback responses
    if (functionName === "init" || functionName === "init_with_guardian") {
      return {
        ...this.createMockEscrow(),
        status: EscrowStatus.Open,
      };
    }

    if (
      functionName === "fund" ||
      functionName === "fund_with_commitment"
    ) {
      return {
        ...this.createMockEscrow(),
        status: EscrowStatus.Funded,
      };
    }

    if (
      functionName === "settle"
    ) {
      return {
        ...this.createMockEscrow(),
        status: EscrowStatus.Settled,
      };
    }

    if (
      functionName === "withdraw" ||
      functionName === "cancel_funding"
    ) {
      return this.createMockEscrow();
    }

    if (functionName === "claim_investor_payout" || functionName === "refund") {
      return undefined; // These return void
    }

    if (functionName === "sweep_terminal_dust") {
      return "100000000"; // Swept amount
    }

    throw new Error(`Unexpected invoke call: ${functionName}`);
  }

  /**
   * Mock getLedger for timing-dependent operations.
   */
  async getLedger(): Promise<{ timestamp: number; sequence: number }> {
    return { timestamp: 1700000000, sequence: 123456 };
  }

  /**
   * Get the invocation log for assertions.
   */
  getInvocationLog(): typeof this.invocationLog {
    return this.invocationLog;
  }

  /**
   * Clear the invocation log.
   */
  clearInvocationLog(): void {
    this.invocationLog = [];
  }

  // ---- Helper factories for mock data ----

  private createMockEscrow(): InvoiceEscrow {
    return {
      invoice_id: "INV001",
      admin: "G" + "A".repeat(55),
      guardian: null,
      sme_address: "G" + "B".repeat(55),
      amount: "100000000000",
      funding_target: "100000000000",
      funded_amount: "100000000000",
      yield_bps: "800",
      maturity: "1700000000",
      status: EscrowStatus.Funded,
    };
  }

  private createMockEscrowSummary(): EscrowSummary {
    return {
      escrow: this.createMockEscrow(),
      has_maturity_lock: false,
      legal_hold: false,
      funding_close_snapshot: this.createMockFundingCloseSnapshot(),
      unique_funder_count: 1,
      is_allowlist_active: false,
      schema_version: SCHEMA_VERSION,
      sme_collateral_commitment: null,
      has_primary_attestation: false,
      attestation_log_length: 0,
      dispute_pause_active: false,
      dispute_pause_expiry: null,
      allowlist_active: false,
    };
  }

  private createMockFundingCloseSnapshot(): FundingCloseSnapshot {
    return {
      total_principal: "100000000000",
      funding_target: "100000000000",
      closed_at_ledger_timestamp: "1700000000",
      closed_at_ledger_sequence: 123456,
    };
  }

  private createMockInvestorPosition(): unknown {
    return {
      contribution: "500000000",
      yield_bps: "800",
      claim_not_before: "1700000000",
      claimed: false,
      refunded: false,
    };
  }
}

// ---------------------------------------------------------------------------
// Integration Tests
// ---------------------------------------------------------------------------

describe("EscrowClient Integration Tests", () => {
  let client: EscrowClient;
  let stub: StubSorobanClient;

  const CONTRACT_ID = "CBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  const ADMIN = "GBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  const SME = "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
  const INVESTOR_1 = "GCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
  const INVESTOR_2 = "GDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD";
  const FUNDING_TOKEN = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
  const TREASURY = "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
  const REGISTRY = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

  beforeEach(() => {
    stub = new StubSorobanClient();
    client = new EscrowClient(
      {
        rpcUrl: "http://localhost:8000/soroban/rpc",
        networkPassphrase: "Test SDF Network ; September 2015",
        contractId: CONTRACT_ID,
      },
      stub,
    );
  });

  // =========================================================================
  // Test: init
  // =========================================================================

  describe("init", () => {
    it("should encode init parameters correctly for XDR", async () => {
      const params: InitParams = {
        admin: ADMIN,
        invoice_id: "INV001",
        sme_address: SME,
        amount: toBaseUnits("100000", 7), // 100000 USDC
        yield_bps: "800", // 8%
        maturity: "1700000000",
        funding_token: FUNDING_TOKEN,
        registry: REGISTRY,
        treasury: TREASURY,
        yield_tiers: [
          { min_lock_secs: "2592000", yield_bps: "900" }, // 30 days, 9%
          { min_lock_secs: "7776000", yield_bps: "1000" }, // 90 days, 10%
        ],
        min_contribution: toBaseUnits("1000", 7),
        max_unique_investors: 100,
        max_per_investor: toBaseUnits("50000", 7),
        legal_hold_clear_delay: "604800",
        funding_deadline: "1700086400",
        yield_slippage_threshold: "50", // 0.5% slippage tolerance
      };

      const result = await client.init(params);

      // Verify the call was recorded
      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const initCall = log[0];

      expect(initCall.method).toBe("invoke");
      expect(initCall.functionName).toBe("init");

      // Validate XDR encoding of each parameter
      const args = initCall.args;
      expect(args[0]).toBe(ADMIN); // admin: Address
      expect(args[1]).toBe("INV001"); // invoice_id: String
      expect(args[2]).toBe(SME); // sme_address: Address
      expect(args[3]).toBe(toBaseUnits("100000", 7)); // amount: i128 (string)
      expect(args[4]).toBe("800"); // yield_bps: i64 (string)
      expect(args[5]).toBe("1700000000"); // maturity: u64 (string)
      expect(args[6]).toBe(FUNDING_TOKEN); // funding_token: Address
      expect(args[7]).toBe(REGISTRY); // registry: Option<Address>
      expect(args[8]).toBe(TREASURY); // treasury: Address
      expect(args[9]).toEqual(params.yield_tiers); // yield_tiers: Vec<YieldTier>
      expect(args[10]).toBe(toBaseUnits("1000", 7)); // min_contribution: i128
      expect(args[11]).toBe(100); // max_unique_investors: u32
      expect(args[12]).toBe(toBaseUnits("50000", 7)); // max_per_investor: i128
      expect(args[13]).toBe("604800"); // legal_hold_clear_delay: u64
      expect(args[14]).toBe("1700086400"); // funding_deadline: u64
      expect(args[15]).toBeNull(); // max_funding_rate: Option<u64>
      expect(args[16]).toBe("50"); // yield_slippage_threshold: i64
      expect(args.slice(17)).toEqual([null, null, null, null]);

      // Verify return type
      expect(result).toBeDefined();
      expect(result.invoice_id).toBe("INV001");
      expect(result.status).toBe(EscrowStatus.Open);
    });

    it("should handle init with minimal parameters", async () => {
      const params: InitParams = {
        admin: ADMIN,
        invoice_id: "INV002",
        sme_address: SME,
        amount: toBaseUnits("50000", 7),
        yield_bps: "500",
        maturity: "0", // No maturity gate
        funding_token: FUNDING_TOKEN,
        registry: null, // Optional
        treasury: TREASURY,
        yield_tiers: null, // No tiered yield
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      const result = await client.init(params);

      const log = stub.getInvocationLog();
      const args = log[0].args;

      expect(args[7]).toBeNull(); // registry is null
      expect(args[9]).toBeNull(); // yield_tiers is null
      expect(args[10]).toBeNull(); // min_contribution is null
      expect(args[11]).toBeNull(); // max_unique_investors is null
      expect(args[12]).toBeNull(); // max_per_investor is null

      expect(result).toBeDefined();
    });

    it("should reject invoice ids longer than the maximum before invoking the network", async () => {
      const boundaryId = "A".repeat(MAX_INVOICE_ID_STRING_LEN);
      const tooLongId = "A".repeat(MAX_INVOICE_ID_STRING_LEN + 1);

      const validParams: InitParams = {
        admin: ADMIN,
        invoice_id: boundaryId,
        sme_address: SME,
        amount: toBaseUnits("1000", 7),
        yield_bps: "800",
        maturity: "0",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      await expect(client.init(validParams)).resolves.toBeDefined();
      expect(stub.getInvocationLog()).toHaveLength(1);

      stub.clearInvocationLog();

      const invalidParams: InitParams = {
        ...validParams,
        invoice_id: tooLongId,
      };

      await expect(client.init(invalidParams)).rejects.toThrow(
        new ValidationError(
          `invoice_id must be 1..=${MAX_INVOICE_ID_STRING_LEN} ASCII characters [A-Za-z0-9_] and cannot exceed ${MAX_INVOICE_ID_STRING_LEN} bytes`,
        ).message,
      );
      expect(stub.getInvocationLog()).toHaveLength(0);
    });

    it("should place the guardian immediately after admin", async () => {
      const params: InitParams = {
        admin: ADMIN,
        invoice_id: "INV003",
        sme_address: SME,
        amount: "500000000000",
        yield_bps: "500",
        maturity: "0",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      await client.initWithGuardian(params, INVESTOR_1);

      const initCall = stub.getInvocationLog()[0];
      expect(initCall.functionName).toBe("init_with_guardian");
      expect(initCall.args[0]).toBe(ADMIN);
      expect(initCall.args[1]).toBe(INVESTOR_1);
      expect(initCall.args[2]).toBe("INV003");
      expect(initCall.args).toHaveLength(22);
    });
  });

  // =========================================================================
  // Test: fund
  // =========================================================================

  describe("fund", () => {
    it("should encode fund parameters correctly for XDR", async () => {
      stub.clearInvocationLog();
      const fundAmount = toBaseUnits("5000", 7);

      const result = await client.fund(INVESTOR_1, fundAmount);

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const fundCall = log[0];

      expect(fundCall.method).toBe("invoke");
      expect(fundCall.functionName).toBe("fund");

      const args = fundCall.args;
      expect(args[0]).toBe(INVESTOR_1); // investor: Address
      expect(args[1]).toBe(fundAmount); // amount: i128 (string)

      expect(result).toBeDefined();
      expect(result.status).toBe(EscrowStatus.Funded);
    });

    it("should support source parameter for auth", async () => {
      stub.clearInvocationLog();
      const fundAmount = toBaseUnits("5000", 7);

      await client.fund(INVESTOR_1, fundAmount, INVESTOR_1);

      const log = stub.getInvocationLog();
      const fundCall = log[0];

      expect(fundCall.source).toBe(INVESTOR_1);
    });

    it("should handle multiple investor funds", async () => {
      stub.clearInvocationLog();

      // Investor 1 funds
      await client.fund(INVESTOR_1, toBaseUnits("5000", 7));
      expect(stub.getInvocationLog()).toHaveLength(1);

      // Investor 2 funds
      await client.fund(INVESTOR_2, toBaseUnits("10000", 7));
      expect(stub.getInvocationLog()).toHaveLength(2);

      const log = stub.getInvocationLog();
      expect(log[0].args[0]).toBe(INVESTOR_1);
      expect(log[1].args[0]).toBe(INVESTOR_2);
    });
  });

  // =========================================================================
  // Test: fund_with_commitment
  // =========================================================================

  describe("fund_with_commitment", () => {
    it("should encode fund_with_commitment parameters correctly for XDR", async () => {
      stub.clearInvocationLog();
      const fundAmount = toBaseUnits("5000", 7);
      const lockSecs = "7776000"; // 90 days

      const result = await client.fundWithCommitment(
        INVESTOR_1,
        fundAmount,
        lockSecs,
      );

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const fundCall = log[0];

      expect(fundCall.method).toBe("invoke");
      expect(fundCall.functionName).toBe("fund_with_commitment");

      const args = fundCall.args;
      expect(args[0]).toBe(INVESTOR_1); // investor: Address
      expect(args[1]).toBe(fundAmount); // amount: i128
      expect(args[2]).toBe(lockSecs); // committedLockSecs: u64

      expect(result).toBeDefined();
    });
  });

  // =========================================================================
  // Test: settle
  // =========================================================================

  describe("settle", () => {
    it("should encode settle with correct parameters for XDR", async () => {
      stub.clearInvocationLog();
      stub.setResponse("settle", {
        invoice_id: "INV001",
        admin: ADMIN,
        sme_address: SME,
        amount: toBaseUnits("100000", 7),
        funding_target: toBaseUnits("100000", 7),
        funded_amount: toBaseUnits("100000", 7),
        yield_bps: "800",
        maturity: "1700000000",
        status: EscrowStatus.Settled,
      });

      const result = await client.settle();

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const settleCall = log[0];

      expect(settleCall.method).toBe("invoke");
      expect(settleCall.functionName).toBe("settle");
      expect(settleCall.args).toHaveLength(0); // No parameters

      expect(result).toBeDefined();
      expect(result.status).toBe(EscrowStatus.Settled);
    });

    it("should support SME auth on settle", async () => {
      stub.clearInvocationLog();

      await client.settle(SME);

      const log = stub.getInvocationLog();
      expect(log[0].source).toBe(SME);
    });
  });

  // =========================================================================
  // Test: claim_investor_payout
  // =========================================================================

  describe("claim_investor_payout", () => {
    it("should encode claim_investor_payout parameters correctly for XDR", async () => {
      stub.clearInvocationLog();

      const result = await client.claimInvestorPayout(INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const claimCall = log[0];

      expect(claimCall.method).toBe("invoke");
      expect(claimCall.functionName).toBe("claim_investor_payout");

      const args = claimCall.args;
      expect(args[0]).toBe(INVESTOR_1); // investor: Address
      expect(args).toHaveLength(1);

      // Result should be undefined (void return)
      expect(result).toBeUndefined();
    });

    it("should support investor auth on claim", async () => {
      stub.clearInvocationLog();

      await client.claimInvestorPayout(INVESTOR_1, INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log[0].source).toBe(INVESTOR_1);
    });

    it("should handle multiple investor claims", async () => {
      stub.clearInvocationLog();

      await client.claimInvestorPayout(INVESTOR_1);
      await client.claimInvestorPayout(INVESTOR_2);

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(2);
      expect(log[0].args[0]).toBe(INVESTOR_1);
      expect(log[1].args[0]).toBe(INVESTOR_2);
    });
  });

  // =========================================================================
  // Test: get_escrow (read-only)
  // =========================================================================

  describe("get_escrow", () => {
    it("should retrieve escrow state with correct parameters for XDR", async () => {
      stub.clearInvocationLog();

      const escrow = await client.getEscrow();

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const getCall = log[0];

      expect(getCall.method).toBe("simulate");
      expect(getCall.functionName).toBe("get_escrow");
      expect(getCall.args).toHaveLength(0); // No parameters

      expect(escrow).toBeDefined();
      expect(escrow.invoice_id).toBe("INV001");
      expect(escrow.status).toBe(EscrowStatus.Funded);
    });

    it("should assert escrow data types", async () => {
      const escrow = await client.getEscrow();

      // Validate types for XDR correctness
      expect(typeof escrow.invoice_id).toBe("string");
      expect(typeof escrow.admin).toBe("string");
      expect(typeof escrow.sme_address).toBe("string");
      expect(typeof escrow.amount).toBe("string"); // i128 as string
      expect(typeof escrow.funding_target).toBe("string");
      expect(typeof escrow.funded_amount).toBe("string");
      expect(typeof escrow.yield_bps).toBe("string"); // i64 as string
      expect(typeof escrow.maturity).toBe("string"); // u64 as string
      expect(typeof escrow.status).toBe("number");
    });

    it("should validate address formats in escrow", async () => {
      const escrow = await client.getEscrow();

      // Stellar addresses start with 'G' or 'C'
      expect(/^[GC][A-Z0-9]{55}$/.test(escrow.admin)).toBe(true);
      expect(/^[GC][A-Z0-9]{55}$/.test(escrow.sme_address)).toBe(true);
    });
  });

  // =========================================================================
  // Test: get_escrow_summary (read-only)
  // =========================================================================

  describe("get_escrow_summary", () => {
    it("should retrieve escrow summary with correct XDR encoding", async () => {
      stub.clearInvocationLog();

      const summary = await client.getEscrowSummary();

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const getCall = log[0];

      expect(getCall.method).toBe("simulate");
      expect(getCall.functionName).toBe("get_escrow_summary");
      expect(getCall.args).toHaveLength(0);

      expect(summary).toBeDefined();
      expect(summary.escrow).toBeDefined();
      expect(summary.has_maturity_lock).toBe(false);
      expect(summary.legal_hold).toBe(false);
    });

    it("should validate summary structure", async () => {
      const summary = await client.getEscrowSummary();

      // Check all required fields are present
      expect(summary.escrow).toBeDefined();
      expect(summary.has_maturity_lock).toBeDefined();
      expect(summary.legal_hold).toBeDefined();
      expect(summary.funding_close_snapshot).toBeDefined();
      expect(summary.unique_funder_count).toBeDefined();
      expect(summary.is_allowlist_active).toBeDefined();
      expect(summary.schema_version).toBeDefined();
      expect(summary.sme_collateral_commitment).toBeDefined();
      expect(summary.has_primary_attestation).toBeDefined();
      expect(summary.attestation_log_length).toBeDefined();
      expect(summary.dispute_pause_active).toBeDefined();
      expect(summary.dispute_pause_expiry).toBeDefined();
      expect(summary.allowlist_active).toBeDefined();

      // Validate types
      expect(typeof summary.has_maturity_lock).toBe("boolean");
      expect(typeof summary.legal_hold).toBe("boolean");
      expect(typeof summary.unique_funder_count).toBe("number");
      expect(typeof summary.is_allowlist_active).toBe("boolean");
      expect(typeof summary.schema_version).toBe("number");
      expect(typeof summary.has_primary_attestation).toBe("boolean");
      expect(typeof summary.attestation_log_length).toBe("number");
      expect(typeof summary.dispute_pause_active).toBe("boolean");
      expect(summary.dispute_pause_expiry).toBeNull();
      expect(typeof summary.allowlist_active).toBe("boolean");
    });

    it("should assert funding close snapshot structure", async () => {
      const summary = await client.getEscrowSummary();
      const snapshot = summary.funding_close_snapshot;

      expect(snapshot).toBeDefined();
      expect(typeof snapshot!.total_principal).toBe("string");
      expect(typeof snapshot!.funding_target).toBe("string");
      expect(typeof snapshot!.closed_at_ledger_timestamp).toBe("string");
      expect(typeof snapshot!.closed_at_ledger_sequence).toBe("number");
    });
  });

  // =========================================================================
  // Test: Metadata and Version Queries
  // =========================================================================

  describe("metadata queries", () => {
    it("should retrieve contract version", async () => {
      stub.clearInvocationLog();

      const version = await client.getVersion();

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_version");
      expect(version).toBe(SCHEMA_VERSION);
    });

    it("should retrieve interface version", async () => {
      stub.clearInvocationLog();

      const version = await client.getInterfaceVersion();

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_interface_version");
      expect(version).toBe(CONTRACT_INTERFACE_VERSION);
    });

    it("should retrieve funding token address", async () => {
      stub.clearInvocationLog();

      const token = await client.getFundingToken();

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_funding_token");
      expect(typeof token).toBe("string");
      expect(/^C/.test(token)).toBe(true); // Contract address
    });

    it("should retrieve treasury address", async () => {
      stub.clearInvocationLog();

      const treasury = await client.getTreasury();

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_treasury");
      expect(typeof treasury).toBe("string");
    });

    it("should retrieve legal hold status", async () => {
      stub.clearInvocationLog();

      const held = await client.getLegalHold();

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_legal_hold");
      expect(typeof held).toBe("boolean");
    });

    it("should retrieve legal hold status from the status entrypoint", async () => {
      stub.clearInvocationLog();

      const held = await client.getLegalHoldStatus();

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_legal_hold_status");
      expect(typeof held).toBe("boolean");
    });

    it("should retrieve unique funder count", async () => {
      stub.clearInvocationLog();

      const count = await client.getUniqueFunderCount();

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_unique_funder_count");
      expect(typeof count).toBe("number");
      expect(count).toBeGreaterThanOrEqual(0);
    });
  });

  // =========================================================================
  // Test: Investor-Specific Queries
  // =========================================================================

  describe("investor-specific queries", () => {
    it("should retrieve investor contribution", async () => {
      stub.clearInvocationLog();

      const contribution = await client.getContribution(INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_contribution");
      expect(log[0].args[0]).toBe(INVESTOR_1);
      expect(typeof contribution).toBe("string");
    });

    it("should compute investor payout preview as a string i128", async () => {
      stub.clearInvocationLog();
      stub.setResponse("compute_investor_payout", "1250000000");

      const payout = await client.computeInvestorPayout(INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      expect(log[0].functionName).toBe("compute_investor_payout");
      expect(log[0].args[0]).toBe(INVESTOR_1);
      expect(typeof payout).toBe("string");
      expect(payout).toBe("1250000000");
    });

    it("should retrieve distributed principal as a string i128", async () => {
      stub.clearInvocationLog();
      stub.setResponse("get_distributed_principal", "987654321");

      const distributed = await client.getDistributedPrincipal();

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      expect(log[0].functionName).toBe("get_distributed_principal");
      expect(log[0].args).toHaveLength(0);
      expect(typeof distributed).toBe("string");
      expect(distributed).toBe("987654321");
    });

    it("should retrieve investor yield basis points", async () => {
      stub.clearInvocationLog();

      const yieldBps = await client.getInvestorYieldBps(INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_investor_yield_bps");
      expect(log[0].args[0]).toBe(INVESTOR_1);
      expect(typeof yieldBps).toBe("string");
    });

    it("should retrieve investor claim not-before timestamp", async () => {
      stub.clearInvocationLog();

      const timestamp = await client.getInvestorClaimNotBefore(INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("get_investor_claim_not_before");
      expect(log[0].args[0]).toBe(INVESTOR_1);
      expect(typeof timestamp).toBe("string");
    });

    it("should check if investor has claimed", async () => {
      stub.clearInvocationLog();

      const claimed = await client.isInvestorClaimed(INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("is_investor_claimed");
      expect(log[0].args[0]).toBe(INVESTOR_1);
      expect(typeof claimed).toBe("boolean");
    });

    it("should check if investor has been refunded", async () => {
      stub.clearInvocationLog();

      const refunded = await client.isInvestorRefunded(INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log[0].functionName).toBe("is_investor_refunded");
      expect(log[0].args[0]).toBe(INVESTOR_1);
      expect(typeof refunded).toBe("boolean");
    });

    it("should retrieve the aggregated investor position", async () => {
      stub.clearInvocationLog();

      const position = await client.getInvestorPosition(INVESTOR_1);

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const getCall = log[0];

      expect(getCall.method).toBe("simulate");
      expect(getCall.functionName).toBe("get_investor_position");
      expect(getCall.args[0]).toBe(INVESTOR_1);

      expect(position).toBeDefined();
      expect(position.contribution).toBe("500000000");
      expect(position.yield_bps).toBe("800");
      expect(position.claim_not_before).toBe("1700000000");
      expect(position.claimed).toBe(false);
      expect(position.refunded).toBe(false);
    });
  });

  // =========================================================================
  // Test: Full Workflow (Integration Scenario)
  // =========================================================================

  describe("full workflow integration", () => {
    it("should execute init → fund → settle → claim workflow", async () => {
      stub.clearInvocationLog();

      // Step 1: Initialize escrow
      const initParams: InitParams = {
        admin: ADMIN,
        invoice_id: "WORKFLOW_TEST_001",
        sme_address: SME,
        amount: toBaseUnits("100000", 7),
        yield_bps: "800",
        maturity: "1700000000",
        funding_token: FUNDING_TOKEN,
        registry: REGISTRY,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      const escrow1 = await client.init(initParams);
      expect(escrow1.status).toBe(EscrowStatus.Open);

      // Step 2: Fund by investor 1
      await client.fund(INVESTOR_1, toBaseUnits("50000", 7));

      // Step 3: Fund by investor 2
      await client.fund(INVESTOR_2, toBaseUnits("50000", 7));

      // Step 4: Settle (after maturity)
      stub.setResponse("settle", {
        ...escrow1,
        status: EscrowStatus.Settled,
        funded_amount: toBaseUnits("100000", 7),
      });

      const settledEscrow = await client.settle();
      expect(settledEscrow.status).toBe(EscrowStatus.Settled);

      // Step 5: Investors claim payout
      await client.claimInvestorPayout(INVESTOR_1);
      await client.claimInvestorPayout(INVESTOR_2);

      // Step 6: Verify escrow summary (mock returns default Funded status, which is fine for this integration test)
      const summary = await client.getEscrowSummary();
      expect(summary.escrow).toBeDefined();
      expect(summary.escrow.invoice_id).toBe("INV001");

      // Verify all method calls were logged
      const log = stub.getInvocationLog();
      expect(log.length).toBeGreaterThanOrEqual(6);

      // Verify sequence of operations
      expect(log[0].functionName).toBe("init");
      expect(log[1].functionName).toBe("fund");
      expect(log[2].functionName).toBe("fund");
      expect(log[3].functionName).toBe("settle");
      expect(log[4].functionName).toBe("claim_investor_payout");
      expect(log[5].functionName).toBe("claim_investor_payout");
    });
  });

  // =========================================================================
  // Test: XDR Encoding Validation
  // =========================================================================

  describe("XDR encoding assertions", () => {
    it("should use string encoding for i128 numeric types", async () => {
      const params: InitParams = {
        admin: ADMIN,
        invoice_id: "XDR_TEST_001",
        sme_address: SME,
        amount: "123456789012345", // Large i128
        yield_bps: "800",
        maturity: "0",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: "100000000",
        max_unique_investors: null,
        max_per_investor: "99999999999",
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      await client.init(params);

      const log = stub.getInvocationLog();
      const args = log[0].args;

      // Verify numeric types are strings (XDR encoded as strings for precision)
      expect(typeof args[3]).toBe("string"); // amount
      expect(typeof args[4]).toBe("string"); // yield_bps
      expect(typeof args[5]).toBe("string"); // maturity
      expect(typeof args[10]).toBe("string"); // min_contribution
      expect(typeof args[12]).toBe("string"); // max_per_investor
    });

    it("should use numeric encoding for u32 types", async () => {
      const params: InitParams = {
        admin: ADMIN,
        invoice_id: "XDR_TEST_002",
        sme_address: SME,
        amount: toBaseUnits("100000", 7),
        yield_bps: "800",
        maturity: "0",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: 250, // u32
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      await client.init(params);

      const log = stub.getInvocationLog();
      const args = log[0].args;

      expect(typeof args[11]).toBe("number"); // max_unique_investors
      expect(args[11]).toBe(250);
    });

    it("should encode Address types correctly", async () => {
      const params: InitParams = {
        admin: ADMIN,
        invoice_id: "XDR_TEST_003",
        sme_address: SME,
        amount: toBaseUnits("100000", 7),
        yield_bps: "800",
        maturity: "0",
        funding_token: FUNDING_TOKEN,
        registry: REGISTRY,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      await client.init(params);

      const log = stub.getInvocationLog();
      const args = log[0].args;

      // Addresses should be strings starting with 'G' or 'C'
      expect(typeof args[0]).toBe("string");
      expect(/^[GC]/.test(args[0] as string)).toBe(true);
      expect(typeof args[2]).toBe("string");
      expect(/^[GC]/.test(args[2] as string)).toBe(true);
    });

    it("should encode YieldTier arrays correctly", async () => {
      const tiers: YieldTier[] = [
        { min_lock_secs: "2592000", yield_bps: "900" },
        { min_lock_secs: "7776000", yield_bps: "1000" },
      ];

      const params: InitParams = {
        admin: ADMIN,
        invoice_id: "XDR_TEST_004",
        sme_address: SME,
        amount: toBaseUnits("100000", 7),
        yield_bps: "800",
        maturity: "0",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: tiers,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      await client.init(params);

      const log = stub.getInvocationLog();
      const args = log[0].args;

      expect(Array.isArray(args[9])).toBe(true);
      expect((args[9] as YieldTier[]).length).toBe(2);
      expect((args[9] as YieldTier[])[0].min_lock_secs).toBe("2592000");
      expect((args[9] as YieldTier[])[1].yield_bps).toBe("1000");
    });

    it("should handle null Option types correctly", async () => {
      const params: InitParams = {
        admin: ADMIN,
        invoice_id: "XDR_TEST_005",
        sme_address: SME,
        amount: toBaseUnits("100000", 7),
        yield_bps: "800",
        maturity: "0",
        funding_token: FUNDING_TOKEN,
        registry: null,
        treasury: TREASURY,
        yield_tiers: null,
        min_contribution: null,
        max_unique_investors: null,
        max_per_investor: null,
        legal_hold_clear_delay: null,
        funding_deadline: null,
        yield_slippage_threshold: null,
      };

      await client.init(params);

      const log = stub.getInvocationLog();
      const args = log[0].args;

      expect(args[7]).toBeNull(); // registry: Option<Address>
      expect(args[9]).toBeNull(); // yield_tiers: Option<Vec<YieldTier>>
      expect(args[10]).toBeNull(); // min_contribution: Option<i128>
      expect(args[12]).toBeNull(); // max_per_investor: Option<i128>
    });
  });

  // =========================================================================
  // Test: Error Handling
  // =========================================================================

  describe("error handling", () => {
    it("should handle unexpected function calls gracefully", async () => {
      // This should throw or handle gracefully
      await expect(stub.invoke(CONTRACT_ID, "unknown_function", [])).rejects.toThrow();
    });

    it("should properly propagate simulate errors", async () => {
      await expect(stub.simulate(CONTRACT_ID, "nonexistent_query", [])).rejects.toThrow();
    });
  });
});

test("appendAttestationDigest invokes the contract with a 32-byte digest", async () => {
  const rpc: SorobanRpcClient = {
    invoke: jest.fn().mockResolvedValue(undefined),
    simulate: jest.fn(),
    getLedger: jest.fn(),
  };
  const client = new EscrowClient({ rpcUrl: "http://localhost", networkPassphrase: "test" }, rpc);
  const digest = new Uint8Array(32).fill(7);

  await client.appendAttestationDigest(digest);

  expect(rpc.invoke).toHaveBeenCalledWith("", "append_attestation_digest", [digest], undefined);
});

test("appendAttestationDigest rejects invalid digest lengths before invoking RPC", async () => {
  const rpc: SorobanRpcClient = {
    invoke: jest.fn(),
    simulate: jest.fn(),
    getLedger: jest.fn(),
  };
  const client = new EscrowClient({ rpcUrl: "http://localhost", networkPassphrase: "test" }, rpc);

  await expect(client.appendAttestationDigest(new Uint8Array(31))).rejects.toBeInstanceOf(ValidationError);
  expect(rpc.invoke).not.toHaveBeenCalled();
});

test("appendAttestationDigest passes rate-limit errors through unchanged", async () => {
  const rateLimitError = new Error("rate limit exceeded");
  const rpc: SorobanRpcClient = {
    invoke: jest.fn().mockRejectedValue(rateLimitError),
    simulate: jest.fn(),
    getLedger: jest.fn(),
  };
  const client = new EscrowClient({ rpcUrl: "http://localhost", networkPassphrase: "test" }, rpc);

  await expect(client.appendAttestationDigest(new Uint8Array(32))).rejects.toBe(rateLimitError);
});


// =========================================================================
// Test: Health Check Methods
// =========================================================================

describe("health check methods", () => {
  let client: EscrowClient;
  let stub: StubSorobanClient;

  const CONTRACT_ID = "CBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

  beforeEach(() => {
    stub = new StubSorobanClient();
    client = new EscrowClient(
      {
        rpcUrl: "http://localhost:8000/soroban/rpc",
        networkPassphrase: "Test SDF Network ; September 2015",
        contractId: CONTRACT_ID,
      },
      stub,
    );
  });

  describe("checkEscrowHealth", () => {
    it("should retrieve quick health check with correct parameters for XDR", async () => {
      stub.clearInvocationLog();

      // Mock response for check_escrow_health
      stub.setResponse("check_escrow_health", {
        warning_type: "healthy",
        funded_ratio_bps: "10000", // 100% funded
        time_to_maturity_secs: "7776000", // 90 days remaining
      });

      const health = await client.checkEscrowHealth();

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const healthCall = log[0];

      expect(healthCall.method).toBe("simulate");
      expect(healthCall.functionName).toBe("check_escrow_health");
      expect(healthCall.args).toHaveLength(0); // No parameters

      expect(health).toBeDefined();
      expect(health.warning_type).toBe("healthy");
      expect(health.funded_ratio_bps).toBe("10000");
      expect(health.time_to_maturity_secs).toBe("7776000");
    });

    it("should validate checkEscrowHealth return type structure", async () => {
      stub.setResponse("check_escrow_health", {
        warning_type: "underfunded",
        funded_ratio_bps: "5000", // 50% funded
        time_to_maturity_secs: "2592000", // 30 days remaining
      });

      const health = await client.checkEscrowHealth();

      // Validate all required fields exist and have correct types
      expect(typeof health.warning_type).toBe("string");
      expect(typeof health.funded_ratio_bps).toBe("string");
      expect(typeof health.time_to_maturity_secs).toBe("string");

      // Validate value ranges
      expect(["healthy", "underfunded", "approaching_maturity", "maturity_passed", "legal_hold"]).toContain(health.warning_type);
      // funded_ratio_bps should be 0-10000 (0-100%)
      const ratio = parseInt(health.funded_ratio_bps);
      expect(ratio).toBeGreaterThanOrEqual(0);
      expect(ratio).toBeLessThanOrEqual(10000);
    });

    it("should handle underfunded warning", async () => {
      stub.setResponse("check_escrow_health", {
        warning_type: "underfunded",
        funded_ratio_bps: "3000", // 30% funded
        time_to_maturity_secs: "86400", // 1 day remaining
      });

      const health = await client.checkEscrowHealth();

      expect(health.warning_type).toBe("underfunded");
      expect(parseInt(health.funded_ratio_bps)).toBeLessThan(10000);
    });

    it("should handle maturity approaching warning", async () => {
      stub.setResponse("check_escrow_health", {
        warning_type: "approaching_maturity",
        funded_ratio_bps: "8000", // 80% funded
        time_to_maturity_secs: "259200", // 3 days remaining
      });

      const health = await client.checkEscrowHealth();

      expect(health.warning_type).toBe("approaching_maturity");
      const timeToMaturity = parseInt(health.time_to_maturity_secs);
      expect(timeToMaturity).toBeLessThan(604800); // Less than 7 days
    });

    it("should handle maturity passed warning", async () => {
      stub.setResponse("check_escrow_health", {
        warning_type: "maturity_passed",
        funded_ratio_bps: "10000",
        time_to_maturity_secs: "-86400", // -1 day (overdue)
      });

      const health = await client.checkEscrowHealth();

      expect(health.warning_type).toBe("maturity_passed");
      expect(parseInt(health.time_to_maturity_secs)).toBeLessThan(0);
    });

    it("should handle legal hold warning", async () => {
      stub.setResponse("check_escrow_health", {
        warning_type: "legal_hold",
        funded_ratio_bps: "10000",
        time_to_maturity_secs: "2592000",
      });

      const health = await client.checkEscrowHealth();

      expect(health.warning_type).toBe("legal_hold");
    });
  });

  describe("getEscrowHealth", () => {
    it("should retrieve full health diagnostic with correct parameters for XDR", async () => {
      stub.clearInvocationLog();

      // Mock response for get_escrow_health
      stub.setResponse("get_escrow_health", {
        warning_type: "healthy",
        funded_ratio_bps: "10000",
        time_to_maturity_secs: "7776000",
        status_label: "Funded",
        is_maturity_locked: false,
        is_legal_held: false,
        unique_funder_count: 5,
      });

      const fullHealth = await client.getEscrowHealth();

      const log = stub.getInvocationLog();
      expect(log).toHaveLength(1);
      const healthCall = log[0];

      expect(healthCall.method).toBe("simulate");
      expect(healthCall.functionName).toBe("get_escrow_health");
      expect(healthCall.args).toHaveLength(0);

      expect(fullHealth).toBeDefined();
      expect(fullHealth.warning_type).toBe("healthy");
      expect(fullHealth.funded_ratio_bps).toBe("10000");
      expect(fullHealth.time_to_maturity_secs).toBe("7776000");
      expect(fullHealth.status_label).toBe("Funded");
      expect(fullHealth.is_maturity_locked).toBe(false);
      expect(fullHealth.is_legal_held).toBe(false);
      expect(fullHealth.unique_funder_count).toBe(5);
    });

    it("should validate getEscrowHealth return type structure", async () => {
      stub.setResponse("get_escrow_health", {
        warning_type: "underfunded",
        funded_ratio_bps: "4500",
        time_to_maturity_secs: "1296000",
        status_label: "Open",
        is_maturity_locked: false,
        is_legal_held: false,
        unique_funder_count: 2,
      });

      const fullHealth = await client.getEscrowHealth();

      // Validate all required fields exist with correct types
      expect(typeof fullHealth.warning_type).toBe("string");
      expect(typeof fullHealth.funded_ratio_bps).toBe("string");
      expect(typeof fullHealth.time_to_maturity_secs).toBe("string");
      expect(typeof fullHealth.status_label).toBe("string");
      expect(typeof fullHealth.is_maturity_locked).toBe("boolean");
      expect(typeof fullHealth.is_legal_held).toBe("boolean");
      expect(typeof fullHealth.unique_funder_count).toBe("number");

      // Validate status label is one of the expected values
      expect(["Open", "Funded", "Settled", "Withdrawn", "Cancelled"]).toContain(fullHealth.status_label);
    });

    it("should handle full health diagnostic with legal hold", async () => {
      stub.setResponse("get_escrow_health", {
        warning_type: "legal_hold",
        funded_ratio_bps: "7500",
        time_to_maturity_secs: "5184000",
        status_label: "Funded",
        is_maturity_locked: true,
        is_legal_held: true,
        unique_funder_count: 10,
      });

      const fullHealth = await client.getEscrowHealth();

      expect(fullHealth.warning_type).toBe("legal_hold");
      expect(fullHealth.is_legal_held).toBe(true);
      expect(fullHealth.is_maturity_locked).toBe(true);
      expect(fullHealth.unique_funder_count).toBe(10);
    });

    it("should handle full health diagnostic in settled state", async () => {
      stub.setResponse("get_escrow_health", {
        warning_type: "healthy",
        funded_ratio_bps: "10000",
        time_to_maturity_secs: "0",
        status_label: "Settled",
        is_maturity_locked: false,
        is_legal_held: false,
        unique_funder_count: 3,
      });

      const fullHealth = await client.getEscrowHealth();

      expect(fullHealth.status_label).toBe("Settled");
      expect(fullHealth.unique_funder_count).toBe(3);
    });

    it("should include all investor metrics in full health check", async () => {
      stub.setResponse("get_escrow_health", {
        warning_type: "healthy",
        funded_ratio_bps: "8500",
        time_to_maturity_secs: "3888000",
        status_label: "Funded",
        is_maturity_locked: false,
        is_legal_held: false,
        unique_funder_count: 15,
      });

      const fullHealth = await client.getEscrowHealth();

      // Full health includes investor count which is useful for operators
      expect(fullHealth.unique_funder_count).toBeGreaterThan(0);
    });
  });

  describe("health check comparison", () => {
    it("should show that check_escrow_health is a subset of get_escrow_health", async () => {
      const quickResponse = {
        warning_type: "underfunded",
        funded_ratio_bps: "6000",
        time_to_maturity_secs: "1814400",
      };

      const fullResponse = {
        ...quickResponse,
        status_label: "Open",
        is_maturity_locked: false,
        is_legal_held: false,
        unique_funder_count: 8,
      };

      stub.setResponse("check_escrow_health", quickResponse);
      stub.setResponse("get_escrow_health", fullResponse);

      stub.clearInvocationLog();

      const quick = await client.checkEscrowHealth();
      const full = await client.getEscrowHealth();

      // Both should have the common fields
      expect(quick.warning_type).toBe(full.warning_type);
      expect(quick.funded_ratio_bps).toBe(full.funded_ratio_bps);
      expect(quick.time_to_maturity_secs).toBe(full.time_to_maturity_secs);

      // Full health has additional diagnostic fields
      expect(full.status_label).toBeDefined();
      expect(full.is_maturity_locked).toBeDefined();
      expect(full.is_legal_held).toBeDefined();
      expect(full.unique_funder_count).toBeDefined();

      // Verify both methods were called independently
      const log = stub.getInvocationLog();
      expect(log).toHaveLength(2);
      expect(log[0].functionName).toBe("check_escrow_health");
      expect(log[1].functionName).toBe("get_escrow_health");
    });
  });
});

test("setLegalHold passes both active and reason arguments", async () => {
  const mockInvoke = jest.fn().mockResolvedValue(undefined);
  const rpc: SorobanRpcClient = {
    invoke: mockInvoke,
    simulate: jest.fn(),
    getLedger: jest.fn(),
  };
  const client = new EscrowClient({ rpcUrl: "http://localhost", networkPassphrase: "test" }, rpc);
  client.setContractId("CTEST123");

  // Test activating legal hold with reason
  await client.setLegalHold(true, "Compliance investigation", "source123");
  expect(mockInvoke).toHaveBeenCalledWith(
    "CTEST123",
    "set_legal_hold",
    [true, "Compliance investigation"],
    "source123"
  );

  // Test deactivating legal hold with empty reason
  mockInvoke.mockClear();
  await client.setLegalHold(false, "", "source456");
  expect(mockInvoke).toHaveBeenCalledWith(
    "CTEST123",
    "set_legal_hold",
    [false, ""],
    "source456"
  );
});

test("setLegalHold rejects empty reason when active=true", async () => {
  const rpc: SorobanRpcClient = {
    invoke: jest.fn(),
    simulate: jest.fn(),
    getLedger: jest.fn(),
  };
  const client = new EscrowClient({ rpcUrl: "http://localhost", networkPassphrase: "test" }, rpc);

  // Should throw when activating with empty reason
  await expect(client.setLegalHold(true, "")).rejects.toThrow(
    "reason must be non-empty when legal hold is activated"
  );

  // Should throw when activating with whitespace-only reason
  await expect(client.setLegalHold(true, "   ")).rejects.toThrow(
    "reason must be non-empty when legal hold is activated"
  );

  // Should not throw when deactivating with empty reason
  const mockInvoke = jest.fn().mockResolvedValue(undefined);
  rpc.invoke = mockInvoke;
  client.setContractId("CTEST123");
  await client.setLegalHold(false, "");
  expect(mockInvoke).toHaveBeenCalled();
});
