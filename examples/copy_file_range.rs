//! Fast file copy using copy_file_range (Linux 4.5+)
//!
//! This is the fastest way to copy files - entirely in-kernel, no userspace buffers.
//! Achieves ~6.7 GB/s on NVMe vs ~1.9 GB/s for read()/write().
//!
//! Use this for pure file copy. Use spill-ring when you need to process data
//! between read and write.
//!
//! Run: cargo run --example copy_file_range -- <input> <output>

use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::time::Instant;

const CHUNK_SIZE: usize = 1024 * 1024; // 1MB chunks

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input_file> <output_file>", args[0]);
        eprintln!();
        eprintln!("Create test file: dd if=/dev/urandom of=input.bin bs=1M count=1024");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let output_path = &args[2];

    let metadata = std::fs::metadata(input_path).expect("Failed to get metadata");
    let file_size = metadata.len() as usize;

    println!("copy_file_range: {} -> {}", input_path, output_path);
    println!("File size: {:.2} GB", file_size as f64 / 1e9);

    let start = Instant::now();

    let inf = File::open(input_path).expect("Failed to open input");
    let outf = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)
        .expect("Failed to create output");

    let mut off_in: libc::loff_t = 0;
    let mut off_out: libc::loff_t = 0;
    let mut remaining = file_size;

    while remaining > 0 {
        let to_copy = remaining.min(CHUNK_SIZE);
        let copied = unsafe {
            libc::syscall(
                libc::SYS_copy_file_range,
                inf.as_raw_fd(),
                &mut off_in,
                outf.as_raw_fd(),
                &mut off_out,
                to_copy,
                0u32,
            )
        };
        if copied < 0 {
            panic!(
                "copy_file_range failed: {}",
                std::io::Error::last_os_error()
            );
        }
        remaining -= copied as usize;
    }

    outf.sync_all().expect("Failed to sync");

    let elapsed = start.elapsed();
    let throughput = file_size as f64 / elapsed.as_secs_f64() / 1e9;

    println!("Time: {:.3}s", elapsed.as_secs_f64());
    println!("Throughput: {:.2} GB/s", throughput);
}
