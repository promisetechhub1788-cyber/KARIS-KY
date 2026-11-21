/// Upper bound on [`LiquifactEscrow::append_attestation_digest`] entries to keep storage bounded.
/// Revocation via [`LiquifactEscrow::revoke_attestation_digest`] does not consume a slot.
pub const MAX_ATTESTATION_APPEND_ENTRIES: u32 = 32;

/// Upper bound on reinvestment audit log entries to keep storage bounded.
pub const MAX_REINVESTMENT_AUDIT_ENTRIES: u32 = 100;

/// Upper bound on [`DataKey::InvestorFundingHistory`] entries per investor to keep
/// per-address persistent storage bounded. Once reached, further checkpoints are not
/// appended (existing history is preserved; see [`LiquifactEscrow::get_investor_history`]).
pub const MAX_INVESTOR_HISTORY_ENTRIES: u32 = 128;

/// Upper bound on [`DataKey::AdminRoles`] entries to keep instance storage/CPU bounded.
pub const MAX_ADMIN_ROLES: u32 = 16;

/// Upper bound on [`MultisigPolicy::signers`] to keep instance storage/CPU bounded.
pub const MAX_MULTISIG_SIGNERS: u32 = 16;

/// Upper bound on [`MultisigPolicy::operations`] to keep instance storage/CPU bounded.
pub const MAX_MULTISIG_OPERATIONS: u32 = 16;

/// Upper bound on batch allowlist mutation entries to keep storage/CPU bounded.
/// Mirrors the spirit of `MAX_ATTESTATION_APPEND_ENTRIES` to limit per-call work.
pub const MAX_INVESTOR_ALLOWLIST_BATCH: u32 = 32;

/// Upper bound on [`LiquifactEscrow::fund_batch`] entries to keep storage/CPU bounded.
/// Mirrors the spirit of `MAX_ATTESTATION_APPEND_ENTRIES` to limit per-call work.
pub const MAX_FUND_BATCH: u32 = 50;

/// Upper bound on [`LiquifactEscrow::batch_claim_investor_payouts`] entries per call, to keep
/// storage/CPU bounded. Mirrors the spirit of `MAX_ATTESTATION_APPEND_ENTRIES`.
pub const MAX_BATCH_CLAIM: u32 = 50;

/// Number of bucketing shards for investor contribution aggregates (256 buckets).
///
/// Buckets partition investor addresses by `hash(address) % INVESTOR_BUCKET_COUNT`
/// so that dashboards can compute total funded ≈ Σ buckets in O(256) instance reads
/// instead of iterating over all per‑investor persistent `InvestorContribution` keys.
///
/// The bucket count is chosen as a power‑of‑two for fast modulo with bit‑masking
/// and provides a good balance between write overhead (+1 instance write per fund)
/// and read granularity for aggregate queries.
pub const INVESTOR_BUCKET_COUNT: u32 = 256;

/// Upper bound on [`LiquifactEscrow::sweep_terminal_dust`] per call (base units of the funding token).
///
/// Caps blast radius if instrumentation mis-estimates “dust”; tune per asset decimals off-chain.
pub const MAX_DUST_SWEEP_AMOUNT: i128 = 100_000_000;

/// Maximum allowed dispute pause duration in seconds (14 days).
///
/// Guards against indefinite admin locks and ensures disputes can be escalated to governance
/// within a bounded operational window. This aligns with standard business dispute resolution
/// SLAs in invoice finance (typically 3–15 days).
///
/// A pause duration exceeding this value will fail with [`EscrowError::DisputePauseDurationExceedsMax`].
pub const MAX_DISPUTE_PAUSE_DURATION_SECS: u64 = 14 * 24 * 60 * 60; // 1,209,600 seconds

/// Maximum UTF-8 byte length for the invoice `String` at init (matches Soroban [`Symbol`] max).
pub const MAX_INVOICE_ID_STRING_LEN: u32 = 32;

/// Minimum instance storage TTL extension horizon for time-sensitive escrow entries.
///
/// `bump_ttl` extends instance-storage entries to avoid rent/archival edge cases when
/// maturity/claim locks are far in the future.
///
/// Named as a constant so operators can reason about and audit the threshold.
pub const INSTANCE_TTL_MIN_EXTENSION_LEDGERS: u32 = 60 * 60; // Approx. 1h at 1 ledger/sec.

/// Minimum persistent storage TTL extension horizon for per-investor allowlist entries.
///
/// When the escrow uses the allowlist gate, investor funding depends on persistent entries.
/// Extending persistent allowlist TTL reduces the risk of silent allowlist disablement.
pub const PERSISTENT_TTL_MIN_EXTENSION_LEDGERS: u32 = 60 * 60; // Approx. 1h at 1 ledger/sec.

/// Upper bound on [`LiquifactEscrow::bump_ttl`]'s `allowlisted` entries per call.
///
/// Each entry costs 5 `extend_ttl` host calls (`InvestorAllowlisted`, `InvestorContribution`,
/// `InvestorEffectiveYield`, `InvestorClaimNotBefore`, `InvestorClaimed`); an unbounded vector
/// risks exceeding the transaction's CPU/resource budget with no partial progress. Mirrors
/// [`MAX_INVESTOR_ALLOWLIST_BATCH`] to limit per-call work.
pub const MAX_TTL_BUMP_BATCH: u32 = 32;

/// Maximum UTF-8 byte length for a snapshot name (must fit in Soroban Symbol).
pub const MAX_SNAPSHOT_NAME_LEN: u32 = 32;

/// Upper bound on state snapshots to keep storage bounded and prevent abuse.
/// Admin may create at most this many named snapshots per escrow instance.
pub const MAX_STATE_SNAPSHOTS: u32 = 16;

/// Pending legal-hold proposals expire one hour after admin authorization.
pub const LEGAL_HOLD_PROPOSAL_TTL_SECS: u64 = 3_600;
