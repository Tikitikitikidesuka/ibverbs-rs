use std::io::{stdin, Read};
use std::time::{Duration, Instant};
use pcie40_rs::multi_fragment_packet::MultiFragmentPacketRef;
use pcie40_rs::pcie40::pcie40_ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40::pcie40_reader::PCIe40Reader;
use pcie40_rs::pcie40::pcie40_stream::PCIe40DAQStreamFormat::MetaFormat;
use pcie40_rs::pcie40::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40::pcie40_stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40::pcie40_stream::{PCIe40StreamManager, PCIe40Stream};
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;

const DEVICE_NAME: &str = "tdtel203_1"; // Changed to match C++ example
const NODES: usize = 500;
const ITERATIONS: usize = 1;
const PACKING_FACTOR: usize = 1000;
const TEST_MEAN_ITERATIONS: u32 = 10;

fn run_test(reader: &mut PCIe40Reader, iterations: usize, nodes: usize) -> Duration {
    let mut total_time = Duration::from_nanos(0);

    for _iter in 0..iterations {
        // Benchmark starts
        let t0 = Instant::now();

        // Simple read_multiple call, as specified
        let mfps = MultiFragmentPacketRef::read_multiple(reader, nodes).unwrap();

        // Discard read MFPs
        mfps.discard().unwrap();

        // Benchmark ends
        let t1 = Instant::now();
        total_time += t1.duration_since(t0);
    }

    total_time
}

fn main() {
    // Create stream
    let mut stream = PCIe40StreamManager::open_by_device_name(DEVICE_NAME, MainStream, MetaFormat).unwrap();
    stream.set_raii_enable_state_close_mode(PreserveEnableState).unwrap();

    // Create controller and get meta alignment
    let controller = PCIe40ControllerManager::open_by_device_name(DEVICE_NAME).unwrap();
    let meta_alignment = controller.meta_alignment().unwrap();

    // Lock the stream and create reader
    let mut stream_guard = stream.lock().unwrap();
    let mut reader = PCIe40Reader::new(stream_guard.map_buffer().unwrap(), meta_alignment).unwrap();

    // Wait for user input
    println!("\n\nStream configured... Press any key to proceed\n");
    stdin().read_exact(&mut [0]).unwrap();

    // Run the benchmark
    let mut test_times = Vec::with_capacity(TEST_MEAN_ITERATIONS as usize);
    let mut avg_test_time = Duration::from_nanos(0);

    for _ in 0..TEST_MEAN_ITERATIONS {
        let test_iter_time = run_test(&mut reader, ITERATIONS, NODES);
        test_times.push(test_iter_time);
        avg_test_time += test_iter_time;
    }

    avg_test_time /= TEST_MEAN_ITERATIONS;

    // Calculate standard deviation
    let avg_ns = avg_test_time.as_nanos() as f64;
    let variance = test_times.iter()
        .map(|duration| {
            let diff = duration.as_nanos() as f64 - avg_ns;
            diff * diff
        })
        .sum::<f64>() / TEST_MEAN_ITERATIONS as f64;

    let std_dev = variance.sqrt();

    println!(
        "Read-discard benchmark (iter={ITERATIONS}, nodes={NODES}): Avg={:.2}ns, Std={:.2}ns",
        avg_ns, std_dev
    );
}