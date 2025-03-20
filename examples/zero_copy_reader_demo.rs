use pcie40_rs::zero_copy_reader::{ZeroCopyReader, ZeroCopyReaderImpl};

struct DemoZeroCopyReaderImpl {
    valid_data_start: usize, // Hardware read pointer
    valid_data_end: usize,   // Local read pointer
    buffer: Vec<u8>,         // Demo buffer
}

impl DemoZeroCopyReaderImpl {
    pub fn new(num_bytes: usize) -> Self {
        Self {
            valid_data_start: 0,
            valid_data_end: 0,
            buffer: (0..num_bytes).into_iter().map(|i| i as u8).collect(),
        }
    }

    pub fn reader(num_bytes: usize) -> ZeroCopyReader<Self> {
        ZeroCopyReader::new(Self::new(num_bytes))
    }
}

impl ZeroCopyReaderImpl for DemoZeroCopyReaderImpl {
    fn data(&self) -> &[u8] {
        &self.buffer[self.valid_data_start..self.valid_data_end]
    }

    fn load_data(&mut self, num_bytes: usize) -> usize {
        let next_data_end = if self.valid_data_end + num_bytes > self.buffer.len() {
            self.buffer.len()
        } else {
            self.valid_data_end + num_bytes
        };

        let loaded_num_bytes = next_data_end - self.valid_data_end;

        self.valid_data_end = next_data_end;

        loaded_num_bytes
    }

    fn discard_data(&mut self, num_bytes: usize) -> usize {
        let next_data_start = if self.valid_data_start + num_bytes > self.valid_data_end {
            self.valid_data_end
        } else {
            self.valid_data_start + num_bytes
        };

        let discarded_num_bytes = next_data_start - self.valid_data_start;

        self.valid_data_start = next_data_start;

        discarded_num_bytes
    }
}


fn main() {
    // Create a new demo reader
    let mut reader = DemoZeroCopyReaderImpl::reader(128);

    // Simulate data being written to the buffer (e.g., by DMA)
    reader.load_data(32);
    println!(
        "Available data after first read: {}",
        reader.data().len()
    );

    // Get a reference to the data
    let data = reader.data();
    println!("First data slice: {:?}", &data[0..4]);
    println!("First data length: {}", data.len());

    // This would fail to compile if we use the data reference afterward:
    // reader.discard_data(16);
    // reader.load_data(16);
    // println!("Using data again: {:?}", data); // This would cause compilation error

    // Now we can modify the reader
    reader.load_data(32);
    println!(
        "Available data after second read: {}",
        reader.data().len()
    );

    // Get a new data reference
    {
        let data = reader.data();
        println!("Second data slice: {:?}", &data[0..4]);
        println!("Second data length: {}", data.len());
        // data goes out of scope here
    }

    // Now we can discard data since the reference is gone
    reader.discard_data(32);
    println!("Available data after discard: {}", reader.data().len());

    // Get another data reference
    let data = reader.data();
    println!("Third data slice: {:?}", &data[0..4]);
    println!("Third data length: {}", data.len());
}
