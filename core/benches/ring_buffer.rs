use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use core::ring_buffer::RingBuffer;
use std::thread;
use std::time::{Duration, Instant};

fn bench_spsc_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("spsc_throughput");
    
    for capacity_pow in [10, 12, 14, 16].iter() {
        let capacity = 1 << capacity_pow;
        group.throughput(Throughput::Elements(1_000_000));
        
        group.bench_with_input(
            BenchmarkId::new("push_pop", format!("2^{}", capacity_pow)),
            &capacity,
            |b, &capacity| {
                b.iter_custom(|iters| {
                    let buffer = RingBuffer::<u64>::new(capacity).unwrap();
                    let (mut producer, mut consumer) = buffer.split();
                    
                    let start = Instant::now();
                    
                    let producer_handle = thread::spawn(move || {
                        for i in 0..iters {
                            while producer.push(i).is_err() {
                                std::hint::spin_loop();
                            }
                        }
                    });
                    
                    let consumer_handle = thread::spawn(move || {
                        for _ in 0..iters {
                            loop {
                                if consumer.pop().is_ok() {
                                    break;
                                }
                                std::hint::spin_loop();
                            }
                        }
                    });
                    
                    producer_handle.join().unwrap();
                    consumer_handle.join().unwrap();
                    
                    start.elapsed()
                });
            },
        );
    }
    
    group.finish();
}

fn bench_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency");
    
    group.bench_function("push", |b| {
        let buffer = RingBuffer::<u64>::new(1024).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        // Keep buffer half full
        for i in 0..500 {
            producer.push(i).unwrap();
        }
        
        b.iter(|| {
            producer.push(black_box(42)).unwrap();
            consumer.pop().unwrap();
        });
    });
    
    group.bench_function("pop", |b| {
        let buffer = RingBuffer::<u64>::new(1024).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        // Keep buffer half full
        for i in 0..500 {
            producer.push(i).unwrap();
        }
        
        b.iter(|| {
            producer.push(42).unwrap();
            black_box(consumer.pop().unwrap());
        });
    });
    
    group.finish();
}

fn bench_ops_per_second(c: &mut Criterion) {
    c.bench_function("ops_per_second_target_20M", |b| {
        let buffer = RingBuffer::<u64>::new(1024).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        b.iter_custom(|iters| {
            let start = Instant::now();
            
            let producer_handle = thread::spawn(move || {
                for i in 0..iters {
                    while producer.push(i).is_err() {
                        std::hint::spin_loop();
                    }
                }
                producer
            });
            
            let consumer_handle = thread::spawn(move || {
                for _ in 0..iters {
                    loop {
                        if consumer.pop().is_ok() {
                            break;
                        }
                        std::hint::spin_loop();
                    }
                }
                consumer
            });
            
            producer = producer_handle.join().unwrap();
            consumer = consumer_handle.join().unwrap();
            
            let elapsed = start.elapsed();
            
            // Print ops/sec for verification
            let ops_per_sec = iters as f64 / elapsed.as_secs_f64();
            if iters >= 1_000_000 {
                println!("Operations per second: {:.2}M", ops_per_sec / 1_000_000.0);
            }
            
            elapsed
        });
    });
}

fn bench_different_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("different_value_sizes");
    
    // Benchmark with different value sizes
    group.bench_function("u8", |b| {
        let buffer = RingBuffer::<u8>::new(1024).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        b.iter(|| {
            for i in 0..100 {
                producer.push(i as u8).unwrap();
            }
            for _ in 0..100 {
                black_box(consumer.pop().unwrap());
            }
        });
    });
    
    group.bench_function("u64", |b| {
        let buffer = RingBuffer::<u64>::new(1024).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        b.iter(|| {
            for i in 0..100 {
                producer.push(i).unwrap();
            }
            for _ in 0..100 {
                black_box(consumer.pop().unwrap());
            }
        });
    });
    
    group.bench_function("512_bytes", |b| {
        let buffer = RingBuffer::<[u8; 512]>::new(1024).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        b.iter(|| {
            for _ in 0..100 {
                producer.push([0u8; 512]).unwrap();
            }
            for _ in 0..100 {
                black_box(consumer.pop().unwrap());
            }
        });
    });
    
    group.finish();
}

fn bench_contention(c: &mut Criterion) {
    let mut group = c.benchmark_group("contention");
    
    group.bench_function("high_contention", |b| {
        b.iter_custom(|iters| {
            let buffer = RingBuffer::<u64>::new(16).unwrap(); // Small buffer for high contention
            let (mut producer, mut consumer) = buffer.split();
            
            let start = Instant::now();
            
            let producer_handle = thread::spawn(move || {
                for i in 0..iters {
                    while producer.push(i).is_err() {
                        // Busy wait - high contention
                    }
                }
            });
            
            let consumer_handle = thread::spawn(move || {
                for _ in 0..iters {
                    while consumer.pop().is_err() {
                        // Busy wait - high contention
                    }
                }
            });
            
            producer_handle.join().unwrap();
            consumer_handle.join().unwrap();
            
            start.elapsed()
        });
    });
    
    group.bench_function("low_contention", |b| {
        b.iter_custom(|iters| {
            let buffer = RingBuffer::<u64>::new(65536).unwrap(); // Large buffer for low contention
            let (mut producer, mut consumer) = buffer.split();
            
            // Pre-fill to reduce contention
            for i in 0..1000 {
                producer.push(i).unwrap();
            }
            
            let start = Instant::now();
            
            let producer_handle = thread::spawn(move || {
                for i in 0..iters {
                    producer.push(i).unwrap();
                }
            });
            
            let consumer_handle = thread::spawn(move || {
                thread::sleep(Duration::from_micros(100)); // Let producer get ahead
                for _ in 0..(iters + 1000) {
                    consumer.pop().unwrap();
                }
            });
            
            producer_handle.join().unwrap();
            consumer_handle.join().unwrap();
            
            start.elapsed()
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_spsc_throughput,
    bench_latency,
    bench_ops_per_second,
    bench_different_sizes,
    bench_contention
);
criterion_main!(benches);