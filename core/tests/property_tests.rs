use proptest::prelude::*;
use core::ring_buffer::{RingBuffer, RingBufferError};
use std::thread;
use std::sync::mpsc;

proptest! {
    #[test]
    fn prop_capacity_power_of_two(capacity in 1usize..=16384) {
        let result = RingBuffer::<u32>::new(capacity);
        
        if capacity.is_power_of_two() {
            prop_assert!(result.is_ok());
            let buffer = result.unwrap();
            prop_assert_eq!(buffer.capacity(), capacity);
        } else {
            prop_assert!(matches!(result, Err(RingBufferError::InvalidCapacity(_))));
        }
    }

    #[test]
    fn prop_push_pop_consistency(
        capacity in (1usize..=10).map(|n| 1 << n), // Powers of 2: 2, 4, 8, ..., 1024
        operations in prop::collection::vec(0u32..1000, 0..100)
    ) {
        let buffer = RingBuffer::<u32>::new(capacity).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        let mut pushed = Vec::new();
        let mut popped = Vec::new();
        
        for value in operations {
            match producer.push(value) {
                Ok(()) => pushed.push(value),
                Err(RingBufferError::BufferFull) => {
                    // Try to pop to make space
                    if let Ok(val) = consumer.pop() {
                        popped.push(val);
                        // Retry push
                        if producer.push(value).is_ok() {
                            pushed.push(value);
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
        
        // Pop remaining items
        while let Ok(val) = consumer.pop() {
            popped.push(val);
        }
        
        // All pushed items should be popped in order
        prop_assert_eq!(pushed.len(), popped.len());
        prop_assert_eq!(pushed, popped);
    }

    #[test]
    fn prop_len_consistency(
        capacity in (1usize..=8).map(|n| 1 << n),
        push_count in 0usize..20,
        pop_count in 0usize..20
    ) {
        let buffer = RingBuffer::<u32>::new(capacity).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        let mut actual_pushed = 0;
        let mut actual_popped = 0;
        
        // Push items
        for i in 0..push_count {
            if producer.push(i as u32).is_ok() {
                actual_pushed += 1;
            }
        }
        
        prop_assert_eq!(consumer.len(), actual_pushed);
        prop_assert_eq!(producer.remaining_capacity(), capacity - 1 - actual_pushed);
        
        // Pop items
        for _ in 0..pop_count {
            if consumer.pop().is_ok() {
                actual_popped += 1;
            }
        }
        
        let remaining = actual_pushed.saturating_sub(actual_popped);
        prop_assert_eq!(consumer.len(), remaining);
        prop_assert_eq!(producer.remaining_capacity(), capacity - 1 - remaining);
    }

    #[test]
    fn prop_concurrent_consistency(
        capacity in (2usize..=8).map(|n| 1 << n),
        values in prop::collection::vec(0u32..1000, 10..100)
    ) {
        let buffer = RingBuffer::<u32>::new(capacity).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        let values_to_send = values.clone();
        let (tx, rx) = mpsc::channel();
        
        // Producer thread
        let producer_handle = thread::spawn(move || {
            for value in values_to_send {
                while producer.push(value).is_err() {
                    thread::yield_now();
                }
            }
        });
        
        // Consumer thread
        let consumer_handle = thread::spawn(move || {
            let mut received = Vec::new();
            let expected_count = rx.recv().unwrap();
            
            for _ in 0..expected_count {
                loop {
                    match consumer.pop() {
                        Ok(val) => {
                            received.push(val);
                            break;
                        }
                        Err(_) => thread::yield_now(),
                    }
                }
            }
            received
        });
        
        tx.send(values.len()).unwrap();
        producer_handle.join().unwrap();
        let received = consumer_handle.join().unwrap();
        
        prop_assert_eq!(values, received);
    }
}