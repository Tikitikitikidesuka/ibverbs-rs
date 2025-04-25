use criterion::{black_box, BenchmarkId, Criterion, criterion_group, criterion_main};
use pcie40_rs::multi_fragment_packet::MultiFragmentPacketRef;
use pcie40_rs::pcie40::pcie40_ctrl::PCIe40ControllerManager;
use pcie40_rs::pcie40::pcie40_reader::PCIe40Reader;
use pcie40_rs::pcie40::pcie40_stream::PCIe40DAQStreamFormat::MetaFormat;
use pcie40_rs::pcie40::pcie40_stream::PCIe40DAQStreamType::MainStream;
use pcie40_rs::pcie40::pcie40_stream::PCIe40StreamHandleEnableStateCloseMode::PreserveEnableState;
use pcie40_rs::pcie40::pcie40_stream::PCIe40StreamManager;
use pcie40_rs::typed_zero_copy_ring_buffer_reader::ZeroCopyRingBufferReadable;

const DEVICE_NAME: &str = "tdtel202_0";
const ITERATIONS: &[i32] = &[16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
const NODES: &[i32] = &[16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

fn setup() -> PCIe40Reader {
    let controller = PCIe40ControllerManager::open_by_device_name(DEVICE_NAME).unwrap();
    let meta_alignment = controller.meta_alignment().unwrap();

    let mut stream =
        PCIe40StreamManager::open_by_device_name(DEVICE_NAME, MainStream, MetaFormat).unwrap();
    stream
        .set_raii_enable_state_close_mode(PreserveEnableState)
        .unwrap();

    let mut stream_guard = stream.lock().unwrap();
    PCIe40Reader::new(stream_guard.map_buffer().unwrap(), meta_alignment).unwrap()
}

fn read_mfps(reader: &mut PCIe40Reader, nodes: usize) {
    let _ = MultiFragmentPacketRef::read_multiple(reader, nodes).unwrap();
}

fn benchmark_pcie40_reader(c: &mut Criterion) {
    let mut group = c.benchmark_group("PCIe40Reader Benchmarks");

    for iterations in ITERATIONS.iter() {
        for nodes in NODES.iter() {
            let param = (*iterations, *nodes);
            group.bench_with_input(
                BenchmarkId::new("read_multiple", format!("iter={},nodes={}", iterations, nodes)),
                &param,
                |b, &(iterations, nodes)| {
                    let mut reader = setup();
                    b.iter(|| {
                        for _ in 0..black_box(iterations) {
                            read_mfps(&mut reader, black_box(nodes as usize));
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, benchmark_pcie40_reader);
criterion_main!(benches);
