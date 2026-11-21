import { EscrowClient, type SorobanRpcClient } from "../src/client";

describe("EscrowClient.batchClaimInvestorPayouts", () => {
  it("invokes the batch claim and returns the claimed count", async () => {
    const invoke = jest.fn().mockResolvedValue(2);
    const rpc: SorobanRpcClient = {
      invoke,
      simulate: jest.fn(),
      getLedger: jest.fn(),
    };
    const client = new EscrowClient(
      { rpcUrl: "https://rpc.example", networkPassphrase: "test", contractId: "contract" },
      rpc,
    );

    await expect(client.batchClaimInvestorPayouts(["investor-a", "investor-b"], "source"))
      .resolves.toBe(2);
    expect(invoke).toHaveBeenCalledWith(
      "contract",
      "batch_claim_investor_payouts",
      [["investor-a", "investor-b"]],
      "source",
    );
  });

  it("rejects an empty investor batch before invoking the RPC", async () => {
    const invoke = jest.fn();
    const rpc: SorobanRpcClient = {
      invoke,
      simulate: jest.fn(),
      getLedger: jest.fn(),
    };
    const client = new EscrowClient(
      { rpcUrl: "https://rpc.example", networkPassphrase: "test", contractId: "contract" },
      rpc,
    );

    await expect(client.batchClaimInvestorPayouts([])).rejects.toThrow("investors must not be empty");
    expect(invoke).not.toHaveBeenCalled();
  });
});