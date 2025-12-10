use infiniband_rs::devices::ibv_device_list;

const DEVICE: &str = "mlx5_0";

fn main() {
    let devices = ibv_device_list().unwrap();
    println!("{devices:?}");

    let device = devices
        .iter()
        .find(|device| device.name() == Some(DEVICE))
        .unwrap();

    println!("{device:?}");

    let ctx = device.open().unwrap();

    drop(devices);

    println!("{ctx:?}");

    let cq = ctx.create_cq(3, 0).unwrap();

    println!("{cq:?}");

    let pd = ctx.allocate_pd().unwrap();

    println!("{pd:?}");
}
