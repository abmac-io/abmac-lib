extern crate std;

use crate::{CollectSpout, ProducerSpout, Spout};

#[test]
fn producer_spout_assigns_unique_ids() {
    let s = ProducerSpout::new(|_id| CollectSpout::<i32>::new());

    let s0 = s.clone();
    let s1 = s.clone();
    let s2 = s.clone();

    assert_eq!(s0.producer_id(), 0);
    assert_eq!(s1.producer_id(), 1);
    assert_eq!(s2.producer_id(), 2);
}

#[test]
fn producer_spout_creates_independent_spouts() {
    let s = ProducerSpout::new(|_id| CollectSpout::<i32>::new());

    let mut s0 = s.clone();
    let mut s1 = s.clone();

    s0.send(1);
    s0.send(2);
    s1.send(10);

    // Each has its own collected items
    assert_eq!(s0.inner().unwrap().items(), &[1, 2]);
    assert_eq!(s1.inner().unwrap().items(), &[10]);
}

#[test]
fn producer_spout_lazy_init() {
    let s = ProducerSpout::new(|_id| CollectSpout::<i32>::new());

    let s0 = s.clone();

    // Inner not initialized until first send
    assert!(s0.inner().is_none());
}

#[test]
fn producer_spout_flush_delegates() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static FLUSH_COUNT: AtomicUsize = AtomicUsize::new(0);

    struct FlushCounter;
    impl Spout<i32> for FlushCounter {
        fn send(&mut self, _item: i32) {}
        fn flush(&mut self) {
            FLUSH_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }

    FLUSH_COUNT.store(0, Ordering::SeqCst);

    let s = ProducerSpout::new(|_id| FlushCounter);
    let mut s0 = s.clone();

    // Flush before init is a no-op
    s0.flush();
    assert_eq!(FLUSH_COUNT.load(Ordering::SeqCst), 0);

    // Send initializes, then flush delegates
    s0.send(1);
    s0.flush();
    assert_eq!(FLUSH_COUNT.load(Ordering::SeqCst), 1);
}

#[test]
fn producer_spout_into_inner() {
    let s = ProducerSpout::new(|_id| CollectSpout::<i32>::new());
    let mut s0 = s.clone();

    s0.send(42);

    let inner = s0.into_inner().unwrap();
    assert_eq!(inner.items(), &[42]);
}
