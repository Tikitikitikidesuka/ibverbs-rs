use env_logger::{Builder, Env};
use pcie40_rs::multi_fragment_packet::{MultiFragmentPacketBuilder, MultiFragmentPacketRef};
use pcie40_rs::pcie40::pcie40_ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40::pcie40_reader::PCIe40Reader;
use pcie40_rs::pcie40::pcie40_stream::PCIe40DAQStreamFormat::MetaFormat;
use pcie40_rs::pcie40::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40::pcie40_stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40::pcie40_stream::PCIe40StreamManager;
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;
use std::time::{Duration, SystemTime};

fn main() {
    const DEVICE_NAME: &str = "tdtel202_0";

    /*
    Builder::from_env(Env::default().default_filter_or("trace"))
        .format_timestamp_secs()
        .format_file(true)
        .format_line_number(true)
        .init();
    */

    let controller = PCIe40ControllerManager::open_by_device_name(DEVICE_NAME).unwrap();
    let meta_alignment = controller.meta_alignment().unwrap();

    let mut stream =
        PCIe40StreamManager::open_by_device_name(DEVICE_NAME, MainStream, MetaFormat).unwrap();
    stream
        .set_raii_enable_state_close_mode(PreserveEnableState)
        .unwrap();

    let mut stream_guard = stream.lock().unwrap();
    let mut reader = PCIe40Reader::new(stream_guard.map_buffer().unwrap(), meta_alignment).unwrap();

    println!(
        "Benchmark: {} ms",
        benchmark(&mut reader, 32, 1000).as_millis()
    );
}

fn benchmark(
    reader: &mut PCIe40Reader,
    iterations: usize,
    nodes: usize,
    packing_factor: usize,
) -> Duration {
    let start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    for _ in 0..iterations {
        let read_mfps = MultiFragmentPacketRef::read_multiple(reader, nodes);
    }
    let end = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
}
