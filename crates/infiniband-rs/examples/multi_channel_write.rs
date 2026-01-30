use infiniband_rs::channel::multi_channel::MultiChannel;
use infiniband_rs::channel::multi_channel::work_request::PeerWriteWorkRequest;
use infiniband_rs::ibverbs::devices::open_device;
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::time::Duration;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = open_device(DEVICE).unwrap();

    let multi_channel = MultiChannel::builder()
        .context(&ctx)
        .num_channels(5)
        .build()
        .unwrap();

    let endpoints = multi_channel.endpoints();
    let mut multi_channel = multi_channel.handshake(endpoints).unwrap();

    let mut mem = [0u8; 10];
    let mr = unsafe { multi_channel.register_shared_mr(&mut mem).unwrap() };

    multi_channel.share_mr(0, &mr).unwrap();
    let mut rmr = multi_channel
        .accept_remote_mr(0, Duration::from_millis(1000))
        .unwrap();

    let (send_mem, recv_mem) = mem.split_at_mut(5);

    send_mem.copy_from_slice(&[1u8, 2u8, 3u8, 4u8, 5u8]);

    println!("Recv mem before: {recv_mem:?}");

    let result = multi_channel.write(PeerWriteWorkRequest::new(
        &[mr.prepare_gather_element(&send_mem).unwrap()],
        rmr.sub_region(5).unwrap(),
    ));

    println!("Recv mem after: {recv_mem:?}");

    println!("{result:?}");
}
