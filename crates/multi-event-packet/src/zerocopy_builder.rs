use std::{num::NonZero, ops::Range};

use ebutils::SourceId;

use crate::{total_header_size, zerocopy_builder::internal::Stage};

mod internal {
    pub(super) trait Stage {
        fn get_mfp_allocation(self) -> Box<[usize]>;
        fn num_mfps(&self) -> usize;
    }
}

pub struct RegisterSizes {
    mfp_sizes: Box<[Option<NonZero<usize>>]>,
}
impl Stage for RegisterSizes {
    fn get_mfp_allocation(self) -> Box<[usize]> {
        bytemuck::allocation::cast_slice_box(self.mfp_sizes)
    }
    fn num_mfps(&self) -> usize {
        self.mfp_sizes.len()
    }
}

pub struct StoreMfps {
    mfp_offsets: Box<[usize]>,
}
impl Stage for StoreMfps {
    fn get_mfp_allocation(self) -> Box<[usize]> {
        self.mfp_offsets
    }

    fn num_mfps(&self) -> usize {
        self.mfp_offsets.len()
    }
}

#[allow(private_bounds)]
pub struct ZeroCopyMepBuilder<S: Stage> {
    buffer: Box<[u32]>,
    stage: S,
}

#[allow(private_bounds)]
impl<S: Stage> ZeroCopyMepBuilder<S> {
    pub fn get_buffer_range(&mut self) -> Range<*mut u32> {
        self.buffer.as_mut_ptr_range()
    }

    pub fn reset(self) -> ZeroCopyMepBuilder<RegisterSizes> {
        let mut mfp_sizes = bytemuck::allocation::cast_slice_box(self.stage.get_mfp_allocation());
        mfp_sizes.fill(None);

        ZeroCopyMepBuilder {
            stage: RegisterSizes { mfp_sizes },
            buffer: self.buffer,
        }
    }

    pub fn num_mfps(&self) -> usize {
        self.stage.num_mfps()
    }
}

impl ZeroCopyMepBuilder<RegisterSizes> {
    pub fn new(buffer_capacity: usize, num_mfps: usize) -> Self {
        ZeroCopyMepBuilder {
            buffer: vec![0u32; buffer_capacity.div_ceil(size_of::<u32>())].into_boxed_slice(),
            stage: RegisterSizes {
                mfp_sizes: vec![None; num_mfps].into_boxed_slice(),
            },
        }
    }

    /// `idx` needs to be in 0..num_mfps, in correct source id order
    pub fn register_mfp(&mut self, idx: usize, size_u32: NonZero<usize>) -> &mut Self {
        self.stage.mfp_sizes[idx] = Some(size_u32);
        self
    }

    pub fn start_assembling(mut self) -> ZeroCopyMepBuilder<StoreMfps> {
        let num_mfps = self.num_mfps();
        assert!(
            self.stage.mfp_sizes.iter().all(Option::is_some),
            "all mfp sizes are set"
        );

        let mut mfp_offsets = self.stage.get_mfp_allocation();

        let header_size_u32 = total_header_size(num_mfps);

        // todo write header
        // todo write src_ids
        // todo write offsets

        let mut requred_size_u32 = header_size_u32;
        for offset in &mut mfp_offsets {
            // offsets initially stores the increments after casting
            let inc = *offset;
            *offset = requred_size_u32;
            requred_size_u32 += inc;
        }

        ZeroCopyMepBuilder {
            buffer: self.buffer,
            stage: StoreMfps { mfp_offsets },
        }
    }
}

impl ZeroCopyMepBuilder<StoreMfps> {}
