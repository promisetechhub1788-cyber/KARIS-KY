# Deployer Security Guide

> **Audience:** Operators, DevOps, and governance teams deploying or managing escrow contract instances on Stellar/Soroban.
>
> **Scope:** Secret key protection, audit practices, post-deployment monitoring, and incident response specific to the karis-ky escrow contract.

---

## 1. Secret Key Protection

### 1.1 Never use a single EOA admin

The [`InvoiceEscrow::admin`] address controls compliance holds, allowlist, attestation, maturity, and funding target updates. A single compromised key grants complete freeze authority over all escrow instances sharing that admin.

**Requirement:** Production deployments **must** use:

- **Multisig governance contract** (e.g., Stellar's built-in multisig, a custom Soroban contract enforcing M-of-N threshold, or a DAO timelock contract).
- **Threshold minimum:** 2-of-3 or higher, with signers geographically distributed and held by distinct legal entities.
- **Key rotation:** annual or per-governance policy; must be coordinated in advance with signers.

### 1.2 HSM or secure enclave for private keys

All signing keys used in deployment, migration, and governance operations must be stored in a hardware security module (HSM) or equivalent (e.g., AWS CloudHSM, YubiHSM, Ledger, Trezor in a trusted setup).

**Never store secret keys in**:
- Environment variables in CI/CD logs or `.env` files.
- Version control repositories (even "deleted" commits remain in history).
- Cloud storage buckets, shared password managers, or unencrypted email.
- Developer laptops or non-dedicated hardware.

**Best practice workflow**:
1. HSM or secure enclave generates the key and never exports the private key.
2. Only the public key is exported for setup.
3. Signing requests are sent to the HSM; only signatures are returned.
4. All signing is auditable in HSM logs.

### 1.3 Multi-step deployment ceremony

Deployment should follow a formal ceremony, not an automated script:

1. **Prepare phase** (offline, on secure systems):
   - Build WASM locally.
   - Generate [`Env::deployer().update_current_contract_wasm(new_wasm_hash)`] call parameters.
   - Verify contract address and hashes offline.

2. **Review phase** (committee + legal):
   - External auditor / security team reviews the build output and changelog.
   - Legal verifies incident response plan and operational runbook are in place.
   - Obtain sign-off from at least 2 independent reviewers.

3. **Sign phase** (HSM-backed multisig):
   - Each signer independently reviews the deployment parameters.
   - Each signer produces a signature using their HSM key.
   - Signatures are combined into a valid multisig transaction.

4. **Submit phase** (sealed network):
   - Deployment is submitted on a secured network connection (VPN, closed network).
   - No rollback without a new formal ceremony and sign-off.

### 1.4 Deployer account

The `SOURCE_SECRET` (Stellar account funding the deployment) does **not** need to be a multisig. However:

- Do not reuse this account for other contract deployments or user funds.
- Fund it with exactly the XLM needed for one deployment; sweep remainder afterward.
- Do not store the secret key; use an HSM or cold storage account.
- Rotate the account after each deployment if possible.

---

## 2. Audit Recommendations

### 2.1 Pre-deployment external security review

Before mainnet deployment, contract code **must** be reviewed by an independent security firm.

**Scope of review**:
- Authorization guards ([`require_auth()`] call placement and argument correctness).
- Arithmetic overflow / underflow in funding, settlement, pro-rata math.
- Storage layout and schema migration paths.
- Token integration assumptions (SEP-41 balance equality, no fee-on-transfer).
- Compliance hold and dispute pause logic (if applicable).

**Timing**: Review must complete at least 2 weeks before mainnet go-live. Security issues flagged in the report must be resolved and re-reviewed before deployment.

**Post-review artifact storage**:
- Store the final audit report in a legally discoverable location (e.g., Dropbox, AWS S3 with MFA delete, GitHub private repo with branch protection).
- Do not mix audit reports and production keys in the same storage.

### 2.2 Dependency audit

Before each build, run:

```bash
cargo audit
cargo outdated
```

Consult [`docs/escrow-dependency-policy.md`](escrow-dependency-policy.md) for the cadence and emergency bump procedures. Any advisory-level dependency issue **must** trigger a minor version release and re-audit before deploying.

### 2.3 Continuous integration and code review

Every merge to `main` must:

1. **Pass CI** (see `README.md`):
   ```bash
   cargo fmt --all -- --check
   cargo clippy -p karis-ky_escrow -- -D warnings
   cargo build
   cargo test
   cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow
   ```

2. **Require at least 2 independent code reviews** from team members with merge authority.

3. **Log all merge decisions** for post-incident analysis (e.g., in GitHub PR comments or Jira).

### 2.4 Staged rollout (testnet → staging → mainnet)

Deploy to testnet **first**, even if a security review was already completed:

1. **Testnet deployment** (low-risk):
   - Deploy the same WASM to testnet with test admin / governance keys.
   - Exercise full lifecycle: `init`, `fund`, `settle`, `withdraw`, `claim_investor_payout`, `sweep_terminal_dust`.
   - Monitor for unexpected behavior, performance issues, or gas cost surprises.
   - Duration: at least 1 week.

2. **Staging deployment** (if applicable):
   - If your organization runs a staging network, repeat testnet checks there.
   - Coordinate with other integrators using escrow to synchronize go-live.
   - Duration: at least 3 days.

3. **Mainnet deployment** (formal ceremony):
   - Follow the multi-step ceremony in §1.3.
   - Only after testnet / staging validation and governance sign-off.

---

## 3. Post-Deployment Monitoring

### 3.1 Real-time event monitoring

The contract emits events for all state transitions. Set up alerting for anomalies:

```
Events to monitor (see docs/EVENT_SCHEMA.md and escrow-events.md):
- EscrowInitialized    → Baseline for new escrow instances
- EscrowFunded         → Normal operation; watch for unusual investor patterns
- EscrowSettled        → Expected after maturity
- EscrowPartialSettle  → Unexpected; debug required
- InvestorClaimRecorded→ Normal; ensure investor identity matches expected participant list
- LegalHoldActivated   → Review the reason immediately; notify compliance team
- DisputePausedEvt     → Review the dispute ticket reference; ensure it aligns with off-chain records
- TransferredDustToTreasuryEvt → Monitor treasury balance; verify sweep amounts
```

**Implementation**:
- Use a Soroban event indexer (e.g., Stellar Indexing Service, custom Horizon polling, or webhook-based ingestion).
- Filter events by contract address.
- Send alerts to Slack, PagerDuty, or your on-call system.
- Retain logs for at least 90 days.
- For `EscrowHealthWarning` ingestion and backend webhook configuration, see
  [Escrow Health Warning Alerts](escrow-health-alerting.md).

### 3.2 Storage audit (periodic)

Every week, verify the escrow instance state against expected values:

1. **Read [`LiquifactEscrow::get_escrow`]** for each active contract instance:
   - Check `funded_amount` matches sum of recent `EscrowFunded` events.
   - Verify `status` is monotone increasing (never decreases or resets).
   - Confirm `admin` and `sme_address` are correct.

2. **Validate invariants** (see `docs/escrow-security-checklist.md` §3):
   - ```
     funded_amount == Σ InvestorContribution(addr) for all addr
     ```
   - If this invariant is violated, **immediately pause the instance** (via `set_legal_hold`) and investigate.

3. **Check [`LiquifactEscrow::get_version`]**:
   - Ensure `DataKey::Version` matches the deployed WASM [`SCHEMA_VERSION`].
   - If they mismatch, the instance may need migration.

### 3.3 Token balance reconciliation

Before each settlement:

1. **Query funding token balance** of the escrow contract.
2. **Compare against `InvoiceEscrow::funded_amount`**:
   - Token balance must be **≥** `funded_amount`.
   - If balance < `funded_amount`, funds may have been stolen or misrouted; **stop settlement immediately** and investigate.

3. **After [`LiquifactEscrow::sweep_terminal_dust`]**:
   - Verify the treasury received the swept amount.
   - Ensure contract balance is still ≥ outstanding investor liabilities.

### 3.4 Quarterly security audit

Every quarter, have a security-focused team member (or external consultant) review:

1. **Event logs** from the past 3 months:
   - Any unusual legal holds (> 1 per quarter warrants review)?
   - Dispute pauses triggered without corresponding support tickets?
   - Admin address changes via `propose_admin` / `accept_admin`?

2. **Storage changes**:
   - Were any schema migrations performed?
   - Did the WASM version change? Verify it matches [`SCHEMA_VERSION`] on-chain.

3. **Incident reports**:
   - Were there any token transfer failures, gas overages, or unexpected errors?
   - Document findings in a log file for historical auditing.

---

## 4. Incident Response Plan

### 4.1 Legal hold as an emergency brake

If any of the following occur, **immediately activate a legal hold** via `set_legal_hold(env, true)`:

- Unauthorized fund withdrawal (SME called `withdraw` unexpectedly).
- Token balance < investor liabilities.
- Unusual dispute pause with no matching support ticket.
- Suspected admin key compromise (any unexpected `propose_admin` call).
- External auditor flags a security issue in the escrow logic.
- Regulatory inquiry or legal freeze demand.

**Effect of legal hold**:
- Blocks `settle`, `withdraw`, `claim_investor_payout`, `fund`, and `fund_with_commitment`.
- Does **not** block reads; indexers can still query state.
- Can be cleared by the current `admin` (requires governance multisig sign-off).

### 4.2 Governance emergency playbook

When a legal hold is activated:

1. **Notify stakeholders** (within 1 hour):
   - Email karis-ky governance list.
   - Notify affected SMEs and investors.
   - Alert legal / compliance.

2. **Investigation phase** (24–48 hours):
   - Root-cause analysis: was the escrow misconfigured, or was there malicious activity?
   - Gather evidence: storage snapshots, event logs, token transfer records.
   - Determine if the hold should remain or if the issue can be remediated.

3. **Resolution phase** (as soon as root cause is clear):
   - If the issue was operational (e.g., SME accidentally called withdraw early), document the incident and clear the hold after governance approval.
   - If the issue was a logic bug, coordinate an emergency patch:
     - Pause the affected escrow instance(s) via additional holds.
     - Deploy a patched WASM to testnet and validate.
     - Run expedited security review (1–2 days).
     - Deploy to mainnet under formal ceremony with governance emergency sign-off.

### 4.3 Disaster recovery: admin key compromise

If the current admin key is compromised:

1. **Do not panic; legal hold is still available.**
   - If you can still sign with the compromised key, use it to call `set_legal_hold(env, true)` immediately.

2. **Propose a new admin** (still using the compromised key, if you have access):
   - Call `propose_admin(env, new_governance_contract_address)`.
   - This queues the address but does **not** transfer authority yet.

3. **Accept as new admin**:
   - The new governance contract calls `accept_admin(env)`.
   - Authority is now transferred; the old key is no longer needed.

4. **Clear the legal hold** (optional, from the new admin):
   - If operations should resume, the new admin calls `set_legal_hold(env, false)`.

**Note**: There is **no timelock** or multisig enforced on the `propose_admin` / `accept_admin` flow. The contract trusts that governance has secured the recipient address. This underscores the importance of using a multisig or DAO contract as `admin`.

### 4.4 Communication template

**For investors:**
```
Subject: karis-ky Escrow Alert: [ESCROW_ID] Temporary Hold

Due to [brief reason: regulatory inquiry / security review / operational issue],
we have temporarily frozen the escrow for invoice [ESCROW_ID].

What this means for you:
- You cannot claim your payout until the hold is lifted.
- Your principal is safe and held by the contract.
- We are investigating and will provide an update within [X hours/days].

Questions? Contact support@karis-ky.com.
```

**For governance:**
```
Action: Emergency legal hold on escrow [ID]
Reason: [detailed technical root cause]
Time activated: [UTC timestamp]
Time to clear: [estimated]
Owner: [team member name / on-call]
Evidence: [links to event logs, storage snapshots, etc.]
```

---

## 5. Compliance and Documentation

### 5.1 Deployment log

Maintain a **permanent, immutable log** of all deployments:

```
Deployment Log (escrow.karis-ky.com, mainnet)

Date        | WASM Hash              | Schema Version | Admin Address                       | Trigger         | Status | Notes
------------|------------------------|----------------|-------------------------------------|-----------------|--------|-------
2025-01-15  | 0xabc123...            | 6              | G...multisig                        | Go-live         | ✓ OK   |
2025-02-01  | 0xdef456...            | 6              | G...multisig                        | Dependency bump | ✓ OK   | CVE-2024-xyz patched
2025-03-10  | 0xghi789...            | 7              | G...multisig                        | Dispute pause   | ✓ OK   | New feature (#257)
```

Store this log in:
- A read-only AWS S3 bucket with versioning and MFA delete enabled.
- A GitHub private repository with branch protection.
- Legal discovery / evidence storage (e.g., Evernote, Notion).

### 5.2 Incident log

Every incident (legal hold, dispute pause, token error, gas overage) must be logged:

```
Incident Log

ID   | Date       | Escrow ID | Type             | Severity | Root Cause           | Resolved | Lessons Learned
-----|------------|-----------|------------------|----------|----------------------|----------|---
INC1 | 2025-01-20 | INV-002   | Unusual withdraw | Medium   | SME typo in timing   | Yes      | Add calendar reminder before SME settle
INC2 | 2025-02-14 | INV-007   | Token balance 0  | Critical | Investor refund bug  | Yes      | Add unit test for refund path
```

### 5.3 External audit report archive

Store all security audit reports, including:
- Audit scope (code version, commit hash).
- Executive summary.
- Detailed findings (high/medium/low severity).
- Remediation status and evidence.
- Auditor signature and timestamp.

Keep these artifacts **indefinitely** for legal discovery and compliance.

---

## 6. Checklist for Deployers

Before mainnet deployment, confirm:

- [ ] Admin is a multisig governance contract (≥ 2-of-3 signers).
- [ ] All signer keys are stored in HSMs or secure enclaves; private keys never exported.
- [ ] Pre-deployment external security review is complete and signed off.
- [ ] Dependency audit (`cargo audit`) shows no advisories.
- [ ] Testnet deployment was successful; full lifecycle tested for ≥ 1 week.
- [ ] Multi-step deployment ceremony is documented and scheduled.
- [ ] Event monitoring and alerting are configured.
- [ ] Weekly storage audit process is in place.
- [ ] Quarterly security audit process is documented.
- [ ] Incident response playbook is shared with all stakeholders.
- [ ] Deployment log and artifact storage are set up.
- [ ] Legal hold button (admin panel) is accessible to on-call team 24/7.
- [ ] Governance has approved the deployment and signed off formally.

---

## 7. Admin-Power Threat Model

> **Single source of truth.** This table mirrors the threat-model review published for issue #240. Any change to admin powers must be reflected here and in the corresponding follow-up issues.

The contract's `admin` (see §1.1) can invoke the following privileged entrypoints. Each row states the worst-case impact of a compromised admin key, the mitigation currently enforced on-chain, and the recommended mitigation where a gap remains.

| Admin power | Worst-case impact | Current mitigation | Recommended mitigation |
|---|---|---|---|
| `update_target` | Redirect funding goals; mislead investors about the raise size | `require_auth()` on admin only | Multisig admin (§1.1); emit + monitor `TargetUpdated` events |
| `update_maturity` (extension) | Extend maturity indefinitely, delaying investor repayment | `require_auth()` on admin only | **Propose/confirm or delay mechanism** — tracked in follow-up [#241](https://github.com/karis-ky/escrow/issues/241); see §7.1 |
| `update_maturity` (shortening) | Accelerate settlement earlier than investors expect | `require_auth()` on admin only | Out of scope for #241; keep admin-only but monitor events |
| `lower_cap` | Reduce the funding cap below already-committed amounts | `require_auth()` on admin only | Multisig admin; emit + monitor `CapLowered` events |
| `set_legal_hold` | Freeze all fund movement (`settle`, `withdraw`, `claim_investor_payout`, `fund`) | `require_auth()` on admin only; reversible by admin | Multisig admin; alerting on `LegalHoldActivated` (§3.1) |
| `set_dispute_pause` | Halt operations pending dispute resolution | `require_auth()` on admin only | Multisig admin; alerting on `DisputePausedEvt` (§3.1) |
| `set_rate_limit` | Throttle or block investor actions | `require_auth()` on admin only | Multisig admin; document limits in operator runbook |
| `upgrade` / `update_current_contract_wasm` | Swap contract code in a single transaction; total loss of funds | `require_auth()` on admin only | **Two-step `propose_upgrade` / `execute_upgrade` with minimum delay** — tracked in follow-up [#242](https://github.com/karis-ky/escrow/issues/242); see §7.2 |
| `archive` | Move instance to archived state; halt operations | `require_auth()` on admin only | Multisig admin; require legal-hold clear before archive |
| `rotate_beneficiary` | Redirect payouts to an attacker-controlled address | `require_auth()` on admin only | Multisig admin; emit + monitor beneficiary-rotation events |
| `propose_admin` / `accept_admin` | Full transfer of admin authority to attacker | Two-step propose/accept, but **no timelock** (§4.3) | Multisig or DAO contract as `admin`; consider timelock on `accept_admin` |

### 7.1 `update_maturity` extension guard (follow-up #241)

Maturity **extensions** are the highest-impact non-upgrade power because they indefinitely delay investor repayment. The recommended design (tracked in [#241](https://github.com/karis-ky/escrow/issues/241)) is a propose/confirm or delay mechanism:

- `propose_maturity_extension(new_maturity)` records the intent and starts a delay window.
- `confirm_maturity_extension()` (or a guardian confirmation) executes the extension after the delay elapses.
- Maturity **shortening** rules are unchanged and remain admin-only.

Until #241 lands, operators must treat any `update_maturity` extension as a high-severity event and require governance multisig sign-off plus off-chain notification to affected investors.

### 7.2 `upgrade` propose/execute flow (follow-up #242)

`upgrade(new_wasm_hash)` currently takes effect immediately on admin authorization, so a single compromised key can swap the contract code in one transaction. The recommended design (tracked in [#242](https://github.com/karis-ky/escrow/issues/242)) is:

- `propose_upgrade(new_wasm_hash)` records the pending hash and emits a `UpgradeProposed` event.
- `execute_upgrade()` applies the pending hash after a minimum delay and emits an `UpgradeExecuted` event.
- The legacy `upgrade` entrypoint is gated behind the new flow until the interface version is bumped.

Operators should monitor for `UpgradeProposed` events and treat any proposal without a matching governance change ticket as a potential key compromise (§4.3).

### 7.3 Follow-up issues

The following follow-ups track the unmitigated high-impact powers identified above:

- [#240](https://github.com/karis-ky/escrow/issues/240) — Threat model review of all admin-only entrypoints (this section).
- [#241](https://github.com/karis-ky/escrow/issues/241) — Require a timelock or guardian confirmation for `update_maturity` extensions.
- [#242](https://github.com/karis-ky/escrow/issues/242) — Guard `upgrade` with a two-step propose/execute and delay.

---

## References

- [`docs/OPERATOR_RUNBOOK.md`](OPERATOR_RUNBOOK.md) — Redeploy vs. upgrade decisions, WASM update ceremony, and **legal hold + dispute pause interaction matrix** (§8).
- [`docs/escrow-security-checklist.md`](escrow-security-checklist.md) — Auth matrix, invariants, and threat model.
- [`docs/escrow-events.md`](escrow-events.md) — Event schema and monitoring targets.
- [`docs/escrow-legal-hold.md`](escrow-legal-hold.md) — Legal hold mechanism and governance implications.
- Stellar / Soroban documentation: https://developers.stellar.org/docs/tools/soroban-cli

