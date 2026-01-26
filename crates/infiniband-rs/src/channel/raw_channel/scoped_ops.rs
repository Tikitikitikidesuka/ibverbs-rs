use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::MultiWorkPollError;
use crate::channel::raw_channel::polling_scope::{PollingScope, ScopedPendingWork};
use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};
use std::io;

impl RawChannel {
    pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, MultiWorkPollError>
    where
        F: for<'scope> FnOnce(&mut PollingScope<'scope, 'env, RawChannel>) -> R,
    {
        PollingScope::run(self, f)
    }
}

impl<'scope, 'env> PollingScope<'scope, 'env, RawChannel> {
    pub fn post_send(
        &mut self,
        sends: impl AsRef<[GatherElement<'env>]>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send(|s| Ok(s), sends)
    }

    pub fn post_send_with_immediate(
        &mut self,
        sends: impl AsRef<[GatherElement<'env>]>,
        imm_data: u32,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send_with_immediate(|s| Ok(s), sends, imm_data)
    }

    pub fn post_send_immediate(&mut self, imm_data: u32) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_send_immediate(|s| Ok(s), imm_data)
    }

    pub fn post_receive(
        &mut self,
        receives: impl AsMut<[ScatterElement<'env>]>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_receive(|s| Ok(s), receives)
    }

    pub fn post_receive_immediate(&mut self) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_receive_immediate(|s| Ok(s))
    }

    pub fn post_write(
        &mut self,
        gather_elements: impl AsRef<[GatherElement<'env>]>,
        remote_slice: &mut RemoteMemorySliceMut<'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_write(|s| Ok(s), gather_elements, remote_slice)
    }

    pub fn post_write_with_immediate(
        &mut self,
        gather_elements: impl AsRef<[GatherElement<'env>]>,
        remote_slice: &mut RemoteMemorySliceMut<'env>,
        imm_data: u32,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_write_with_immediate(|s| Ok(s), gather_elements, remote_slice, imm_data)
    }

    pub fn post_read(
        &mut self,
        scatter_elements: impl AsMut<[ScatterElement<'env>]>,
        remote_slice: &RemoteMemorySlice<'env>,
    ) -> io::Result<ScopedPendingWork<'scope>> {
        self.channel_post_read(|s| Ok(s), scatter_elements, remote_slice)
    }
}
