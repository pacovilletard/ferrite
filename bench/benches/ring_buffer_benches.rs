use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use core::ring_buffer::RingBuffer;

fn ring_buffer_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_throughput");

    for &size in &[1024, 4096, 16384] {
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &s| {
            let buffer = RingBuffer::<u64>::new(s).unwrap();
            let (mut producer, mut consumer) = buffer.split();

            let half_cap = (s / 2) - 1;
            for i in 0..half_cap {
                producer.push(i as u64).unwrap();
            }

            b.iter(|| {
                producer.push(0).unwrap();
                consumer.pop().unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(benches, ring_buffer_benchmark);
criterion_main!(benches);