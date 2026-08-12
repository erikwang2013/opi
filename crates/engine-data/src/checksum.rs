//! .opid 校验和（FNV-1a 64 位）。

const OFFSET: u64 = 0xcbf29ce484222325;
const PRIME: u64 = 0x100000001b3;

/// FNV-1a 64 位哈希（自实现，不引入依赖）。.opid 校验和专用。
pub fn fnv1a64(data: &[u8]) -> u64 {
    data.iter().fold(OFFSET, |h, &b| (h ^ b as u64).wrapping_mul(PRIME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }
}
