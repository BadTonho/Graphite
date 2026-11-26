//! A small ZeroMQ transport: ZMTP 3.0 over TCP with the NULL security mechanism, which is what
//! Jupyter kernels speak. Only what the Jupyter client needs is here: connecting to or accepting
//! a peer, the READY handshake and multipart messages.

use std::io::{self, Read, Write};
#[cfg(test)]
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;

const FLAG_MORE: u8 = 0x01;
const FLAG_LONG: u8 = 0x02;
const FLAG_COMMAND: u8 = 0x04;

/// Frames larger than this are refused instead of allocated, so a broken peer cannot exhaust
/// memory. Notebook outputs with images stay well below it.
const MAX_FRAME: u64 = 256 * 1024 * 1024;

/// The two ends of one connection. Shared between threads by reference, never by duplicating
/// the handle: on Windows a duplicated socket is inheritable, so a process started later (another
/// kernel, a terminal) would keep the connection open after Graphite closes it.
pub(crate) struct Socket {
    stream: Arc<TcpStream>,
}

fn greeting() -> [u8; 64] {
    let mut bytes = [0u8; 64];
    bytes[0] = 0xff;
    bytes[9] = 0x7f;
    // Version 3.0.
    bytes[10] = 3;
    bytes[11] = 0;
    bytes[12..16].copy_from_slice(b"NULL");
    bytes
}

fn ready_command(socket_type: &str, identity: Option<&[u8]>) -> Vec<u8> {
    let mut body = Vec::new();
    body.push(5);
    body.extend_from_slice(b"READY");
    let mut property = |name: &str, value: &[u8]| {
        body.push(name.len() as u8);
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(&(value.len() as u32).to_be_bytes());
        body.extend_from_slice(value);
    };
    property("Socket-Type", socket_type.as_bytes());
    if let Some(identity) = identity {
        property("Identity", identity);
    }
    let mut frame = vec![FLAG_COMMAND, body.len() as u8];
    frame.extend_from_slice(&body);
    frame
}

fn protocol_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

impl Socket {
    fn handshake(
        mut stream: TcpStream,
        socket_type: &str,
        identity: Option<&[u8]>,
    ) -> io::Result<Self> {
        stream.set_nodelay(true).ok();
        stream.write_all(&greeting())?;
        stream.write_all(&ready_command(socket_type, identity))?;
        let mut peer = [0u8; 64];
        stream.read_exact(&mut peer)?;
        if peer[0] != 0xff || peer[9] != 0x7f || peer[10] < 3 {
            return Err(protocol_error("the peer does not speak ZMTP 3"));
        }
        let mut socket = Self {
            stream: Arc::new(stream),
        };
        // The peer's READY: a command frame we only need to see go by.
        let (flags, _) = socket.read_frame()?;
        if flags & FLAG_COMMAND == 0 {
            return Err(protocol_error("the peer did not send READY"));
        }
        Ok(socket)
    }

    /// Connects to a listening peer as a socket of `socket_type` (`DEALER`, `SUB`, ...).
    pub(crate) fn connect(
        address: &str,
        socket_type: &str,
        identity: Option<&[u8]>,
    ) -> io::Result<Self> {
        let stream = TcpStream::connect(address)?;
        Self::handshake(stream, socket_type, identity)
    }

    #[cfg(test)]
    /// Accepts one peer on `listener`, as a socket of `socket_type` (`ROUTER`, `PUB`, ...).
    pub(crate) fn accept(listener: &TcpListener, socket_type: &str) -> io::Result<Self> {
        let (stream, _) = listener.accept()?;
        Self::handshake(stream, socket_type, None)
    }

    /// Another owner of the same connection, for reading on its own thread.
    pub(crate) fn share(&self) -> Self {
        Self {
            stream: self.stream.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    pub(crate) fn shutdown(&self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }

    fn read_frame(&mut self) -> io::Result<(u8, Vec<u8>)> {
        let mut flags = [0u8; 1];
        (&*self.stream).read_exact(&mut flags)?;
        let flags = flags[0];
        let size = if flags & FLAG_LONG != 0 {
            let mut bytes = [0u8; 8];
            (&*self.stream).read_exact(&mut bytes)?;
            u64::from_be_bytes(bytes)
        } else {
            let mut byte = [0u8; 1];
            (&*self.stream).read_exact(&mut byte)?;
            byte[0] as u64
        };
        if size > MAX_FRAME {
            return Err(protocol_error("a frame is too large"));
        }
        let mut body = vec![0u8; size as usize];
        (&*self.stream).read_exact(&mut body)?;
        Ok((flags, body))
    }

    /// Receives one multipart message, skipping protocol commands.
    pub(crate) fn recv(&mut self) -> io::Result<Vec<Vec<u8>>> {
        let mut frames = Vec::new();
        loop {
            let (flags, body) = self.read_frame()?;
            if flags & FLAG_COMMAND != 0 {
                continue;
            }
            frames.push(body);
            if flags & FLAG_MORE == 0 {
                return Ok(frames);
            }
        }
    }

    /// Sends one multipart message.
    pub(crate) fn send(&mut self, frames: &[Vec<u8>]) -> io::Result<()> {
        let mut bytes = Vec::new();
        for (index, frame) in frames.iter().enumerate() {
            let mut flags = if index + 1 < frames.len() {
                FLAG_MORE
            } else {
                0
            };
            if frame.len() > 255 {
                flags |= FLAG_LONG;
                bytes.push(flags);
                bytes.extend_from_slice(&(frame.len() as u64).to_be_bytes());
            } else {
                bytes.push(flags);
                bytes.push(frame.len() as u8);
            }
            bytes.extend_from_slice(frame);
        }
        (&*self.stream).write_all(&bytes)
    }

    /// Subscribes a `SUB` socket to every message.
    pub(crate) fn subscribe_all(&mut self) -> io::Result<()> {
        self.send(&[vec![1u8]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn pair(client_type: &'static str, server_type: &'static str) -> (Socket, Socket) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let server = thread::spawn(move || Socket::accept(&listener, server_type).unwrap());
        let client = Socket::connect(&address, client_type, Some(b"me")).unwrap();
        (client, server.join().unwrap())
    }

    #[test]
    fn multipart_messages_cross_the_connection_both_ways() {
        let (mut client, mut server) = pair("DEALER", "ROUTER");
        client
            .send(&[b"one".to_vec(), Vec::new(), b"three".to_vec()])
            .unwrap();
        assert_eq!(
            server.recv().unwrap(),
            vec![b"one".to_vec(), Vec::new(), b"three".to_vec()]
        );
        server.send(&[b"reply".to_vec()]).unwrap();
        assert_eq!(client.recv().unwrap(), vec![b"reply".to_vec()]);
    }

    #[test]
    fn long_frames_use_the_eight_byte_length() {
        let (mut client, mut server) = pair("DEALER", "ROUTER");
        let big = vec![7u8; 70_000];
        client
            .send(&[big.clone(), vec![1; 255], vec![2; 256]])
            .unwrap();
        let received = server.recv().unwrap();
        assert_eq!(received[0], big);
        assert_eq!(received[1].len(), 255);
        assert_eq!(received[2].len(), 256);
    }

    #[test]
    fn a_subscriber_announces_itself_and_receives_published_messages() {
        let (mut subscriber, mut publisher) = pair("SUB", "PUB");
        subscriber.subscribe_all().unwrap();
        // The publisher sees the subscription as a message starting with 1.
        assert_eq!(publisher.recv().unwrap(), vec![vec![1u8]]);
        publisher
            .send(&[b"topic".to_vec(), b"body".to_vec()])
            .unwrap();
        assert_eq!(
            subscriber.recv().unwrap(),
            vec![b"topic".to_vec(), b"body".to_vec()]
        );
    }

    #[test]
    fn a_peer_that_is_not_zmtp_is_rejected() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let fake = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.write_all(&[b'H'; 64]).unwrap();
            let mut sink = [0u8; 128];
            let _ = stream.read(&mut sink);
        });
        let error = Socket::connect(&address, "DEALER", None).err().unwrap();
        // Usually a protocol error; a peer that hangs up first gives a reset or abort instead.
        assert!(!error.to_string().is_empty());
        fake.join().unwrap();
    }

    #[test]
    fn a_closed_connection_ends_the_read_with_an_error() {
        let (mut client, server) = pair("DEALER", "ROUTER");
        server.shutdown();
        drop(server);
        assert!(client.recv().is_err());
    }

    #[test]
    fn oversized_frames_are_refused() {
        let (mut client, server) = pair("DEALER", "ROUTER");
        let mut raw: &TcpStream = &server.stream;
        let mut header = vec![FLAG_LONG];
        header.extend_from_slice(&(MAX_FRAME + 1).to_be_bytes());
        raw.write_all(&header).unwrap();
        assert_eq!(
            client.recv().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn a_read_timeout_returns_instead_of_blocking() {
        let (mut client, _server) = pair("DEALER", "ROUTER");
        client
            .set_read_timeout(Some(Duration::from_millis(30)))
            .unwrap();
        let error = client.recv().unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ));
    }
}
