// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 字节串工具：有序区间扫描（`symbols` 的关键字索引与 engine-data 的二分上界共用）。

/// 字节后继：末字节 +1（带进位）；全 `0xFF` 返回 None（表示「无后继，上界 = 表尾」）。
///
/// 用于把「前缀区间」转成「下界 .. 后继下界」：`[lo, hi)` 覆盖 `p` 的全部前缀匹配。
/// 进位是细活（`\xFF` 逐位回退），只此一份实现 —— 两份必然漂移。
pub fn byte_successor(p: &[u8]) -> Option<Vec<u8>> {
    let mut b = p.to_vec();
    let mut i = b.len();
    while i > 0 {
        i -= 1;
        let (nb, overflow) = b[i].overflowing_add(1);
        b[i] = nb;
        if !overflow {
            return Some(b);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increments_last_byte() {
        assert_eq!(byte_successor(b"h"), Some(b"i".to_vec()));
        assert_eq!(byte_successor(b"hao"), Some(b"hap".to_vec()));
        assert_eq!(byte_successor(b""), None);
    }

    #[test]
    fn carries_and_exhausts() {
        // 末字节溢出 → 进位到前一位，末位归零
        assert_eq!(byte_successor(&[b'h', 0xFF]), Some(vec![b'i', 0x00]));
        // 全 0xFF → 无后继
        assert_eq!(byte_successor(&[0xFF, 0xFF]), None);
        assert_eq!(byte_successor(&[0xFF]), None);
    }
}
