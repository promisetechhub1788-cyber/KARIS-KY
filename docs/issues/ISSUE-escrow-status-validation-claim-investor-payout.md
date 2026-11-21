# Issue: Validate Escrow Status Before Investor Payout Claims

**Type:** Security bug
**Status:** Backlog, ready for review and assignment
**Priority:** High
**Severity:** High
**Related:** [Escrow lifecycle](../escrow-lifecycle.md), [State model ADR](../adr/ADR-001-state-model.md), [Settlement flow ADR](../adr/ADR-003-settlement-flow.md), [Escrow error messages](../escrow-error-messages.md)

## Description

Harden `claim_investor_payout` so an investor can claim only after the escrow has
actually reached a claimable settlement state. The claim path must validate the
escrow lifecycle status and the settled amount before recording a payout claim
or emitting a successful claim event.

The security concern is that investor participation and a payout calculation
are separate from escrow lifecycle state. An investor may have a positive
contribution while the escrow remains open and unfunded. A positive contribution
must never be treated as evidence that the escrow is settled or that a payout
is available.

The invariant to enforce is:

- `status == 0` (`OPEN`): no payout claim is allowed, regardless of investor
  contribution or token balance.
- `status == 1` (`FUNDED`): a claim is allowed only for a valid partial
  settlement with `SettledAmount > 0`, if partial claims are supported.
- `status == 2` (`SETTLED`): a claim is allowed, subject to contribution,
  maturity/lock, legal-hold, pause, and idempotency guards.
- Other terminal states, including withdrawn or cancelled, are not claimable
  unless an explicitly documented product rule says otherwise.

## Current Behavior

The intended lifecycle is `0 -> 1 -> 2`: funding moves an escrow from open to
funded once the funding target is reached, and settlement moves it to settled.
`settle` rejects any escrow that is not funded and stores the amount settled for
partial settlement accounting.

In the checked-out source, `claim_investor_payout` currently reads the escrow
status and accepts either status `2` or status `1` with a positive
`DataKey::SettledAmount`. The delegated claim entrypoint contains the same
claimability predicate. This appears to be a partial mitigation of the
catalogued issue and must be compared with the deployed WASM and the backlog
revision that originally reported the gap.

The security issue remains actionable until the guard is confirmed in the
release artifact and protected by regression tests. In particular, the direct
and delegated paths must not drift, and a stale or inconsistent
`SettledAmount` value must not make an open/unfunded escrow claimable.

## Steps to Reproduce

### Vulnerable-version reproduction

1. Deploy or check out the revision containing the vulnerable claim path.
2. Initialize an escrow with a funding target greater than the first investor's
   contribution, for example a target of `1,000` units.
3. Fund the escrow with `100` units from investor `A`. Confirm that
   `funded_amount == 100` and `status == 0` (`OPEN`), because the target has not
   been reached.
4. Authenticate as investor `A` and call `claim_investor_payout(A)`.
5. Inspect the result, investor claim marker/history, payout event, and any
   payout transfer.
6. Repeat through `claim_investor_payout_as_delegate` after configuring a
   valid delegate for investor `A`.
7. Observe that the vulnerable implementation can proceed despite the escrow
   not being funded or settled.

### Regression reproduction for the checked-out source

1. Run `cargo test -p escrow claim_investor_payout_before_settle_panics`.
2. Add or run an equivalent case where the investor has a positive contribution
   below the funding target and calls the claim entrypoint.
3. Assert the call fails with `EscrowError::InvestorClaimNotSettled` (127),
   leaves the claim marker and investor history unchanged, and emits no
   successful payout-claim event.
4. Repeat the assertion for the delegated claim entrypoint.

## Expected Behavior

- A claim is rejected unless the escrow is in an explicitly claimable state.
- An open/unfunded escrow (`status == 0`) always rejects claims, even when the
  caller has contributed funds or `SettledAmount` is unexpectedly present.
- A funded escrow (`status == 1`) is claimable only when partial settlement is
  intentionally supported and `SettledAmount > 0`; a zero, missing, negative,
  or inconsistent settled amount is rejected.
- A fully settled escrow (`status == 2`) can be claimed by a participating
  investor after all other applicable guards pass.
- Rejected claims do not set claimed state, append payout history, transfer
  tokens, or emit a successful claim event.
- Direct and delegated claim entrypoints enforce identical escrow-state rules.
- The failure uses the typed `InvestorClaimNotSettled` error (code 127), or a
  newly introduced, documented error only if the existing code cannot express
  the invariant clearly.

## Actual Behavior

The catalogued defect reports that `claim_investor_payout` does not reliably
validate escrow status before allowing a payout claim. That permits an investor
with a positive contribution to attempt a claim while the escrow is still
unfunded/open, bypassing the lifecycle precondition that settlement must occur
first.

The current source includes a status predicate, but it is not sufficient to
close the backlog item without verification: the deployed artifact may predate
it, the delegated path may differ in another release, and a positive
`SettledAmount` must not override an open status. The release process should
therefore treat this as a security regression until both entrypoints and the
artifact are tested.

## Proposed Solution

1. Load the escrow state before any claim state mutation or payout calculation.
2. Add one shared claimability helper used by both direct and delegated claims.
   It should require `status == 2`, or `status == 1` plus a valid positive
   partial-settlement amount when partial claims are a supported feature.
3. Explicitly reject `status == 0`, `status >= 3`, and inconsistent settlement
   metadata. Do not infer settlement from investor contribution, token balance,
   maturity, or the existence of a claim-related storage key.
4. Keep the guard before claim-marker writes, history writes, event emission,
   payout transfers, and any externally observable side effects.
5. Preserve the existing ordering of authentication, legal-hold/dispute-pause,
   contribution, status, lock/maturity, and idempotency checks unless the
   security review identifies a reason to change it.
6. Add direct and delegated regression tests for below-target funding, zero
   settled amount, valid partial settlement, full settlement, withdrawn state,
   cancelled state, and stale/inconsistent settlement metadata.
7. Verify the compiled WASM hash and deployed contract behavior match the fixed
   source before closing the issue.

## Environment Context

- **Repository:** `KARIS-KY`
- **Contract:** `escrow` Soroban smart contract
- **Platform:** Stellar Soroban
- **Language/toolchain:** Rust, Cargo, Soroban SDK
- **Entrypoints:** `claim_investor_payout`; delegated equivalent
  `claim_investor_payout_as_delegate`
- **Lifecycle states:** `0 = open`, `1 = funded`, `2 = settled`,
  `3 = withdrawn`, `4 = cancelled`
- **Relevant storage:** `DataKey::Escrow`, `DataKey::SettledAmount`, and
  persistent per-investor contribution/claim records
- **Relevant error:** `EscrowError::InvestorClaimNotSettled` (127)
- **Verification commands:** `cargo test -p escrow`; focused settlement and
  delegation tests; deployed-WASM/hash verification on each supported network
- **Operational impact:** investor accounting, payout events, indexers, and
  downstream reconciliation may all be affected if an invalid claim is
  recorded or paid.

## Acceptance Criteria

- [ ] The direct claim entrypoint rejects every open/unfunded escrow with typed
      error 127, even when the caller has a positive contribution.
- [ ] The delegated claim entrypoint applies the same status and settlement
      validation as the direct entrypoint.
- [ ] Status `1` claims are accepted only for a documented valid partial
      settlement with `SettledAmount > 0`; zero, negative, missing, or stale
      values cannot make a claimable state.
- [ ] Status `2` claims continue to work for eligible investors, including
      existing full-settlement behavior.
- [ ] Statuses `3` and `4` are rejected unless a separate documented payout
      policy explicitly allows them.
- [ ] Validation occurs before claim-marker/history writes, payout transfers,
      and successful claim-event emission.
- [ ] Failed claims leave escrow state, investor claim state, history, balances,
      and successful-event output unchanged.
- [ ] Regression tests cover below-target funding, unfunded direct claim,
      unfunded delegated claim, valid partial settlement, full settlement,
      withdrawn/cancelled states, and inconsistent settlement metadata.
- [ ] Tests confirm repeated valid claims remain idempotent and do not weaken
      the status guard.
- [ ] The fixed source, generated WASM, and deployed contract behavior are
      verified to be aligned before release.
- [ ] State-machine, error, SDK, and operator documentation describe the claim
      precondition and the status meanings accurately.

## Assignment Notes

Before assignment, confirm which deployed version introduced the gap and whether
any network has accepted or recorded invalid claims. Review event/indexer data
for claims made before settlement, determine whether remediation or investor
notification is needed, and compare the deployed WASM hash with the checked-out
source. The implementation should remain narrowly scoped to claimability
validation and regression coverage; accounting remediation, if required,
should be tracked separately.
