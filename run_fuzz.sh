#!/bin/bash
# Quick fuzzing runner for karis-ky escrow contract
# Usage: ./run_fuzz.sh [target] [duration] [workers]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/escrow"

# Defaults
TARGET="${1:-escrow_funding_operations}"
DURATION="${2:-60}"
WORKERS="${3:-4}"

case "$TARGET" in
    escrow_funding_operations|escrow_settlement_flow|escrow_yield_calculations|escrow_migrate|escrow_attestations)
        ;;
    *)
        echo -e "${RED}Error: unknown target '$TARGET'.${NC}" >&2
        echo "Available targets: escrow_funding_operations, escrow_settlement_flow, escrow_yield_calculations, escrow_migrate, escrow_attestations" >&2
        exit 2
        ;;
esac

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Karis-KY Escrow Fuzzer ===${NC}"
echo "Target: $TARGET"
echo "Duration: ${DURATION}s"
echo "Workers: $WORKERS"
echo ""

# Check for nightly
if ! command -v rustup &> /dev/null; then
    echo -e "${RED}Error: rustup not found. Please install Rust.${NC}"
    exit 1
fi

echo -e "${YELLOW}Installing/updating nightly toolchain...${NC}"
rustup toolchain install nightly --quiet

# Check for cargo-fuzz
if ! cargo +nightly fuzz --version &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-fuzz...${NC}"
    cargo install cargo-fuzz
fi

echo -e "${YELLOW}Starting fuzzer...${NC}"
echo ""

# Determine libfuzzer flags based on workers
if [ "$WORKERS" -gt 1 ]; then
    LIBFUZZER_FLAGS="-max_total_time=$DURATION -jobs=$WORKERS -workers=$WORKERS -max_len=10240"
else
    LIBFUZZER_FLAGS="-max_total_time=$DURATION -max_len=10240"
fi

# Run the fuzzer
if cargo +nightly fuzz run "$TARGET" -- $LIBFUZZER_FLAGS; then
    echo ""
    echo -e "${GREEN}✓ Fuzzing completed successfully!${NC}"
    echo "Corpus saved to: fuzz/corpus/$TARGET/"
else
    EXIT_CODE=$?
    echo ""
    echo -e "${RED}✗ Fuzzing exited with code $EXIT_CODE${NC}"
    if [ -d "fuzz/artifacts/$TARGET" ]; then
        CRASH_COUNT=$(find "fuzz/artifacts/$TARGET" -name "crash-*" | wc -l)
        if [ "$CRASH_COUNT" -gt 0 ]; then
            echo -e "${RED}Found $CRASH_COUNT crash(es):${NC}"
            find "fuzz/artifacts/$TARGET" -name "crash-*" -exec ls -lh {} \;
            echo ""
            echo "To reproduce:"
            echo "  cargo +nightly fuzz run $TARGET -- fuzz/artifacts/$TARGET/crash-*"
        fi
    fi
    exit $EXIT_CODE
fi
