use infinibuilder::ibverbs::connection::IbvConnectionBuilder;
use infinibuilder::rdma_connection::{RdmaConnection, RdmaNamedRemoteMemoryRegionConnection, RdmaPostReceiveImmediateDataConnection, RdmaPostSendImmediateDataConnection, RdmaWorkCompletion, RdmaWorkRequest, RdmaWorkRequestStatus};
use infinibuilder::spin_poll::spin_poll_timeout_batched;
use std::time::Duration;
use infinibuilder::rdma_network_node::RdmaNamedMemory;

fn main() {
    let id: u32 = std::env::args()
        .nth(1)
        .expect("Usage: <program> <id>")
        .parse()
        .expect("Invalid ID");

    let mem_id = "keo33";
    let mut mem = if id == 0 { vec![0u8; 8] } else { vec![1u8; 8] };

    let mut conn_prep = IbvConnectionBuilder::new()
        .ibv_device("mlx5_0")
        .cq_params(32, 512)
        .lock_clone()
        .register_mr(RdmaNamedMemory::new(mem_id, mem.as_mut_ptr(), mem.len()))
        .build()
        .unwrap();

    let local_endpoint = conn_prep.endpoint();

    let serialized = serde_json::to_string(&local_endpoint).unwrap();
    println!("Local endpoint:\n{serialized}\n");

    println!("Paste the remote endpoint JSON:");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    let remote_endpoint = serde_json::from_str(&input.trim()).unwrap();

    let mut conn = conn_prep.connect(remote_endpoint).unwrap();

    println!("Comms...");

    // Write
    if id == 0 {
        let mut wr0 = conn.post_send_immediate_data(42).unwrap();
        let mut wr1 = conn.post_send_immediate_data(42).unwrap();

        wr0.spin_poll_batched(Duration::from_millis(5000), 1024).unwrap();
        wr1.spin_poll_batched(Duration::from_millis(5000), 1024).unwrap();
    } else {
        let mut wr0 = conn.post_receive_immediate_data().unwrap();
        let mut wr1 = conn.post_receive_immediate_data().unwrap();

        wr0.spin_poll_batched(Duration::from_millis(5000), 1024).unwrap();
        wr1.spin_poll_batched(Duration::from_millis(5000), 1024).unwrap();
    }
}
