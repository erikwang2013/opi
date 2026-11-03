// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// opi-clip 的宿主侧投影类型（从 ClipSync.ets 拆出：2026-11-03 传输功能要复用，
// 且 ClipSync 已到 500 行门禁边上）。字段名与 crates/opi-ffi/src/cabi_clip/encode.rs
// 的 snake_case JSON 一一对应；解析仍在 ClipSync（peers/status 刷新）。
// 真源上限见 crates/opi-clip/src/types.rs —— 宿主侧提前拒绝只是为了少等一次复制。

export interface ClipPeer {
  instanceId: string;
  name: string;
  address: string;
  fingerprint: string;
  trusted: boolean;
  connected: boolean;
}

export interface ClipStatus {
  running: boolean;
  instanceId: string;
  fingerprint: string;
  deviceName: string;
  peers: number;
}

export interface ClipPairingPrompt {
  instanceId: string;
  name: string;
  sas: string;
  /** true = 这次配对由对端发起。 */
  incoming: boolean;
}

export interface RawPeer {
  instance_id?: string;
  name?: string;
  address?: string;
  fingerprint?: string;
  trusted?: boolean;
  connected?: boolean;
}

export interface RawStatus {
  running?: boolean;
  instance_id?: string | null;
  fingerprint?: string | null;
  device_name?: string;
  peers?: number;
}

export interface ClipConfig {
  data_dir: string;
  tcp_port: number;
  discovery_port: number;
  device_name: string;
}

function peerOf(raw: RawPeer): ClipPeer | null {
  if (typeof raw.instance_id !== 'string' || raw.instance_id.length === 0) {
    return null;
  }
  return {
    instanceId: raw.instance_id,
    name: typeof raw.name === 'string' ? raw.name : '',
    address: typeof raw.address === 'string' ? raw.address : '',
    fingerprint: typeof raw.fingerprint === 'string' ? raw.fingerprint : '',
    trusted: raw.trusted === true,
    connected: raw.connected === true,
  };
}

/** peers() JSON → 列表；坏格式 → null（保留上一份，避免一次解析失败把设备清空）。 */
export function parsePeerJson(json: string): ClipPeer[] | null {
  let raw: RawPeer[];
  try {
    raw = JSON.parse(json) as RawPeer[];
  } catch (e) {
    return null;
  }
  const out: ClipPeer[] = [];
  for (let i = 0; i < raw.length; i++) {
    const peer: ClipPeer | null = peerOf(raw[i]);
    if (peer !== null) {
      out.push(peer);
    }
  }
  return out;
}

/** status() JSON → 投影；坏格式 → null。 */
export function parseStatusJson(json: string): ClipStatus | null {
  let raw: RawStatus;
  try {
    raw = JSON.parse(json) as RawStatus;
  } catch (e) {
    return null;
  }
  return {
    running: raw.running === true,
    instanceId: typeof raw.instance_id === 'string' ? raw.instance_id : '',
    fingerprint: typeof raw.fingerprint === 'string' ? raw.fingerprint : '',
    deviceName: typeof raw.device_name === 'string' ? raw.device_name : '',
    peers: typeof raw.peers === 'number' ? raw.peers : 0,
  };
}
