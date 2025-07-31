use ::core::ring_buffer::RingBuffer;
use std::thread;
use std::time::Instant;

fn main() {
    println!("Ring Buffer Performance Test");
    println!("============================\n");
    
    // Test different buffer sizes
    for &capacity_pow in &[10, 12, 14, 16, 18] {
        let capacity = 1 << capacity_pow;
        println!("Testing capacity 2^{} = {} elements", capacity_pow, capacity);
        
        let buffer = RingBuffer::<u64>::new(capacity).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        let iterations = 10_000_000u64;
        
        let start = Instant::now();
        
        let producer_handle = thread::spawn(move || {
            let start = Instant::now();
            for i in 0..iterations {
                while producer.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
            let elapsed = start.elapsed();
            println!("  Producer: {} ops in {:?}", iterations, elapsed);
            let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
            println!("  Producer rate: {:.2}M ops/sec", ops_per_sec / 1_000_000.0);
        });
        
        let consumer_handle = thread::spawn(move || {
            let start = Instant::now();
            for _ in 0..iterations {
                loop {
                    if consumer.pop().is_ok() {
                        break;
                    }
                    std::hint::spin_loop();
                }
            }
            let elapsed = start.elapsed();
            println!("  Consumer: {} ops in {:?}", iterations, elapsed);
            let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();
            println!("  Consumer rate: {:.2}M ops/sec", ops_per_sec / 1_000_000.0);
        });
        
        producer_handle.join().unwrap();
        consumer_handle.join().unwrap();
        
        let total_elapsed = start.elapsed();
        let total_ops = iterations;
        let ops_per_sec = total_ops as f64 / total_elapsed.as_secs_f64();
        
        println!("  Total time: {:?}", total_elapsed);
        println!("  Overall rate: {:.2}M ops/sec", ops_per_sec / 1_000_000.0);
        
        if ops_per_sec >= 20_000_000.0 {
            println!("  ✓ PASSED: Exceeds 20M ops/sec target!");
        } else {
            println!("  ✗ FAILED: Below 20M ops/sec target");
        }
        
        println!();
    }
    
    // Latency test
    println!("\nLatency Test (1M operations)");
    println!("=============================");
    
    let buffer = RingBuffer::<u64>::new(1024).unwrap();
    let (mut producer, mut consumer) = buffer.split();
    
    // Pre-fill buffer halfway
    for i in 0..500 {
        producer.push(i).unwrap();
    }
    
    let mut latencies = Vec::with_capacity(1_000_000);
    
    for i in 0..1_000_000 {
        let start = Instant::now();
        producer.push(i).unwrap();
        let push_time = start.elapsed();
        
        let start = Instant::now();
        consumer.pop().unwrap();
        let pop_time = start.elapsed();
        
        latencies.push((push_time, pop_time));
    }
    
    // Calculate percentiles
    let mut push_times: Vec<_> = latencies.iter().map(|(p, _)| p.as_nanos()).collect();
    let mut pop_times: Vec<_> = latencies.iter().map(|(_, p)| p.as_nanos()).collect();
    
    push_times.sort_unstable();
    pop_times.sort_unstable();
    
    let p50 = push_times.len() / 2;
    let p90 = push_times.len() * 90 / 100;
    let p99 = push_times.len() * 99 / 100;
    let p999 = push_times.len() * 999 / 1000;
    
    println!("Push latencies:");
    println!("  p50:  {} ns", push_times[p50]);
    println!("  p90:  {} ns", push_times[p90]);
    println!("  p99:  {} ns", push_times[p99]);
    println!("  p99.9: {} ns", push_times[p999]);
    
    println!("\nPop latencies:");
    println!("  p50:  {} ns", pop_times[p50]);
    println!("  p90:  {} ns", pop_times[p90]);
    println!("  p99:  {} ns", pop_times[p99]);
    println!("  p99.9: {} ns", pop_times[p999]);
}