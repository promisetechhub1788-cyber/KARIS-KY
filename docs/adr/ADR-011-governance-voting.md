# ADR-011: Governance Voting Mechanism for Schema Upgrades

**Status:** Accepted  
**Date:** 2024-01-01  
**Deciders:** Platform team, community  
**Related:** [escrow-governance-voting.md](../escrow-governance-voting.md), RFC-005

---

## Context

Schema version upgrades (v6 → v7, v7 → v8, etc.) change the on-chain data
model and contract interfaces that integrators depend on. Historically these
upgrades were applied by an admin key alone, which gave integrators no advance
notice and no way to object to a change that could break their tooling.

We need a mechanism that:

- Gives token holders a formal say in schema upgrades before deployment.
- Provides a predictable timeline so integrators can plan and upgrade.
- Produces an auditable, on-chain record of who approved what and when.
- Retains a fallback path for emergency security patches.

The governance voting mechanism described in
[`docs/escrow-governance-voting.md`](../escrow-governance-voting.md) has been
implemented. This ADR records *why* it was designed this way and what
alternatives were rejected.

---

## Decision

We adopt a **token-weighted, snapshot-based, on-chain voting mechanism** for
schema upgrades, implemented as a Soroban governance contract that the escrow
contract consults before executing an upgrade.

Key elements of the decision:

1. **On-chain voting.** Proposals, votes, and results live in the governance
   contract's storage. Vote history is immutable and auditable.
2. **Token-weighted voting power.** A holder's voting power equals their token
   balance, so stake correlates with influence.
3. **Snapshot at submission.** Voting power is fixed at proposal submission
   time, preventing vote buying during the voting window.
4. **51% approval threshold** of participating voters
   (`yes ≥ 0.51 × (yes + no)`), with a fixed 7-day voting period.
5. **Separation of concerns.** The governance contract manages proposals and
   tallies; the escrow contract executes the approved WASM upgrade via
   `upgrade_via_governance`.
6. **Admin emergency fallback.** `upgrade_admin_fallback` allows an admin to
   deploy a critical patch immediately, with the reason logged for audit.

---

## Alternatives Considered

### On-chain vs. off-chain voting

- **Off-chain voting (e.g. signed messages / Snapshot-style).** Cheaper and
  faster, but results are not natively verifiable on-chain, requiring a trusted
  relayer to bridge the outcome to the escrow contract. This reintroduces a
  trust assumption exactly where we want to remove one. **Rejected.**
- **On-chain voting (chosen).** Higher gas cost per vote, but the tally is
  trustless, tamper-evident, and directly consumable by the escrow contract.
  The cost is acceptable given the low frequency of schema upgrades.

### Voting power model

- **One-address-one-vote.** Resistant to plutocracy but trivially Sybil-able
  and misaligned with the token-based economics of the platform. **Rejected.**
- **Token-weighted voting (chosen).** Aligns influence with economic stake and
  is simple to verify against the token contract.

### Voting power timing

- **Live balance at vote time.** Allows an actor to acquire tokens mid-vote to
  swing the outcome (vote buying). **Rejected.**
- **Snapshot at submission (chosen).** Freezes voting power when the proposal
  is created, removing the incentive to buy tokens during the voting window.

### Approval threshold

- **Absolute majority of total supply.** Makes it easy for a small number of
  non-participants to block legitimate upgrades. **Rejected.**
- **51% of participating voters (chosen).** Reflects the will of those who
  actually vote while still requiring a clear majority.

### Execution authority

- **Admin-only upgrades.** No community input; the status quo we are moving
  away from. **Rejected** as the primary path, retained only as an emergency
  fallback.
- **Governance-only upgrades (chosen).** The escrow contract executes only
  after the governance contract confirms approval.

---

## Consequences

### Positive

- Integrators get advance notice and a defined window to prepare.
- Upgrade decisions are transparent and auditable on-chain.
- Snapshot-based power removes a class of vote-buying attacks.
- The admin fallback preserves the ability to respond to critical bugs.

### Negative

- On-chain voting costs gas per vote, which may reduce participation.
- A 7-day voting period delays non-emergency upgrades.
- Low participation can let a minority of participating voters decide an
  outcome (mitigated by the 51% threshold but not eliminated).
- The admin fallback is a centralization vector; it is mitigated by mandatory
  logging and post-mortem review, not removed.

### Neutral

- Adds a governance contract as a new dependency of the escrow contract.
- Requires the escrow contract to store a governance address and the last
  executed proposal ID for auditing.

---

## References

- [`docs/escrow-governance-voting.md`](../escrow-governance-voting.md) — full
  design and workflow examples for the governance voting mechanism.
- RFC-005 — original proposal and community discussion.
