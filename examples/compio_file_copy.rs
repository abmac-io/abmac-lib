//! Pipelined file copy using spill-ring with compio (io_uring/IOCP).
//!
//! Demonstrates:
//! - **Backpressure**: reader waits when ring is full (prevents data loss)
//! - **Overlapped I/O**: reader and writer run concurrently via spill-ring
//! - **io_uring**: completion-based async I/O for both reads and writes
//!
//! Performance: ~1.9 GB/s on NVMe (vs ~260 MB/s with sequential read-then-write)
//!
//! Run with:
//!   cargo run --example compio_file_copy --features std --release -- input.bin output.bin
//!
//! Create test file:
//!   dd if=/dev/urandom of=input.bin bs=1M count=1024

use compio::{
    buf::BufResult,
    fs::File,
    io::{AsyncReadAtExt, AsyncWriteAtExt},
};
use spill_ring::SpillRing;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

// Optimal settings from benchmarking:
// - 512KB chunks: best throughput (256KB-512KB sweet spot)
// - 64 chunk ring: ~32MB buffer, good balance of memory vs throughput
const CHUNK_SIZE: usize = 512 * 1024;
const RING_CAPACITY: usize = 64;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input_file> <output_file>", args[0]);
        eprintln!();
        eprintln!("Create test file: dd if=/dev/urandom of=input.bin bs=1M count=1024");
        std::process::exit(1);
    }

    let input_path = args[1].clone();
    let output_path = args[2].clone();

    let metadata = std::fs::metadata(&input_path).expect("Failed to get metadata");
    let file_size = metadata.len();

    println!("Pipelined file copy with spill-ring + compio (io_uring)");
    println!("========================================================");
    println!("Input:  {}", input_path);
    println!("Output: {}", output_path);
    println!(
        "Buffer: {} x {}KB = {}MB",
        RING_CAPACITY,
        CHUNK_SIZE / 1024,
        (CHUNK_SIZE * RING_CAPACITY) / (1024 * 1024)
    );
    println!(
        "File:   {:.2} GB",
        file_size as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!();

    let start = Instant::now();

    // Shared state between threads
    let done_reading = Arc::new(AtomicBool::new(false));

    // SPSC ring: reader pushes, writer pops
    // Using DropSink means we MUST implement backpressure to avoid data loss
    let ring = Arc::new(SpillRing::<Vec<u8>, RING_CAPACITY>::new());

    // Reader thread - uses compio/io_uring for async reads
    let reader_ring = Arc::clone(&ring);
    let done_flag = Arc::clone(&done_reading);
    let reader = thread::spawn(move || {
        compio::runtime::Runtime::new().unwrap().block_on(async {
            let file = File::open(&input_path).await.expect("Failed to open input");

            let mut offset: u64 = 0;
            let mut chunks_read: u64 = 0;

            while offset < file_size {
                let to_read = std::cmp::min(CHUNK_SIZE as u64, file_size - offset) as usize;
                let buf = vec![0u8; to_read];

                let BufResult(result, buf) = file.read_exact_at(buf, offset).await;
                result.expect("Failed to read");

                // BACKPRESSURE: wait until ring has space
                // This prevents eviction (data loss) when writer is slower
                while reader_ring.len() >= RING_CAPACITY - 1 {
                    std::hint::spin_loop();
                }

                reader_ring.push(buf);
                offset += to_read as u64;
                chunks_read += 1;
            }

            done_flag.store(true, Ordering::Release);
            chunks_read
        })
    });

    // Writer thread - uses compio/io_uring for async writes
    let writer_ring = Arc::clone(&ring);
    let done_flag = Arc::clone(&done_reading);
    let writer = thread::spawn(move || {
        compio::runtime::Runtime::new().unwrap().block_on(async {
            let mut output = File::create(&output_path)
                .await
                .expect("Failed to create output");

            let mut write_offset: u64 = 0;
            let mut spins = 0u32;

            loop {
                if let Some(chunk) = writer_ring.pop() {
                    let len = chunk.len() as u64;
                    let BufResult(result, _) = output.write_all_at(chunk, write_offset).await;
                    result.expect("Failed to write");
                    write_offset += len;
                    spins = 0;
                } else if done_flag.load(Ordering::Acquire) {
                    // Reader done - drain any remaining chunks
                    while let Some(chunk) = writer_ring.pop() {
                        let len = chunk.len() as u64;
                        let BufResult(result, _) = output.write_all_at(chunk, write_offset).await;
                        result.expect("Failed to write");
                        write_offset += len;
                    }
                    break;
                } else {
                    // Ring empty but reader still running - brief spin then yield
                    spins += 1;
                    if spins > 1000 {
                        thread::yield_now();
                        spins = 0;
                    }
                }
            }

            output.sync_all().await.expect("Failed to sync");
            write_offset
        })
    });

    let chunks_read = reader.join().expect("Reader panicked");
    let bytes_written = writer.join().expect("Writer panicked");

    let elapsed = start.elapsed();
    let throughput_mbs = bytes_written as f64 / elapsed.as_secs_f64() / (1024.0 * 1024.0);
    let throughput_gbs = throughput_mbs / 1024.0;

    println!("Results:");
    println!("  Chunks:     {}", chunks_read);
    println!(
        "  Written:    {:.2} GB",
        bytes_written as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    println!("  Time:       {:.3}s", elapsed.as_secs_f64());
    println!(
        "  Throughput: {:.0} MB/s ({:.2} GB/s)",
        throughput_mbs, throughput_gbs
    );
}
