//! App payloads. pNet carries these bytes and does not interpret them.
//!
//! Each payload starts with version 1 and a type byte. A transfer sample is
//! eight 512-byte chunks. Delay is the round trip of one ping, in milliseconds.

pub const VERSION: u8 = 1;
pub const PING: u8 = 0x01;
pub const PONG: u8 = 0x02;
pub const XFER: u8 = 0x03;
pub const XFER_ACK: u8 = 0x04;
pub const REPORT: u8 = 0x10;

pub const XFER_CHUNKS: u16 = 8;
pub const XFER_CHUNK: usize = 512;

pub const PING_WAIT: std::time::Duration = std::time::Duration::from_secs(2);
pub const XFER_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Ping {
        id: u64,
    },
    Pong {
        id: u64,
    },
    Xfer {
        id: u64,
        index: u16,
        total: u16,
    },
    XferAck {
        id: u64,
        received: u16,
    },
    Report {
        target: [u8; 16],
        online: bool,
        delay_ms: u32,
        bytes_per_sec: u32,
        at_ms: u64,
        /// `server`, `lan`, `punch`, `session`, or `unknown` when the node did not say.
        path: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtoError {
    Short,
    Version,
    Type,
}

impl std::fmt::Display for ProtoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtoError::Short => f.write_str("payload is short"),
            ProtoError::Version => f.write_str("payload version is not 1"),
            ProtoError::Type => f.write_str("payload type is unknown"),
        }
    }
}

pub fn encode(message: &Message) -> Vec<u8> {
    let mut buf = vec![VERSION];
    match message {
        Message::Ping { id } => {
            buf.push(PING);
            buf.extend_from_slice(&id.to_be_bytes());
        }
        Message::Pong { id } => {
            buf.push(PONG);
            buf.extend_from_slice(&id.to_be_bytes());
        }
        Message::Xfer { id, index, total } => {
            buf.push(XFER);
            buf.extend_from_slice(&id.to_be_bytes());
            buf.extend_from_slice(&index.to_be_bytes());
            buf.extend_from_slice(&total.to_be_bytes());
            buf.extend(std::iter::repeat_n(0xA5, XFER_CHUNK));
        }
        Message::XferAck { id, received } => {
            buf.push(XFER_ACK);
            buf.extend_from_slice(&id.to_be_bytes());
            buf.extend_from_slice(&received.to_be_bytes());
        }
        Message::Report {
            target,
            online,
            delay_ms,
            bytes_per_sec,
            at_ms,
            path,
        } => {
            buf.push(REPORT);
            buf.extend_from_slice(target);
            buf.push(u8::from(*online));
            buf.extend_from_slice(&delay_ms.to_be_bytes());
            buf.extend_from_slice(&bytes_per_sec.to_be_bytes());
            buf.extend_from_slice(&at_ms.to_be_bytes());
            if let Some(byte) = crate::node::path_byte(path) {
                buf.push(byte);
            }
        }
    }
    buf
}

pub fn decode(bytes: &[u8]) -> Result<Message, ProtoError> {
    if bytes.len() < 2 {
        return Err(ProtoError::Short);
    }
    if bytes[0] != VERSION {
        return Err(ProtoError::Version);
    }
    let rest = &bytes[2..];
    match bytes[1] {
        PING => Ok(Message::Ping {
            id: read_u64(rest)?,
        }),
        PONG => Ok(Message::Pong {
            id: read_u64(rest)?,
        }),
        XFER => {
            if rest.len() < 12 {
                return Err(ProtoError::Short);
            }
            Ok(Message::Xfer {
                id: read_u64(rest)?,
                index: u16::from_be_bytes([rest[8], rest[9]]),
                total: u16::from_be_bytes([rest[10], rest[11]]),
            })
        }
        XFER_ACK => {
            if rest.len() < 10 {
                return Err(ProtoError::Short);
            }
            Ok(Message::XferAck {
                id: read_u64(rest)?,
                received: u16::from_be_bytes([rest[8], rest[9]]),
            })
        }
        REPORT => {
            if rest.len() < 16 + 1 + 4 + 4 + 8 {
                return Err(ProtoError::Short);
            }
            let mut target = [0u8; 16];
            target.copy_from_slice(&rest[..16]);
            Ok(Message::Report {
                target,
                online: rest[16] != 0,
                delay_ms: u32::from_be_bytes(rest[17..21].try_into().unwrap()),
                bytes_per_sec: u32::from_be_bytes(rest[21..25].try_into().unwrap()),
                at_ms: u64::from_be_bytes(rest[25..33].try_into().unwrap()),
                path: crate::node::path_name(rest.get(33).copied()).to_string(),
            })
        }
        _ => Err(ProtoError::Type),
    }
}

fn read_u64(rest: &[u8]) -> Result<u64, ProtoError> {
    if rest.len() < 8 {
        return Err(ProtoError::Short);
    }
    let mut id = [0u8; 8];
    id.copy_from_slice(&rest[..8]);
    Ok(u64::from_be_bytes(id))
}

/// Round-trip delay. A clock that did not move is reported as 0.
pub fn delay_ms(elapsed: std::time::Duration) -> u32 {
    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
}

/// Bytes per second from the chunks the peer said it received.
pub fn bytes_per_sec(received_chunks: u16, elapsed: std::time::Duration) -> u32 {
    let bytes = u128::from(received_chunks) * XFER_CHUNK as u128;
    let ms = elapsed.as_millis().max(1);
    u32::try_from(bytes * 1000 / ms).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        let report = Message::Report {
            target: [0xAB; 16],
            online: true,
            delay_ms: 42,
            bytes_per_sec: 80_000,
            at_ms: 1_700_000_000_000,
            path: "lan".into(),
        };
        assert_eq!(decode(&encode(&report)).unwrap(), report);
        let xfer = Message::Xfer {
            id: 7,
            index: 3,
            total: XFER_CHUNKS,
        };
        let bytes = encode(&xfer);
        assert_eq!(bytes.len(), 2 + 8 + 2 + 2 + XFER_CHUNK);
        assert_eq!(decode(&bytes).unwrap(), xfer);
        assert_eq!(
            decode(&encode(&Message::Pong { id: 9 })).unwrap(),
            Message::Pong { id: 9 }
        );
    }

    #[test]
    fn rate_uses_received_chunks() {
        let rate = bytes_per_sec(8, std::time::Duration::from_millis(1000));
        assert_eq!(rate, 8 * 512);
        assert_eq!(delay_ms(std::time::Duration::from_millis(15)), 15);
    }
}
