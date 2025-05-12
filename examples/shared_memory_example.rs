use log::{debug, info, LevelFilter};
use nix::sys::stat::Mode;
use std::io::{Read, Write};
use std::process::Command;
use pcie40_rs::shared_memory_buffer::shared_memory::SharedMemory;

// Setup logging with env_logger if needed
fn setup_logging() {
    env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .init();
}

fn main() {
    setup_logging();

    // Example 1: Basic shared memory creation and usage
    basic_shared_memory_example();

    // Example 2: IPC with shared memory (conceptual)
    ipc_shared_memory_example();
}

fn basic_shared_memory_example() {
    info!("Running basic shared memory example");

    // Define the shared memory path and size
    let shmem_path = "/test_shared_memory";
    let shmem_size = 1024; // 1KB

    // Set permission mode (readable/writable by owner and group)
    let permission_mode = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IWGRP;

    // Step 1: Create the shared memory segment
    let shared_memory = match SharedMemory::create(shmem_path, shmem_size, permission_mode) {
        Ok(shm) => {
            info!("Created shared memory at {} with size {}", shmem_path, shmem_size);
            shm
        },
        Err(e) => {
            info!("Failed to create shared memory: {}. Trying to open existing...", e);
            // Try to open if it already exists
            SharedMemory::open(shmem_path).expect("Failed to open existing shared memory")
        }
    };

    // Step 2: Map the shared memory to process address space
    let mut mapped_memory = shared_memory.map().expect("Failed to map shared memory");

    // Step 3: Write data to the shared memory
    let test_message = b"Hello from shared memory!";
    unsafe {
        let buffer = mapped_memory.as_slice_mut();
        // Write the test message
        buffer[..test_message.len()].copy_from_slice(test_message);
        // Write a null terminator
        buffer[test_message.len()] = 0;

        info!("Wrote message to shared memory: {:?}", std::str::from_utf8(test_message).unwrap());
    }

    // Step 4: Read data back from the shared memory
    unsafe {
        let buffer = mapped_memory.as_slice();
        let mut read_message = Vec::new();

        // Read until null terminator
        for &byte in buffer.iter() {
            if byte == 0 {
                break;
            }
            read_message.push(byte);
        }

        info!("Read message from shared memory: {:?}",
              std::str::from_utf8(&read_message).unwrap());
    }

    // Step 5: Unmap the shared memory (optional, as it would be done on drop)
    let shared_memory = mapped_memory.unmap().expect("Failed to unmap shared memory");

    // Cleanup: remove the shared memory segment
    // Note: In a real application, you might keep this around for IPC
    info!("Cleaning up shared memory segment");
    unsafe {
        libc::shm_unlink(std::ffi::CString::new(shmem_path)
            .unwrap()
            .as_ptr());
    }
}

fn ipc_shared_memory_example() {
    info!("Running IPC shared memory example");

    // In a real IPC scenario, this would be running in a different process
    // but we'll simulate the concept here

    let shmem_path = "/ipc_shared_memory";
    let shmem_size = 4096; // 4KB
    let permission_mode = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IWGRP;

    // Process 1: Create shared memory and write data
    {
        info!("Process 1: Creating shared memory and writing data");

        let shared_memory = SharedMemory::create(shmem_path, shmem_size, permission_mode)
            .expect("Failed to create shared memory");

        let mut mapped_memory = shared_memory.map()
            .expect("Failed to map shared memory");

        // Define a simple data structure to share
        #[repr(C)]
        struct SharedData {
            counter: u32,
            message: [u8; 128],
        }

        // Initialize shared data
        unsafe {
            let buffer = mapped_memory.as_slice_mut();
            let shared_data = buffer.as_mut_ptr() as *mut SharedData;

            (*shared_data).counter = 42;

            let message = b"Data from Process 1";
            let message_len = message.len().min(127); // Ensure it fits with null terminator

            std::ptr::copy_nonoverlapping(
                message.as_ptr(),
                (*shared_data).message.as_mut_ptr(),
                message_len
            );

            // Add null terminator
            (*shared_data).message[message_len] = 0;
        }

        info!("Process 1: Data written to shared memory");
        // In a real application, the mapped_memory would be kept mapped
        // as long as the IPC is active, but we'll unmap it to simulate
        // process separation
    }

    // Process 2: Open existing shared memory and read data
    {
        info!("Process 2: Opening shared memory and reading data");

        let shared_memory = SharedMemory::open(shmem_path)
            .expect("Failed to open shared memory");

        let mapped_memory = shared_memory.map()
            .expect("Failed to map shared memory");

        // Read back the data structure
        unsafe {
            #[repr(C)]
            struct SharedData {
                counter: u32,
                message: [u8; 128],
            }

            let buffer = mapped_memory.as_slice();
            let shared_data = buffer.as_ptr() as *const SharedData;

            let counter = (*shared_data).counter;

            // Convert message to string
            let mut message_len = 0;
            while message_len < 128 && (*shared_data).message[message_len] != 0 {
                message_len += 1;
            }

            let message = std::str::from_utf8(
                &(*shared_data).message[..message_len]
            ).unwrap();

            info!("Process 2: Read data from shared memory:");
            info!("  Counter: {}", counter);
            info!("  Message: {}", message);
        }
    }

    // Cleanup shared memory
    info!("Cleaning up IPC shared memory segment");
    unsafe {
        libc::shm_unlink(std::ffi::CString::new(shmem_path)
            .unwrap()
            .as_ptr());
    }
}