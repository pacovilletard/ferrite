use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::error::Error;
use std::fmt;

/// Error types for ring buffer operations
#[derive(Debug, Clone, PartialEq)]
pub enum RingBufferError {
    /// Capacity must be a power of two and greater than 0
    InvalidCapacity(usize),
    /// Buffer is full, cannot push more items
    BufferFull,
    /// Buffer is empty, cannot pop items
    BufferEmpty,
}

impl fmt::Display for RingBufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RingBufferError::InvalidCapacity(cap) => {
                write!(f, "Invalid capacity: {}. Must be a power of two and greater than 0", cap)
            }
            RingBufferError::BufferFull => write!(f, "Buffer is full"),
            RingBufferError::BufferEmpty => write!(f, "Buffer is empty"),
        }
    }
}

impl Error for RingBufferError {}

/// A high-performance lock-free single-producer single-consumer (SPSC) ring buffer
/// 
/// This implementation provides:
/// - Cache-line padding to avoid false sharing between producer and consumer
/// - Power-of-two capacity for efficient mask-based wrapping
/// - Relaxed memory ordering for indices with acquire-release at boundaries
/// - Zero allocations in the hot path
/// - Wait-free operations for both producer and consumer
/// 
/// # Thread Safety
/// 
/// This buffer is designed for exactly one producer thread and one consumer thread.
/// Using multiple producers or consumers will result in undefined behavior.
/// 
/// # Performance
/// 
/// Designed to achieve ≥20M operations per second on modern hardware.
/// Uses cache-line alignment and relaxed atomics to minimize contention.
/// 
/// # Example
/// 
/// ```
/// use core::ring_buffer::RingBuffer;
/// 
/// // Create a buffer with capacity 1024
/// let buffer = RingBuffer::<u32>::new(1024).unwrap();
/// let (mut producer, mut consumer) = buffer.split();
/// 
/// // Producer thread
/// std::thread::spawn(move || {
///     for i in 0..100 {
///         while producer.push(i).is_err() {
///             std::thread::yield_now();
///         }
///     }
/// });
/// 
/// // Consumer thread
/// for _ in 0..100 {
///     loop {
///         if let Ok(value) = consumer.pop() {
///             println!("Got: {}", value);
///             break;
///         }
///         std::thread::yield_now();
///     }
/// }
/// ```
#[repr(align(64))]
pub struct RingBuffer<T> {
    /// Internal storage with cache-line alignment
    buffer: Box<[UnsafeCell<MaybeUninit<T>>]>,
    /// Capacity minus one, used as a bitmask for wrapping
    mask: usize,
    /// Shared state between producer and consumer
    shared: Arc<SharedState>,
}

/// Shared state with cache-line padding to avoid false sharing
#[repr(C)]
struct SharedState {
    /// Producer write position
    head: CachePadded<AtomicUsize>,
    /// Consumer read position  
    tail: CachePadded<AtomicUsize>,
}

/// Cache-line padding wrapper to avoid false sharing
#[repr(align(64))]
struct CachePadded<T> {
    value: T,
}

impl<T> RingBuffer<T> {
    /// Creates a new ring buffer with the specified capacity
    /// 
    /// # Arguments
    /// 
    /// * `capacity` - The desired capacity. Must be a power of two and greater than 0.
    /// 
    /// # Returns
    /// 
    /// * `Ok(RingBuffer<T>)` - A new ring buffer
    /// * `Err(RingBufferError)` - If capacity is invalid
    /// 
    /// # Example
    /// 
    /// ```
    /// let buffer = RingBuffer::<u32>::new(1024).unwrap();
    /// ```
    pub fn new(capacity: usize) -> Result<Self, RingBufferError> {
        if capacity == 0 || !capacity.is_power_of_two() {
            return Err(RingBufferError::InvalidCapacity(capacity));
        }

        let mut buffer = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(UnsafeCell::new(MaybeUninit::uninit()));
        }

        Ok(RingBuffer {
            buffer: buffer.into_boxed_slice(),
            mask: capacity - 1,
            shared: Arc::new(SharedState {
                head: CachePadded { value: AtomicUsize::new(0) },
                tail: CachePadded { value: AtomicUsize::new(0) },
            }),
        })
    }

    /// Returns the capacity of the ring buffer
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Splits the ring buffer into producer and consumer halves
    /// 
    /// After calling this method, the original RingBuffer is consumed.
    /// The producer can push items and the consumer can pop items.
    /// 
    /// # Example
    /// 
    /// ```
    /// let buffer = RingBuffer::<u32>::new(1024).unwrap();
    /// let (producer, consumer) = buffer.split();
    /// ```
    pub fn split(self) -> (Producer<T>, Consumer<T>) {
        let buffer_ptr = Box::into_raw(self.buffer) as *mut UnsafeCell<MaybeUninit<T>>;
        let capacity = self.mask + 1;
        
        let producer = Producer {
            buffer: buffer_ptr,
            mask: self.mask,
            capacity,
            shared: self.shared.clone(),
            cached_tail: 0,
        };

        let consumer = Consumer {
            buffer: buffer_ptr,
            mask: self.mask,
            capacity,
            shared: self.shared,
            cached_head: 0,
        };

        (producer, consumer)
    }
}

/// Producer half of the ring buffer
pub struct Producer<T> {
    buffer: *mut UnsafeCell<MaybeUninit<T>>,
    mask: usize,
    capacity: usize,
    shared: Arc<SharedState>,
    cached_tail: usize,
}

/// Consumer half of the ring buffer
pub struct Consumer<T> {
    buffer: *mut UnsafeCell<MaybeUninit<T>>,
    mask: usize,
    capacity: usize,
    shared: Arc<SharedState>,
    cached_head: usize,
}

unsafe impl<T: Send> Send for Producer<T> {}
unsafe impl<T: Send> Send for Consumer<T> {}

impl<T> Producer<T> {
    /// Attempts to push an item into the buffer
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - Item was successfully pushed
    /// * `Err(RingBufferError::BufferFull)` - Buffer is full
    pub fn push(&mut self, value: T) -> Result<(), RingBufferError> {
        let head = self.shared.head.value.load(Ordering::Relaxed);
        let next_head = (head + 1) & self.mask;

        if next_head == self.cached_tail {
            self.cached_tail = self.shared.tail.value.load(Ordering::Acquire);
            if next_head == self.cached_tail {
                return Err(RingBufferError::BufferFull);
            }
        }

        unsafe {
            let slot = &mut *(*self.buffer.add(head)).get();
            slot.write(value);
        }

        self.shared.head.value.store(next_head, Ordering::Release);
        Ok(())
    }

    /// Returns the number of items that can be pushed without blocking
    pub fn remaining_capacity(&self) -> usize {
        let head = self.shared.head.value.load(Ordering::Relaxed);
        let tail = self.shared.tail.value.load(Ordering::Acquire);
        
        if head >= tail {
            self.capacity - 1 - (head - tail)
        } else {
            tail - head - 1
        }
    }

    /// Checks if the buffer is full
    pub fn is_full(&self) -> bool {
        self.remaining_capacity() == 0
    }
}

impl<T> Consumer<T> {
    /// Attempts to pop an item from the buffer
    /// 
    /// # Returns
    /// 
    /// * `Ok(T)` - Successfully popped an item
    /// * `Err(RingBufferError::BufferEmpty)` - Buffer is empty
    pub fn pop(&mut self) -> Result<T, RingBufferError> {
        let tail = self.shared.tail.value.load(Ordering::Relaxed);

        if tail == self.cached_head {
            self.cached_head = self.shared.head.value.load(Ordering::Acquire);
            if tail == self.cached_head {
                return Err(RingBufferError::BufferEmpty);
            }
        }

        let value = unsafe {
            let slot = &mut *(*self.buffer.add(tail)).get();
            slot.assume_init_read()
        };

        let next_tail = (tail + 1) & self.mask;
        self.shared.tail.value.store(next_tail, Ordering::Release);

        Ok(value)
    }

    /// Returns the number of items available to pop
    pub fn len(&self) -> usize {
        let head = self.shared.head.value.load(Ordering::Acquire);
        let tail = self.shared.tail.value.load(Ordering::Relaxed);
        
        if head >= tail {
            head - tail
        } else {
            self.capacity - tail + head
        }
    }

    /// Checks if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        // Producer is responsible for cleaning up the buffer
        unsafe {
            let buffer = std::slice::from_raw_parts_mut(self.buffer, self.capacity);
            let _ = Box::from_raw(buffer);
        }
    }
}

impl<T> Drop for Consumer<T> {
    fn drop(&mut self) {
        // Consumer doesn't own the buffer, so nothing to do
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_valid_capacity() {
        assert!(RingBuffer::<u32>::new(16).is_ok());
        assert!(RingBuffer::<u32>::new(1024).is_ok());
    }

    #[test]
    fn test_new_invalid_capacity() {
        assert!(matches!(
            RingBuffer::<u32>::new(0),
            Err(RingBufferError::InvalidCapacity(0))
        ));
        assert!(matches!(
            RingBuffer::<u32>::new(15),
            Err(RingBufferError::InvalidCapacity(15))
        ));
    }

    #[test]
    fn test_push_pop() {
        let buffer = RingBuffer::<u32>::new(16).unwrap();
        let (mut producer, mut consumer) = buffer.split();

        assert!(producer.push(42).is_ok());
        assert_eq!(consumer.pop(), Ok(42));
    }

    #[test]
    fn test_buffer_full() {
        let buffer = RingBuffer::<u32>::new(4).unwrap();
        let (mut producer, mut consumer) = buffer.split();

        // Fill buffer (capacity - 1 items)
        assert!(producer.push(1).is_ok());
        assert!(producer.push(2).is_ok());
        assert!(producer.push(3).is_ok());

        // Buffer should be full now
        assert!(producer.is_full());
        assert_eq!(producer.push(4), Err(RingBufferError::BufferFull));

        // Pop one item
        assert_eq!(consumer.pop(), Ok(1));

        // Should be able to push again
        assert!(producer.push(4).is_ok());
    }

    #[test]
    fn test_buffer_empty() {
        let buffer = RingBuffer::<u32>::new(16).unwrap();
        let (mut producer, mut consumer) = buffer.split();

        assert!(consumer.is_empty());
        assert_eq!(consumer.pop(), Err(RingBufferError::BufferEmpty));

        producer.push(42).unwrap();
        assert!(!consumer.is_empty());
    }

    #[test]
    fn test_capacity_and_len() {
        let buffer = RingBuffer::<u32>::new(16).unwrap();
        assert_eq!(buffer.capacity(), 16);
        
        let (mut producer, mut consumer) = buffer.split();
        assert_eq!(consumer.len(), 0);
        assert_eq!(producer.remaining_capacity(), 15); // capacity - 1

        producer.push(1).unwrap();
        producer.push(2).unwrap();
        
        assert_eq!(consumer.len(), 2);
        assert_eq!(producer.remaining_capacity(), 13);
    }

    #[test]
    fn test_wrap_around() {
        let buffer = RingBuffer::<u32>::new(4).unwrap();
        let (mut producer, mut consumer) = buffer.split();

        // Fill and empty multiple times to test wrap-around
        for round in 0..10 {
            for i in 0..3 {
                producer.push(round * 10 + i).unwrap();
            }
            for i in 0..3 {
                assert_eq!(consumer.pop(), Ok(round * 10 + i));
            }
        }
    }
}