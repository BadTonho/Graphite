//! Configurable WebSocket tunnel & relay protocol client (RFC 6455 over std `TcpStream`).

use ring::digest;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RelayEnvelope {
    Hello {
        client_id: String,
        session_id: String,
    },
    EditSnapshot {
        session_id: String,
        payload_json: String,
    },
    PortOpen {
        local_port: u16,
        remote_port: u16,
    },
    PortClose {
        local_port: u16,
    },
    Ping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedWsUrl {
    pub host_port: String,
    pub host_header: String,
    pub path_and_query: String,
    pub tls: bool,
}

/// Parses `ws://host[:port]/path` or `wss://host[:port]/path` tunnel endpoints.
pub(crate) fn parse_ws_url(url: &str) -> Result<ParsedWsUrl, String> {
    let trimmed = url.trim();
    let (tls, rest) = if let Some(r) = trimmed.strip_prefix("ws://") {
        (false, r)
    } else if let Some(r) = trimmed.strip_prefix("wss://") {
        (true, r)
    } else {
        return Err("Tunnel URL must start with ws:// or wss://".into());
    };
    let (authority, path_raw) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err("Tunnel URL missing host".into());
    }
    let default_port = if tls { 443 } else { 80 };
    let host_port = if authority.contains(':') {
        authority.to_owned()
    } else {
        format!("{authority}:{default_port}")
    };
    Ok(ParsedWsUrl {
        host_port,
        host_header: authority.to_owned(),
        path_and_query: path_raw.to_owned(),
        tls,
    })
}

pub(crate) fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

pub(crate) fn compute_ws_accept_key(client_key: &str) -> String {
    let mut ctx = digest::Context::new(&digest::SHA1_FOR_LEGACY_USE_ONLY);
    ctx.update(client_key.as_bytes());
    ctx.update(WS_GUID.as_bytes());
    let hash = ctx.finish();
    base64_encode(hash.as_ref())
}

/// Encodes one RFC 6455 WebSocket frame (`masked` must be `true` for client -> server).
pub(crate) fn encode_ws_frame(opcode: u8, payload: &[u8], masked: bool) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | (opcode & 0x0f));
    let mask_bit = if masked { 0x80u8 } else { 0x00u8 };
    let len = payload.len();
    if len < 126 {
        frame.push(mask_bit | (len as u8));
    } else if len <= u16::MAX as usize {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    if masked {
        let mask = [0x37u8, 0xfa, 0x21, 0x3d];
        frame.extend_from_slice(&mask);
        for (i, &b) in payload.iter().enumerate() {
            frame.push(b ^ mask[i % 4]);
        }
    } else {
        frame.extend_from_slice(payload);
    }
    frame
}

#[cfg(test)]
pub(crate) fn read_ws_frame(reader: &mut impl Read) -> Result<(u8, Vec<u8>), String> {
    let mut header = [0u8; 2];
    reader
        .read_exact(&mut header)
        .map_err(|e| format!("WebSocket header read error: {e}"))?;
    let opcode = header[0] & 0x0f;
    let masked = (header[1] & 0x80) != 0;
    let len_code = header[1] & 0x7f;
    let payload_len: usize = match len_code {
        126 => {
            let mut ext = [0u8; 2];
            reader
                .read_exact(&mut ext)
                .map_err(|e| format!("WebSocket len16 read error: {e}"))?;
            u16::from_be_bytes(ext) as usize
        }
        127 => {
            let mut ext = [0u8; 8];
            reader
                .read_exact(&mut ext)
                .map_err(|e| format!("WebSocket len64 read error: {e}"))?;
            u64::from_be_bytes(ext) as usize
        }
        n => n as usize,
    };
    if payload_len > 16 * 1024 * 1024 {
        return Err("WebSocket frame exceeds 16 MiB limit".into());
    }
    let mut mask = [0u8; 4];
    if masked {
        reader
            .read_exact(&mut mask)
            .map_err(|e| format!("WebSocket mask read error: {e}"))?;
    }
    let mut payload = vec![0u8; payload_len];
    reader
        .read_exact(&mut payload)
        .map_err(|e| format!("WebSocket payload read error: {e}"))?;
    if masked {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
    }
    Ok((opcode, payload))
}

/// Active TCP WebSocket connection to a configurable relay server (`ws://`).
pub(crate) struct WebSocketTunnelClient {
    stream: TcpStream,
}

impl WebSocketTunnelClient {
    pub fn connect(url: &str) -> Result<Self, String> {
        let parsed = parse_ws_url(url)?;
        if parsed.tls {
            return Err("wss:// relay tunnels require an external TLS proxy; use ws:// for local/SSH-forwarded relays".into());
        }
        let mut stream = TcpStream::connect(&parsed.host_port)
            .map_err(|e| format!("Tunnel TCP connect failed: {e}"))?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

        let client_key = base64_encode(b"graphite-ws-key1");
        let expected_accept = compute_ws_accept_key(&client_key);
        let req = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
            parsed.path_and_query, parsed.host_header, client_key
        );
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("WebSocket handshake write error: {e}"))?;

        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        while !buf.ends_with(b"\r\n\r\n") {
            stream
                .read_exact(&mut byte)
                .map_err(|e| format!("WebSocket handshake read error: {e}"))?;
            buf.push(byte[0]);
            if buf.len() > 8192 {
                return Err("WebSocket handshake response too large".into());
            }
        }
        let header_str = String::from_utf8_lossy(&buf);
        if !header_str.contains("101") || !header_str.contains(&expected_accept) {
            return Err("Invalid WebSocket upgrade response from relay".into());
        }
        Ok(Self { stream })
    }

    pub fn send_envelope(&mut self, env: &RelayEnvelope) -> Result<(), String> {
        let json = serde_json::to_vec(env).map_err(|e| e.to_string())?;
        let frame = encode_ws_frame(0x1, &json, true);
        self.stream
            .write_all(&frame)
            .map_err(|e| format!("Tunnel send failed: {e}"))
    }

    #[cfg(test)]
    pub fn recv_envelope(&mut self) -> Result<RelayEnvelope, String> {
        let (opcode, payload) = read_ws_frame(&mut self.stream)?;
        if opcode == 0x8 {
            return Err("Tunnel closed by relay".into());
        }
        serde_json::from_slice::<RelayEnvelope>(&payload)
            .map_err(|e| format!("Invalid relay payload: {e}"))
    }

    pub fn close(mut self) {
        let close_frame = encode_ws_frame(0x8, &[], true);
        let _ = self.stream.write_all(&close_frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    #[test]
    fn websocket_tunnel_client_completes_rfc6455_handshake_and_relays_envelopes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(conn.try_clone().unwrap());
            let mut key = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some((k, v)) = line.split_once(':') {
                    if k.eq_ignore_ascii_case("sec-websocket-key") {
                        key = v.trim().to_owned();
                    }
                }
            }
            let accept = compute_ws_accept_key(&key);
            let resp = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
            );
            conn.write_all(resp.as_bytes()).unwrap();

            let (opcode, payload) = read_ws_frame(&mut reader).unwrap();
            assert_eq!(opcode, 0x1);
            let frame = encode_ws_frame(0x1, &payload, false);
            conn.write_all(&frame).unwrap();
        });

        let url = format!("ws://{addr}/relay");
        let mut client = WebSocketTunnelClient::connect(&url).unwrap();
        let hello = RelayEnvelope::Hello {
            client_id: "c1".into(),
            session_id: "s1".into(),
        };
        client.send_envelope(&hello).unwrap();
        let echoed = client.recv_envelope().unwrap();
        assert_eq!(echoed, hello);
        client.close();
        server.join().unwrap();
    }
}
