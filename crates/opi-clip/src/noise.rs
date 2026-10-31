// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Noise 握手薄封装。协议模式固定；所有密码学由 snow 实现。
//! - 配对：Noise_XX，双方静态公钥在握手后取得，再由 6 位 SAS 人工确认。
//! - 会话：Noise_KK，双方公钥来自信任表。

use crate::ClipError;
use crate::frame::{read_frame_blocking, write_frame};
use crate::identity::Identity;
use crate::types::MAX_FRAME_BYTES;
use snow::params::NoiseParams;
use snow::{Builder, HandshakeState, TransportState};
use std::io::{Read, Write};

const PAIR_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
const SESSION_PARAMS: &str = "Noise_KK_25519_ChaChaPoly_SHA256";

pub(crate) struct PairHandshake {
    pub transport: TransportState,
    pub remote_static: Vec<u8>,
    pub handshake_hash: Vec<u8>,
}

pub(crate) fn pair_params() -> Result<NoiseParams, ClipError> {
    PAIR_PARAMS
        .parse()
        .map_err(|e| ClipError::invalid(format!("noise params: {e}")))
}

fn session_params() -> Result<NoiseParams, ClipError> {
    SESSION_PARAMS
        .parse()
        .map_err(|e| ClipError::invalid(format!("noise params: {e}")))
}

pub(crate) fn xx_initiator<S: Read + Write>(
    stream: &mut S,
    identity: &Identity,
    prologue: &[u8],
) -> Result<PairHandshake, ClipError> {
    let mut handshake = Builder::new(pair_params()?)
        .local_private_key(&identity.private_key)?
        .prologue(prologue)?
        .build_initiator()?;
    send_handshake(stream, &mut handshake, &[])?; // -> e
    recv_handshake(stream, &mut handshake)?; // <- e, ee, s, es
    send_handshake(stream, &mut handshake, &[])?; // -> s, se
    finish(handshake)
}

pub(crate) fn xx_responder<S: Read + Write>(
    stream: &mut S,
    identity: &Identity,
    prologue: &[u8],
) -> Result<PairHandshake, ClipError> {
    let mut handshake = Builder::new(pair_params()?)
        .local_private_key(&identity.private_key)?
        .prologue(prologue)?
        .build_responder()?;
    recv_handshake(stream, &mut handshake)?; // <- e
    send_handshake(stream, &mut handshake, &[])?; // -> e, ee, s, es
    recv_handshake(stream, &mut handshake)?; // <- s, se
    finish(handshake)
}

pub(crate) fn kk_initiator<S: Read + Write>(
    stream: &mut S,
    identity: &Identity,
    remote_static: &[u8],
    prologue: &[u8],
) -> Result<PairHandshake, ClipError> {
    let mut handshake = Builder::new(session_params()?)
        .local_private_key(&identity.private_key)?
        .remote_public_key(remote_static)?
        .prologue(prologue)?
        .build_initiator()?;
    send_handshake(stream, &mut handshake, &[])?; // -> e, es, ss
    recv_handshake(stream, &mut handshake)?; // <- e, ee, se
    finish(handshake)
}

pub(crate) fn kk_responder<S: Read + Write>(
    stream: &mut S,
    identity: &Identity,
    remote_static: &[u8],
    prologue: &[u8],
) -> Result<PairHandshake, ClipError> {
    let mut handshake = Builder::new(session_params()?)
        .local_private_key(&identity.private_key)?
        .remote_public_key(remote_static)?
        .prologue(prologue)?
        .build_responder()?;
    recv_handshake(stream, &mut handshake)?; // <- e, es, ss
    send_handshake(stream, &mut handshake, &[])?; // -> e, ee, se
    finish(handshake)
}

fn finish(handshake: HandshakeState) -> Result<PairHandshake, ClipError> {
    let handshake_hash = handshake.get_handshake_hash().to_vec();
    let remote_static = handshake
        .get_remote_static()
        .map(Vec::from)
        .ok_or_else(|| ClipError::invalid("noise peer static key missing"))?;
    if remote_static.len() != 32 {
        return Err(ClipError::invalid("noise peer static key length"));
    }
    let transport = handshake.into_transport_mode()?;
    Ok(PairHandshake {
        transport,
        remote_static,
        handshake_hash,
    })
}

fn send_handshake<S: Write>(
    stream: &mut S,
    handshake: &mut HandshakeState,
    payload: &[u8],
) -> Result<(), ClipError> {
    let mut buffer = vec![0u8; MAX_FRAME_BYTES];
    let len = handshake.write_message(payload, &mut buffer)?;
    write_frame(stream, &buffer[..len])?;
    Ok(())
}

fn recv_handshake<S: Read>(
    stream: &mut S,
    handshake: &mut HandshakeState,
) -> Result<(), ClipError> {
    let frame = read_frame_blocking(stream)?;
    let mut output = vec![0u8; MAX_FRAME_BYTES];
    let len = handshake.read_message(&frame, &mut output)?;
    if len != 0 {
        return Err(ClipError::invalid("unexpected noise handshake payload"));
    }
    Ok(())
}

/// 加密并写入一帧应用消息。
pub(crate) fn send_encrypted<S: Write>(
    stream: &mut S,
    transport: &mut TransportState,
    plaintext: &[u8],
) -> Result<(), ClipError> {
    let mut buffer = vec![0u8; plaintext.len() + 64];
    let len = transport.write_message(plaintext, &mut buffer)?;
    write_frame(stream, &buffer[..len])?;
    Ok(())
}

/// 解密一帧应用消息；帧本身由调用方分帧读取。
pub(crate) fn decrypt(transport: &mut TransportState, frame: &[u8]) -> Result<Vec<u8>, ClipError> {
    let mut output = vec![0u8; MAX_FRAME_BYTES];
    let len = transport.read_message(frame, &mut output)?;
    output.truncate(len);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PROTOCOL_VERSION;

    #[test]
    fn params_parse() {
        assert!(pair_params().is_ok());
        assert!(session_params().is_ok());
        assert_eq!(PROTOCOL_VERSION, 1);
    }
}
