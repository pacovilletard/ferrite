use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use core::ring_buffer::RingBuffer;
use pprof::criterion::{Output, PProfProfiler};
use std::time::Duration;

/// Benchmark single-threaded throughput with various buffer sizes
fn throughput_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_throughput");
    group.measurement_time(Duration::from_secs(10));

    for &size in &[1024, 4096, 16384, 65536] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &buffer_size| {
            b.iter_batched(
                || {
                    let buffer = RingBuffer::<u64>::new(buffer_size).unwrap();
                    buffer.split()
                },
                |(mut producer, mut consumer)| {
                    // Benchmark sustained producer/consumer pattern
                    for i in 0..buffer_size {
                        producer.push(black_box(i as u64)).unwrap();
                        black_box(consumer.pop().unwrap());
                    }
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

/// Benchmark latency percentiles for push/pop operations
fn latency_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_latency");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);

    for &size in &[1024, 16384] {
        // Measure push latency when buffer is empty
        group.bench_with_input(
            BenchmarkId::new("push_empty", size),
            &size,
            |b, &buffer_size| {
                b.iter_batched(
                    || {
                        let buffer = RingBuffer::<u64>::new(buffer_size).unwrap();
                        buffer.split()
                    },
                    |(mut producer, _consumer)| {
                        producer.push(black_box(42)).unwrap();
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // Measure pop latency when buffer has one element
        group.bench_with_input(
            BenchmarkId::new("pop_single", size),
            &size,
            |b, &buffer_size| {
                b.iter_batched(
                    || {
                        let buffer = RingBuffer::<u64>::new(buffer_size).unwrap();
                        let (mut producer, consumer) = buffer.split();
                        producer.push(42).unwrap();
                        consumer
                    },
                    |mut consumer| {
                        black_box(consumer.pop().unwrap());
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark behavior at boundary conditions (empty/full)
fn boundary_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_boundaries");
    let buffer_size = 4096;

    // Benchmark alternating between empty and full states
    group.bench_function("empty_full_transitions", |b| {
        let buffer = RingBuffer::<u64>::new(buffer_size).unwrap();
        let (mut producer, mut consumer) = buffer.split();

        b.iter(|| {
            // Fill buffer to capacity
            for i in 0..(buffer_size - 1) {
                producer.push(black_box(i as u64)).unwrap();
            }
            // Empty buffer completely
            for _ in 0..(buffer_size - 1) {
                black_box(consumer.pop().unwrap());
            }
        });
    });

    // Benchmark wrap-around behavior
    group.bench_function("wrap_around", |b| {
        let buffer = RingBuffer::<u64>::new(buffer_size).unwrap();
        let (mut producer, mut consumer) = buffer.split();

        // Pre-fill half the buffer
        for i in 0..(buffer_size / 2) {
            producer.push(i as u64).unwrap();
        }

        b.iter(|| {
            // Push and pop to force wrap-around
            for i in 0..100 {
                producer.push(black_box(i)).unwrap();
                black_box(consumer.pop().unwrap());
            }
        });
    });

    group.finish();
}

/// Benchmark contention patterns in SPSC scenario
fn contention_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_contention");
    group.measurement_time(Duration::from_secs(5));

    // Simulate producer-heavy workload
    group.bench_function("producer_heavy", |b| {
        let buffer = RingBuffer::<u64>::new(16384).unwrap();
        let (mut producer, mut consumer) = buffer.split();

        b.iter(|| {
            // Producer does 10x more work than consumer
            for i in 0..10 {
                if producer.push(black_box(i)).is_ok() {
                    // Successfully pushed
                }
            }
            if let Ok(val) = consumer.pop() {
                black_box(val);
            }
        });
    });

    // Simulate consumer-heavy workload
    group.bench_function("consumer_heavy", |b| {
        let buffer = RingBuffer::<u64>::new(16384).unwrap();
        let (mut producer, mut consumer) = buffer.split();

        // Pre-fill buffer
        for i in 0..8192 {
            producer.push(i).unwrap();
        }

        b.iter(|| {
            // Consumer does 10x more work than producer
            for _ in 0..10 {
                if let Ok(val) = consumer.pop() {
                    black_box(val);
                }
            }
            producer.push(black_box(42)).ok();
        });
    });

    group.finish();
}

// Configure criterion to use pprof for flamegraph generation
fn criterion_config() -> Criterion {
    Criterion::default()
        .with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)))
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets = throughput_benchmark, latency_benchmark, boundary_benchmark, contention_benchmark
}

criterion_main!(benches);
