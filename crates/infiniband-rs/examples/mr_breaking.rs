use log::LevelFilter::Debug;
use infiniband_rs::connection::builder::IbvConnectionBuilder;
use simple_logger::SimpleLogger;
use infiniband_rs::devices::ibv_device_open;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = ibv_device_open(DEVICE).unwrap();

    let prep_conn = IbvConnectionBuilder::new(&ctx).build().unwrap();
    let endpoint = prep_conn.endpoint();
    let mut conn = prep_conn.handshake(endpoint).unwrap();

    let mut mem = vec![0u8; 1024];
    let mr = conn
        .register_mr("asdf", &mut mem)
        .unwrap();

    let (send_mem, rest) = mem.split_at_mut(4);
    let (recv_mem, _rest) = rest.split_at_mut(4);
    recv_mem.copy_from_slice(&[0, 0, 0, 0]);
    send_mem.copy_from_slice(&[1, 2, 3, 4]);
    println!("Running scoped connection and polling...");
    println!("before recv: {:?}", &recv_mem[0..4]);
    let ge = mr.prepare_gather_element(recv_mem).unwrap();
    let se = mr.prepare_scatter_element(send_mem).unwrap();

    conn.scope(|s| {
        let wr0 = s
            .post_receive(&[ge])
            .unwrap();
        let wr1 = s.post_send(&[se]).unwrap();
        wr0.spin_poll().unwrap();
        wr1.spin_poll().unwrap();
    });
    drop(conn);
    println!("after recv: {:?}", &recv_mem[0..4]);
}
