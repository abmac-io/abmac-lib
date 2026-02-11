use crate::MpscRing;
use spout::CollectSpout;
use std::sync::{Arc, Mutex};
use std::vec;

#[test]
fn basic_mpsc() {
    let collected = Arc::new(Mutex::new(CollectSpout::new()));
    let producers = MpscRing::<u64, 8, _>::with_sink(2, collected.clone());

    // Producer 0
    producers[0].push(1);
    producers[0].push(2);

    // Producer 1
    producers[1].push(10);
    producers[1].push(20);

    // Drop producers to flush to sink
    drop(producers);

    let items = collected.lock().unwrap().items().to_vec();
    assert_eq!(items.len(), 4);
    assert!(items.contains(&1));
    assert!(items.contains(&2));
    assert!(items.contains(&10));
    assert!(items.contains(&20));
}

#[test]
fn producer_overflow_to_sink() {
    let collected = Arc::new(Mutex::new(CollectSpout::new()));

    let producers = MpscRing::<u64, 4, _>::with_sink(1, collected.clone());
    let producer = producers.into_iter().next().unwrap();

    // Overflow - push 10 items into ring of size 4
    for i in 0..10 {
        producer.push(i);
    }

    // First 6 items should have been evicted to sink
    assert_eq!(collected.lock().unwrap().items().len(), 6);

    // Drop flushes remaining 4
    drop(producer);
    assert_eq!(collected.lock().unwrap().items().len(), 10);
}

#[test]
fn empty_producers_drop_sink() {
    // With DropSpout (default), items just get dropped
    let producers = MpscRing::<u64, 8>::new(4);
    assert_eq!(producers.len(), 4);
    for p in &producers {
        assert!(p.is_empty());
    }
}

#[test]
fn single_producer() {
    let collected = Arc::new(Mutex::new(CollectSpout::new()));
    let producers = MpscRing::<u64, 16, _>::with_sink(1, collected.clone());

    let producer = producers.into_iter().next().unwrap();
    for i in 0..10 {
        producer.push(i);
    }

    drop(producer);

    let items = collected.lock().unwrap().items().to_vec();
    assert_eq!(items, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
}

#[cfg(feature = "std")]
mod worker_pool_tests {
    use super::*;

    #[test]
    fn basic_worker_pool() {
        let mut pool = MpscRing::<u64, 64>::pool(2).spawn(|ring, _id, count: &u64| {
            for i in 0..*count {
                ring.push(i);
            }
        });

        pool.run(&50);

        let mut consumer = pool.into_consumer();
        let mut sink = CollectSpout::new();
        consumer.drain(&mut sink);

        let items = sink.into_items();
        assert_eq!(items.len(), 100); // 2 workers * 50 each
    }

    #[test]
    fn worker_pool_overflow() {
        // Small ring to force overflow
        let mut pool = MpscRing::<u64, 8>::pool(1).spawn(|ring, _id, count: &u64| {
            for i in 0..*count {
                ring.push(i);
            }
        });

        pool.run(&100);

        let mut consumer = pool.into_consumer();
        let mut sink = CollectSpout::new();
        consumer.drain(&mut sink);

        // Only last 8 items should remain in ring
        let items = sink.into_items();
        assert_eq!(items.len(), 8);
    }

    #[test]
    fn worker_pool_num_rings() {
        let pool = MpscRing::<u64, 64>::pool(7).spawn(|_ring, _id, _args: &()| {});
        assert_eq!(pool.num_rings(), 7);
    }

    #[test]
    fn worker_pool_empty() {
        let pool = MpscRing::<u64, 64>::pool(4).spawn(|_ring, _id, _args: &()| {});
        let consumer = pool.into_consumer();
        assert!(consumer.is_empty());
        assert_eq!(consumer.num_producers(), 4);
    }

    #[test]
    fn worker_pool_multiple_run_calls() {
        let mut pool = MpscRing::<u64, 128>::pool(2).spawn(|ring, _id, count: &u64| {
            for i in 0..*count {
                ring.push(i);
            }
        });

        pool.run(&10);
        pool.run(&10);

        let mut consumer = pool.into_consumer();
        let mut sink = CollectSpout::new();
        consumer.drain(&mut sink);

        // Should have 40 items total (2 rings × 20 items each)
        let items = sink.into_items();
        assert_eq!(items.len(), 40);
    }

    #[test]
    fn worker_pool_worker_ids() {
        // Each worker should receive a unique ID from 0..num_workers
        let mut pool = MpscRing::<u64, 64>::pool(4).spawn(|ring, id, _args: &()| {
            // Push worker ID so we can verify from the consumer side
            ring.push(id as u64);
        });

        pool.run(&());

        let mut consumer = pool.into_consumer();
        let mut sink = CollectSpout::new();
        consumer.drain(&mut sink);

        let mut ids = sink.into_items();
        ids.sort();
        assert_eq!(ids, vec![0, 1, 2, 3]);
    }

    #[test]
    fn worker_pool_different_args_per_run() {
        let mut pool = MpscRing::<u64, 128>::pool(1).spawn(|ring, _id, val: &u64| {
            ring.push(*val);
        });

        pool.run(&42);
        pool.run(&99);

        let mut consumer = pool.into_consumer();
        let mut sink = CollectSpout::new();
        consumer.drain(&mut sink);

        let items = sink.into_items();
        assert_eq!(items, vec![42, 99]);
    }

    #[test]
    fn worker_pool_with_sink() {
        let collected = Arc::new(Mutex::new(CollectSpout::new()));

        let mut pool = MpscRing::<u64, 4, _>::pool_with_sink(2, collected.clone()).spawn(
            |ring, _id, count: &u64| {
                for i in 0..*count {
                    ring.push(i);
                }
            },
        );

        // Push 10 items per worker into ring of size 4 — forces overflow to sink
        pool.run(&10);

        // Overflow items went to the CollectSpout
        let overflowed = collected.lock().unwrap().items().len();
        assert!(overflowed > 0);

        let mut consumer = pool.into_consumer();
        let mut drain_sink = CollectSpout::new();
        consumer.drain(&mut drain_sink);

        // Remaining items in rings
        let drained = drain_sink.into_items().len();

        // Total: overflowed + drained = 20 (2 workers × 10 each)
        assert_eq!(overflowed + drained, 20);
    }

    #[test]
    fn worker_pool_drop_without_consume() {
        // Pool should drop cleanly without calling into_consumer
        let mut pool = MpscRing::<u64, 64>::pool(4).spawn(|ring, _id, count: &u64| {
            for i in 0..*count {
                ring.push(i);
            }
        });

        pool.run(&100);
        drop(pool); // Should not panic or hang
    }
}
