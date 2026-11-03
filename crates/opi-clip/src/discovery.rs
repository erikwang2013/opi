// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! UDP 组播/广播发现。Beacon 固定 32 字节，只含弱身份与连接参数，不含剪贴板内容。
//! 发现套接字开启地址/端口复用，同机多实例（含 loopback 测试）可共用默认端口。

use crate::ClipError;
use crate::crypto::fingerprint;
use crate::types::{
    DISCOVERY_GROUP, DISCOVERY_GROUP_LEGACY, Fingerprint, InstanceId, PROTOCOL_VERSION,
};
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
pub(crate) fn bind(port: u16, local_ip: Option<Ipv4Addr>) -> Result<UdpSocket, ClipError> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    // 组播场景下 SO_REUSEADDR 即可让同机多个实例绑定同一端口；SO_REUSEPORT
    // 在部分平台不可用，组播接收不需要它。
    socket.set_reuse_address(true)?;
    socket.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)).into())?;
    // 明确组播出口：Wi-Fi + 蜂窝/VPN 并存时，内核默认出口不一定是局域网那块网卡。
    let iface = local_ip.unwrap_or(Ipv4Addr::UNSPECIFIED);
    // 组播不是唯一通路：有的 AP/防火墙会丢组播。加入失败不能让整个内核起不来，
    // 后面的广播与 /24 探测仍然可用。两个组都加入：旧客户端还在 239.255.42.99。
    for group in [DISCOVERY_GROUP, DISCOVERY_GROUP_LEGACY] {
        if socket.join_multicast_v4(&group, &iface).is_ok() {
            continue;
        }
        if !iface.is_unspecified() {
            let _ = socket.join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED);
        }
    }
    // 失败时靠广播/探测；不要为了组播失败拒绝启动。
    // 同机回环让测试与桌面多实例可用；TTL=1 把发现限制在局域网。
    socket.set_multicast_loop_v4(true)?;
    socket.set_multicast_ttl_v4(1)?;
    if !iface.is_unspecified() {
        let _ = socket.set_multicast_if_v4(&iface);
    }
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

pub(crate) fn send_beacon(
    socket: &UdpSocket,
    port: u16,
    beacon: Beacon,
    local_ip: Option<Ipv4Addr>,
    prefix: Option<u8>,
) -> std::io::Result<()> {
    let bytes = beacon.encode();
    // 组播之外再发受限广播与**子网定向广播**：不少手机/路由器会丢掉
    // 255.255.255.255，但同一子网的广播地址能到；只发一种就会「互相发现不了」。
    let mut targets = vec![
        SocketAddr::from((DISCOVERY_GROUP, port)),
        SocketAddr::from((DISCOVERY_GROUP_LEGACY, port)),
        SocketAddr::from((Ipv4Addr::BROADCAST, port)),
    ];
    if let Some(ip) = local_ip.or_else(local_ipv4) {
        let directed = SocketAddr::from((broadcast_for(ip, prefix), port));
        if !targets.contains(&directed) {
            targets.push(directed);
        }
    }
    for target in targets {
        let _ = socket.send_to(&bytes, target);
    }
    Ok(())
}

/// 该地址/前缀的定向广播地址；缺前缀时按 /24（家宽最常见）。
pub(crate) fn broadcast_for(ip: Ipv4Addr, prefix: Option<u8>) -> Ipv4Addr {
    let p = prefix.unwrap_or(24).min(32);
    if p == 0 {
        return Ipv4Addr::BROADCAST;
    }
    let mask = u32::MAX << (32 - u32::from(p));
    Ipv4Addr::from(u32::from(ip) | !mask)
}

/// 广播/组播都被丢弃时的兜底：对**本机所在 /24** 的每台主机发探测 beacon。
/// 收到探测的一方会单播回一个普通 beacon（见 `discovery_loop`）。
///
/// 前缀不是 /24 也照样扫：家宽常见 /22、/16，定向广播常被路由器丢掉；扫本机
/// 所在 /24 既覆盖同网段邻居，又不会因为前缀大而发几万个包。
pub(crate) fn send_probes(
    socket: &UdpSocket,
    port: u16,
    mut beacon: Beacon,
    local_ip: Option<Ipv4Addr>,
    _prefix: Option<u8>,
) -> std::io::Result<()> {
    let Some(ip) = local_ip.or_else(local_ipv4) else {
        return Ok(());
    };
    let o = ip.octets();
    beacon.capabilities |= crate::types::CAP_PROBE;
    let bytes = beacon.encode();
    for host in 1u8..=254 {
        if host == o[3] {
            continue;
        }
        let target = SocketAddr::from((Ipv4Addr::new(o[0], o[1], o[2], host), port));
        let _ = socket.send_to(&bytes, target);
    }
    Ok(())
}

/// 手动连接：向指定 IPv4 的发现端口发一个探测 beacon，对端会单播回复。
pub(crate) fn send_probe_to(
    socket: &UdpSocket,
    port: u16,
    mut beacon: Beacon,
    ip: Ipv4Addr,
) -> std::io::Result<()> {
    beacon.capabilities |= crate::types::CAP_PROBE;
    socket
        .send_to(&beacon.encode(), SocketAddr::from((ip, port)))
        .map(|_| ())
}

/// 探测本机默认出口 IPv4（UDP connect 不真正发包）；用于推导 /24 广播地址。
fn local_ipv4() -> Option<Ipv4Addr> {
    let probe = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    probe.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).ok()?;
    match probe.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(ip) => Some(ip),
        _ => None,
    }
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
    fn broadcast_uses_the_real_prefix() {
        assert_eq!(
            broadcast_for(Ipv4Addr::new(192, 168, 1, 5), None),
            Ipv4Addr::new(192, 168, 1, 255)
        );
        assert_eq!(
            broadcast_for(Ipv4Addr::new(10, 1, 2, 3), Some(16)),
            Ipv4Addr::new(10, 1, 255, 255)
        );
        assert_eq!(
            broadcast_for(Ipv4Addr::new(10, 1, 2, 3), Some(24)),
            Ipv4Addr::new(10, 1, 2, 255)
        );
    }

    #[test]
    fn probes_are_unicast_and_flagged_for_reply() {
        let rx = UdpSocket::bind((Ipv4Addr::new(127, 0, 0, 2), 0)).unwrap();
        rx.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let port = rx.local_addr().unwrap().port();
        let tx = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let beacon = beacon_for(id(7), &[3u8; 32], 5000, 1);
        send_probes(&tx, port, beacon, Some(Ipv4Addr::LOCALHOST), None).unwrap();
        let mut buf = [0u8; 64];
        let (len, _) = rx
            .recv_from(&mut buf)
            .expect("probe should reach the /24 host");
        let got = Beacon::decode(&buf[..len]).unwrap();
        assert_eq!(got.instance_id, beacon.instance_id);
        assert_ne!(got.capabilities & crate::types::CAP_PROBE, 0);
    }

    #[test]
    fn probes_still_run_on_wide_subnets() {
        // /16 家宽：定向广播可能被丢，仍要扫本机所在 /24（而不是直接放弃）。
        let rx = UdpSocket::bind((Ipv4Addr::new(127, 0, 0, 3), 0)).unwrap();
        rx.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let port = rx.local_addr().unwrap().port();
        let tx = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let beacon = beacon_for(id(8), &[3u8; 32], 5000, 1);
        send_probes(&tx, port, beacon, Some(Ipv4Addr::LOCALHOST), Some(16)).unwrap();
        let mut buf = [0u8; 64];
        let (len, _) = rx
            .recv_from(&mut buf)
            .expect("wide subnet must still probe /24");
        assert_eq!(
            Beacon::decode(&buf[..len]).unwrap().instance_id,
            beacon.instance_id
        );
    }

    #[test]
    fn two_sockets_on_same_port_see_each_other() {
        let port = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let ip = local_ipv4().unwrap_or(Ipv4Addr::LOCALHOST);
        let a = bind(port, Some(ip)).unwrap();
        let b = bind(port, Some(ip)).unwrap();
        let ba = beacon_for(id(7), &[3u8; 32], 5000, 1);
        let bb = beacon_for(id(8), &[3u8; 32], 5000, 1);
        a.send_to(&ba.encode(), (DISCOVERY_GROUP, port)).unwrap();
        b.send_to(&bb.encode(), (DISCOVERY_GROUP, port)).unwrap();
        let mut buf = [0u8; 64];
        a.set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        b.set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        let mut seen_a = Vec::new();
        let mut seen_b = Vec::new();
        for _ in 0..2 {
            let (len, _) = a.recv_from(&mut buf).expect("a must receive multicast");
            seen_a.push(Beacon::decode(&buf[..len]).unwrap().instance_id);
            let (len, _) = b.recv_from(&mut buf).expect("b must receive multicast");
            seen_b.push(Beacon::decode(&buf[..len]).unwrap().instance_id);
        }
        assert!(seen_a.contains(&id(8)), "a must hear b");
        assert!(seen_b.contains(&id(7)), "b must hear a");
    }

    #[test]
    fn primary_group_is_link_local_and_legacy_is_kept() {
        // 224.0.0.0/24 是链路本地组播，AP/路由器必须转发；239/8 容易被 AP 过滤。
        assert_eq!(DISCOVERY_GROUP.octets()[0], 224);
        assert_eq!(DISCOVERY_GROUP_LEGACY.octets()[0], 239);
        assert_ne!(DISCOVERY_GROUP, DISCOVERY_GROUP_LEGACY);
    }

    #[test]
    fn beacon_has_no_content_field() {
        // 固定长度本身就是内容不存在的结构保证：加字段只能改协议版本。
        let beacon = beacon_for(id(5), &[9u8; 32], 5000, 1);
        assert_eq!(beacon.encode().len(), BEACON_LEN);
    }
}
