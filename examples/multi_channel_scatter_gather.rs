use ibverbs_rs::channel::TransportError;
use ibverbs_rs::channel::polling_scope::ScopeError;
use ibverbs_rs::ibverbs;
use ibverbs_rs::ibverbs::error::IbvError;
use ibverbs_rs::multi_channel::MultiChannel;
use ibverbs_rs::multi_channel::work_request::{PeerReceiveWorkRequest, PeerSendWorkRequest};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = ibverbs::open_device(DEVICE).unwrap();
    let pd = ctx.allocate_pd().unwrap();

    let multi_channel = MultiChannel::builder()
        .pd(&pd)
        .num_channels(5)
        .build()
        .unwrap();

    let endpoints = multi_channel.endpoints();
    let mut multi_channel = multi_channel.handshake(endpoints).unwrap();

    let mut mem = [0u8; 10];
    let mr = pd.register_local_mr_slice(&mem).unwrap();

    let (send_mem, recv_mem) = mem.split_at_mut(5);
    send_mem.copy_from_slice(&[1u8, 2u8, 3u8, 4u8, 5u8]);

    println!("Recv mem before: {recv_mem:?}");

    let send_sges: Vec<Vec<_>> = send_mem
        .chunks(1)
        .map(|chunk| vec![mr.gather_element(chunk)])
        .collect();

    let mut recv_sges: Vec<Vec<_>> = recv_mem
        .chunks_mut(1)
        .map(|chunk| vec![mr.scatter_element(chunk)])
        .collect();

    let result = multi_channel.scope(|s| {
        let scatter_sends = send_sges
            .iter()
            .enumerate()
            .map(|(peer, sges)| PeerSendWorkRequest::new(peer, sges));

        let gather_receives = recv_sges
            .iter_mut()
            .enumerate()
            .map(|(peer, sges)| PeerReceiveWorkRequest::new(peer, sges));

        // 3. Post
        s.post_scatter_send(scatter_sends)?;
        s.post_gather_receive(gather_receives)?;

        Ok::<(), ScopeError<TransportError>>(())
    });

    println!("Recv mem after: {recv_mem:?}");
    println!("{result:?}");
}
