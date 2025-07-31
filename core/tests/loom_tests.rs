#![cfg(loom)]

use loom::sync::Arc;
use loom::thread;
use ::core::ring_buffer::RingBuffer;

#[test]
fn loom_spsc_basic() {
    let mut config = loom::model::Config::default();
    config.preemption_bound = Some(3);
    
    loom::model_with_config(config, || {
        let buffer = RingBuffer::<u32>::new(4).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        let producer_handle = thread::spawn(move || {
            for i in 0..3 {
                while producer.push(i).is_err() {
                    thread::yield_now();
                }
            }
        });
        
        let consumer_handle = thread::spawn(move || {
            let mut values = Vec::new();
            for _ in 0..3 {
                loop {
                    match consumer.pop() {
                        Ok(val) => {
                            values.push(val);
                            break;
                        }
                        Err(_) => thread::yield_now(),
                    }
                }
            }
            values
        });
        
        producer_handle.join().unwrap();
        let values = consumer_handle.join().unwrap();
        
        assert_eq!(values, vec![0, 1, 2]);
    });
}

#[test]
fn loom_spsc_full_buffer() {
    let mut config = loom::model::Config::default();
    config.preemption_bound = Some(3);
    
    loom::model_with_config(config, || {
        let buffer = RingBuffer::<u32>::new(2).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        // Producer tries to fill the buffer
        let producer_handle = thread::spawn(move || {
            let mut pushed = 0;
            for i in 0..10 {
                if producer.push(i).is_ok() {
                    pushed += 1;
                }
            }
            pushed
        });
        
        // Consumer slowly consumes
        let consumer_handle = thread::spawn(move || {
            let mut consumed = Vec::new();
            thread::yield_now();
            
            for _ in 0..10 {
                if let Ok(val) = consumer.pop() {
                    consumed.push(val);
                }
            }
            consumed
        });
        
        let pushed = producer_handle.join().unwrap();
        let consumed = consumer_handle.join().unwrap();
        
        // Buffer can hold at most capacity - 1 items
        assert!(pushed <= 1);
        assert_eq!(pushed, consumed.len());
    });
}

#[test]
fn loom_memory_ordering() {
    let mut config = loom::model::Config::default();
    config.preemption_bound = Some(4);
    
    loom::model_with_config(config, || {
        let buffer = RingBuffer::<Box<u32>>::new(4).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        let producer_handle = thread::spawn(move || {
            for i in 0..3 {
                let boxed = Box::new(i * 100);
                while producer.push(boxed).is_err() {
                    thread::yield_now();
                }
            }
        });
        
        let consumer_handle = thread::spawn(move || {
            let mut sum = 0;
            for _ in 0..3 {
                loop {
                    match consumer.pop() {
                        Ok(boxed) => {
                            sum += *boxed;
                            break;
                        }
                        Err(_) => thread::yield_now(),
                    }
                }
            }
            sum
        });
        
        producer_handle.join().unwrap();
        let sum = consumer_handle.join().unwrap();
        
        // 0 + 100 + 200 = 300
        assert_eq!(sum, 300);
    });
}

#[test]
fn loom_wrap_around() {
    let mut config = loom::model::Config::default();
    config.preemption_bound = Some(3);
    
    loom::model_with_config(config, || {
        let buffer = RingBuffer::<u32>::new(2).unwrap();
        let (mut producer, mut consumer) = buffer.split();
        
        let producer_handle = thread::spawn(move || {
            for round in 0..5 {
                while producer.push(round).is_err() {
                    thread::yield_now();
                }
            }
        });
        
        let consumer_handle = thread::spawn(move || {
            let mut values = Vec::new();
            for _ in 0..5 {
                loop {
                    match consumer.pop() {
                        Ok(val) => {
                            values.push(val);
                            break;
                        }
                        Err(_) => thread::yield_now(),
                    }
                }
            }
            values
        });
        
        producer_handle.join().unwrap();
        let values = consumer_handle.join().unwrap();
        
        assert_eq!(values, vec![0, 1, 2, 3, 4]);
    });
}