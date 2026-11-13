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
pub(crate) use spec::{installed, pick_spec, KernelSpec};

/// Reads a message a test captured from a scripted handle.
#[cfg(test)]
pub(crate) fn decode_for_tests(key: &[u8], frames: &[Vec<u8>]) -> protocol::Message {
    protocol::decode(key, frames).expect("a well-formed, correctly signed message")
}
