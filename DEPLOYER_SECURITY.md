# Deployer Security Guide

> **Key Terms:** [funding close snapshot](docs/glossary.md#funding-close-snapshot) · [pro-rata denominator](docs/glossary.md#pro-rata-denominator) · [SME](docs/glossary.md#sme)

This guide covers the security considerations and operational practices for deploying and managing the escrow contracts.

## Overview

Deployers are responsible for the secure configuration, deployment, and ongoing administration of the escrow system. This document outlines the key security practices, threat model, and operational procedures.

## Roles and Responsibilities

### Deployer

The deployer is the account that deploys the escrow contracts and configures the initial parameters. The deployer typically holds administrative privileges during the setup phase.

### Administrator

The administrator manages ongoing operations, including parameter updates, attestation management, and compliance actions.

## Deployment Checklist

Before deploying to production, ensure the following:

1. **Key Management**: Use a hardware wallet or secure key management solution for the deployer account.
2. **Parameter Review**: Verify all initialization parameters against the [escrow-init-parameters.md](docs/escrow-init-parameters.md) documentation.
3. **Fund Parameters**: Confirm fund parameters match the [escrow-fund-parameters.md](docs/escrow-fund-parameters.md) specification.
4. **Attestations**: Review the [escrow-attestations.md](docs/escrow-attestations.md) documentation for attestation requirements.
5. **Compliance**: Ensure compliance requirements are met per the [escrow-compliance-guide.md](docs/escrow-compliance-guide.md).

## Key Management

### Deployer Keys

- Store deployer private keys in a hardware wallet or secure enclave.
- Never commit private keys or mnemonics to version control.
- Use separate accounts for deployment and ongoing administration where possible.
- Rotate keys according to your organization's security policy.

### Administrator Keys

- Administrators should use multi-signature wallets for critical operations.
- Document all administrator key holders and their responsibilities.
- Establish clear procedures for key rotation and emergency access.

## Threat Model

### Compromised Deployer Key

If the deployer key is compromised, an attacker could:

- Reconfigure contract parameters.
- Initiate unauthorized fund movements.
- Modify attestation requirements.

**Mitigation**: Use hardware wallets, multi-signature schemes, and monitor for unexpected transactions.

### Malicious Administrator

A malicious administrator could abuse privileged functions to:

- Freeze or unfreeze funds inappropriately.
- Modify compliance settings.
- Approve fraudulent attestations.

**Mitigation**: Implement multi-signature requirements, establish audit trails, and separate duties across multiple administrators.

### Admin-Only Entrypoints

The table below enumerates every admin-only entrypoint and the worst-case impact of a compromised admin key, along with the mitigations currently in place and those recommended. Follow-up issues are linked for any unmitigated high-impact power.

| Admin power | Worst-case impact | Current mitigation | Recommended mitigation |
| --- | --- | --- | --- |
| `update_target` | Redirect escrow target, misdirecting investor expectations and downstream accounting | Admin authorization only | Multi-signature admin + monitoring/alerting on target changes |
| `update_maturity` (extension) | Extend maturity indefinitely, delaying investor repayment | Admin authorization only | Timelock or guardian confirmation for extensions ([#764](https://github.com/example/issues/764)) |
| `update_maturity` (shortening) | Accelerate repayment schedule | Admin authorization only | Multi-signature admin; unchanged by the extension guard |
| `lower_cap` | Reduce the cap below committed amounts, blocking participation | Admin authorization only | Multi-signature admin + event monitoring |
| `pause` | Halt all operations, denying access to funds | Admin authorization only | Multi-signature admin + documented emergency runbook |
| `rate limit` | Throttle or block withdrawals/operations | Admin authorization only | Multi-signature admin + monitoring |
| `upgrade` | Swap contract code in a single transaction, enabling arbitrary logic | Admin authorization only | Two-step `propose_upgrade`/`execute_upgrade` with delay ([#765](https://github.com/example/issues/765)) |
| `archive` | Archive the contract, preventing further operations | Admin authorization only | Multi-signature admin + monitoring |
| `rotate beneficiary` | Redirect beneficiary payouts to an attacker-controlled account | Admin authorization only | Multi-signature admin + confirmation delay |

**High-impact powers without a timelock or guardian today:** `update_maturity` extensions ([#764](https://github.com/example/issues/764)) and `upgrade` ([#765](https://github.com/example/issues/765)). Both are tracked as follow-ups and are expected to gain a propose/confirm or delay mechanism.

### Maturity Extensions

Extending maturity with `update_maturity` delays investor repayment and is therefore treated as a sensitive action. Extensions must be gated behind a timelock or guardian confirmation: the extension is proposed, held for the configured delay (or confirmed by the guardian), and only then applied. Shortening maturity is out of scope for this guard and retains its existing rules. See [#764](https://github.com/example/issues/764) for the tracking issue.

### Contract Upgrades

`upgrade(new_wasm_hash)` must not take effect immediately on admin authorization. Upgrades follow a two-step flow:

1. `propose_upgrade(hash)` — records the proposed wasm hash and emits a proposal event.
2. `execute_upgrade()` — after the minimum delay has elapsed, applies the proposed hash and emits an execution event.

Both steps emit events so operators can monitor for unexpected proposals. The legacy immediate `upgrade` entrypoint is gated behind this flow until the interface version is bumped. See [#765](https://github.com/example/issues/765) for the tracking issue.

### Front-Running

Attackers may attempt to front-run deployment or configuration transactions.

**Mitigation**: Use private transaction pools where available, and monitor mempool activity during critical operations.

## Operational Security

### Monitoring

- Monitor all administrative transactions and alert on unexpected activity.
- Track contract state changes, especially around the [funding close snapshot](docs/glossary.md#funding-close-snapshot).
- Set up alerts for parameter changes and attestation updates.

### Incident Response

1. **Detect**: Identify the incident through monitoring or reports.
2. **Contain**: Pause operations if possible and secure remaining funds.
3. **Assess**: Determine the scope and impact of the incident.
4. **Remediate**: Apply fixes and restore normal operations.
5. **Review**: Conduct a post-mortem and update procedures.

### Access Control

- Follow the principle of least privilege.
- Regularly review and revoke unnecessary permissions.
- Maintain an up-to-date inventory of all privileged accounts.

## Compliance Considerations

Deployers must ensure the system is configured to meet applicable regulatory requirements. See the [escrow-compliance-guide.md](docs/escrow-compliance-guide.md) for detailed compliance procedures.

### Legal Holds

Understand how legal holds interact with fund operations. See [escrow-legal-hold.md](docs/escrow-legal-hold.md) for details.

## State Machine

The escrow contracts follow a defined state machine. Review [state-machine.md](docs/state-machine.md) to understand valid state transitions and their security implications.

## Audit and Verification

- Conduct regular security audits of the deployed contracts.
- Verify contract source code matches deployed bytecode.
- Maintain records of all audits and their findings.
- Address audit findings promptly and document remediation.

## Additional Resources

- [OPERATOR_RUNBOOK.md](docs/OPERATOR_RUNBOOK.md) — Operational procedures
- [investor-quick-start.md](docs/investor-quick-start.md) — Investor onboarding
- [docs/glossary.md](docs/glossary.md) — Key terms and definitions
