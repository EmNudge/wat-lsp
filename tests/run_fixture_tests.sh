#!/bin/bash
# Run all fixture tests through wat-check to verify they parse without errors
# Usage: ./tests/run_fixture_tests.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "Building wat-check..."
cargo build --release --bin wat-check

echo ""
echo "Running fixture tests..."
echo "========================"

FAILED=0
PASSED=0

for file in "$SCRIPT_DIR"/fixtures/*.wat; do
    if [ -f "$file" ]; then
        filename=$(basename "$file")
        if "$PROJECT_DIR/target/release/wat-check" "$file" 2>&1; then
            echo "PASS: $filename"
            ((PASSED++))
        else
            echo "FAIL: $filename"
            ((FAILED++))
        fi
    fi
done

echo ""
echo "========================"
echo "Results: $PASSED passed, $FAILED failed"

if [ $FAILED -gt 0 ]; then
    exit 1
fi

echo "All fixture tests passed!"
