use infiniband_rs::channel::TransportError;
use infiniband_rs::channel::polling_scope::ScopeError;
use infiniband_rs::ibverbs;
use infiniband_rs::multi_channel::MultiChannel;
use infiniband_rs::multi_channel::work_request::{PeerReceiveWorkRequest, PeerSendWorkRequest};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;
use std::io;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = ibverbs::open_device(DEVICE).unwrap();
    let pd = ctx.allocate_pd().unwrap();

    let multi_channel = MultiChannel::builder()
        .pd(&pd)
        .num_channels(3)
        .build()
        .unwrap();

    let endpoints = multi_channel.endpoints();
    let mut multi_channel = multi_channel.handshake(endpoints).unwrap();

    let mut mem = [0u8; 8];
    let mr = pd.register_local_mr_slice(&mem).unwrap();
    let (send_mem, recv_mem) = mem.split_at_mut(4);

    println!("Recv mem before: {recv_mem:?}");

    let result = multi_channel.scope(|s| {
        send_mem.copy_from_slice(&[1u8; 4]);

        let send_sge = [mr.gather_element_unchecked(&send_mem[0..4])];
        let mut recv_sge = [mr.scatter_element_unchecked(&mut recv_mem[0..4])];

        s.post_receive(PeerReceiveWorkRequest::new(1, &mut recv_sge))?;

        s.post_send(PeerSendWorkRequest::new(1, &send_sge))?;

        Ok::<(), ScopeError<TransportError>>(())
    });

    println!("Recv mem after: {recv_mem:?}");
    println!("{result:?}");
}
