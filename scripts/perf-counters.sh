#!/bin/bash

# Collect hardware performance counters during benchmark execution
# Requires Linux perf tools

set -e

OUTPUT_DIR="./target/perf-results"
mkdir -p "$OUTPUT_DIR"

echo "=== Collecting Hardware Performance Counters ==="

# Check if perf is available
if ! command -v perf &> /dev/null; then
    echo "perf command not found. Installing perf tools is required for hardware counter collection."
    echo "On Ubuntu/Debian: sudo apt-get install linux-tools-common linux-tools-generic linux-tools-$(uname -r)"
    exit 1
fi

# Enable perf events
echo "Enabling perf events (requires sudo)..."
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid > /dev/null

# Define the events to monitor
EVENTS="cpu-cycles,instructions,cache-references,cache-misses,branch-instructions,branch-misses,L1-dcache-loads,L1-dcache-load-misses"

echo "Monitoring events: $EVENTS"

# Run benchmarks with perf record
echo "Running benchmarks with performance counter collection..."

# Throughput benchmark with perf stat
sudo perf stat -e "$EVENTS" -o "$OUTPUT_DIR/throughput-perf.txt" \
    cargo bench --bench ring_buffer_benches throughput_benchmark

# Latency benchmark with perf stat
sudo perf stat -e "$EVENTS" -o "$OUTPUT_DIR/latency-perf.txt" \
    cargo bench --bench ring_buffer_benches latency_benchmark

# Boundary benchmark with perf stat
sudo perf stat -e "$EVENTS" -o "$OUTPUT_DIR/boundary-perf.txt" \
    cargo bench --bench ring_buffer_benches boundary_benchmark

# Contention benchmark with perf stat
sudo perf stat -e "$EVENTS" -o "$OUTPUT_DIR/contention-perf.txt" \
    cargo bench --bench ring_buffer_benches contention_benchmark

# Generate summary report
echo ""
echo "=== Performance Counter Summary ==="

for file in "$OUTPUT_DIR"/*.txt; do
    if [ -f "$file" ]; then
        benchmark=$(basename "$file" -perf.txt)
        echo ""
        echo "--- $benchmark ---"
        grep -E "(instructions|cache-misses|branch-misses)" "$file" | head -n 6
    fi
done

# Calculate key metrics
echo ""
echo "=== Key Metrics ==="

# Extract and calculate IPC (Instructions Per Cycle)
for file in "$OUTPUT_DIR"/*.txt; do
    if [ -f "$file" ]; then
        benchmark=$(basename "$file" -perf.txt)
        
        # Extract values using awk
        cycles=$(grep "cpu-cycles" "$file" | awk '{print $1}' | tr -d ',')
        instructions=$(grep "instructions" "$file" | awk '{print $1}' | tr -d ',')
        
        if [ -n "$cycles" ] && [ -n "$instructions" ] && [ "$cycles" != "0" ]; then
            ipc=$(awk "BEGIN {printf \"%.3f\", $instructions / $cycles}")
            echo "$benchmark IPC: $ipc"
        fi
        
        # Cache miss rate
        cache_refs=$(grep "cache-references" "$file" | awk '{print $1}' | tr -d ',')
        cache_misses=$(grep "cache-misses" "$file" | awk '{print $1}' | tr -d ',')
        
        if [ -n "$cache_refs" ] && [ -n "$cache_misses" ] && [ "$cache_refs" != "0" ]; then
            miss_rate=$(awk "BEGIN {printf \"%.2f\", ($cache_misses / $cache_refs) * 100}")
            echo "$benchmark Cache Miss Rate: ${miss_rate}%"
        fi
        
        # Branch misprediction rate
        branch_inst=$(grep "branch-instructions" "$file" | awk '{print $1}' | tr -d ',')
        branch_misses=$(grep "branch-misses" "$file" | awk '{print $1}' | tr -d ',')
        
        if [ -n "$branch_inst" ] && [ -n "$branch_misses" ] && [ "$branch_inst" != "0" ]; then
            mispredict_rate=$(awk "BEGIN {printf \"%.2f\", ($branch_misses / $branch_inst) * 100}")
            echo "$benchmark Branch Misprediction Rate: ${mispredict_rate}%"
        fi
        
        echo ""
    fi
done

echo "Performance counter results saved to: $OUTPUT_DIR"