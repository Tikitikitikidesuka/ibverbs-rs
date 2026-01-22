use std::io;
use crate::single_channel::SingleChannel;
use delegate::delegate;
use crate::channel::pending_work::{PendingWork, WorkSpinPollResult};
use crate::channel::scoped::{ChannelScope, ChannelScopeError};
use crate::ibverbs::scatter_gather_element::{GatherElement, ScatterElement};

impl SingleChannel {
    delegate! {
        to self.channel {
            pub fn scope<'env, F, R>(&'env mut self, f: F) -> Result<R, ChannelScopeError>
            where
                F: for<'scope> FnOnce(&mut ChannelScope<'scope, 'env>) -> R;

            pub fn send<'a>(&mut self, sends: impl AsRef<[ScatterElement<'a>]>) -> WorkSpinPollResult;
            pub fn send_with_immediate<'a>(
                &mut self,
                sends: impl AsRef<[ScatterElement<'a>]>,
                imm_data: u32,
            ) -> WorkSpinPollResult;
            pub fn receive<'a>(&mut self, receives: impl AsMut<[GatherElement<'a>]>) -> WorkSpinPollResult;

            pub unsafe fn send_unpolled<'a>(
                &mut self,
                sends: impl AsRef<[ScatterElement<'a>]>,
            ) -> io::Result<PendingWork<'a>>;
            pub unsafe fn send_with_immediate_unpolled<'a>(
                &mut self,
                sends: impl AsRef<[ScatterElement<'a>]>,
                imm_data: u32,
            ) -> io::Result<PendingWork<'a>>;
            pub unsafe fn receive_unpolled<'a>(
                &mut self,
                mut receives: impl AsMut<[GatherElement<'a>]>,
            ) -> io::Result<PendingWork<'a>>;
        }
    }
}
