//! FNV-1a 64-bit. Used for the trajectory hash and the config hash.

#[derive(Clone, Copy, Debug)]
pub struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Fnv(0xCBF2_9CE4_8422_2325)
    }
}

impl Fnv {
    pub fn new() -> Fnv {
        Fnv::default()
    }

    pub fn from_state(state: u64) -> Fnv {
        Fnv(state)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }

    pub fn write_u64(&mut self, v: u64) {
        self.write(&v.to_le_bytes());
    }

    pub fn write_f64(&mut self, v: f64) {
        self.write_u64(v.to_bits());
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

pub fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut h = Fnv::new();
    h.write(bytes);
    h.finish()
}
