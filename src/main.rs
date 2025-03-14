use pcie40_rs::mfp_reader::PCIe40MFPReader;

fn main() {
    let reader = PCIe40MFPReader::open_by_device_name("tdtel203_0", 1).unwrap();
}