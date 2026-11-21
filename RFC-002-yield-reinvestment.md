# RFC-002: Yield Reinvestment

**Status:** Implemented

## Summary

This RFC describes the design for allowing accrued escrow yield to be
reinvested back into the escrow principal rather than withdrawn. The design has
been implemented via the `reinvest_yield` entrypoint.

## Motivation

Escrows may accrue yield over their lifetime. Withdrawing that yield moves
funds out of the escrow and breaks compounding. Reinvestment keeps the yield
working inside the escrow.

## Design

- A new `reinvest_yield` entrypoint accepts an escrow identifier, a caller, and
  an optional amount.
- Preconditions, authorization, and error handling are defined in the
  implementation.
- Errors are numbered 181–184 after the error renumbering.

## User Documentation

See `docs/escrow-yield-reinvestment.md` for the user-facing guide, including
preconditions, errors 181–184, and usage examples.

## Status

Implemented.
