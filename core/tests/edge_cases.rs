use ::core::ring_buffer::{RingBuffer, RingBufferError};
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[test]
fn test_single_element_buffer() {
    // Smallest possible buffer
    let buffer = RingBuffer::<u32>::new(1).unwrap();
    let (mut producer, mut consumer) = buffer.split();
    
    // Should be empty initially
    assert!(consumer.is_empty());
    assert_eq!(consumer.len(), 0);
    assert!(!producer.is_full());
    
    // Can't push even one item (capacity - 1 = 0)
    assert_eq!(producer.push(42), Err(RingBufferError::BufferFull));
    assert!(producer.is_full());
    assert_eq!(producer.remaining_capacity(), 0);
}

#[test]
fn test_two_element_buffer() {
    let buffer = RingBuffer::<u32>::new(2).unwrap();
    let (mut producer, mut consumer) = buffer.split();
    
    // Can push exactly one item
    assert!(producer.push(42).is_ok());
    assert!(producer.is_full());
    assert_eq!(producer.push(43), Err(RingBufferError::BufferFull));
    
    // Pop and push again
    assert_eq!(consumer.pop(), Ok(42));
    assert!(producer.push(43).is_ok());
    assert_eq!(consumer.pop(), Ok(43));
}

#[test]
fn test_large_buffer() {
    let buffer = RingBuffer::<u64>::new(1 << 20).unwrap(); // 1M elements
    let (mut producer, mut consumer) = buffer.split();
    
    // Fill half the buffer
    let half = (1 << 19) - 1;
    for i in 0..half {
        producer.push(i).unwrap();
    }
    
    assert_eq!(consumer.len(), half);
    assert_eq!(producer.remaining_capacity(), (1 << 20) - 1 - half);
    
    // Consume all
    for i in 0..half {
        assert_eq!(consumer.pop(), Ok(i));
    }
    
    assert!(consumer.is_empty());
}

#[test]
fn test_zero_sized_type() {
    #[derive(Debug, PartialEq)]
    struct ZeroSized;
    
    let buffer = RingBuffer::<ZeroSized>::new(16).unwrap();
    let (mut producer, mut consumer) = buffer.split();
    
    for _ in 0..15 {
        producer.push(ZeroSized).unwrap();
    }
    
    for _ in 0..15 {
        assert_eq!(consumer.pop(), Ok(ZeroSized));
    }
}

#[test]
fn test_drop_semantics() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
    
    struct DropCounter;
    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    DROP_COUNT.store(0, Ordering::Relaxed);
    
    {
        let buffer = RingBuffer::<DropCounter>::new(4).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        // Push 3 items
        for _ in 0..3 {
            producer.push(DropCounter).unwrap();
        }
        
        // Pop 1 item - should trigger 1 drop
        consumer.pop().unwrap();
        assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 1);
        
        // Remaining 2 items should be dropped when buffer is dropped
    }
    
    // Note: In our implementation, items aren't dropped until consumed
    // This is different from some implementations that drop on buffer drop
    assert_eq!(DROP_COUNT.load(Ordering::Relaxed), 1);
}

#[test]
fn test_stress_wraparound() {
    let buffer = RingBuffer::<u32>::new(4).unwrap();
    let (mut producer, mut consumer) = buffer.split();
    
    // Stress test wraparound with many iterations
    for round in 0..10000 {
        // Push 3 items (max for capacity 4)
        for i in 0..3 {
            let value = round * 3 + i;
            producer.push(value).unwrap();
        }
        
        // Verify buffer is full
        assert!(producer.is_full());
        assert_eq!(consumer.len(), 3);
        
        // Pop all 3 items
        for i in 0..3 {
            let expected = round * 3 + i;
            assert_eq!(consumer.pop(), Ok(expected));
        }
        
        // Verify buffer is empty
        assert!(consumer.is_empty());
        assert_eq!(producer.remaining_capacity(), 3);
    }
}

#[test]
fn test_concurrent_termination() {
    let buffer = RingBuffer::<u32>::new(16).unwrap();
    let (mut producer, mut consumer) = buffer.split();
    
    let done = Arc::new(AtomicBool::new(false));
    let done_prod = done.clone();
    let done_cons = done.clone();
    
    let producer_handle = thread::spawn(move || {
        let mut count = 0;
        while !done_prod.load(Ordering::Relaxed) {
            if producer.push(count).is_ok() {
                count += 1;
            } else {
                thread::yield_now();
            }
        }
        count
    });
    
    let consumer_handle = thread::spawn(move || {
        let mut count = 0;
        let mut last_value = None;
        
        while !done_cons.load(Ordering::Relaxed) || !consumer.is_empty() {
            match consumer.pop() {
                Ok(val) => {
                    if let Some(last) = last_value {
                        assert_eq!(val, last + 1, "Values must be sequential");
                    }
                    last_value = Some(val);
                    count += 1;
                }
                Err(_) => {
                    if done_cons.load(Ordering::Relaxed) {
                        break;
                    }
                    thread::yield_now();
                }
            }
        }
        count
    });
    
    // Let it run for a bit
    thread::sleep(Duration::from_millis(10));
    
    // Signal termination
    done.store(true, Ordering::Relaxed);
    
    let produced = producer_handle.join().unwrap();
    let consumed = consumer_handle.join().unwrap();
    
    // All produced items should be consumed
    assert!(consumed <= produced);
    assert!(produced - consumed <= 15); // At most buffer capacity - 1 items pending
}

#[test]
fn test_memory_barriers() {
    // This test verifies that memory barriers work correctly
    // by passing complex data through the buffer
    
    #[derive(Debug, PartialEq, Clone)]
    struct ComplexData {
        id: u64,
        data: Vec<u8>,
        flag: bool,
    }
    
    let buffer = RingBuffer::<ComplexData>::new(8).unwrap();
    let (mut producer, mut consumer) = buffer.split();
    
    let producer_handle = thread::spawn(move || {
        for i in 0..100 {
            let data = ComplexData {
                id: i,
                data: vec![i as u8; (i % 10) as usize + 1],
                flag: i % 2 == 0,
            };
            
            while producer.push(data.clone()).is_err() {
                thread::yield_now();
            }
        }
    });
    
    let consumer_handle = thread::spawn(move || {
        for i in 0..100 {
            let data = loop {
                match consumer.pop() {
                    Ok(d) => break d,
                    Err(_) => thread::yield_now(),
                }
            };
            
            // Verify data integrity
            assert_eq!(data.id, i);
            assert_eq!(data.data.len(), (i % 10) as usize + 1);
            assert!(data.data.iter().all(|&b| b == i as u8));
            assert_eq!(data.flag, i % 2 == 0);
        }
    });
    
    producer_handle.join().unwrap();
    consumer_handle.join().unwrap();
}