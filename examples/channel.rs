use ibverbs_rs::channel::polling_scope::ScopeError;
use ibverbs_rs::channel::{Channel, TransportError};
use ibverbs_rs::ibverbs;
use ibverbs_rs::ibverbs::work::{ReceiveWorkRequest, SendWorkRequest};
use log::LevelFilter::Debug;
use simple_logger::SimpleLogger;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = ibverbs::open_device(DEVICE).unwrap();
    ctx.device().bind_thread_to_numa().unwrap();
    let pd = ctx.allocate_pd().unwrap();
    let channel = Channel::builder().pd(&pd).build().unwrap();

    let mut mem = [0u8; 8];
    let mr = pd.register_local_mr_slice(&mem).unwrap();

    let endpoint = channel.endpoint();
    let mut channel = channel.handshake(endpoint).unwrap();

    mem[0..4].fill(8);

    println!("Mem before exchange: {mem:?}");

    channel
        .scope(|s| {
            let (send_mem, recv_mem) = mem.split_at_mut(4);

            let send_ge = [mr.gather_element_unchecked(send_mem)];
            let mut recv_se = [mr.scatter_element_unchecked(recv_mem)];

            s.post_receive(ReceiveWorkRequest::new(&mut recv_se))?;
            s.post_send(SendWorkRequest::new(&send_ge))?;

            Ok::<(), ScopeError<TransportError>>(())
        })
        .unwrap();

    println!("Mem after exchange: {mem:?}");
}
