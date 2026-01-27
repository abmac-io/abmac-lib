extern crate std;

use std::{vec, vec::Vec};

use crate::{BatchSink, CollectSink, DropSink, FnSink, ReduceSink, Sink, SpillRing, sink};

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
fn batch_sink_batches_items() {
    let mut sink: BatchSink<i32, CollectSink<Vec<i32>>> = BatchSink::new(3, CollectSink::new());

    sink.send(1);
    sink.send(2);
    // Not yet forwarded
    assert_eq!(sink.inner().items().len(), 0);
    assert_eq!(sink.buffered(), 2);

    sink.send(3);
    // Batch forwarded
    assert_eq!(sink.inner().items(), vec![vec![1, 2, 3]]);
    assert_eq!(sink.buffered(), 0);

    sink.send(4);
    sink.send(5);
    // Flush remaining
    sink.flush();
    assert_eq!(
        sink.into_inner().into_items(),
        vec![vec![1, 2, 3], vec![4, 5]]
    );
}

#[test]
fn batch_sink_exact_threshold() {
    let mut sink: BatchSink<i32, CollectSink<Vec<i32>>> = BatchSink::new(2, CollectSink::new());

    sink.send(1);
    sink.send(2);
    sink.send(3);
    sink.send(4);

    assert_eq!(sink.inner().items(), vec![vec![1, 2], vec![3, 4]]);
}

#[test]
fn batch_sink_flush_empty_is_noop() {
    let mut sink: BatchSink<i32, CollectSink<Vec<i32>>> = BatchSink::new(10, CollectSink::new());
    sink.flush();
    assert!(sink.into_inner().into_items().is_empty());
}

#[test]
fn batch_sink_with_ring_chain() {
    // ring -> BatchSink -> CollectSink
    // Reduces cascade traffic
    let batch_sink: BatchSink<i32, CollectSink<Vec<i32>>> = BatchSink::new(100, CollectSink::new());
    let ring = SpillRing::<i32, 4, _>::with_sink(batch_sink);

    for i in 0..1000 {
        ring.push(i);
    }

    // Evictions batched into groups of 100
    let batches = ring.sink().inner().items();
    assert!(batches.len() >= 9); // ~996 evictions / 100 = 9+ batches
    assert!(batches.iter().all(|b| b.len() <= 100));
}

#[test]
fn reduce_sink_reduces_batches() {
    let mut sink = ReduceSink::new(
        4,
        |batch: Vec<i32>| batch.iter().sum::<i32>(),
        CollectSink::new(),
    );

    for i in 1..=8 {
        sink.send(i);
    }
    sink.flush();

    // [1+2+3+4=10, 5+6+7+8=26]
    assert_eq!(sink.into_inner().into_items(), vec![10, 26]);
}

#[test]
fn reduce_sink_flush_partial() {
    let mut sink = ReduceSink::new(5, |batch: Vec<i32>| batch.len() as i32, CollectSink::new());

    sink.send(1);
    sink.send(2);
    sink.send(3);
    sink.flush();

    // Partial batch of 3 items
    assert_eq!(sink.into_inner().into_items(), vec![3]);
}

#[test]
fn reduce_sink_type_transform() {
    use std::string::{String, ToString};
    // Transform i32 -> String
    let mut sink = ReduceSink::new(
        2,
        |batch: Vec<i32>| std::format!("{:?}", batch),
        CollectSink::<String>::new(),
    );

    sink.send(1);
    sink.send(2);
    sink.send(3);
    sink.send(4);
    sink.flush();

    assert_eq!(
        sink.into_inner().into_items(),
        vec!["[1, 2]".to_string(), "[3, 4]".to_string()]
    );
}

#[test]
fn reduce_sink_accessors() {
    let sink: ReduceSink<i32, usize, _, CollectSink<usize>> =
        ReduceSink::new(10, |b: Vec<i32>| b.len(), CollectSink::new());
    assert_eq!(sink.threshold(), 10);
    assert_eq!(sink.buffered(), 0);
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
