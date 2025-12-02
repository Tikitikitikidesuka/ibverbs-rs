use ebutils::SourceId;

use crate::zerocopy_builder::internal::Stage;

mod internal {
    pub(super) trait Stage {}
}

pub struct RegisterSizes;
impl Stage for RegisterSizes {}

pub struct StoreMfps<'a> {
    header: &'a mut [u32],
    mfps: Vec<&'a mut [u32]>,
}
impl<'a> Stage for StoreMfps<'a> {}

#[allow(private_bounds)]
pub struct ZeroCopyMepBuilder<S: Stage> {
    mfp_sizes: Vec<(SourceId, usize)>,
    stage: S,
}

#[allow(private_bounds)]
impl<S: Stage> ZeroCopyMepBuilder<S> {
    pub fn reset(mut self) -> ZeroCopyMepBuilder<RegisterSizes> {
        self.mfp_sizes.clear();

        ZeroCopyMepBuilder {
            mfp_sizes: self.mfp_sizes,
            stage: RegisterSizes,
        }
    }
}

impl ZeroCopyMepBuilder<RegisterSizes> {
    pub fn new() -> Self {
        ZeroCopyMepBuilder {
            mfp_sizes: Vec::new(),
            stage: RegisterSizes,
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        ZeroCopyMepBuilder {
            mfp_sizes: Vec::with_capacity(capacity),
            stage: RegisterSizes,
        }
    }

    pub fn register_mfp(&mut self, soruce_id: SourceId, size: usize) -> &mut Self {
        self.mfp_sizes.push((soruce_id, size));
        self
    }

    pub fn construct_in<'a>(
        mut self,
        location: &mut &'a mut [u32],
    ) -> ZeroCopyMepBuilder<StoreMfps<'a>> {
        self.mfp_sizes.sort_by_key(|t| t.0);

        // todo
        let required_size_u32 = todo!();
        let mut allocaed = location
            .split_off_mut(..required_size_u32)
            .expect("enough space left");

        let header_size = todo!();
        let header = allocaed.split_off_mut(..header_size).expect("enough space");
        let mut mfps = Vec::with_capacity(self.mfp_sizes.len());
        for mfp_size in &self.mfp_sizes {
            mfps.push(allocaed.split_off_mut(..mfp_size.1).expect("enough space"));
        }

        ZeroCopyMepBuilder {
            mfp_sizes: self.mfp_sizes,
            stage: StoreMfps { header, mfps },
        }
    }
}

impl<'a> ZeroCopyMepBuilder<StoreMfps<'a>> {}

impl Default for ZeroCopyMepBuilder<RegisterSizes> {
    fn default() -> Self {
        Self::new()
    }
}
