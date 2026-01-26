use crate::channel::raw_channel::RawChannel;
use crate::channel::raw_channel::pending_work::WorkSpinPollResult;
use crate::ibverbs::remote_memory_region::{RemoteMemorySlice, RemoteMemorySliceMut};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};

impl RawChannel {
    pub fn send<'a>(&'a mut self, sends: impl AsRef<[GatherElement<'a>]>) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_send(sends)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn send_with_immediate<'a>(
        &'a mut self,
        sends: impl AsRef<[GatherElement<'a>]>,
        imm_data: u32,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_send_with_immediate(sends, imm_data)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn send_immediate(&mut self, imm_data: u32) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_send_immediate(imm_data)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn receive<'a>(
        &'a mut self,
        receives: impl AsMut<[ScatterElement<'a>]>,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_receive(receives)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn receive_immediate(&mut self) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_receive_immediate()?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn write<'a>(
        &'a mut self,
        gather_elements: impl AsRef<[GatherElement<'a>]>,
        remote_slice: &mut RemoteMemorySliceMut<'a>,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_write(gather_elements, remote_slice)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn write_with_immediate<'a>(
        &'a mut self,
        gather_elements: impl AsRef<[GatherElement<'a>]>,
        remote_slice: &mut RemoteMemorySliceMut<'a>,
        imm_data: u32,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| {
            s.post_write_with_immediate(gather_elements, remote_slice, imm_data)?
                .spin_poll()
        });
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }

    pub fn read<'a>(
        &'a mut self,
        scatter_elements: impl AsMut<[ScatterElement<'a>]>,
        remote_slice: &RemoteMemorySlice<'a>,
    ) -> WorkSpinPollResult {
        let res = self.scope(|s| s.post_read(scatter_elements, remote_slice)?.spin_poll());
        debug_assert!(
            res.is_ok(),
            "unreachable scope error (single WR, manual poll)"
        );
        res.unwrap()
    }
}
