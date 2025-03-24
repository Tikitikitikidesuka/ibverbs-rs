/*
use std::ffi::CString;
use std::{ptr, thread};
use std::slice;
use std::time::Duration;
// Assuming bindgen created these bindings
use pcie40_rs::bindings::*;

fn main() {
    unsafe {
        // Get device ID by name
        let device_name = CString::new("tdtel203_0").unwrap();
        let device_id = p40_id_find(device_name.as_ptr());
        if device_id < 0 {
            eprintln!("Failed to find device 'tdtel203_0'");
            return;
        }

        println!("Found device 'tdtel203_0' with ID: {}", device_id);

        // Open the device
        let id_fd = p40_id_open(device_id);
        if id_fd < 0 {
            eprintln!("Failed to open device");
            return;
        }

        // Open stream (assuming stream 0)
        let stream = P40_DAQ_STREAM_P40_DAQ_STREAM_MAIN as i32;
        let stream_fd = p40_stream_open(device_id, stream);
        if stream_fd < 0 {
            p40_id_close(id_fd);
            eprintln!("Failed to open stream");
            return;
        }

        // Check if stream is enabled
        let enabled = p40_stream_enabled(stream_fd);
        if enabled <= 0 {
            p40_stream_close(stream_fd, ptr::null_mut());
            p40_id_close(id_fd);
            eprintln!("Stream not enabled");
            return;
        }

        // Lock and map buffer
        if p40_stream_lock(stream_fd) < 0 {
            p40_stream_close(stream_fd, ptr::null_mut());
            p40_id_close(id_fd);
            eprintln!("Failed to lock stream");
            return;
        }

        let buffer = p40_stream_map(stream_fd);
        if buffer.is_null() {
            p40_stream_unlock(stream_fd);
            p40_stream_close(stream_fd, ptr::null_mut());
            p40_id_close(id_fd);
            eprintln!("Failed to map buffer");
            return;
        }

        // Get buffer size
        let buffer_size = p40_stream_get_host_buf_bytes(stream_fd);
        if buffer_size < 0 {
            p40_stream_unlock(stream_fd);
            p40_stream_close(stream_fd, buffer);
            p40_id_close(id_fd);
            eprintln!("Failed to get buffer size");
            return;
        }

        println!("Buffer size: {} bytes", buffer_size);

        // Get read offset
        let read_offset = p40_stream_get_host_buf_read_off(stream_fd);
        if read_offset < 0 {
            p40_stream_unlock(stream_fd);
            p40_stream_close(stream_fd, buffer);
            p40_id_close(id_fd);
            eprintln!("Failed to get read offset");
            return;
        }

        // Check available data

        let mut available = p40_stream_get_host_buf_bytes_used(stream_fd);

        while available == 0 {
            thread::sleep(Duration::from_millis(100));
            available = p40_stream_get_host_buf_bytes_used(stream_fd);
        }

        if available < 0 {
            p40_stream_unlock(stream_fd);
            p40_stream_close(stream_fd, buffer);
            p40_id_close(id_fd);
            eprintln!("Failed to get available data");
            return;
        }

        println!("Available data: {} bytes at offset {}", available, read_offset);



        if available > 0 {
            // Read some data (up to 1024 bytes)
            let read_size = std::cmp::min(4096, available as usize);
            let data_ptr = (buffer as usize + read_offset as usize) as *const u8;
            let data = slice::from_raw_parts(data_ptr, read_size);

            // Print data in a nice hex format
            println!("Hexadecimal dump of first {} bytes:", read_size);
            for (i, chunk) in data.chunks(16).enumerate() {
                // Print offset
                print!("{:04x}: ", i * 16);

                // Print hex values
                for (j, &byte) in chunk.iter().enumerate() {
                    print!("{:02x} ", byte);

                    // Add extra space after 8 bytes for readability
                    if j == 7 {
                        print!(" ");
                    }
                }

                // Pad for alignment if chunk is less than 16 bytes
                if chunk.len() < 16 {
                    let padding = 16 - chunk.len();
                    for _ in 0..padding {
                        print!("   "); // 3 spaces for each missing byte
                    }

                    // Add extra space for the missing middle separator if needed
                    if chunk.len() <= 7 {
                        print!(" ");
                    }
                }

                // Print ASCII representation
                print!(" | ");
                for &byte in chunk {
                    // Print printable ASCII characters, substitute others with a dot
                    if byte >= 32 && byte <= 126 {
                        print!("{}", byte as char);
                    } else {
                        print!(".");
                    }
                }
                println!();
            }

            // Acknowledge the read
            let free_result = p40_stream_free_host_buf_bytes(stream_fd, read_size);
            if free_result < 0 {
                println!("Warning: Failed to acknowledge data");
            }
        } else {
            println!("No data available");
        }

        // Clean up
        p40_stream_unlock(stream_fd);
        p40_stream_close(stream_fd, buffer);
        p40_id_close(id_fd);

        println!("Successfully cleaned up resources");
    }
}
*/

use pcie40_rs::old_mfp_reader::{PCIe40Reader, PCIe40Error};

fn main() -> Result<(), PCIe40Error> {
    // Open the device by name (uses MAIN stream by default)
    let mut reader = PCIe40Reader::open("tdtel203_0")?;
    println!("Successfully opened device: {}", reader.name());
    println!("Buffer size: {} bytes", reader.buffer_size());

    // Configure MFP mode with packing factor 1
    reader.configure_mfp(1)?;
    println!("Configured MFP mode");

    // Read and process up to 100 MFPs
    let mut mfp_count = 0;
    let mut end_of_run = false;

    println!("Starting to read MFPs...");
    while mfp_count < 100 && !end_of_run {
        // Try to read an MFP
        match reader.try_read_mfp()? {
            Some(mfp) => {
                println!("Read MFP #{}: event ID: {}, size: {} bytes, fragments: {}",
                         mfp_count + 1,
                         mfp.header().ev_id(),
                         mfp.header().packet_size(),
                         mfp.header().n_banks()
                );

                // Process each fragment in the MFP
                for (idx, (fragment, frag_type)) in mfp.fragments().enumerate() {
                    println!("  Fragment {}: type {}, size {} bytes",
                             idx, frag_type, fragment.len());

                    // First few bytes of each fragment (up to 16)
                    if fragment.len() > 0 {
                        let display_bytes = std::cmp::min(16, fragment.len());
                        println!("    First bytes: {:?}", &fragment[..display_bytes]);
                    }
                }

                // Check for end-of-run marker
                if mfp.is_end_run() {
                    println!("End of run detected!");
                    end_of_run = true;
                }

                mfp_count += 1;

                // Acknowledge every 10 MFPs to free buffer space
                if mfp_count % 10 == 0 {
                    reader.acknowledge_read()?;
                    println!("Acknowledged reading {} MFPs", mfp_count);
                }
            },
            None => {
                // No data available, wait a bit
                println!("No data available, waiting...");
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Print buffer occupancy for debug
                println!("Buffer occupancy: {} bytes", reader.buffer_occupancy()?);
            }
        }
    }

    // Final acknowledge for any remaining data
    reader.acknowledge_read()?;
    println!("Finished reading {} MFPs", mfp_count);

    // Reader will be automatically cleaned up when it goes out of scope
    // thanks to the Drop implementation

    Ok(())
}