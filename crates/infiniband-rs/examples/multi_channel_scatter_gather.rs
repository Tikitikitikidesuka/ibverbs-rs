use infiniband_rs::channel::multi_channel::MultiChannel;
use infiniband_rs::ibverbs::devices::open_device;
use infiniband_rs::ibverbs::work_request::{ReceiveWorkRequest, SendWorkRequest};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;

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
    let mr = multi_channel.register_local_mr(&mut mem).unwrap();

    let (send_mem, recv_mem) = mem.split_at_mut(5);

    send_mem.copy_from_slice(&[1u8, 2u8, 3u8, 4u8, 5u8]);

    println!("Recv mem before: {recv_mem:?}");

    let result = multi_channel.scope(|s| {
        let scatter_sends = send_mem
            .chunks(1)
            .map(|chunk| SendWorkRequest::new(vec![mr.prepare_gather_element(chunk).unwrap()]))
            .enumerate();
        let gather_receives = recv_mem
            .chunks_mut(1)
            .map(|chunk| ReceiveWorkRequest::new(vec![mr.prepare_scatter_element(chunk).unwrap()]))
            .enumerate();

        s.post_scatter_send(scatter_sends).unwrap();
        s.post_gather_receive(gather_receives).unwrap();
    });

    println!("Recv mem after: {recv_mem:?}");

    println!("{result:?}");
}
