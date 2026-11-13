//! The connection to a Jupyter kernel: finding kernels, starting one, and the message protocol
//! over ZeroMQ sockets.

mod connection;
mod handle;
mod interrupt;
mod protocol;
mod spec;
mod zmtp;

#[allow(unused_imports)]
pub(crate) use connection::Connection;
#[allow(unused_imports)]
pub(crate) use handle::{launch, Command, KernelHandle, Notify};
#[cfg(test)]
pub(crate) use handle::attach;
#[allow(unused_imports)]
pub(crate) use protocol::{KernelEvent, KernelState};
#[allow(unused_imports)]
pub(crate) use spec::{installed, KernelSpec};
