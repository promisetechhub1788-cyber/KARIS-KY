# Canary Deployment Checklist

Fillable checklist for canary deployments. Print and use for each staged rollout.

---

## Phase 1: Pre-Canary Planning (Day -3 to -1)

### Code preparation
- [ ] New WASM built: `cargo build --target wasm32v1-none --release`
- [ ] All CI gates pass:
  - [ ] Format: `cargo fmt --all -- --check`
  - [ ] Linter: `cargo clippy -p karis-ky_escrow -- -D warnings`
  - [ ] Tests: `cargo test -p karis-ky_escrow`
  - [ ] Coverage: `cargo llvm-cov --fail-under-lines 95`
- [ ] Security code review completed and approved
- [ ] ADR/changelog updated for new version

### Testnet validation
- [ ] Upload WASM to testnet
- [ ] Deploy to testnet canary instances
- [ ] Run smoke tests:
  - [ ] `get_version` returns expected schema
  - [ ] `get_escrow` reads successfully
  - [ ] Investor funding works (if applicable)
  - [ ] Settlement/claims work (if applicable)
- [ ] Automated smoke test passes: `scripts/canary-smoke-test.sh <CONTRACT_ID> testnet`
  - [ ] `get_version` returns a non-empty value
  - [ ] `get_escrow_health` reports healthy
  - [ ] `get_escrow` reads escrow state without error
- [ ] No errors in testnet logs
- [ ] Performance within baseline ±15%

### Canary instance selection
- [ ] Canary instances identified: ________________
- [ ] Canary count: _____ (typically 2-3 instances, 5-10% of base)
- [ ] Mix of instance states documented:
  - [ ] Open status (funding in progress)
  - [ ] Funded status (ready for settlement)
  - [ ] Variety of investor counts
- [ ] Inventory JSON created and verified
- [ ] All canary instances have `is_canary: true` flag

### Governance approval
- [ ] Stage 1 (canary) approval request submitted to governance
- [ ] Vote scheduled: _____________________
- [ ] Vote result: APPROVED / REJECTED
- [ ] If approved, date/time approved: ______________

### CanaryOperator readiness
- [ ] CanaryOperator assigned: _____________________
- [ ] CanaryOperator key funded with XLM (for gas)
- [ ] CanaryOperator has read access to all instances (verified)
- [ ] CanaryOperator trained on canary procedures
- [ ] CanaryOperator on-call for next 72 hours

### Monitoring setup
- [ ] Dashboard created for canary metrics
- [ ] Health check scripts prepared (`canary_health_1h.sh`, etc.)
- [ ] Rollback plan documented
- [ ] Incident response contact list prepared

---

## Phase 2: Canary Deployment (Day 0)

### Pre-deployment verification
- [ ] All Pre-Canary Planning items checked off above
- [ ] Governance has approved Stage 1
- [ ] WASM hash recorded: `_______________________________`
- [ ] Canary instances list confirmed (no production mixed in):
  - [ ] Instance 1: __________ (is_canary verified)
  - [ ] Instance 2: __________ (is_canary verified)
  - [ ] Instance 3: __________ (is_canary verified)

### Legal hold activation
- [ ] Legal hold activated on all canary instances
- [ ] Status verified: all instances report `legal_hold_active: true`
- [ ] Timestamp recorded: ______________

### WASM upload
- [ ] Build completed and verified
- [ ] WASM uploaded to mainnet
- [ ] WASM hash verified matches testnet deployment
- [ ] Hash recorded: `_______________________________`

### Deployment execution
- [ ] Upgrade invocation issued to instance 1
  - [ ] Invocation successful (no timeout/error)
  - [ ] Timestamp: ______________
- [ ] Upgrade invocation issued to instance 2
  - [ ] Invocation successful
  - [ ] Timestamp: ______________
- [ ] Upgrade invocation issued to instance 3
  - [ ] Invocation successful
  - [ ] Timestamp: ______________

### Post-deployment verification
- [ ] Version query returns expected schema on all instances
- [ ] `get_escrow` reads without errors on all instances
- [ ] Escrow status unchanged (before ≈ after)
- [ ] Funded amount unchanged (state integrity)
- [ ] Legal hold remains active (as set pre-deployment)
- [ ] Automated smoke test passes on each canary instance:
      `scripts/canary-smoke-test.sh <CONTRACT_ID> mainnet`
  - [ ] `get_version` returns a non-empty value
  - [ ] `get_escrow_health` reports healthy
  - [ ] `get_escrow` reads escrow state without error

### Legal hold clearance
- [ ] Legal hold cleared on all canary instances
- [ ] Status verified: all instances report `legal_hold_active: false`
- [ ] Timestamp recorded: ______________
- [ ] Canary deployment complete: ______________

---

## Phase 3: Canary Monitoring — 1-Hour Checkpoint

**Time: T + 1 hour after deployment**

### Health check execution
- [ ] Health check script run: `bash canary_health_1h.sh`
- [ ] Report generated: `canary_report_1h.txt`
- [ ] All instances responding: YES / NO
- [ ] Any RPC errors? YES / NO
  - If YES, describe: ________________________________________

### Instance-by-instance verification
**Instance 1: __________**
- [ ] Version query successful
- [ ] get_escrow returns data without error
- [ ] Status unchanged
- [ ] Legal hold: FALSE (confirmed cleared)

**Instance 2: __________**
- [ ] Version query successful
- [ ] get_escrow returns data without error
- [ ] Status unchanged
- [ ] Legal hold: FALSE

**Instance 3: __________**
- [ ] Version query successful
- [ ] get_escrow returns data without error
- [ ] Status unchanged
- [ ] Legal hold: FALSE

### Initial metrics
- [ ] RPC error rate: _____ %
- [ ] Gas usage normal: YES / NO
- [ ] Any contract panics: NO
- [ ] Any investor complaints: NO
- [ ] Summary: ________________________________________________

---

## Phase 4: Canary Monitoring — 24-Hour Checkpoint

**Time: T + 24 hours after deployment**

### Continuous monitoring review
- [ ] No alerts triggered in 24h window
- [ ] No escalations to ops team
- [ ] Logs reviewed for anomalies: YES / NO
  - If found, describe: _____________________________________

### Business metrics (if applicable)
- [ ] Investor funding (if any): ____ new deposits
  - [ ] All successful (no failures)
- [ ] Settlement transactions (if any): ____ transactions
  - [ ] All successful (100% success rate)
- [ ] Investor claims (if any): ____ claims
  - [ ] All successful

### Performance metrics
- [ ] Gas usage vs. baseline: _____ % (acceptable: ±15%)
- [ ] RPC latency: _____ ms (acceptable: < 1000ms)
- [ ] Error rate: _____ % (acceptable: < 0.1%)
- [ ] All instances healthy: YES / NO

### State integrity check
- [ ] Escrow state still matches post-deployment (no drift)
- [ ] No unexpected legal holds activated
- [ ] No investor disputes reported

### Assessment
- [ ] Canary health: GOOD / CONCERNING / FAILED
- [ ] Notes: ________________________________________________

---

## Phase 5: Canary Monitoring — 72-Hour Checkpoint (Final)

**Time: T + 72 hours after deployment**

### Aggregated metrics
- [ ] Total RPC errors over 72h: _____ (acceptable: < 0.1%)
- [ ] Total investor complaints: _____ (acceptable: 0)
- [ ] Settlement success rate: _____ % (acceptable: 100%)
- [ ] Average gas usage vs. baseline: _____ % (acceptable: ±10%)
- [ ] Uptime: _____ % (acceptable: > 99%)

### Promotion criteria check
| Criterion | Threshold | Actual | Status |
|-----------|-----------|--------|--------|
| Error rate | < 0.1% | ____ | ✓/✗ |
| Investor complaints | 0 | ____ | ✓/✗ |
| Settlement success | 100% | ____ | ✓/✗ |
| RPC availability | > 99% | ____ | ✓/✗ |
| Gas usage | ±10% baseline | ____ | ✓/✗ |

**Overall canary result:** PASS / FAIL

### Final logs and artifacts
- [ ] 72-hour monitoring report compiled
- [ ] All logs archived
- [ ] Metrics exported for governance review
- [ ] Incident report (if any): ________________________________

---

## Phase 6: Governance Approval for Production

### Canary report preparation
- [ ] Executive summary written
- [ ] Metrics dashboard screenshot taken
- [ ] Error logs reviewed (none / minor issues only)
- [ ] Recommendation: PROMOTE / HOLD / ROLLBACK

### Governance vote
- [ ] Stage 2 (production) approval request submitted
- [ ] Vote scheduled: _____________________
- [ ] Canary report attached to vote
- [ ] Vote result: APPROVED / REJECTED / DEFERRED
- [ ] If approved, date/time: ______________

**If REJECTED or DEFERRED:**
- [ ] Reason documented: ____________________________________
- [ ] Corrective action (if any): ____________________________
- [ ] Next steps: _________________________________________

---

## Phase 7: Production Deployment (Admin-only)

### Admin preparation
- [ ] Admin (multisig) notified of governance approval
- [ ] Admin gathered for signing session
- [ ] WASM hash verified (same as canary): `_____________`
- [ ] Production instance list confirmed
- [ ] Production deployment executed
- [ ] Post-deployment smoke test passes:
      `scripts/canary-smoke-test.sh <CONTRACT_ID> mainnet`

---

## Automated Smoke Test

The canary smoke test is automated via `scripts/canary-smoke-test.sh`. It
accepts a contract ID and performs read-only checks against three endpoints:

| Endpoint | Assertion |
|----------|-----------|
| `get_version` | Returns a non-empty version/schema value |
| `get_escrow_health` | Reports a healthy status |
| `get_escrow` | Reads escrow state without error |

Usage:

```bash
scripts/canary-smoke-test.sh <CONTRACT_ID> [NETWORK] [SOURCE]
```

- `CONTRACT_ID` — Stellar contract ID of the canary deployment (required)
- `NETWORK` — Stellar network to target (default: `testnet`)
- `SOURCE` — Identity/source account for the read-only calls (default: `canary-smoke`)

The script is idempotent: it only performs read-only queries and never mutates
contract state, so it is safe to run multiple times. It exits `0` when all
checks pass, `1` when a check fails, and `2` on usage/configuration errors.

In CI, the smoke test runs against a testnet canary contract. PR builds that do
not have a canary deployed can skip it by setting `SKIP_CANARY_SMOKE=1`.
