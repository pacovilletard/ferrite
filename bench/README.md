# Ferrite Benchmark Suite

This module contains comprehensive benchmarks for the Ferrite ring buffer implementation.

## Benchmarks

### 1. Throughput Benchmark (`spsc_throughput`)
Measures sustained throughput for various buffer sizes (1K, 4K, 16K, 64K elements).
- Tests producer/consumer pattern performance
- Reports operations per second

### 2. Latency Benchmark (`spsc_latency`)
Measures operation latencies with percentile analysis:
- Push latency when buffer is empty
- Pop latency with single element
- Provides P50, P90, P95, P99 percentiles

### 3. Boundary Benchmark (`spsc_boundaries`)
Tests edge cases and boundary conditions:
- Empty/full state transitions
- Buffer wrap-around behavior
- Memory access patterns at boundaries

### 4. Contention Benchmark (`spsc_contention`)
Simulates different workload patterns:
- Producer-heavy workload (10:1 ratio)
- Consumer-heavy workload (1:10 ratio)
- Tests behavior under asymmetric load

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark group
cargo bench throughput
cargo bench latency
cargo bench boundary
cargo bench contention

# Generate flamegraphs (requires cargo-flamegraph)
cargo flamegraph --bench ring_buffer_benches

# Collect hardware performance counters (Linux only)
./scripts/perf-counters.sh
```

## Performance Regression Detection

The CI pipeline automatically:
1. Stores baseline results from main branch
2. Compares PR results against baseline
3. Fails if regression exceeds 5% threshold
4. Generates performance reports with metrics

## Flamegraph Generation

Flamegraphs are generated for each benchmark group showing:
- CPU time distribution
- Hot paths in the code
- Function call hierarchy

## Hardware Performance Counters

When running on Linux with perf tools:
- Instructions per cycle (IPC)
- Cache miss rates
- Branch misprediction rates
- L1 data cache performance

## Baseline P99 Latency

P99 latencies are automatically tracked and reported:
- View in Criterion HTML reports
- Stored in JSON for regression tracking
- Displayed in CI performance reports