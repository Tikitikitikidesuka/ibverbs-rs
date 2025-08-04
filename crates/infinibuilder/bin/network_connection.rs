use std::env;
use ibverbs::QueuePairEndpoint;
use infinibuilder::{IbBNetworkConfig, IbBNodeConfig};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <rank_id>", args[0]);
        std::process::exit(1);
    }

    let rank_id: u32 = args[1].parse().expect("Invalid rank ID");


    let network = IbBNetworkConfig::new()
        .add_node(IbBNodeConfig::new("tdeb01", "keo", 0, "keo"))
        .add_node(IbBNodeConfig::new("tdeb02", "keo", 1, "keo"))
        .add_node(IbBNodeConfig::new("tdeb03", "keo", 2, "keo"))
        .add_node(IbBNodeConfig::new("tdeb04", "keo", 3, "keo"));

    let connected = network.connect_infiniband(rank_id, my_qp_endpoint(rank_id));

    connected.iter().for_each(|node| {
        println!("{node:?}");
    })
}

fn my_qp_endpoint(rank_id: u32) -> QueuePairEndpoint {
    match rank_id {
        0 => serde_json::from_str("{\"num\":12384,\"lid\":0,\"gid\":null}").unwrap(),
        1 => serde_json::from_str("{\"num\":12384,\"lid\":1,\"gid\":null}").unwrap(),
        2 => serde_json::from_str("{\"num\":12384,\"lid\":2,\"gid\":null}").unwrap(),
        3 => serde_json::from_str("{\"num\":12384,\"lid\":3,\"gid\":null}").unwrap(),
        _ => serde_json::from_str("Crash!!!").unwrap(),
    }

}
