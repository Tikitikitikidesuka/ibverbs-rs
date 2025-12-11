use ibverbs_sys::ibv_access_flags;
use infiniband_rs::devices::ibv_device_list;
use std::ptr::slice_from_raw_parts_mut;

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

    let mut memory = vec![0u8; 1024];
    let mr1 = unsafe {
        pd.register_mr_with_permissions(
            slice_from_raw_parts_mut(memory.as_mut_ptr(), memory.len()),
            ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
        )
    }
    .unwrap();
    let mr2 = unsafe {
        pd.register_mr_with_permissions(
            slice_from_raw_parts_mut(memory.as_mut_ptr(), memory.len()),
            ibv_access_flags::IBV_ACCESS_LOCAL_WRITE,
        )
    }
    .unwrap();

    println!("{mr1:?}");
    println!("{mr2:?}");
}
