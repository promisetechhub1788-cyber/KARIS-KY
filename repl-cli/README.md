# KARIS-KY REPL CLI - Health Check Commands

This is a REPL (Read-Eval-Print Loop) command-line interface for the KARIS-KY escrow contract, with a focus on health check and diagnostic commands for on-call operators.

## Overview

The REPL CLI provides a command-line interface to interact with the KARIS-KY escrow contract deployed on the Stellar network. Currently, it exposes health check and health metrics endpoints with color-coded output to make diagnostics quick and easy for operators.

## Features

- **Health Check Command** (`check_health`) - Quick assessment of escrow health status
- **Health Metrics Command** (`get_health`) - Detailed metrics view
- **Color-Coded Output** - Visual indicators for severity levels:
  - 🟢 **Green** - Healthy status (code 0)
  - 🟡 **Yellow** - Warning conditions (codes 4001, 4002, 4004)
  - 🔴 **Red** - Critical conditions (code 4003)
- **Interactive REPL Loop** - Command history, line editing, autocomplete

## Installation & Build

### Prerequisites

- Rust 1.75+ (for edition 2021)
- Cargo
- Stellar testnet/mainnet access (for live contract calls)

### Build

From the workspace root:

```bash
cargo build -p repl-cli --release
```

The binary will be at `target/release/repl-cli` (or `repl-cli.exe` on Windows).

### Run

```bash
cargo run -p repl-cli

# Or directly with the binary:
./target/release/repl-cli
```

## Commands Reference

### `check_health`

**Purpose**: Perform a quick health check on the escrow contract.

**Returns**: 
- `warning_type` - Health warning code (u32)
  - `0` - No warning (healthy)
  - `4001` - Low funding ratio (< 50% funded)
  - `4002` - Close to maturity (< 1 day remaining)
  - `4003` - Over maturity (past maturity, unfunded, and open)
  - `4004` - Reserved for future use

- `funded_ratio_bps` - Funding ratio in basis points (0–10,000+)
  - Example: `5000` = 50% funded
  
- `time_to_maturity_secs` - Seconds until maturity
  - Positive = future maturity
  - Negative = past maturity
  - `i64::MAX` = no maturity constraint

**Output**: Color-coded status line with warning description.

**Usage**:
```
repl-cli> check_health

═══════════════════════════════════════════════════════
ESCROW HEALTH CHECK
═══════════════════════════════════════════════════════

Status: ✓ HEALTHY
Warning Code: 0
Description: Healthy - No warning

Funding Ratio: 75.00%
Time to Maturity: 5 days, 0 hours

═══════════════════════════════════════════════════════
```

**Severity Mapping**:
| Code | Severity | Condition |
|------|----------|-----------|
| 0 | ✓ Green | Healthy |
| 4001 | ⚠ Yellow | Low funding ratio (< 50%) |
| 4002 | ⚠ Yellow | Close to maturity (< 1 day) |
| 4003 | ✗ Red | Over maturity (past maturity, unfunded, open) |

### `get_health`

**Purpose**: Retrieve detailed health metrics for the escrow contract.

**Returns**:
- `funding_progress_percent` (u32) - Funding progress as percentage (0–100)
- `days_to_maturity` (i64) - Days until maturity
  - Negative if past maturity
  - 0 if no maturity constraint
  
- `unique_investor_count` (u32) - Number of unique investors
- `average_contribution_size` (i128) - Average contribution per investor (stroops)
- `estimated_yield_payout` (i128) - Total estimated yield (stroops)

**Output**: Pretty-printed table with detailed metrics.

**Usage**:
```
repl-cli> get_health

═══════════════════════════════════════════════════════
ESCROW HEALTH METRICS
═══════════════════════════════════════════════════════

Funding Progress: 75%
Days to Maturity: 5
Unique Investors: 42
Avg. Contribution: 238095 stroops
Est. Yield Payout: 500000 stroops

═══════════════════════════════════════════════════════
```

### `help [command]`

**Purpose**: Display help information for available commands.

**Usage**:
```
repl-cli> help
repl-cli> help check_health
repl-cli> help get_health
```

### `fund_with_commitment`

**Purpose**: Test and simulate first-time investor contribution with commitment lock and view selected yield tier.

**Arguments**:
- `<investor>` - The investor address (Stellar format)
- `<amount>` - Principal contribution in stroops
- `<lock_secs>` - Lock duration in seconds

**Usage**:
```
repl-cli> fund_with_commitment GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2 50000000 86400
{
  "status": "success",
  "investor": "GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2",
  "amount": 50000000,
  "lock_secs": 86400,
  "yield_tier_selected": {
    "tier_index": 1,
    "effective_yield_bps": 650
  },
  "claim_not_before": 1700086400
}
```

### `fund-batch`

**Purpose**: Fund multiple investors in a single `fund_batch` contract call from a CSV file. This speeds up batch funding for operators.

**Arguments**:
- `<path-to-csv>` - Path to a CSV file containing `(address, amount)` pairs

**CSV Format**:
- One `address,amount` pair per line
- `amount` is in stroops (integer)
- Blank lines are ignored
- Lines starting with `#` are treated as comments and ignored
- At most 50 entries per call (matching the contract's `MAX_FUND_BATCH`)

Example CSV (`investors.csv`):
```
# address,amount (stroops)
GDPM3QMXN3APYMYBNPIBMVJHD3FQJSCAEBFHDZZS3MSVVUAOTBMVYF2,50000000
GABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRSTUV,25000000
```

**Usage**:
```
repl-cli> fund-batch investors.csv
{
  "status": "success",
  "entries": 2,
  "total_amount": 75000000
}
```

### `quit` / `exit`

**Purpose**: Exit the REPL CLI.

**Usage**:
```
repl-cli> quit
repl-cli> exit
```

## Warning Codes Reference

The `check_health` command returns warning codes that indicate different health conditions:

### Code 0: Healthy ✓ (Green)
No warning condition detected. The escrow is:
- Adequately funded (≥ 50%)
- Not close to maturity (> 1 day remaining)
- Not past maturity (if maturity constraint exists)

### Code 4001: Low Funding Ratio ⚠ (Yellow)
The escrow has a low funding ratio (< 50% funded):
- If close to maturity (< 1 day), operators should accelerate fundraising
- If time permits, monitor funding progress

**Action**: Check funding progress and investor updates.

### Code 4002: Close to Maturity ⚠ (Yellow)
The escrow is approaching maturity (< 1 day remaining):
- Funding is adequate (≥ 50%)
- Operators should prepare for settlement
- Monitor for last-minute investor withdrawals

**Action**: Prepare settlement process; monitor funding stability.

### Code 4003: Over Maturity ✗ (Red) — CRITICAL
The escrow has passed maturity, remains open, and is not fully funded:
- **Critical**: Contract cannot settle until fully funded or closed by admin
- Operator action required to resolve

**Action**: Immediately contact admin; consider closing or extending escrow.

## Architecture

### Data Flow

1. **User Input** → Command Parser
2. **Command Parser** → Command Enum
3. **Command Enum** → Handler (check_health/get_health)
4. **Handler** → Contract RPC Call (Soroban)
5. **Contract Response** → Formatter
6. **Formatter** → Colored Output → Terminal

### Color Output

Uses the `colored` crate for cross-platform ANSI color support:
- 🟢 `.green()` for healthy status
- 🟡 `.yellow()` for warnings
- 🔴 `.red()` for critical issues
- 🔵 `.bright_blue()` for section headers

### REPL Loop

Uses `rustyline` for an interactive command-line interface with:
- Command history
- Line editing
- Interrupt handling (Ctrl+C)
- EOF handling (Ctrl+D)

## Configuration

### Network Selection (Future Enhancement)

The current implementation uses mock data. Future versions will support:
```
repl-cli> network switch testnet
repl-cli> network switch mainnet
```

### Contract Address (Future Enhancement)

Specify the escrow contract address:
```
repl-cli> contract set CDKLKDLKDLKDLKD...
```

## Development

### Project Structure

```
repl-cli/
├── Cargo.toml           # Dependencies and build configuration
├── README.md            # This file
└── src/
    └── main.rs          # Main REPL loop and command handlers
```

### Dependencies

- `rustyline = "14.0"` - Interactive REPL with line editing
- `colored = "2.1"` - ANSI color output
- `serde_json = "1.0"` - JSON serialization
- `soroban-cli = "21.7"` - Soroban SDK for contract interaction
- `tokio = "1.37"` - Async runtime
- `anyhow = "1.0"` - Error handling

### Adding New Commands

1. Add variant to `Command` enum in `main.rs`
2. Add parsing logic in `parse_command()`
3. Add handler in the REPL loop
4. Update help text in `display_help()`

Example:
```rust
pub enum Command {
    CheckHealth,
    GetHealth,
    MyNewCommand { arg: String },  // Add here
    // ...
}

// In parse_command():
Some("my_new_command") => {
    let arg = parts.get(1).ok_or(anyhow!("Argument required"))?;
    Ok(Command::MyNewCommand { arg: arg.to_string() })
}

// In REPL loop:
Ok(Command::MyNewCommand { arg }) => {
    // Handle command
}
```

## Troubleshooting

### Build Fails
- Ensure Rust 1.75+ is installed: `rustc --version`
- Update dependencies: `cargo update`

### Commands Not Found
- Type `help` to see available commands
- Check spelling (commands are lowercase with underscores)

### Connection Issues (Future)
- Verify network connectivity
- Check contract address: `network info`
- Review RPC endpoint configuration

## Future Enhancements

- [ ] Live contract invocation via Soroban RPC
- [ ] Network switching (testnet/mainnet)
- [ ] Contract address configuration
- [ ] State inspection commands (get, state, history)
- [ ] Debugging commands (trace, breakpoint, snapshot)
- [ ] Transaction execution (call, dry-run)
- [ ] Event history and indexing
- [ ] CSV/JSON export of metrics
- [ ] Automated alerting and notifications

## Documentation

For more information on the KARIS-KY contract health check implementation, see:
- `docs/adr/ADR-008-escrow-health-warnings.md` - Architecture decision record
- `FEATURE_220_REPL_DESIGN.md` - Full REPL CLI design specification
- `escrow/src/lib.rs` - Contract implementation

## Acceptance Criteria

✅ Both `check_health` and `get_health` commands appear in help output  
✅ Output is color-coded by warning severity (green/yellow/red)  
✅ Commands are documented in repl-cli/README.md  
✅ `cargo build` for repl-cli passes  

## License

Same as KARIS-KY main project.

## Contact & Support

For issues or questions, please open an issue on the project GitHub repository or contact the KARIS-KY team.
