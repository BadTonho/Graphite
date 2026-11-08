//! Versioned simulation protocol. This is not the Code OSS wire protocol.
use crate::{CommandMetadata, DocumentEvent, ServiceError, ServiceRequest, ServiceResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
pub const VERSION: u32 = 2;
pub const MAX_FRAME: usize = 16 * 1024 * 1024;
pub const MAX_PENDING: usize = 128;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RpcRequest {
    /// Native Graphite host only: activation completed successfully.
    Activated,
    Service {
        request: ServiceRequest,
        #[serde(default)]
        parent: Option<u64>,
    },
    RegisterCommand(CommandMetadata),
    RegisterSaveParticipant(crate::SaveParticipantMetadata),
    ParticipateInSave {
        resource: u64,
        request: crate::SaveParticipantRequest,
    },
    SubscribeDocuments,
    Dispose {
        resource: u64,
    },
    Invoke {
        command: String,
        arguments: Value,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Message {
    Hello {
        version: u32,
        session: u64,
    },
    Ready {
        version: u32,
        session: u64,
    },
    Request {
        session: u64,
        id: u64,
        request: RpcRequest,
    },
    Response {
        session: u64,
        id: u64,
        result: ServiceResult<Value>,
    },
    Cancel {
        session: u64,
        id: u64,
    },
    WaitingForUser {
        session: u64,
        id: u64,
    },
    Resumed {
        session: u64,
        id: u64,
    },
    Event {
        session: u64,
        resource: u64,
        event: DocumentEvent,
    },
    Shutdown {
        session: u64,
    },
}
impl Message {
    pub fn session(&self) -> u64 {
        match self {
            Self::Hello { session, .. }
            | Self::Ready { session, .. }
            | Self::Request { session, .. }
            | Self::Response { session, .. }
            | Self::Cancel { session, .. }
            | Self::WaitingForUser { session, .. }
            | Self::Resumed { session, .. }
            | Self::Event { session, .. }
            | Self::Shutdown { session } => *session,
        }
    }
}
pub fn encode(message: &Message) -> ServiceResult<Vec<u8>> {
    let bytes = serde_json::to_vec(message).map_err(|e| ServiceError::Protocol(e.to_string()))?;
    if bytes.len() > MAX_FRAME {
        return Err(ServiceError::Protocol("Message exceeds 16 MiB".into()));
    }
    let mut frame = Vec::with_capacity(bytes.len() + 4);
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend_from_slice(&bytes);
    Ok(frame)
}
pub fn write_message(writer: &mut impl Write, message: &Message) -> ServiceResult<()> {
    writer
        .write_all(&encode(message)?)
        .and_then(|_| writer.flush())
        .map_err(|e| ServiceError::Io(e.to_string()))
}
pub fn read_message(reader: &mut impl Read) -> ServiceResult<Option<Message>> {
    let mut header = [0; 4];
    match reader.read(&mut header[..1]) {
        Ok(0) => return Ok(None),
        Ok(_) => {}
        Err(e) => return Err(ServiceError::Io(e.to_string())),
    }
    reader
        .read_exact(&mut header[1..])
        .map_err(|e| ServiceError::Protocol(e.to_string()))?;
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > MAX_FRAME {
        return Err(ServiceError::Protocol("Invalid frame length".into()));
    }
    let mut bytes = vec![0; size];
    reader
        .read_exact(&mut bytes)
        .map_err(|e| ServiceError::Protocol(e.to_string()))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| ServiceError::Protocol(e.to_string()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_reject_truncation_invalid_json_and_oversize() {
        let msg = Message::Hello {
            version: VERSION,
            session: 7,
        };
        let frame = encode(&msg).unwrap();
        assert!(matches!(
            read_message(&mut frame.as_slice()).unwrap(),
            Some(Message::Hello { session: 7, .. })
        ));
        for invalid in [
            vec![0, 0],
            vec![0, 0, 0, 2, b'{'],
            vec![255, 255, 255, 255],
            vec![0, 0, 0, 1, b'?'],
        ] {
            assert!(read_message(&mut invalid.as_slice()).is_err());
        }
        assert!(encode(&Message::Request {
            session: 1,
            id: 1,
            request: RpcRequest::Invoke {
                command: "x".into(),
                arguments: Value::String("x".repeat(MAX_FRAME))
            }
        })
        .is_err());
    }
}
