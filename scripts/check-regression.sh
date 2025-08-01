#!/bin/bash

# Performance regression detection script
# Compares current benchmark results against baseline
# Exit code: 0 if no regression, 1 if regression detected

set -e

BASELINE_DIR="./target/criterion"
THRESHOLD_PERCENT=5  # Consider >5% slowdown as regression

echo "=== Performance Regression Check ==="

# Function to extract time from criterion JSON
extract_time() {
    local json_file=$1
    if [ -f "$json_file" ]; then
        # Extract mean time in nanoseconds
        jq -r '.mean.point_estimate' "$json_file" 2>/dev/null || echo "0"
    else
        echo "0"
    fi
}

# Check if we have baseline data
if [ ! -d "$BASELINE_DIR" ]; then
    echo "No baseline data found. This run will establish the baseline."
    exit 0
fi

# Run benchmarks and save results
echo "Running benchmarks..."
cargo bench --bench ring_buffer_benches -- --save-baseline current

REGRESSION_FOUND=0

# Compare each benchmark group
for group_dir in "$BASELINE_DIR"/*; do
    if [ -d "$group_dir" ]; then
        group_name=$(basename "$group_dir")
        echo ""
        echo "Checking $group_name..."
        
        # Check each benchmark in the group
        for bench_dir in "$group_dir"/*; do
            if [ -d "$bench_dir" ]; then
                bench_name=$(basename "$bench_dir")
                
                # Skip if not a benchmark directory
                if [ "$bench_name" = "base" ] || [ "$bench_name" = "new" ] || [ "$bench_name" = "current" ]; then
                    continue
                fi
                
                # Get baseline and current times
                baseline_file="$bench_dir/base/estimates.json"
                current_file="$bench_dir/new/estimates.json"
                
                if [ ! -f "$current_file" ]; then
                    current_file="$bench_dir/current/estimates.json"
                fi
                
                if [ -f "$baseline_file" ] && [ -f "$current_file" ]; then
                    baseline_time=$(extract_time "$baseline_file")
                    current_time=$(extract_time "$current_file")
                    
                    if [ "$baseline_time" != "0" ] && [ "$current_time" != "0" ]; then
                        # Calculate percentage change
                        change=$(awk "BEGIN {printf \"%.2f\", (($current_time - $baseline_time) / $baseline_time) * 100}")
                        
                        # Check if regression threshold exceeded
                        if (( $(echo "$change > $THRESHOLD_PERCENT" | bc -l) )); then
                            echo "  ❌ REGRESSION: $bench_name is ${change}% slower (threshold: ${THRESHOLD_PERCENT}%)"
                            REGRESSION_FOUND=1
                        elif (( $(echo "$change < -$THRESHOLD_PERCENT" | bc -l) )); then
                            echo "  ✅ IMPROVEMENT: $bench_name is ${change#-}% faster"
                        else
                            echo "  ✓ $bench_name: ${change}% change (within threshold)"
                        fi
                    fi
                fi
            fi
        done
    fi
done

echo ""
if [ $REGRESSION_FOUND -eq 1 ]; then
    echo "❌ Performance regression detected!"
    exit 1
else
    echo "✅ No performance regressions detected"
    exit 0
fi