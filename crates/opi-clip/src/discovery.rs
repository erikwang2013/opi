// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! UDP 组播/广播发现。Beacon 固定 32 字节，只含弱身份与连接参数，不含剪贴板内容。
//! 发现套接字开启地址/端口复用，同机多实例（含 loopback 测试）可共用默认端口。

use crate::ClipError;
use crate::crypto::fingerprint;
use crate::types::{DISCOVERY_GROUP, Fingerprint, InstanceId, PROTOCOL_VERSION};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

pub(crate) const BEACON_LEN: usize = 32;
const MAGIC: [u8; 4] = *b"OPIC";
const READ_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Beacon {
    pub instance_id: InstanceId,
    pub fingerprint: Fingerprint,
    pub tcp_port: u16,
    pub capabilities: u8,
}

impl Beacon {
    pub(crate) fn encode(self) -> [u8; BEACON_LEN] {
        let mut out = [0u8; BEACON_LEN];
        out[..4].copy_from_slice(&MAGIC);
        out[4] = PROTOCOL_VERSION;
        out[5] = self.capabilities;
        out[6..22].copy_from_slice(self.instance_id.as_bytes());
        out[22..30].copy_from_slice(self.fingerprint.as_bytes());
        out[30..32].copy_from_slice(&self.tcp_port.to_be_bytes());
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != BEACON_LEN || bytes[..4] != MAGIC {
            return None;
        }
        if bytes[4] != PROTOCOL_VERSION {
            return None;
        }
        let tcp_port = u16::from_be_bytes([bytes[30], bytes[31]]);
        if tcp_port == 0 {
            return None;
        }
        let mut instance = [0u8; 16];
        let mut fp = [0u8; 8];
        instance.copy_from_slice(&bytes[6..22]);
        fp.copy_from_slice(&bytes[22..30]);
        Some(Self {
            instance_id: InstanceId::from_bytes(instance),
            fingerprint: Fingerprint::from_bytes(fp),
            tcp_port,
            capabilities: bytes[5],
        })
    }

    pub(crate) fn peer_addr(source: SocketAddr, tcp_port: u16) -> SocketAddr {
        SocketAddr::new(source.ip(), tcp_port)
    }
}

/// 绑定发现端口并加入组播组；失败时启动直接报错（没有发现就无法配对/连接）。
/// 允许同机多实例共享端口（beacon 不含任何敏感内容）。
pub(crate) fn bind(port: u16) -> Result<UdpSocket, ClipError> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    // 组播场景下 SO_REUSEADDR 即可让同机多个实例绑定同一端口；SO_REUSEPORT
    // 在部分平台不可用，组播接收不需要它。
    socket.set_reuse_address(true)?;
    socket.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)).into())?;
    socket.join_multicast_v4(&DISCOVERY_GROUP, &Ipv4Addr::UNSPECIFIED)?;
    // 同机回环让测试与桌面多实例可用；TTL=1 把发现限制在局域网。
    socket.set_multicast_loop_v4(true)?;
    socket.set_multicast_ttl_v4(1)?;
    socket.set_broadcast(true)?;
    let socket: UdpSocket = socket.into();
    socket.set_read_timeout(Some(READ_TIMEOUT))?;
    Ok(socket)
}

pub(crate) fn beacon_for(
    instance_id: InstanceId,
    public_key: &[u8],
    tcp_port: u16,
    capabilities: u8,
) -> Beacon {
    Beacon {
        instance_id,
        fingerprint: fingerprint(public_key),
        tcp_port,
        capabilities,
    }
}

pub(crate) fn send_beacon(socket: &UdpSocket, port: u16, beacon: Beacon) -> std::io::Result<()> {
    let bytes = beacon.encode();
    let group = SocketAddr::from((DISCOVERY_GROUP, port));
    let broadcast = SocketAddr::from((Ipv4Addr::BROADCAST, port));
    socket.send_to(&bytes, group)?;
    // 广播在部分平台/网络被禁用；组播成功即可，广播失败不致命。
    let _ = socket.send_to(&bytes, broadcast);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> InstanceId {
        InstanceId::from_bytes([n; 16])
    }

    #[test]
    fn beacon_roundtrip() {
        let beacon = Beacon {
            instance_id: id(1),
            fingerprint: Fingerprint::from_bytes([2; 8]),
            tcp_port: 47123,
            capabilities: 3,
        };
        assert_eq!(Beacon::decode(&beacon.encode()).unwrap(), beacon);
        assert_eq!(beacon.encode().len(), BEACON_LEN);
    }

    #[test]
    fn beacon_rejects_malformed() {
        let beacon = Beacon {
            instance_id: id(1),
            fingerprint: Fingerprint::from_bytes([2; 8]),
            tcp_port: 47123,
            capabilities: 3,
        };
        let mut bytes = beacon.encode().to_vec();
        bytes[0] = b'X';
        assert!(Beacon::decode(&bytes).is_none());
        let mut bytes = beacon.encode().to_vec();
        bytes[4] = PROTOCOL_VERSION + 1;
        assert!(Beacon::decode(&bytes).is_none());
        let mut bytes = beacon.encode().to_vec();
        bytes[30] = 0;
        bytes[31] = 0;
        assert!(Beacon::decode(&bytes).is_none());
        assert!(Beacon::decode(&bytes[..31]).is_none());
    }

    #[test]
    fn beacon_has_no_content_field() {
        // 固定长度本身就是内容不存在的结构保证：加字段只能改协议版本。
        let beacon = beacon_for(id(5), &[9u8; 32], 5000, 1);
        assert_eq!(beacon.encode().len(), BEACON_LEN);
    }
}
