# Escrow Contract Fuzzing Guide

## Quick Start

### Prerequisites

Ensure you have the Rust nightly toolchain and cargo-fuzz installed:

```bash
rustup install nightly
cargo install cargo-fuzz
```

### Running the Fuzz Targets

From the `escrow/` directory:

```bash
# Run a single fuzz target indefinitely
cd escrow
cargo +nightly fuzz run escrow_funding_operations

# Run all fuzz targets (each indefinitely)
cargo +nightly fuzz run --all

# Run with a time limit (60 seconds)
cargo +nightly fuzz run escrow_funding_operations -- -max_total_time=60

# Run with multiple workers (faster exploration)
cargo +nightly fuzz run escrow_funding_operations -- -jobs=4 -workers=4

# Run with input size limit and timeout
cargo +nightly fuzz run escrow_funding_operations -- -max_len=10240 -timeout=5
```

## Fuzz Targets Overview

### 1. **escrow_funding_operations**

Tests funding invariants with random investor deposits.

**Invariants:**
- Funded amount ≤ funding target (in open status)
- Funded amount ≤ invoice amount
- Yield bps ∈ [0, 10000]
- Status advances when funded ≥ target
- Immutable fields remain unchanged

**Typical inputs:**
- Invoice amounts (1..1 billion)
- Funding targets
- Multiple investor deposits
- Yield rates

**Quick run:**
```bash
cargo +nightly fuzz run escrow_funding_operations -- -max_total_time=30
```

**Expected result:** ✅ All invariants pass (no crashes)

---

### 2. **escrow_settlement_flow**

Tests settlement operations and time-lock enforcement.

**Invariants:**
- Snapshot exists post-funding
- Status = 2 after settlement
- Escrow data immutable post-settlement
- Cannot re-settle
- Status monotonically increasing

**Typical inputs:**
- Maturity offsets
- Time advancement steps
- Early settlement attempts
- Multiple investor funds

**Quick run:**
```bash
cargo +nightly fuzz run escrow_settlement_flow -- -max_total_time=30
```

**Expected result:** ✅ All invariants pass, time-lock respected

---

### 3. **escrow_yield_calculations**

Tests yield computation and investor payouts.

**Invariants:**
- Yield bps ∈ [0, 10000]
- Snapshot total ≥ funded amount
- Effective yield ≤ base yield
- Claim locks in past/at settlement
- Yield immutable post-settlement
- No double-claims

**Typical inputs:**
- Base yields (0..10000 bps)
- Investor contributions
- Settlement claims
- Multiple investors

**Quick run:**
```bash
cargo +nightly fuzz run escrow_yield_calculations -- -max_total_time=30
```

**Expected result:** ✅ All invariants pass, yields valid

---

### 4. **escrow_attestations**

Exercises arbitrary append, revoke, and filtered/paginated query sequences.

**Invariants:**
- Append-log length never exceeds `MAX_ATTESTATION_APPEND_ENTRIES` (32)
- Invalid or repeated revocations fail without panicking the harness
- Queries do not panic and returned pages/counts remain within capacity

**Quick run:**
```bash
./run_fuzz.sh escrow_attestations 60 4
```

**Expected result:** ✅ Arbitrary operation sequences preserve the bounded log

---

## Corpus Files

The fuzzer builds a corpus of interesting inputs in `fuzz/corpus/`:

```bash
# View corpus for a target
ls -la escrow/fuzz/corpus/escrow_funding_operations/

# Corpus automatically replayed on each run (regression testing)
# To manually add a corpus entry:
cp my_interesting_input escrow/fuzz/corpus/escrow_funding_operations/
```

## Reproducing Crashes

If fuzzing finds a crash:

```bash
# List crash artifacts
ls escrow/fuzz/artifacts/escrow_funding_operations/

# Reproduce the crash
cargo +nightly fuzz run escrow_funding_operations -- \
  escrow/fuzz/artifacts/escrow_funding_operations/crash-abc123
```

The crash input will be printed with the invariant violation message, enabling debugging.

## Advanced Fuzzing

### Parallel Fuzzing (Faster)

```bash
# Use 4 workers in parallel for faster coverage
cargo +nightly fuzz run escrow_funding_operations -- \
  -jobs=4 -workers=4 -max_total_time=300
```

### Custom Input Sizes

```bash
# Limit to 5KB inputs (faster execution)
cargo +nightly fuzz run escrow_funding_operations -- -max_len=5120

# Allow larger inputs (slower but better coverage)
cargo +nightly fuzz run escrow_funding_operations -- -max_len=50000
```

### Merge Corpus Files

```bash
# Combine corpus from multiple runs
cargo +nightly fuzz cmin escrow_funding_operations \
  fuzz/corpus/escrow_funding_operations/ \
  /path/to/other/corpus/
```

## Integration with CI

### Nightly Fuzz Job

A scheduled workflow runs the fuzz targets every night. It is defined in
`.github/workflows/ci.yml` (job `fuzz-nightly`) — see that file for the
authoritative configuration.

**Schedule:** Runs nightly at 02:00 UTC via a cron trigger
(`schedule: - cron: '0 2 * * *'`). It can also be started manually with
`workflow_dispatch`.

**Duration and workers:** Each fuzz target runs for a bounded duration of
**300 seconds** (`-max_total_time=300`) with **4 parallel workers**
(`-jobs=4 -workers=4`), driven by `escrow/run_fuzz.sh`. The run is bounded so
the job always terminates; it is not always-on fuzzing.

**Failure surfacing:** Any crash or invariant violation causes the fuzz target
to exit non-zero, which fails the CI job and surfaces the failure in the
Actions run. Crash artifacts are uploaded as workflow artifacts for
reproduction (they are not committed to git).

To run the same bounded fuzzing locally:

```bash
cd escrow
./run_fuzz.sh escrow_funding_operations 300 4
./run_fuzz.sh escrow_settlement_flow 300 4
./run_fuzz.sh escrow_yield_calculations 300 4
./run_fuzz.sh escrow_attestations 300 4
```

For pull requests, a shorter bounded fuzz run may also be triggered; see
`.github/workflows/ci.yml` for the exact triggers and durations.

## Interpreting Fuzz Output

```
#0      READ units: 0
#1024   READ units: 1  L: 32/1024 MS: 4 ShuffleBytes-
#2048   READ units: 2  L: 64/1024 MS: 3 CopyPart-
...
#1000000 READ units: 15 L: 256/1024 MS: 5 MutationDispatch-
```

Key fields:
- `#N` — Number of iterations
- `READ units` — Corpus size
- `L: A/B` — Input length / max length
- `MS` — Mutation strategy applied
- `rss` — Memory usage
- `time` — Elapsed time
- `units/s` — Iterations per second

## Debugging a Failure

When an invariant fails:

```
thread 'main' panicked at 'INVARIANT VIOLATION: funded_amount (999999999) > funding_target (500000000) in open status'
```

1. **Note the input:** Fuzzer saves to `fuzz/artifacts/escrow_funding_operations/crash-*`
2. **Reproduce locally:**
   ```bash
   cargo +nightly fuzz run escrow_funding_operations -- \
     escrow/fuzz/artifacts/escrow_funding_operations/crash-abc123
   ```
3. **Add test case:** Extract the input and add to `escrow/src/tests/` for permanent regression testing
4. **Fix the bug:** Modify the contract to respect the invariant
5. **Verify:** Re-run the fuzz target to confirm fix

## Performance Tuning

| Issue | Solution |
|-------|----------|
| Too slow | Reduce `-max_len`, use `-jobs=4` |
| Too much memory | Reduce `-max_len`, limit corpus size |
| Not finding bugs | Increase `-max_total_time`, add corpus seeds |
| Corpus too large | Use `cargo +nightly fuzz cmin` to minimize |

## Expected Runtime

On a modern laptop:

| Target | 30 seconds | Coverage |
|--------|------------|----------|
| escrow_funding_operations | ~5-10K iterations | Good |
| escrow_settlement_flow | ~2-5K iterations | Good |
| escrow_yield_calculations | ~3-7K iterations | Good |

Longer runs (hours) on continuous integration servers find more edge cases.

## Troubleshooting

### "Found an interesting crash"

This means an invariant violation was found. See "Reproducing Crashes" above.

### "Compiler errors"

Ensure Soroban SDK and dependencies are compatible:

```bash
cd escrow
cargo update
cargo +nightly fuzz run escrow_funding_operations
```

### "Timeout (exit code 77)"

An input takes too long. Either:

1. Reduce input size: `cargo +nightly fuzz run escrow_funding_operations -- -max_len=5120`
2. Increase timeout: `cargo +nightly fuzz run escrow_funding_operations -- -timeout=10`
3. Optimize contract code

### "Out of memory"

The fuzzer is using too much memory:

```bash
# Reduce input size and run single-threaded
cargo +nightly fuzz run escrow_funding_operations -- -max_len=5120 -jobs=1
```

## Files

```
escrow/
├── fuzz/
│   ├── Cargo.toml                          # Fuzz harness crate config
│   ├── README.md                           # Detailed fuzz documentation
│   ├── fuzz_targets/
│   │   ├── escrow_funding_operations.rs    # Funding invariant tests
│   │   ├── escrow_settlement_flow.rs       # Settlement flow tests
│   │   └── escrow_yield_calculations.rs    # Yield calculation tests
│   ├── corpus/                              # Interesting inputs (auto-populated)
│   │   ├── escrow_funding_operations/
│   │   ├── escrow_settlement_flow/
│   │   └── escrow_yield_calculations/
│   ├── artifacts/                           # Crashes (auto-populated)
│   │   ├── escrow_funding_operations/
│   │   ├── escrow_settlement_flow/
│   │   └── escrow_yield_calculations/
│   └── .gitignore
```

## See Also

- `escrow/fuzz/README.md` — Detailed fuzz harness documentation
- `escrow/src/lib.rs` — Contract implementation
- `docs/adr/` — Architecture decision records
