//! Deterministic PRNG: xoshiro256** seeded through splitmix64.
//! One stream per subsystem, derived from the run seed.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn from_seed(seed: u64) -> Rng {
        let mut st = seed;
        let s = [
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
            splitmix64(&mut st),
        ];
        Rng { s }
    }

    /// Independent stream: mixes the parent's current state with a stream id.
    /// Does not advance the parent.
    pub fn derive(&self, stream: u64) -> Rng {
        let mixed = self.s[0] ^ self.s[1].rotate_left(17) ^ stream.wrapping_mul(0xD6E8_FEB8_6659_FD93);
        Rng::from_seed(mixed)
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in [0, n). Panics on n == 0.
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "Rng::below(0)");
        (self.f64() * n as f64) as usize
    }

    /// Symmetric noise in [-1, 1).
    pub fn signed(&mut self) -> f64 {
        self.f64() * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::from_seed(7);
        let mut b = Rng::from_seed(7);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn derived_streams_differ() {
        let root = Rng::from_seed(1);
        let mut a = root.derive(0);
        let mut b = root.derive(1);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn f64_in_unit_interval() {
        let mut r = Rng::from_seed(3);
        for _ in 0..10_000 {
            let x = r.f64();
            assert!((0.0..1.0).contains(&x));
        }
    }

    #[test]
    fn below_in_range() {
        let mut r = Rng::from_seed(5);
        for _ in 0..10_000 {
            assert!(r.below(7) < 7);
        }
    }
}
