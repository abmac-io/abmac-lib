extern crate std;

use std::{vec, vec::Vec};

use crate::{CollectSink, DropSink, FnSink, Sink, SpillRing, sink};

#[test]
fn new_ring_is_empty() {
    let ring: SpillRing<i32, 4> = SpillRing::new();
    assert!(ring.is_empty());
    assert!(!ring.is_full());
    assert_eq!(ring.len(), 0);
    assert_eq!(ring.capacity(), 4);
}

#[test]
fn push_and_pop() {
    let ring: SpillRing<i32, 4> = SpillRing::new();

    ring.push(1);
    ring.push(2);
    ring.push(3);

    assert_eq!(ring.len(), 3);
    assert_eq!(ring.pop(), Some(1));
    assert_eq!(ring.pop(), Some(2));
    assert_eq!(ring.pop(), Some(3));
    assert_eq!(ring.pop(), None);
}

#[test]
fn eviction_to_sink() {
    // N=4 main buffer, items evicted directly to sink
    let sink = CollectSink::new();
    let ring = SpillRing::<i32, 4, _>::with_sink(sink);

    ring.push(1);
    ring.push(2);
    ring.push(3);
    ring.push(4);
    assert!(ring.sink().items().is_empty()); // Nothing evicted yet

    ring.push(5); // Evicts 1 directly to sink
    assert_eq!(ring.sink().items(), vec![1]);

    ring.push(6); // Evicts 2 directly to sink
    assert_eq!(ring.sink().items(), vec![1, 2]);

    // Ring now contains [3, 4, 5, 6]
    assert_eq!(ring.pop(), Some(3));
    assert_eq!(ring.pop(), Some(4));
    assert_eq!(ring.pop(), Some(5));
    assert_eq!(ring.pop(), Some(6));
}

#[test]
fn flush_to_sink() {
    let sink = CollectSink::new();
    let mut ring = SpillRing::<i32, 4, _>::with_sink(sink);

    ring.push(1);
    ring.push(2);
    ring.push(3);
    ring.push(4);
    ring.push(5); // Evicts 1 directly to sink
    ring.push(6); // Evicts 2 directly to sink

    // Sink already has [1, 2] from direct eviction
    assert_eq!(ring.sink().items(), vec![1, 2]);

    // Flush remaining buffer items to sink
    let count = ring.flush();
    assert_eq!(count, 4); // 3, 4, 5, 6
    assert!(ring.is_empty());
    assert_eq!(ring.sink().items(), vec![1, 2, 3, 4, 5, 6]);
}

#[test]
fn peek_oldest_and_newest() {
    let ring: SpillRing<i32, 4> = SpillRing::new();

    assert_eq!(ring.peek(), None);
    assert_eq!(ring.peek_back(), None);

    ring.push(1);
    assert_eq!(ring.peek(), Some(&1));
    assert_eq!(ring.peek_back(), Some(&1));

    ring.push(2);
    ring.push(3);
    assert_eq!(ring.peek(), Some(&1));
    assert_eq!(ring.peek_back(), Some(&3));
}

#[test]
fn iteration() {
    let ring: SpillRing<i32, 4> = SpillRing::new();

    ring.push(1);
    ring.push(2);
    ring.push(3);

    let items: Vec<i32> = ring.iter().copied().collect();
    assert_eq!(items, std::vec![1, 2, 3]);
}

#[test]
fn iter_mut() {
    let mut ring: SpillRing<i32, 4> = SpillRing::new();

    ring.push(1);
    ring.push(2);
    ring.push(3);

    for item in ring.iter_mut() {
        *item *= 10;
    }

    let items: Vec<i32> = ring.iter().copied().collect();
    assert_eq!(items, vec![10, 20, 30]);
}

#[test]
fn flush_clears_buffer() {
    let sink = CollectSink::new();
    let mut ring = SpillRing::<i32, 4, _>::with_sink(sink);

    ring.push(1);
    ring.push(2);
    ring.push(3);

    ring.flush();

    assert!(ring.is_empty());
    assert_eq!(ring.sink().items(), vec![1, 2, 3]);
}

#[test]
fn wraparound() {
    let ring: SpillRing<i32, 4> = SpillRing::new();

    // Fill and wrap around multiple times
    for i in 0..12 {
        ring.push(i);
    }

    // Should contain [8, 9, 10, 11]
    assert_eq!(ring.pop(), Some(8));
    assert_eq!(ring.pop(), Some(9));
    assert_eq!(ring.pop(), Some(10));
    assert_eq!(ring.pop(), Some(11));
}

#[test]
fn get_by_index() {
    let ring: SpillRing<i32, 4> = SpillRing::new();

    ring.push(10);
    ring.push(20);
    ring.push(30);

    assert_eq!(ring.get(0), Some(&10));
    assert_eq!(ring.get(1), Some(&20));
    assert_eq!(ring.get(2), Some(&30));
    assert_eq!(ring.get(3), None);
}

#[test]
fn fn_sink_receives_evicted() {
    let evicted = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let evicted_clone = evicted.clone();

    let ring = SpillRing::<i32, 2, _>::with_sink(FnSink(move |x| {
        evicted_clone.lock().unwrap().push(x);
    }));

    ring.push(1);
    ring.push(2);
    ring.push(3); // Evicts 1 directly to sink

    // Sink should have received 1 immediately
    assert_eq!(*evicted.lock().unwrap(), vec![1]);
}

#[test]
fn drop_flushes_to_sink() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SINK_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct CountingSink;
    impl Sink<i32> for CountingSink {
        fn send(&mut self, _item: i32) {
            SINK_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    SINK_COUNT.store(0, Ordering::SeqCst);

    {
        let ring = SpillRing::<i32, 4, _>::with_sink(CountingSink);
        ring.push(1);
        ring.push(2);
        ring.push(3);
        // 3 items in ring, none sent to sink yet
        assert_eq!(SINK_COUNT.load(Ordering::SeqCst), 0);
    }
    // Ring dropped, all 3 items should be flushed to sink
    assert_eq!(SINK_COUNT.load(Ordering::SeqCst), 3);
}

#[test]
fn drop_with_default_sink_drops_items() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct DropCounter;
    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    DROP_COUNT.store(0, Ordering::SeqCst);

    {
        let ring: SpillRing<DropCounter, 4> = SpillRing::new();
        ring.push(DropCounter);
        ring.push(DropCounter);
        ring.push(DropCounter);
        // 3 items in ring, none dropped yet
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 0);
    }
    // Ring dropped with DropSink, all 3 items should be dropped
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 3);
}

#[test]
fn overflow_sends_to_sink_immediately() {
    // N=2 main buffer, evicted items go directly to sink
    let sink = CollectSink::new();
    let ring = SpillRing::<i32, 2, _>::with_sink(sink);

    ring.push(1);
    ring.push(2);
    // Main buffer full: [1, 2]
    assert!(ring.sink().items().is_empty());

    ring.push(3); // Evicts 1 directly to sink
    assert_eq!(ring.sink().items(), vec![1]);

    ring.push(4); // Evicts 2 directly to sink
    assert_eq!(ring.sink().items(), vec![1, 2]);

    ring.push(5); // Evicts 3 directly to sink
    assert_eq!(ring.sink().items(), vec![1, 2, 3]);

    ring.push(6); // Evicts 4 directly to sink
    assert_eq!(ring.sink().items(), vec![1, 2, 3, 4]);

    // Main buffer: [5, 6]
    assert_eq!(ring.pop(), Some(5));
    assert_eq!(ring.pop(), Some(6));
}

#[test]
fn clear_drop_ignores_sink() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SINK_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct CountingSink;
    impl Sink<i32> for CountingSink {
        fn send(&mut self, _item: i32) {
            SINK_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    SINK_COUNT.store(0, Ordering::SeqCst);

    let ring = SpillRing::<i32, 4, _>::with_sink(CountingSink);
    ring.push(1);
    ring.push(2);
    ring.push(3);

    ring.clear_drop();

    assert!(ring.is_empty());
    // Sink should NOT have been called
    assert_eq!(SINK_COUNT.load(Ordering::SeqCst), 0);

    // Prevent drop from calling sink by clearing again
    ring.clear_drop();
}

#[test]
fn push_and_flush() {
    let sink = CollectSink::new();
    let mut ring = SpillRing::<i32, 4, _>::with_sink(sink);

    ring.push_and_flush(1);
    assert!(ring.is_empty());
    assert_eq!(ring.sink().items(), vec![1]);

    ring.push(2);
    ring.push(3);
    ring.push_and_flush(4);
    assert!(ring.is_empty());
    assert_eq!(ring.sink().items(), vec![1, 2, 3, 4]);
}

// Sink-specific tests (from modes-core/src/sinks/tests.rs)

#[test]
fn drop_sink_accepts_items() {
    let mut sink = DropSink;
    sink.send(1);
    sink.send(2);
    sink.send(3);
    // Items are dropped, no way to verify except that it compiles
}

#[test]
fn fn_sink_calls_closure() {
    let mut collected = Vec::new();
    {
        let mut sink = FnSink(|x: i32| collected.push(x));
        sink.send(1);
        sink.send(2);
        sink.send(3);
    }
    assert_eq!(collected, vec![1, 2, 3]);
}

#[test]
fn collect_sink_gathers_items() {
    let mut sink = CollectSink::new();
    sink.send(10);
    sink.send(20);
    sink.send(30);
    assert_eq!(sink.items(), vec![10, 20, 30]);
}

#[test]
fn sink_with_different_types() {
    let mut string_sink = CollectSink::new();
    string_sink.send("hello");
    string_sink.send("world");
    assert_eq!(string_sink.items(), vec!["hello", "world"]);

    let mut tuple_sink = CollectSink::new();
    tuple_sink.send((1, "a"));
    tuple_sink.send((2, "b"));
    assert_eq!(tuple_sink.items(), vec![(1, "a"), (2, "b")]);
}

#[test]
fn fn_flush_sink_calls_both_closures() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEND_COUNT: AtomicUsize = AtomicUsize::new(0);
    static FLUSH_COUNT: AtomicUsize = AtomicUsize::new(0);

    SEND_COUNT.store(0, Ordering::SeqCst);
    FLUSH_COUNT.store(0, Ordering::SeqCst);

    let mut s = sink(
        |_: i32| {
            SEND_COUNT.fetch_add(1, Ordering::SeqCst);
        },
        || {
            FLUSH_COUNT.fetch_add(1, Ordering::SeqCst);
        },
    );

    s.send(1);
    s.send(2);
    s.send(3);
    assert_eq!(SEND_COUNT.load(Ordering::SeqCst), 3);
    assert_eq!(FLUSH_COUNT.load(Ordering::SeqCst), 0);

    s.flush();
    assert_eq!(FLUSH_COUNT.load(Ordering::SeqCst), 1);

    s.flush();
    assert_eq!(FLUSH_COUNT.load(Ordering::SeqCst), 2);
}

#[test]
fn fn_flush_sink_with_unit_flush() {
    let mut collected = Vec::new();
    {
        // Using () for flush (no-op)
        let mut s = sink(|x: i32| collected.push(x), ());
        s.send(10);
        s.send(20);
        s.flush(); // Should be a no-op
    }
    assert_eq!(collected, vec![10, 20]);
}

#[test]
fn drop_sink_flush_is_noop() {
    let mut s = DropSink;
    <DropSink as Sink<i32>>::flush(&mut s); // Should not panic
}

#[test]
fn clear_flushes_to_sink() {
    let sink = CollectSink::new();
    let mut ring = SpillRing::<i32, 4, _>::with_sink(sink);

    ring.push(1);
    ring.push(2);
    ring.push(3);

    ring.clear();

    assert!(ring.is_empty());
    assert_eq!(ring.sink().items(), vec![1, 2, 3]);
}

#[test]
fn default_creates_empty_ring() {
    let ring: SpillRing<i32, 4> = SpillRing::default();
    assert!(ring.is_empty());
    assert_eq!(ring.capacity(), 4);
}

// Test trait implementations
use crate::traits::{RingConsumer, RingProducer};

#[test]
fn ring_producer_trait() {
    let mut ring: SpillRing<i32, 4> = SpillRing::new();

    // try_push
    assert!(RingProducer::try_push(&mut ring, 1).is_ok());
    assert!(RingProducer::try_push(&mut ring, 2).is_ok());
    assert!(RingProducer::try_push(&mut ring, 3).is_ok());
    assert!(RingProducer::try_push(&mut ring, 4).is_ok());

    // is_full
    assert!(RingProducer::is_full(&ring));

    // try_push when full returns Err
    assert_eq!(RingProducer::try_push(&mut ring, 5), Err(5));

    // capacity, len, is_empty
    assert_eq!(RingProducer::capacity(&ring), 4);
    assert_eq!(RingProducer::len(&ring), 4);
    assert!(!RingProducer::is_empty(&ring));
}

#[test]
fn ring_consumer_trait() {
    let mut ring: SpillRing<i32, 4> = SpillRing::new();
    ring.push(10);
    ring.push(20);

    // peek
    assert_eq!(RingConsumer::peek(&ring), Some(&10));

    // try_pop
    assert_eq!(RingConsumer::try_pop(&mut ring), Some(10));
    assert_eq!(RingConsumer::try_pop(&mut ring), Some(20));
    assert_eq!(RingConsumer::try_pop(&mut ring), None);

    // is_empty, len, capacity
    assert!(RingConsumer::is_empty(&ring));
    assert_eq!(RingConsumer::len(&ring), 0);
    assert_eq!(RingConsumer::capacity(&ring), 4);
}

#[test]
fn iter_nth() {
    let ring: SpillRing<i32, 8> = SpillRing::new();
    ring.push(10);
    ring.push(20);
    ring.push(30);
    ring.push(40);
    ring.push(50);

    let mut iter = ring.iter();

    // Skip 2, get 3rd element
    assert_eq!(iter.nth(2), Some(&30));
    // Next after nth should be 4th element
    assert_eq!(iter.next(), Some(&40));
    // nth beyond remaining
    assert_eq!(iter.nth(10), None);
}

#[test]
fn iter_mut_size_hint() {
    let mut ring: SpillRing<i32, 4> = SpillRing::new();
    ring.push(1);
    ring.push(2);
    ring.push(3);

    let iter = ring.iter_mut();
    assert_eq!(iter.size_hint(), (3, Some(3)));
}

#[test]
fn drain_removes_all_items() {
    let mut ring: SpillRing<i32, 4> = SpillRing::new();
    ring.push(1);
    ring.push(2);
    ring.push(3);

    let drained: Vec<_> = ring.drain().collect();
    assert_eq!(drained, vec![1, 2, 3]);
    assert!(ring.is_empty());
}

#[test]
fn extend_adds_items() {
    let mut ring: SpillRing<i32, 8> = SpillRing::new();
    ring.extend([1, 2, 3]);
    assert_eq!(ring.len(), 3);
    assert_eq!(ring.pop(), Some(1));
    assert_eq!(ring.pop(), Some(2));
    assert_eq!(ring.pop(), Some(3));
}

#[test]
fn extend_with_overflow_evicts() {
    let mut ring: SpillRing<i32, 4> = SpillRing::new();
    ring.extend(0..10);
    // Only last 4 items remain (6, 7, 8, 9)
    assert_eq!(ring.len(), 4);
    assert_eq!(ring.pop(), Some(6));
}

#[cfg(feature = "std")]
mod channel_sink_tests {
    use crate::{ChannelSink, Sink, SpillRing};
    use std::sync::mpsc;

    #[test]
    fn channel_sink_sends_evicted_items() {
        let (tx, rx) = mpsc::channel();
        let ring = SpillRing::<i32, 4, _>::with_sink(ChannelSink::new(tx));

        // Fill ring
        ring.push(1);
        ring.push(2);
        ring.push(3);
        ring.push(4);

        // No evictions yet
        assert!(rx.try_recv().is_err());

        // Trigger evictions
        ring.push(5); // evicts 1
        ring.push(6); // evicts 2

        assert_eq!(rx.try_recv(), Ok(1));
        assert_eq!(rx.try_recv(), Ok(2));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn channel_sink_with_flush() {
        let (tx, rx) = mpsc::channel();
        let mut ring = SpillRing::<i32, 4, _>::with_sink(ChannelSink::new(tx));

        ring.push(10);
        ring.push(20);
        ring.push(30);

        ring.flush();

        let items: Vec<_> = rx.try_iter().collect();
        assert_eq!(items, vec![10, 20, 30]);
    }

    #[test]
    fn channel_sink_drop_sends_remaining() {
        let (tx, rx) = mpsc::channel();

        {
            let ring = SpillRing::<i32, 4, _>::with_sink(ChannelSink::new(tx));
            ring.push(1);
            ring.push(2);
            // Ring dropped here, flushes to sink
        }

        let items: Vec<_> = rx.try_iter().collect();
        assert_eq!(items, vec![1, 2]);
    }

    #[test]
    fn channel_sink_accessors() {
        let (tx, rx) = mpsc::channel();
        let sink = ChannelSink::new(tx);

        // Test sender() accessor
        sink.sender().send(42).unwrap();
        assert_eq!(rx.recv(), Ok(42));

        // Test into_sender()
        let sender = sink.into_sender();
        sender.send(99).unwrap();
        assert_eq!(rx.recv(), Ok(99));
    }

    #[test]
    fn channel_sink_ignores_disconnected_receiver() {
        let (tx, rx) = mpsc::channel::<i32>();
        let mut sink = ChannelSink::new(tx);

        // Drop receiver
        drop(rx);

        // send should not panic
        sink.send(1);
        sink.send(2);
    }

    #[test]
    fn channel_sink_mpsc_pattern() {
        // Multiple rings sending to one receiver
        let (tx, rx) = mpsc::channel();

        let ring1 = SpillRing::<i32, 2, _>::with_sink(ChannelSink::new(tx.clone()));
        let ring2 = SpillRing::<i32, 2, _>::with_sink(ChannelSink::new(tx.clone()));
        drop(tx); // Drop original sender

        // Fill and overflow both rings
        ring1.push(10);
        ring1.push(11);
        ring1.push(12); // evicts 10

        ring2.push(20);
        ring2.push(21);
        ring2.push(22); // evicts 20

        // Both evictions should arrive at receiver
        let mut evicted: Vec<_> = rx.try_iter().collect();
        evicted.sort();
        assert_eq!(evicted, vec![10, 20]);
    }
}

// Concurrency tests (only run with atomics feature)
// SpillRing is SPSC (single-producer, single-consumer) safe with atomics.
#[cfg(not(feature = "no-atomics"))]
mod concurrency {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Test SPSC: one producer thread, one consumer thread.
    #[test]
    fn spsc_producer_consumer() {
        let ring = Arc::new(SpillRing::<usize, 64>::new());
        let num_items: usize = 10_000;

        let producer_ring = Arc::clone(&ring);
        let producer = thread::spawn(move || {
            for i in 0..num_items {
                producer_ring.push(i);
            }
        });

        let consumer_ring = Arc::clone(&ring);
        let consumer = thread::spawn(move || {
            let mut received = Vec::new();
            let mut last_value: Option<usize> = None;
            let mut spins = 0;

            loop {
                if let Some(v) = consumer_ring.pop() {
                    // Verify monotonic ordering (SPSC guarantees this)
                    if let Some(last) = last_value {
                        assert!(v > last, "values not monotonic: {} then {}", last, v);
                    }
                    last_value = Some(v);
                    received.push(v);
                    spins = 0;
                } else {
                    spins += 1;
                    // Give up after many empty spins (producer done, items evicted)
                    if spins > 100_000 {
                        break;
                    }
                    thread::yield_now();
                }
            }
            received
        });

        producer.join().expect("producer panicked");
        let received = consumer.join().expect("consumer panicked");

        // Consumer should have received some items (not all due to eviction)
        assert!(!received.is_empty());
        // All received values should be valid (in expected range)
        for &v in &received {
            assert!(v < num_items, "invalid value received: {}", v);
        }
    }

    /// Stress test: rapid push/pop in SPSC pattern.
    #[test]
    fn spsc_stress() {
        let ring = Arc::new(SpillRing::<usize, 16>::new());
        let iterations = 50_000;

        let producer_ring = Arc::clone(&ring);
        let producer = thread::spawn(move || {
            for i in 0..iterations {
                producer_ring.push(i);
            }
        });

        let consumer_ring = Arc::clone(&ring);
        let consumer = thread::spawn(move || {
            let mut count = 0;
            let mut last: Option<usize> = None;
            let mut spins = 0;
            loop {
                if let Some(v) = consumer_ring.pop() {
                    // Verify ordering
                    if let Some(l) = last {
                        assert!(v > l, "ordering violated: {} then {}", l, v);
                    }
                    last = Some(v);
                    count += 1;
                    spins = 0;
                } else {
                    spins += 1;
                    if spins > 100_000 {
                        break;
                    }
                }
            }
            count
        });

        producer.join().expect("producer panicked");
        let consumed = consumer.join().expect("consumer panicked");

        // Should have consumed some items
        assert!(consumed > 0);
    }

    /// Test that consumer sees consistent state during producer activity.
    #[test]
    fn spsc_len_consistency() {
        let ring = Arc::new(SpillRing::<usize, 32>::new());

        let producer_ring = Arc::clone(&ring);
        let producer = thread::spawn(move || {
            for i in 0..5000 {
                producer_ring.push(i);
                if i % 100 == 0 {
                    thread::yield_now();
                }
            }
        });

        let consumer_ring = Arc::clone(&ring);
        let consumer = thread::spawn(move || {
            for _ in 0..1000 {
                let len = consumer_ring.len();
                // len should never exceed capacity
                assert!(len <= 32, "len {} exceeds capacity", len);
                thread::yield_now();
            }
        });

        producer.join().expect("producer panicked");
        consumer.join().expect("consumer panicked");
    }
}

// ProducerSink tests
use crate::ProducerSink;

#[test]
fn producer_sink_assigns_unique_ids() {
    let sink = ProducerSink::new(|_id| CollectSink::<i32>::new());

    let sink0 = sink.clone();
    let sink1 = sink.clone();
    let sink2 = sink.clone();

    assert_eq!(sink0.producer_id(), 0);
    assert_eq!(sink1.producer_id(), 1);
    assert_eq!(sink2.producer_id(), 2);
}

#[test]
fn producer_sink_creates_independent_sinks() {
    let sink = ProducerSink::new(|_id| CollectSink::<i32>::new());

    let mut sink0 = sink.clone();
    let mut sink1 = sink.clone();

    sink0.send(1);
    sink0.send(2);
    sink1.send(10);

    // Each has its own collected items
    assert_eq!(sink0.inner().unwrap().items(), &[1, 2]);
    assert_eq!(sink1.inner().unwrap().items(), &[10]);
}

#[test]
fn producer_sink_lazy_init() {
    let sink = ProducerSink::new(|_id| CollectSink::<i32>::new());

    let sink0 = sink.clone();

    // Inner not initialized until first send
    assert!(sink0.inner().is_none());
}

#[test]
fn producer_sink_flush_delegates() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static FLUSH_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct FlushCounter;
    impl Sink<i32> for FlushCounter {
        fn send(&mut self, _item: i32) {}
        fn flush(&mut self) {
            FLUSH_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    FLUSH_COUNT.store(0, Ordering::SeqCst);

    let sink = ProducerSink::new(|_id| FlushCounter);
    let mut sink0 = sink.clone();

    // Flush before init is a no-op
    sink0.flush();
    assert_eq!(FLUSH_COUNT.load(Ordering::SeqCst), 0);

    // Send initializes, then flush delegates
    sink0.send(1);
    sink0.flush();
    assert_eq!(FLUSH_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn producer_sink_into_inner() {
    let sink = ProducerSink::new(|_id| CollectSink::<i32>::new());
    let mut sink0 = sink.clone();

    sink0.send(42);

    let inner = sink0.into_inner().unwrap();
    assert_eq!(inner.items(), &[42]);
}

#[test]
fn producer_sink_with_mpsc_ring() {
    use crate::MpscRing;

    let sink = ProducerSink::new(|_id| CollectSink::<u64>::new());

    let (producers, consumer) = MpscRing::<u64, 4, _>::with_sink(3, sink);

    // Push enough to cause evictions
    for (i, producer) in producers.into_iter().enumerate() {
        for j in 0..10u64 {
            producer.push(i as u64 * 100 + j);
        }
        // Can't easily collect back without threads, just drop
    }

    // Consumer should have empty rings (producers dropped)
    assert_eq!(consumer.num_producers(), 0);
}

// Ring chaining tests (SpillRing as Sink)

#[test]
fn ring_chaining_basic() {
    // ring1 overflows into ring2
    let ring2: SpillRing<i32, 4> = SpillRing::new();
    let ring1 = SpillRing::<i32, 2, _>::with_sink(ring2);

    ring1.push(1);
    ring1.push(2);
    // ring1 full: [1, 2], ring2 empty

    ring1.push(3); // evicts 1 to ring2
    ring1.push(4); // evicts 2 to ring2
    // ring1: [3, 4], ring2: [1, 2]

    assert_eq!(ring1.pop(), Some(3));
    assert_eq!(ring1.pop(), Some(4));

    // Access ring2 via sink
    assert_eq!(ring1.sink().pop(), Some(1));
    assert_eq!(ring1.sink().pop(), Some(2));
}

#[test]
fn ring_chaining_cascade_overflow() {
    // ring1 -> ring2 -> CollectSink
    // When ring2 also overflows, items go to final sink
    let final_sink = CollectSink::new();
    let ring2 = SpillRing::<i32, 2, _>::with_sink(final_sink);
    let ring1 = SpillRing::<i32, 2, _>::with_sink(ring2);

    // Push 6 items through ring1 (cap 2) -> ring2 (cap 2) -> final_sink
    for i in 1..=6 {
        ring1.push(i);
    }

    // ring1: [5, 6] (most recent)
    // ring2: [3, 4] (evicted from ring1, overflow of 1,2 went to final_sink)
    // final_sink: [1, 2]

    assert_eq!(ring1.sink().sink().items(), vec![1, 2]);
    assert_eq!(ring1.sink().pop(), Some(3));
    assert_eq!(ring1.sink().pop(), Some(4));
    assert_eq!(ring1.pop(), Some(5));
    assert_eq!(ring1.pop(), Some(6));
}

#[test]
fn ring_chaining_flush_cascades() {
    let final_sink = CollectSink::new();
    let ring2 = SpillRing::<i32, 4, _>::with_sink(final_sink);
    let mut ring1 = SpillRing::<i32, 4, _>::with_sink(ring2);

    ring1.push(1);
    ring1.push(2);
    ring1.push(3);

    // Flush ring1 -> items go to ring2
    ring1.flush();
    assert!(ring1.is_empty());
    assert_eq!(ring1.sink().len(), 3);

    // Flush ring2 -> items go to final_sink
    unsafe { ring1.sink_mut_unchecked() }.flush();
    assert!(ring1.sink().is_empty());
    assert_eq!(ring1.sink().sink().items(), vec![1, 2, 3]);
}

#[test]
fn ring_chaining_drop_flushes_all() {
    use std::sync::{Arc, Mutex};

    let collected = Arc::new(Mutex::new(Vec::new()));
    let collected_clone = collected.clone();

    {
        let final_sink = FnSink(move |x: i32| {
            collected_clone.lock().unwrap().push(x);
        });
        let ring2 = SpillRing::<i32, 4, _>::with_sink(final_sink);
        let ring1 = SpillRing::<i32, 4, _>::with_sink(ring2);

        ring1.push(10);
        ring1.push(20);
        ring1.push(30);
        // ring1 dropped here -> flushes to ring2 -> ring2 dropped -> flushes to final_sink
    }

    assert_eq!(*collected.lock().unwrap(), vec![10, 20, 30]);
}
