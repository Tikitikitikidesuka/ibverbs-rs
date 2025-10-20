use infinibuilder::restructure::ibverbs::connection::IbvConnectionBuilder;
use infinibuilder::restructure::rdma_connection::{
    RdmaConnection, RdmaWorkRequest, RdmaWorkRequestStatus,
};
use infinibuilder::restructure::spin_poll::spin_poll_batched;
use std::time::Duration;

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
        .register_mr(mem_id, mem.as_mut_ptr(), mem.len())
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

    println!("Mem: {mem:?}");
    println!("Comms...");

    // Post/Receive
    /*
    let mut wr = if id == 0 {
        conn.post_send(mr, 0..4, Some(33)).unwrap()
    } else {
        conn.post_receive(mr, 4..8).unwrap()
    };

    let wc_result = spin_poll_batched(
        || match wr.poll().unwrap() {
            RdmaWorkRequestStatus::Pending => None,
            RdmaWorkRequestStatus::Success(wc) => Some(Ok(wc)),
            RdmaWorkRequestStatus::Error(error) => Some(Err(error)),
        },
        Duration::from_millis(5000),
        1024,
    )
        .unwrap();

    match wc_result {
        Ok(wc) => println!("Success: {wc}"),
        Err(error) => println!("Error: {error:?}"),
    };
    */

    // Write
    if id == 0 {
        let rmr = conn.remote_mr(mem_id).unwrap();
        let mut wr = conn.post_send_immediate_data(42).unwrap();

        let wc_result = spin_poll_batched(
            || match wr.poll().unwrap() {
                RdmaWorkRequestStatus::Pending => None,
                RdmaWorkRequestStatus::Success(wc) => Some(Ok(wc)),
                RdmaWorkRequestStatus::Error(error) => Some(Err(error)),
            },
            Duration::from_millis(5000),
            1024,
        )
        .unwrap();

        match wc_result {
            Ok(wc) => println!("Success: {wc}"),
            Err(error) => println!("Error: {error:?}"),
        };
    } else {
        let mut wr = conn.post_receive_immediate_data().unwrap();

        let wc_result = spin_poll_batched(
            || match wr.poll().unwrap() {
                RdmaWorkRequestStatus::Pending => None,
                RdmaWorkRequestStatus::Success(wc) => Some(Ok(wc)),
                RdmaWorkRequestStatus::Error(error) => Some(Err(error)),
            },
            Duration::from_millis(5000),
            1024,
        )
        .unwrap();

        match wc_result {
            Ok(wc) => println!("Success: {wc}"),
            Err(error) => println!("Error: {error:?}"),
        };
    }

    println!("Mem: {mem:?}");
}
