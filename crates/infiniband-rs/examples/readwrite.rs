use infiniband_rs::connection::builder::IbvConnectionBuilder;
use infiniband_rs::connection::connection::IbvConnection;
use infiniband_rs::devices::ibv_device_open;

const DEVICE: &str = "mlx5_0";

fn main() {
    let ctx = ibv_device_open(DEVICE).unwrap();
    let conn = IbvConnectionBuilder::new(&ctx).build().unwrap();
    let endpoint = conn.endpoint();
    let mut conn = conn.handshake(endpoint).unwrap();

    let mut mem = [0u8; 1024];
    let mr = conn.register_mr("mem", &mut mem).unwrap();

}