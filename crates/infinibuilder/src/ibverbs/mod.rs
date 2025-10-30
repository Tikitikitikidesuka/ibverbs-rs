use derivative::Derivative;

pub mod connection;
pub mod work_request;
pub mod work_completion;
pub mod work_error;
pub mod network_node;
pub mod init;

mod completion_queue;

#[derive(Debug)]
pub(super) struct Named<T> {
    pub name: String,
    pub data: T,
}

impl<T> Named<T> {
    pub fn new(name: impl Into<String>, data: T) -> Self {
        Self { name: name.into(), data }
    }
}
