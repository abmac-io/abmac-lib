//! MPSC (Multiple-Producer, Single-Consumer) benchmarks.
//!
//! Zero-overhead MPSC: each producer owns its own ring, no shared state.
//! Run with `--features no-atomics` for maximum performance.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use spill_ring::{CollectSink, MpscRing, ProducerSink, Sink, SpillRing, collect_producers};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

/// Benchmark MPSC throughput with varying producer counts.
fn mpsc_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpsc_throughput");

    for num_producers in [1, 2, 4, 8] {
        let iterations_per_producer = 100_000u64;
        let total = iterations_per_producer * num_producers as u64;
        group.throughput(Throughput::Elements(total));

        group.bench_with_input(
            BenchmarkId::new("producers", num_producers),
            &num_producers,
            |b, &n| {
                b.iter(|| {
                    let (producers, consumer) = MpscRing::<u64, 1024>::new(n);

                    thread::scope(|s| {
                        for producer in producers {
                            s.spawn(move || {
                                for i in 0..iterations_per_producer {
                                    producer.push(black_box(i));
                                }
                                producer // Return for collection
                            });
                        }
                    });

                    // Note: In real usage, you'd collect producers back
                    // For benchmark, we just measure the push throughput
                    black_box(consumer.num_producers())
                })
            },
        );
    }
    group.finish();
}

/// Benchmark with full producer collection and drain.
fn mpsc_full_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpsc_full_cycle");

    for num_producers in [1, 2, 4, 8] {
        let iterations_per_producer = 50_000u64;
        let total = iterations_per_producer * num_producers as u64;
        group.throughput(Throughput::Elements(total));

        group.bench_with_input(
            BenchmarkId::new("producers", num_producers),
            &num_producers,
            |b, &n| {
                b.iter(|| {
                    let (producers, mut consumer) = MpscRing::<u64, 1024>::new(n);

                    let finished_producers: Vec<_> = thread::scope(|s| {
                        producers
                            .into_iter()
                            .map(|producer| {
                                s.spawn(move || {
                                    for i in 0..iterations_per_producer {
                                        producer.push(black_box(i));
                                    }
                                    producer
                                })
                            })
                            .collect::<Vec<_>>()
                            .into_iter()
                            .map(|h| h.join().unwrap())
                            .collect()
                    });

                    collect_producers(finished_producers, &mut consumer);
                    black_box(consumer.drain().count())
                })
            },
        );
    }
    group.finish();
}

/// Compare MPSC vs single-threaded baseline.
fn mpsc_vs_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpsc_vs_single");

    let total_items = 200_000u64;
    group.throughput(Throughput::Elements(total_items));

    // Single-threaded baseline
    group.bench_function("single_thread", |b| {
        b.iter(|| {
            let ring: SpillRing<u64, 1024> = SpillRing::new();
            for i in 0..total_items {
                ring.push(black_box(i));
            }
            black_box(ring.len())
        })
    });

    // MPSC with 4 producers (50k each)
    group.bench_function("mpsc_4_producers", |b| {
        b.iter(|| {
            let (producers, mut consumer) = MpscRing::<u64, 1024>::new(4);
            let per_producer = total_items / 4;

            let finished: Vec<_> = thread::scope(|s| {
                producers
                    .into_iter()
                    .map(|producer| {
                        s.spawn(move || {
                            for i in 0..per_producer {
                                producer.push(black_box(i));
                            }
                            producer
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .collect()
            });

            collect_producers(finished, &mut consumer);
            black_box(consumer.drain().count())
        })
    });

    group.finish();
}

/// Benchmark scaling - how throughput scales with producer count.
fn mpsc_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("mpsc_scaling");

    // Fixed total work, split across producers
    let total_items = 400_000u64;

    for num_producers in [1, 2, 4, 8] {
        let per_producer = total_items / num_producers as u64;
        group.throughput(Throughput::Elements(total_items));

        group.bench_with_input(
            BenchmarkId::new("producers", num_producers),
            &num_producers,
            |b, &n| {
                b.iter(|| {
                    let (producers, mut consumer) = MpscRing::<u64, 2048>::new(n);

                    let finished: Vec<_> = thread::scope(|s| {
                        producers
                            .into_iter()
                            .map(|producer| {
                                s.spawn(move || {
                                    for i in 0..per_producer {
                                        producer.push(black_box(i));
                                    }
                                    producer
                                })
                            })
                            .collect::<Vec<_>>()
                            .into_iter()
                            .map(|h| h.join().unwrap())
                            .collect()
                    });

                    collect_producers(finished, &mut consumer);
                    black_box(consumer.total_len())
                })
            },
        );
    }
    group.finish();
}

/// Compare ProducerSink vs manual custom sink implementation.
fn producer_sink_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("producer_sink_overhead");

    let iterations_per_producer = 100_000u64;
    let num_producers = 4;
    let total = iterations_per_producer * num_producers as u64;
    group.throughput(Throughput::Elements(total));

    // Using ProducerSink helper
    group.bench_function("producer_sink", |b| {
        b.iter(|| {
            let sink = ProducerSink::new(|_id| CollectSink::<u64>::new());
            let (producers, mut consumer) =
                MpscRing::<u64, 1024, _>::with_sink(num_producers, sink);

            let finished: Vec<_> = thread::scope(|s| {
                producers
                    .into_iter()
                    .map(|producer| {
                        s.spawn(move || {
                            for i in 0..iterations_per_producer {
                                producer.push(black_box(i));
                            }
                            producer
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .collect()
            });

            collect_producers(finished, &mut consumer);
            black_box(consumer.drain().count())
        })
    });

    // Manual custom sink (what user would write without ProducerSink)
    struct ManualSink {
        inner: Option<CollectSink<u64>>,
        #[allow(dead_code)]
        producer_id: usize,
        next_id: Arc<AtomicUsize>,
    }

    impl Clone for ManualSink {
        fn clone(&self) -> Self {
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            Self {
                inner: None,
                producer_id: id,
                next_id: Arc::clone(&self.next_id),
            }
        }
    }

    impl Sink<u64> for ManualSink {
        fn send(&mut self, item: u64) {
            if self.inner.is_none() {
                self.inner = Some(CollectSink::new());
            }
            self.inner.as_mut().unwrap().send(item);
        }
        fn flush(&mut self) {
            if let Some(inner) = &mut self.inner {
                inner.flush();
            }
        }
    }

    group.bench_function("manual_sink", |b| {
        b.iter(|| {
            let sink = ManualSink {
                inner: None,
                producer_id: 0,
                next_id: Arc::new(AtomicUsize::new(0)),
            };
            let (producers, mut consumer) =
                MpscRing::<u64, 1024, _>::with_sink(num_producers, sink);

            let finished: Vec<_> = thread::scope(|s| {
                producers
                    .into_iter()
                    .map(|producer| {
                        s.spawn(move || {
                            for i in 0..iterations_per_producer {
                                producer.push(black_box(i));
                            }
                            producer
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .collect()
            });

            collect_producers(finished, &mut consumer);
            black_box(consumer.drain().count())
        })
    });

    // No sink (DropSink) baseline
    group.bench_function("drop_sink", |b| {
        b.iter(|| {
            let (producers, mut consumer) = MpscRing::<u64, 1024>::new(num_producers);

            let finished: Vec<_> = thread::scope(|s| {
                producers
                    .into_iter()
                    .map(|producer| {
                        s.spawn(move || {
                            for i in 0..iterations_per_producer {
                                producer.push(black_box(i));
                            }
                            producer
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .collect()
            });

            collect_producers(finished, &mut consumer);
            black_box(consumer.drain().count())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    mpsc_throughput,
    mpsc_full_cycle,
    mpsc_vs_single,
    mpsc_scaling,
    producer_sink_overhead,
);
criterion_main!(benches);
