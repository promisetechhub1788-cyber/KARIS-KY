# Escrow Dependency Policy (`soroban-sdk` and host compatibility)

This policy defines how maintainers track, evaluate, and roll out Soroban dependency updates for
`karis-ky_escrow`.

## 1) Version pin strategy

- Pin `soroban-sdk` and related Soroban crates in `Cargo.toml` to an explicit compatible series.
- Keep `Cargo.lock` committed so CI and reviewers execute the same resolved dependency graph.
- Avoid broad, implicit upgrades in feature branches unrelated to dependency maintenance.

## 2) Update cadence

### Weekly checks (lightweight)

- Review upstream Soroban/Stellar release notes for:
  - host behavior changes
  - VM/runtime breaking changes
  - deprecations affecting contract APIs or testutils
- Run dependency audit commands and open a tracking issue for any actionable findings.

### Monthly checks (full maintenance cycle)

- Evaluate patch/minor updates for `soroban-sdk` and direct transitive risk dependencies.
- Create a dedicated dependency-update branch.
- Run full escrow validation suite (`fmt`, `clippy`, `test`, `llvm-cov` policy gate).
- Record migration impact (if any) on contract behavior, events, or storage assumptions.

## 3) Regression testing for contract upgrades

For every dependency bump candidate:

1. Run existing CI gate commands from `README.md`.
2. Re-run scenario-critical tests:
   - funding to funded transition
   - settlement and claim path
   - legal hold gating
   - dust sweep guards
3. Validate event compatibility for indexers (`docs/EVENT_SCHEMA.md` expectations).
4. Confirm no accidental storage schema drift unless intentionally planned.

If behavior changes are detected, document them in the PR and propose explicit migration/redeploy
guidance before merge.

## 4) Tracking upstream breaking host changes

Maintainers should track Soroban host/runtime notes as first-class input for release risk:

- Monitor upstream release notes and advisories on each weekly check.
- Flag any host-level semantic change that could affect:
  - auth boundaries
  - event shape/order assumptions
  - token call behavior and balance-delta checks
  - ledger timestamp/sequence assumptions in tests

Open a dependency-risk issue immediately when uncertain impact exists, even before code changes.

## 5) Emergency bump process (security advisory or critical regression)

When a high-severity advisory or breakage is announced:

1. Open an incident issue (`severity`, affected versions, suspected blast radius).
2. Create emergency branch (example: `hotfix/deps-soroban-<version>`).
3. Apply minimal dependency bump and lockfile update.
4. Run mandatory checks:
   - `cargo fmt --all -- --check`
   - `cargo clippy -p karis-ky_escrow -- -D warnings`
   - `cargo test -p karis-ky_escrow`
   - `cargo llvm-cov --features testutils --fail-under-lines 95 --summary-only -p karis-ky_escrow`
5. Perform merge dry-run against `upstream/main`.
6. Open PR with:
   - advisory reference
   - risk assessment
   - rollback plan
   - explicit note on token-economics assumptions remaining out of scope per
     `escrow/src/external_calls.rs`.

## 6) Automated dependency gating (cargo-deny and npm audit)

Dependency hygiene is enforced in CI by two complementary gates. Both must pass before a
dependency-related PR can merge.

### 6.1) `cargo-deny` policy

`cargo-deny` runs against the committed `Cargo.lock` and enforces the following checks:

- **advisories**: fails on any vulnerability with a severity at or above `medium`. Lower-severity
  advisories may be accepted only via an explicit, justified entry in `deny.toml`.
- **licenses**: only the licenses listed below are allowlisted. Any new license requires a policy
  update in this document and a reviewer sign-off before the allowlist entry is added.
- **bans**: duplicate crate versions are denied by default. The crates listed below are explicitly
  skipped because their duplication is transitive and not actionable by this repository.
- **sources**: only crates.io and the pinned Soroban git sources are permitted.

#### Allowlisted licenses

| License | Rationale |
| --- | --- |
| `MIT` | Permissive, compatible with project licensing. |
| `Apache-2.0` | Permissive, compatible with project licensing. |
| `Apache-2.0 WITH LLVM-exception` | Required by Rust toolchain-adjacent crates. |
| `BSD-2-Clause` | Permissive, no copyleft obligations. |
| `BSD-3-Clause` | Permissive, no copyleft obligations. |
| `ISC` | Permissive, functionally equivalent to MIT. |
| `Unicode-DFS-2016` | Required by Unicode data crates. |
| `Zlib` | Permissive, no copyleft obligations. |

Any license outside this table (including copyleft licenses such as `GPL-*`, `AGPL-*`, or `MPL-*`)
fails the gate and must be resolved by replacing the dependency or by an explicit ADR.

#### Skipped duplicate crates (`bans.skip`)

| Crate | Justification |
| --- | --- |
| `syn` | Multiple major versions are pulled in transitively by proc-macro dependencies; we cannot unify them without upstream changes. |
| `windows-sys` | Version skew originates from platform-specific transitive dependencies and does not affect the escrow contract build target. |

Skipped duplicates are reviewed on each monthly maintenance cycle. If a skip is no longer required
because upstream has unified versions, remove the entry.

#### Temporarily accepted advisories

Accepted advisories are recorded in `deny.toml` with an `id`, a `reason`, and an expiry/review date.
Each entry must include:

- the advisory identifier (e.g. `RUSTSEC-YYYY-NNNN`),
- the affected crate and version range,
- a justification explaining why the escrow contract is not exploitable,
- a review cadence (default: re-evaluated on the next monthly maintenance cycle).

There are currently no temporarily accepted Rust advisories. When one is added, it must be listed
here in the same PR that updates `deny.toml`.

#### Ignored-advisory parity between `deny.toml` and `cargo audit`

CI runs both `cargo deny` (advisories check) and `cargo audit`. To prevent the two tools from
drifting apart, the set of ignored advisories must be identical in both configurations:

- `deny.toml` — `[advisories] ignore` entries.
- `.cargo/audit.toml` — `[advisories] ignore` entries.

`scripts/check_advisory_ignore_sync.py` diffs the two ignore lists and fails CI when they diverge
(an advisory ignored in one file but not the other). The script is run as a CI step alongside the
existing dependency gates.

Policy for accepting an advisory:

- Any newly accepted advisory must be added to **both** `deny.toml` and `.cargo/audit.toml` in the
  **same PR**.
- Each entry must include a justification and a review cadence (default: re-evaluated on the next
  monthly maintenance cycle), and be listed in the "Temporarily accepted advisories" section above.
- Removing an advisory from the ignore lists must likewise be done in both files in the same PR.

### 6.2) `npm audit` gating

The JavaScript toolchain (docs, linting, and CI helper scripts) is gated by `npm audit`:

- CI runs `npm audit --audit-level=high` and fails on any `high` or `critical` advisory.
- `moderate` and `low` advisories do not fail the build but are reported in the CI log and triaged
  during the weekly check.
- Fixes are applied via `npm audit fix` where safe; breaking upgrades require a dedicated PR.

#### Temporarily accepted npm advisories

Temporarily accepted npm advisories are recorded in this table. Each entry requires a
justification and a review cadence (default: re-evaluated on the next monthly maintenance cycle).

| Advisory | Package | Justification | Review cadence |
| --- | --- | --- | --- |
| _(none)_ | — | No npm advisories are currently accepted. | Monthly |

When an advisory is accepted, add a row here in the same PR that adds the corresponding
`overrides`/`audit` configuration, and reference the tracking issue.

## 7) Scope boundaries

- This policy governs dependency update process and verification, not token-economics support
  expansion.
- Unsupported token models (fee-on-transfer/rebasing/hook behavior) remain out of scope unless
  explicitly accepted in a separate ADR and implementation PR.
