/**
 * Full lifecycle example for the Escrow SDK.
 *
 * This script walks through the complete escrow lifecycle against a real
 * Soroban RPC endpoint:
 *
 *   1. validateInterfaceVersion  (read-only sanity check)
 *   2. init                      (as ADMIN)
 *   3. fund                      (as INVESTOR)
 *   4. settle                    (as SME)
 *   5. claimInvestorPayout       (as INVESTOR)
 *
 * ---------------------------------------------------------------------------
 * Required environment variables
 * ---------------------------------------------------------------------------
 *   SOROBAN_RPC_URL       Soroban RPC endpoint, e.g. https://soroban-testnet.stellar.org
 *   NETWORK_PASSPHRASE    Stellar network passphrase, e.g. "Test SDF Network ; September 2015"
 *   ESCROW_CONTRACT_ID    Deployed escrow contract id (C...)
 *   ADMIN_SECRET          Secret key (S...) for the escrow admin / SME owner
 *   SME_SECRET            Secret key (S...) for the SME (invoice originator)
 *   INVESTOR_SECRET       Secret key (S...) for the investor / funder
 *
 * ---------------------------------------------------------------------------
 * Optional environment variables
 * ---------------------------------------------------------------------------
 *   FUNDING_TOKEN         Token contract id used for funding (defaults to native SAC)
 *   TREASURY              Treasury address for fees (defaults to admin address)
 *   INVOICE_ID            Invoice identifier string (defaults to "INV-EXAMPLE-001")
 *   AMOUNT                Funding amount in base units (defaults to "1000000000")
 *   YIELD_BPS             Yield in basis points (defaults to "500")
 *   MATURITY              Maturity as unix seconds (defaults to now + 30 days)
 *
 * ---------------------------------------------------------------------------
 * Run command
 * ---------------------------------------------------------------------------
 *   SOROBAN_RPC_URL=... \
 *   NETWORK_PASSPHRASE=... \
 *   ESCROW_CONTRACT_ID=... \
 *   ADMIN_SECRET=... \
 *   SME_SECRET=... \
 *   INVESTOR_SECRET=... \
 *   npx ts-node sdk-ts/examples/full-lifecycle.ts
 *
 * NOTE: Secret values are never logged. Only public addresses (G...) or
 * redacted placeholders are printed.
 */

import {
  EscrowClient,
  ESCROW_STATUS_LABELS,
  ESCROW_ERROR_LABELS,
  classifyError,
  fromBaseUnits,
  type SorobanRpcClient,
  type EscrowStatus,
} from '../src';

// ---------------------------------------------------------------------------
// Environment handling
// ---------------------------------------------------------------------------

const REQUIRED_ENV = [
  'SOROBAN_RPC_URL',
  'NETWORK_PASSPHRASE',
  'ESCROW_CONTRACT_ID',
  'ADMIN_SECRET',
  'SME_SECRET',
  'INVESTOR_SECRET',
] as const;

type RequiredEnvName = (typeof REQUIRED_ENV)[number];

function readRequiredEnv(): Record<RequiredEnvName, string> {
  const missing: string[] = [];
  const values: Partial<Record<RequiredEnvName, string>> = {};

  for (const name of REQUIRED_ENV) {
    const value = process.env[name];
    if (value === undefined || value.trim() === '') {
      missing.push(name);
    } else {
      values[name] = value;
    }
  }

  if (missing.length > 0) {
    console.error('Missing required environment variable(s):');
    for (const name of missing) {
      console.error(`  - ${name}`);
    }
    console.error(
      '\nSee the doc comment at the top of this file for the full list of ' +
        'required and optional variables, and the run command.',
    );
    process.exit(1);
  }

  return values as Record<RequiredEnvName, string>;
}

function readOptionalEnv(name: string, fallback: string): string {
  const value = process.env[name];
  return value === undefined || value.trim() === '' ? fallback : value;
}

// ---------------------------------------------------------------------------
// Redaction helpers (never log secrets)
// ---------------------------------------------------------------------------

/**
 * Redact a secret key so it can be safely printed. Only the first 4 and last
 * 4 characters are shown; the middle is replaced with asterisks.
 */
function redactSecret(secret: string): string {
  if (secret.length <= 8) {
    return '****';
  }
  return `${secret.slice(0, 4)}${'*'.repeat(secret.length - 8)}${secret.slice(-4)}`;
}

// ---------------------------------------------------------------------------
// Soroban RPC adapter
// ---------------------------------------------------------------------------

/**
 * Minimal SorobanRpcClient adapter.
 *
 * The SDK's `SorobanRpcClient` interface requires `invoke`, `simulate`, and
 * `getLedger`. This example ships a clearly-marked placeholder that throws
 * with instructions, because wiring a real RPC client depends on the exact
 * version of `@stellar/stellar-sdk` you are using.
 *
 * To make this example run end-to-end, replace the bodies below with calls
 * into your Soroban RPC client, e.g.:
 *
 *   import { SorobanRpc, TransactionBuilder, Networks } from '@stellar/stellar-sdk';
 *   const server = new SorobanRpc.Server(rpcUrl);
 *   // invoke: build + sign + send + poll
 *   // simulate: server.simulateTransaction(tx)
 *   // getLedger: server.getLatestLedger()
 *
 * The adapter is intentionally constructed with the RPC URL and network
 * passphrase so a real implementation has everything it needs.
 */
class PlaceholderSorobanRpcClient implements SorobanRpcClient {
  constructor(
    private readonly rpcUrl: string,
    private readonly networkPassphrase: string,
  ) {}

  private notImplemented(method: string): never {
    throw new Error(
      `SorobanRpcClient.${method} is not implemented in this example.\n` +
        `Replace PlaceholderSorobanRpcClient with a real adapter that talks to ` +
        `the Soroban RPC endpoint (${this.rpcUrl}) on network ` +
        `"${this.networkPassphrase}".\n` +
        `See the doc comment above the class for a sketch of a real ` +
        `implementation using @stellar/stellar-sdk.`,
    );
  }

  async invoke(..._args: unknown[]): Promise<unknown> {
    return this.notImplemented('invoke');
  }

  async simulate(..._args: unknown[]): Promise<unknown> {
    return this.notImplemented('simulate');
  }

  async getLedger(..._args: unknown[]): Promise<unknown> {
    return this.notImplemented('getLedger');
  }
}

// ---------------------------------------------------------------------------
// Step runner
// ---------------------------------------------------------------------------

async function runStep<T>(
  label: string,
  fn: () => Promise<T>,
): Promise<T | undefined> {
  console.log(`\n=== ${label} ===`);
  try {
    const result = await fn();
    console.log(`[ok] ${label}`);
    return result;
  } catch (err) {
    const classified = classifyError(err);
    const code = classified?.code;
    const codeLabel =
      code !== undefined && code in ESCROW_ERROR_LABELS
        ? ESCROW_ERROR_LABELS[code as keyof typeof ESCROW_ERROR_LABELS]
        : 'UNKNOWN_ERROR';

    console.error(`[fail] ${label}`);
    console.error(`  code:    ${code ?? 'n/a'} (${codeLabel})`);
    console.error(`  message: ${classified?.message ?? String(err)}`);
    if (classified?.details !== undefined) {
      console.error(`  details: ${JSON.stringify(classified.details)}`);
    }
    return undefined;
  }
}

function printStatus(label: string, status: EscrowStatus): void {
  const statusLabel =
    status in ESCROW_STATUS_LABELS
      ? ESCROW_STATUS_LABELS[status as keyof typeof ESCROW_STATUS_LABELS]
      : String(status);
  console.log(`  ${label}: ${status} (${statusLabel})`);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  const env = readRequiredEnv();

  const fundingToken = readOptionalEnv('FUNDING_TOKEN', '');
  const treasury = readOptionalEnv('TREASURY', '');
  const invoiceId = readOptionalEnv('INVOICE_ID', 'INV-EXAMPLE-001');
  const amount = readOptionalEnv('AMOUNT', '1000000000');
  const yieldBps = readOptionalEnv('YIELD_BPS', '500');
  const maturity = readOptionalEnv(
    'MATURITY',
    String(Math.floor(Date.now() / 1000) + 30 * 24 * 60 * 60),
  );

  console.log('Escrow full-lifecycle example');
  console.log('-----------------------------');
  console.log(`  rpc url:        ${env.SOROBAN_RPC_URL}`);
  console.log(`  network:        ${env.NETWORK_PASSPHRASE}`);
  console.log(`  contract id:    ${env.ESCROW_CONTRACT_ID}`);
  console.log(`  admin secret:   ${redactSecret(env.ADMIN_SECRET)}`);
  console.log(`  sme secret:     ${redactSecret(env.SME_SECRET)}`);
  console.log(`  investor secret:${redactSecret(env.INVESTOR_SECRET)}`);
  console.log(`  funding token:  ${fundingToken || '(default / native)'}`);
  console.log(`  treasury:       ${treasury || '(default / admin)'}`);
  console.log(`  invoice id:     ${invoiceId}`);
  console.log(`  amount:         ${amount} (${fromBaseUnits(amount)} units)`);
  console.log(`  yield bps:      ${yieldBps}`);
  console.log(`  maturity:       ${maturity}`);

  const rpc = new PlaceholderSorobanRpcClient(
    env.SOROBAN_RPC_URL,
    env.NETWORK_PASSPHRASE,
  );

  const client = new EscrowClient({
    rpc,
    contractId: env.ESCROW_CONTRACT_ID,
    networkPassphrase: env.NETWORK_PASSPHRASE,
  });

  // 1. validateInterfaceVersion -------------------------------------------------
  const versionOk = await runStep('validateInterfaceVersion', async () => {
    const result = await client.validateInterfaceVersion();
    console.log(`  interface version check: ${JSON.stringify(result)}`);
    return result;
  });
  if (versionOk === undefined) {
    console.error('\nAborting: interface version validation failed.');
    process.exit(1);
  }

  // 2. init (as admin) ----------------------------------------------------------
  const initResult = await runStep('init (admin)', async () => {
    const result = await client.init({
      signer: env.ADMIN_SECRET,
      invoiceId,
      amount,
      yieldBps,
      maturity,
      fundingToken: fundingToken || undefined,
      treasury: treasury || undefined,
    });
    console.log(`  init result: ${JSON.stringify(result)}`);
    return result;
  });
  if (initResult === undefined) {
    console.error('\nAborting: init failed.');
    process.exit(1);
  }

  const afterInit = await runStep('status after init', async () => {
    const status = await client.getStatus();
    printStatus('status', status);
    return status;
  });
  if (afterInit === undefined) {
    console.error('\nAborting: could not read status after init.');
    process.exit(1);
  }

  // 3. fund (as investor) -------------------------------------------------------
  const fundResult = await runStep('fund (investor)', async () => {
    const result = await client.fund({
      signer: env.INVESTOR_SECRET,
      amount,
    });
    console.log(`  fund result: ${JSON.stringify(result)}`);
    return result;
  });
  if (fundResult === undefined) {
    console.error('\nAborting: fund failed.');
    process.exit(1);
  }

  const afterFund = await runStep('status after fund', async () => {
    const status = await client.getStatus();
    printStatus('status', status);
    return status;
  });
  if (afterFund === undefined) {
    console.error('\nAborting: could not read status after fund.');
    process.exit(1);
  }

  // 4. settle (as SME) ----------------------------------------------------------
  const settleResult = await runStep('settle (sme)', async () => {
    const result = await client.settle({
      signer: env.SME_SECRET,
    });
    console.log(`  settle result: ${JSON.stringify(result)}`);
    return result;
  });
  if (settleResult === undefined) {
    console.error('\nAborting: settle failed.');
    process.exit(1);
  }

  const afterSettle = await runStep('status after settle', async () => {
    const status = await client.getStatus();
    printStatus('status', status);
    return status;
  });
  if (afterSettle === undefined) {
    console.error('\nAborting: could not read status after settle.');
    process.exit(1);
  }

  // 5. claimInvestorPayout (as investor) ---------------------------------------
  const claimResult = await runStep('claimInvestorPayout (investor)', async () => {
    const result = await client.claimInvestorPayout({
      signer: env.INVESTOR_SECRET,
    });
    console.log(`  claim result: ${JSON.stringify(result)}`);
    return result;
  });
  if (claimResult === undefined) {
    console.error('\nAborting: claimInvestorPayout failed.');
    process.exit(1);
  }

  const afterClaim = await runStep('status after claim', async () => {
    const status = await client.getStatus();
    printStatus('status', status);
    return status;
  });
  if (afterClaim === undefined) {
    console.error('\nAborting: could not read status after claim.');
    process.exit(1);
  }

  console.log('\nFull lifecycle completed successfully.');
}

main().catch((err) => {
  const classified = classifyError(err);
  const code = classified?.code;
  const codeLabel =
    code !== undefined && code in ESCROW_ERROR_LABELS
      ? ESCROW_ERROR_LABELS[code as keyof typeof ESCROW_ERROR_LABELS]
      : 'UNKNOWN_ERROR';

  console.error('\nUnhandled error in full-lifecycle example:');
  console.error(`  code:    ${code ?? 'n/a'} (${codeLabel})`);
  console.error(`  message: ${classified?.message ?? String(err)}`);
  if (classified?.details !== undefined) {
    console.error(`  details: ${JSON.stringify(classified.details)}`);
  }
  process.exit(1);
});