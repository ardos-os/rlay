/// Fast non-cryptographic hasher used for frame-local layout maps.
#[derive(Debug)]
pub(crate) struct FastHasher(u64);

impl Default for FastHasher {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.0 ^= u64::from(value);
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }

    fn write_u16(&mut self, value: u16) {
        self.mix_u64(u64::from(value));
    }

    fn write_u32(&mut self, value: u32) {
        self.mix_u64(u64::from(value));
    }

    fn write_u64(&mut self, value: u64) {
        self.mix_u64(value);
    }

    fn write_usize(&mut self, value: usize) {
        self.mix_u64(value as u64);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

impl FastHasher {
    fn mix_u64(&mut self, value: u64) {
        self.0 ^= value;
        self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

use std::hash::Hasher;
