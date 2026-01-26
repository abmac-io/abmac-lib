//! MPSC zero-contention example: Each producer writes to its own file.
//!
//! Demonstrates `ProducerSink` for zero-contention MPSC patterns where each
//! producer gets its own independent sink.
//!
//! Run with: cargo run --example mpsc_zero_contention

use spill_ring::{FnFlushSink, MpscRing, ProducerSink, collect_producers};
use std::{
    fs::File,
    io::{BufWriter, Write},
    thread,
};

/// A sensor reading with timestamp and value.
#[derive(Clone)]
struct SensorReading {
    timestamp: u64,
    sensor_id: u32,
    value: f64,
}

impl SensorReading {
    fn to_bytes(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0..8].copy_from_slice(&self.timestamp.to_le_bytes());
        buf[8..12].copy_from_slice(&self.sensor_id.to_le_bytes());
        buf[12..20].copy_from_slice(&self.value.to_le_bytes());
        buf
    }
}

fn main() -> std::io::Result<()> {
    const NUM_PRODUCERS: usize = 4;
    const READINGS_PER_PRODUCER: u64 = 250_000;
    const TOTAL_READINGS: u64 = NUM_PRODUCERS as u64 * READINGS_PER_PRODUCER;
    const RING_CAPACITY: usize = 1024;

    println!("MPSC Zero-Contention Example");
    println!("============================");
    println!("Producers: {}", NUM_PRODUCERS);
    println!("Readings per producer: {}", READINGS_PER_PRODUCER);
    println!("Total readings: {}", TOTAL_READINGS);
    println!("Ring capacity per producer: {}", RING_CAPACITY);
    println!();
    println!("Each producer writes evictions to its own file - ZERO lock contention!");
    println!();

    // ProducerSink creates independent sinks via factory function.
    // Each clone gets a unique producer_id (0, 1, 2, ...).
    let sink = ProducerSink::new(|producer_id| {
        let path = format!("mpsc_producer_{}.bin", producer_id);
        let file = File::create(&path).expect("failed to create file");
        let mut writer = BufWriter::new(file);

        FnFlushSink::new(
            move |item: SensorReading| {
                writer.write_all(&item.to_bytes()).unwrap();
            },
            || {},
        )
    });

    // Create MPSC ring - each producer gets its own file sink
    let (producers, mut consumer) =
        MpscRing::<SensorReading, RING_CAPACITY, _>::with_sink(NUM_PRODUCERS, sink);

    // Spawn producers - each has its own file sink, zero contention
    let finished_producers: Vec<_> = thread::scope(|s| {
        producers
            .into_iter()
            .enumerate()
            .map(|(producer_id, producer)| {
                s.spawn(move || {
                    for i in 0..READINGS_PER_PRODUCER {
                        let reading = SensorReading {
                            timestamp: i,
                            sensor_id: producer_id as u32,
                            value: (i as f64 + producer_id as f64).sin(),
                        };
                        producer.push(reading);
                    }
                    producer
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect()
    });

    // Collect producers back
    collect_producers(finished_producers, &mut consumer);

    // Drain remaining items (what's left in each ring after producers finish)
    let mut drain_counts: Vec<u64> = vec![0; NUM_PRODUCERS];

    // Write drained items to a merge file
    let mut merge_file = BufWriter::new(File::create("mpsc_merged.bin")?);

    for item in consumer.drain() {
        let pid = item.sensor_id as usize;
        merge_file.write_all(&item.to_bytes())?;
        drain_counts[pid] += 1;
    }
    merge_file.flush()?;

    // Drop consumer to flush all sinks' BufWriters
    drop(consumer);

    println!("Results:");
    println!("  Items generated: {}", TOTAL_READINGS);
    println!();

    // Verify by reading back the per-producer files
    println!("Per-producer verification:");
    let mut all_match = true;
    let mut total_from_files = 0u64;

    for (pid, &drained) in drain_counts.iter().enumerate() {
        let eviction_path = format!("mpsc_producer_{}.bin", pid);
        let eviction_count = std::fs::read(&eviction_path)
            .map(|data| (data.len() / 20) as u64)
            .unwrap_or(0);

        let total_for_producer = eviction_count + drained;
        total_from_files += total_for_producer;

        let expected = READINGS_PER_PRODUCER;
        let status = if total_for_producer == expected {
            "PASS"
        } else {
            all_match = false;
            "FAIL"
        };

        println!(
            "  Producer {}: evicted={}, drained={}, total={} (expected {}) [{}]",
            pid, eviction_count, drained, total_for_producer, expected, status
        );

        // Cleanup producer file
        let _ = std::fs::remove_file(&eviction_path);
    }

    println!();
    println!(
        "  Total items: {} (expected {})",
        total_from_files, TOTAL_READINGS
    );

    if all_match && total_from_files == TOTAL_READINGS {
        println!();
        println!("Overall Status: PASS - all items accounted for!");
    } else {
        println!();
        println!("Overall Status: FAIL - item count mismatch!");
        std::process::exit(1);
    }

    // Cleanup
    std::fs::remove_file("mpsc_merged.bin")?;

    Ok(())
}
