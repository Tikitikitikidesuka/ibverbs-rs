use infinibuilder::barrier::dissemination::DisseminationBarrier;
use infinibuilder::ibverbs::init::create_ibv_network_node;
use infinibuilder::network_config::RawNetworkConfig;
use infinibuilder::rdma_connection::RdmaWorkRequest;
use infinibuilder::rdma_network_node::{
    RdmaBarrierNetworkNode, RdmaGroupNetworkNode, RdmaNamedMemory,
    RdmaNamedMemoryRegionNetworkNode, RdmaTransportSendReceiveNetworkNode,
};
use std::io::Write;
use std::str::FromStr;
use std::time::Duration;
use std::{env, fs};
use infinibuilder::barrier::RdmaNetworkBarrier;

fn main() {
    const TRANSPORT_MR_ID: &str = "transport";

    let args = parse_args();
    let rank_id: usize = args.rank_id;

    let json_network = fs::read_to_string(args.config_file).unwrap();
    let network_config = serde_json::from_str::<RawNetworkConfig>(&json_network)
        .unwrap()
        .take_nodes(args.num_nodes);

    let mut memory = vec![0u8; 1024];
    let transport_mrs = vec![RdmaNamedMemory::new(
        TRANSPORT_MR_ID,
        memory.as_mut_ptr(),
        memory.len(),
    )];

    let mut node = create_ibv_network_node(
        rank_id,
        32,
        512,
        network_config,
        transport_mrs, //vec![(TRANSPORT_MR_ID, memory.as_mut_ptr(), memory.len())],
        DisseminationBarrier::new(),
    )
        .unwrap();

    /*
    if rank_id != 0 {
        print!("Press Enter to enter barrier...");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        node.barrier(
            &node
                .self_group(|rank_id| vec![1, 2, 3].contains(rank_id))
                .unwrap(),
            Duration::from_millis(10000),
        )
        .unwrap();
        println!("Barrier 1 done!!!\n\n");
    }

    if rank_id != 1 {
        print!("Press Enter to enter barrier...");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        node.barrier(
            &node
                .self_group(|rank_id| vec![0, 2, 3].contains(rank_id))
                .unwrap(),
            Duration::from_millis(10000),
        )
        .unwrap();
        println!("Barrier 2 done!!!\n\n");
    }
    */

    let transport_mr = node.local_mr(TRANSPORT_MR_ID).unwrap();

    match rank_id {
        0 => {
            input_sync_all(&mut node);

            // Sending...
            memory[0..16].copy_from_slice((0..16).collect::<Vec<_>>().as_slice());
            node.post_send(1, &transport_mr, 0..16, None)
                .unwrap()
                .spin_poll_batched(Duration::from_millis(1000), 1024).unwrap();
        },
        1 => {
            println!("Memory before receive: {:?}", &memory[..16]);

            // Receiving...
            let mut wr = node.post_receive(0, &transport_mr, 0..16)
                .unwrap();

            input_sync_all(&mut node);

            wr.spin_poll_batched(Duration::from_millis(1000), 1024).unwrap();

            println!("Memory after receive: {:?}", &memory[..16]);
        },
        _ => {}
    }
}

fn input_sync_all<MR, RMR, NB, Node>(node: &mut Node)
where
    Node: RdmaBarrierNetworkNode<MR, RMR, NB> + RdmaGroupNetworkNode,
    NB: RdmaNetworkBarrier<MR, RMR>,
{
    print!("Press Enter to enter barrier...");
    std::io::stdout().flush().unwrap();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    node.barrier(&node.group_all(), Duration::from_millis(10000))
        .unwrap();
    println!("Barrier 3 done!!!\n\n");
}

struct Args {
    rank_id: usize,
    num_nodes: usize,
    config_file: String,
}

fn parse_args() -> Args {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} config_file num_nodes rank_id", args[0]);
        std::process::exit(1);
    }

    let config_file = args[1].parse().unwrap();
    let num_nodes = usize::from_str(args[2].as_str()).unwrap();
    let rank_id = usize::from_str(args[3].as_str()).unwrap();

    Args {
        rank_id,
        num_nodes,
        config_file,
    }
}
