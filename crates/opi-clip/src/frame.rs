// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! TCP 外层分帧：4 字节大端长度 + payload；明文 preamble 与 Noise 消息共用。
//! 会话阶段用 [`FrameReader`] 在短读超时下保留半帧状态，避免超时截断流。

use crate::ClipError;
use crate::types::{InstanceId, MAX_FRAME_BYTES, PROTOCOL_VERSION};
use std::io::{self, Read, Write};

const MAGIC: [u8; 4] = *b"OPIC";
pub(crate) const LINK_HEADER_LEN: usize = 4 + 1 + 1 + 16 + 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinkMode {
    Session = 1,
    Pairing = 2,
}

impl LinkMode {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Session),
            2 => Some(Self::Pairing),
            _ => None,
        }
    }
}

/// TCP 连接的第一帧（明文）：用途与两端实例 ID。ID 同时进入 Noise prologue，
/// 篡改会让握手哈希不一致而失败；KK 会话还会用信任表校验发起方静态公钥。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LinkHeader {
    pub mode: LinkMode,
    pub initiator: InstanceId,
    pub responder: InstanceId,
}

impl LinkHeader {
    pub(crate) fn encode(self) -> [u8; LINK_HEADER_LEN] {
        let mut out = [0u8; LINK_HEADER_LEN];
        out[..4].copy_from_slice(&MAGIC);
        out[4] = PROTOCOL_VERSION;
        out[5] = self.mode as u8;
        out[6..22].copy_from_slice(self.initiator.as_bytes());
        out[22..38].copy_from_slice(self.responder.as_bytes());
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ClipError> {
        if bytes.len() != LINK_HEADER_LEN {
            return Err(ClipError::invalid("link header length"));
        }
        if bytes[..4] != MAGIC {
            return Err(ClipError::invalid("link header magic"));
        }
        if bytes[4] != PROTOCOL_VERSION {
            return Err(ClipError::invalid("protocol version mismatch"));
        }
        let mode = LinkMode::from_u8(bytes[5]).ok_or_else(|| ClipError::invalid("link mode"))?;
        let mut a = [0u8; 16];
        let mut b = [0u8; 16];
        a.copy_from_slice(&bytes[6..22]);
        b.copy_from_slice(&bytes[22..38]);
        Ok(Self {
            mode,
            initiator: InstanceId::from_bytes(a),
            responder: InstanceId::from_bytes(b),
        })
    }
}

pub(crate) fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> io::Result<()> {
    if payload.is_empty() || payload.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame payload size out of range",
        ));
    }
    writer.write_all(&(payload.len() as u32).to_be_bytes())?;
    writer.write_all(payload)?;
    writer.flush()
}

fn frame_len(header: [u8; 4]) -> io::Result<usize> {
    let len = u32::from_be_bytes(header) as usize;
    if len == 0 || len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame length out of range",
        ));
    }
    Ok(len)
}

/// 握手阶段用：读不到完整帧就算连接失败（此时流不可恢复）。
pub(crate) fn read_frame_blocking<R: Read>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    reader.read_exact(&mut header)?;
    let len = frame_len(header)?;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

/// 会话阶段用：短读超时返回 `Ok(None)`，已读到的字节保留在内部状态里。
#[derive(Default)]
pub(crate) struct FrameReader {
    header: [u8; 4],
    header_len: usize,
    payload: Vec<u8>,
    payload_len: usize,
    payload_read: usize,
}

impl FrameReader {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn next<R: Read>(&mut self, reader: &mut R) -> io::Result<Option<Vec<u8>>> {
        loop {
            if self.payload_len > 0 {
                while self.payload_read < self.payload_len {
                    let start = self.payload_read;
                    match reader.read(&mut self.payload[start..]) {
                        Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                        Ok(n) => self.payload_read += n,
                        Err(e) if is_timeout(&e) => return Ok(None),
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) => return Err(e),
                    }
                }
                self.header_len = 0;
                self.payload_len = 0;
                self.payload_read = 0;
                return Ok(Some(std::mem::take(&mut self.payload)));
            }

            while self.header_len < 4 {
                match reader.read(&mut self.header[self.header_len..]) {
                    Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                    Ok(n) => self.header_len += n,
                    Err(e) if is_timeout(&e) => return Ok(None),
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
            let len = frame_len(self.header)?;
            self.payload = vec![0u8; len];
            self.payload_len = len;
            self.payload_read = 0;
        }
    }
}

fn is_timeout(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn id(n: u8) -> InstanceId {
        InstanceId::from_bytes([n; 16])
    }

    #[test]
    fn link_header_roundtrip() {
        let header = LinkHeader {
            mode: LinkMode::Pairing,
            initiator: id(1),
            responder: id(2),
        };
        assert_eq!(LinkHeader::decode(&header.encode()).unwrap(), header);
        let mut bad = header.encode();
        bad[0] = b'X';
        assert!(LinkHeader::decode(&bad).is_err());
        let mut bad = header.encode();
        bad[5] = 9;
        assert!(LinkHeader::decode(&bad).is_err());
    }

    #[test]
    fn frame_roundtrip_and_limits() {
        for size in [1, 2, 4096, MAX_FRAME_BYTES] {
            let mut wire = Vec::new();
            let payload = vec![0xAB; size];
            write_frame(&mut wire, &payload).unwrap();
            assert_eq!(
                read_frame_blocking(&mut Cursor::new(&wire)).unwrap(),
                payload
            );
        }
        let mut wire = Vec::new();
        assert!(write_frame(&mut wire, &[]).is_err());
        assert!(write_frame(&mut wire, &vec![0u8; MAX_FRAME_BYTES + 1]).is_err());
    }

    #[test]
    fn frame_rejects_bad_lengths_without_allocating() {
        let mut zero = Cursor::new(vec![0u8; 4]);
        assert_eq!(
            read_frame_blocking(&mut zero).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let mut too_big = Cursor::new(((MAX_FRAME_BYTES + 1) as u32).to_be_bytes().to_vec());
        assert_eq!(
            read_frame_blocking(&mut too_big).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let mut truncated = Cursor::new(vec![0u8, 0, 0, 8, 1, 2]);
        assert_eq!(
            read_frame_blocking(&mut truncated).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    /// 每次最多吐 2 字节，并周期性返回 WouldBlock，验证半帧状态能跨超时保留。
    struct Trickle {
        data: Vec<u8>,
        pos: usize,
        budget: usize,
    }

    impl Read for Trickle {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pos >= self.data.len() {
                return Ok(0);
            }
            if self.budget == 0 {
                self.budget = 5;
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let n = buf.len().min(2).min(self.data.len() - self.pos);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            self.budget -= 1;
            Ok(n)
        }
    }

    #[test]
    fn frame_reader_survives_partial_reads_and_timeouts() {
        let payload = vec![0x5A; 100];
        let mut wire = Vec::new();
        write_frame(&mut wire, &payload).unwrap();
        write_frame(&mut wire, &payload).unwrap();
        let mut reader = FrameReader::new();
        let mut source = Trickle {
            data: wire,
            pos: 0,
            budget: 3,
        };
        let mut got = Vec::new();
        let mut guard = 0;
        while got.len() < 2 {
            guard += 1;
            assert!(guard < 10_000);
            if let Some(frame) = reader.next(&mut source).unwrap() {
                got.push(frame);
            }
        }
        assert_eq!(got, vec![payload.clone(), payload]);
    }
}
