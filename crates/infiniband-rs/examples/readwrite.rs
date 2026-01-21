use infiniband_rs::connection::builder::ConnectionBuilder;
use infiniband_rs::connection::connection::Connection;
use infiniband_rs::devices::open_device;

const DEVICE: &str = "mlx5_0";

fn main() {
    let ctx = open_device(DEVICE).unwrap();
    let conn = ConnectionBuilder::new(&ctx).build().unwrap();
    let endpoint = conn.endpoint();
    let mut conn = conn.handshake(endpoint).unwrap();

    let mut mem = [0u8; 1024];
    let mr = conn.register_mr(&mut mem).unwrap();
}
