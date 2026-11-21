# Escrow Write API Reference

This document lists every mutating (write) entrypoint in the escrow contract, the authorization each requires, the states in which it is valid, the conditions that block it, and the events it emits.

## Conventions

- **Auth** — the caller identity that must authorize the call. `admin` = contract admin/owner; `buyer` / `seller` = the escrow parties; `arbiter` = designated dispute arbiter; `any` = any authenticated caller (subject to the checks below).
- **Valid states** — the escrow lifecycle states in which the entrypoint may be invoked. States: `Created`, `Funded`, `Active`, `Disputed`, `Completed`, `Cancelled`.
- **Blocking conditions** — conditions that cause the call to be rejected regardless of state:
  - **Legal hold** — a legal hold is active on the escrow; all mutating calls are rejected until it is lifted.
  - **Dispute pause** — a dispute is open and the escrow is paused; only dispute-resolution entrypoints are permitted.

## Mutating entrypoints

| Entrypoint | Auth | Valid states | Blocking conditions | Events emitted |
| --- | --- | --- | --- | --- |
| `create_escrow` | `buyer` | — (creates a new escrow) | Legal hold (on the referenced account) | `EscrowCreated` |
| `fund_escrow` | `buyer` | `Created` | Legal hold, Dispute pause | `EscrowFunded` |
| `activate_escrow` | `seller` | `Funded` | Legal hold, Dispute pause | `EscrowActivated` |
| `deposit` | `buyer` | `Funded`, `Active` | Legal hold, Dispute pause | `Deposited` |
| `release_funds` | `buyer` | `Active` | Legal hold, Dispute pause | `FundsReleased` |
| `refund` | `seller` | `Active` | Legal hold, Dispute pause | `Refunded` |
| `cancel_escrow` | `buyer`, `seller` | `Created`, `Funded` | Legal hold, Dispute pause | `EscrowCancelled` |
| `open_dispute` | `buyer`, `seller` | `Funded`, `Active` | Legal hold | `DisputeOpened` |
| `resolve_dispute` | `arbiter` | `Disputed` | Legal hold | `DisputeResolved` |
| `close_dispute` | `arbiter` | `Disputed` | Legal hold | `DisputeClosed` |
| `set_arbiter` | `admin` | `Created`, `Funded`, `Active` | Legal hold, Dispute pause | `ArbiterUpdated` |
| `update_fee` | `admin` | `Created`, `Funded`, `Active` | Legal hold, Dispute pause | `FeeUpdated` |
| `place_legal_hold` | `admin` | `Created`, `Funded`, `Active`, `Disputed` | — | `LegalHoldPlaced` |
| `lift_legal_hold` | `admin` | `Created`, `Funded`, `Active`, `Disputed` | — | `LegalHoldLifted` |
| `pause_escrow` | `admin` | `Funded`, `Active` | Legal hold | `EscrowPaused` |
| `unpause_escrow` | `admin` | `Disputed` (paused) | Legal hold | `EscrowUnpaused` |
| `complete_escrow` | `buyer`, `seller` | `Active` | Legal hold, Dispute pause | `EscrowCompleted` |

## Notes

- `place_legal_hold` and `lift_legal_hold` are the only entrypoints permitted while a legal hold is active; all other mutating calls are rejected.
- While a dispute pause is active, only `resolve_dispute`, `close_dispute`, `place_legal_hold`, `lift_legal_hold`, and `unpause_escrow` are permitted.
- Terminal states (`Completed`, `Cancelled`) reject all mutating entrypoints except `place_legal_hold` / `lift_legal_hold`.
