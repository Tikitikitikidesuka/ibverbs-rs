use log::LevelFilter::Debug;
use infiniband_rs::connection::builder::ConnectionBuilder;
use simple_logger::SimpleLogger;
use infiniband_rs::devices::open_device;

const DEVICE: &str = "mlx5_0";

fn main() {
    SimpleLogger::new().with_level(Debug).init().unwrap();

    let ctx = open_device(DEVICE).unwrap();

    let prep_conn = ConnectionBuilder::new(&ctx).build().unwrap();
    let endpoint = prep_conn.endpoint();
    let mut conn = prep_conn.handshake(endpoint).unwrap();

    let mut mem = vec![0u8; 1024];
    let mr = conn
        .register_mr(&mut mem)
        .unwrap();

    let (send_mem, rest) = mem.split_at_mut(4);
    let (recv_mem, _rest) = rest.split_at_mut(4);
    recv_mem.copy_from_slice(&[0, 0, 0, 0]);
    send_mem.copy_from_slice(&[1, 2, 3, 4]);
    println!("Running scoped connection and polling...");
    println!("before recv: {:?}", &recv_mem[0..4]);
    let ge0 = mr.prepare_gather_element(recv_mem).unwrap();
    let ge1 = mr.prepare_gather_element(send_mem).unwrap();
    let se = mr.prepare_scatter_element(send_mem).unwrap();

    let mut wr0 = unsafe { conn.send_unpolled(&mut [se]).unwrap() };
    let mut wr1 = unsafe { conn.send_unpolled(&mut [se]).unwrap() };
    let mut wr0 = unsafe { conn.receive_unpolled(&mut [ge0]).unwrap() };
    let mut wr1 = unsafe { conn.receive_unpolled(&mut [ge0]).unwrap() };
    drop(mr);
    wr0.spin_poll().unwrap();
    wr1.spin_poll().unwrap();


    println!("after recv: {:?}", &recv_mem[0..4]);
}
