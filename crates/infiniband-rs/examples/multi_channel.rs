use infiniband_rs::channel::multi_channel::MultiChannel;
use infiniband_rs::channel::multi_channel::work_request::{
    PeerReceiveWorkRequest, PeerSendWorkRequest,
};
use infiniband_rs::ibverbs::devices::open_device;
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::{io, ptr};

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = open_device(DEVICE).unwrap();

    let multi_channel = MultiChannel::builder()
        .context(&ctx)
        .num_channels(3)
        .build()
        .unwrap();

    let endpoints = multi_channel.endpoints();
    let mut multi_channel = multi_channel.handshake(endpoints).unwrap();

    let mut mem = [0u8; 8];
    let mr = multi_channel.register_local_mr(&mut mem).unwrap();
    let (send_mem, recv_mem) = mem.split_at_mut(4);

    println!("Recv mem before: {recv_mem:?}");

    let result = multi_channel.scope(|s| {
        send_mem.copy_from_slice(&[1u8; 4]);

        let send_sge = [mr.prepare_gather_element(&send_mem[0..4]).unwrap()];
        let mut recv_sge = [mr.prepare_scatter_element(&mut recv_mem[0..4]).unwrap()];

        s.post_receive(PeerReceiveWorkRequest::new(1, &mut recv_sge))?;

        s.post_send(PeerSendWorkRequest::new(1, &send_sge))?;

        Ok::<(), io::Error>(())
    });

    println!("Recv mem after: {recv_mem:?}");
    println!("{result:?}");
}
