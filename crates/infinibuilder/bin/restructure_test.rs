use std::time::Duration;
use infinibuilder::restructure::ibverbs::connection::IbvConnectionBuilder;
use infinibuilder::restructure::rdma_connection::{RdmaConnection, RdmaWorkRequest, RdmaWorkRequestStatus};
use infinibuilder::restructure::spin_poll::spin_poll_batched;

fn main() {
    let id: u32 = std::env::args()
        .nth(1)
        .expect("Usage: <program> <id>")
        .parse()
        .expect("Invalid ID");

    let mut mem = vec![0u8; 1024];

    let conn_builder = IbvConnectionBuilder::new().with_ibv_device("mlx5_0").unwrap()
        .create_pd().unwrap()
        .create_cq(32, 512).unwrap()
        .create_qp().unwrap();

    let mr = conn_builder.register_mr(mem.as_mut_ptr(), mem.len()).unwrap();

    let local_endpoint = conn_builder.endpoint();

    let serialized = serde_json::to_string(&local_endpoint).unwrap();
    println!("Local endpoint:\n{serialized}\n");

    println!("Paste the remote endpoint JSON:");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    let remote_endpoint = serde_json::from_str(&input.trim()).unwrap();

    let mut conn = conn_builder.connect(remote_endpoint).unwrap();


    let mut wr =
        if id == 0 {
            unsafe { conn.post_send(mr, 0..4, Some(33)).unwrap() }
        } else {
            unsafe { conn.post_receive(mr, 4..8).unwrap() }
        };

    let wc_result = spin_poll_batched(|| match wr.poll() {
        RdmaWorkRequestStatus::Pending => None,
        RdmaWorkRequestStatus::Success(wc) => Some(Ok(wc)),
        RdmaWorkRequestStatus::Error(error) => Some(Err(error)),
    }, Duration::from_millis(5000), 1024).unwrap();

    match wc_result {
        Ok(wc) => println!("Success: {wc:?}"),
        Err(error) => println!("Error: {error:?}"),
    };
}
