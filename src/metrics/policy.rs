//! Policy fingerprint: P(action | state bin) with fixed absolute bins and
//! Dirichlet smoothing. Observable from behavior alone.

use serde::{Deserialize, Serialize};

use crate::body::N_ACTIONS;
use crate::metrics::trajectory::Row;

pub const N_ACT: usize = N_ACTIONS;
/// E bin (3) × H bin (3) × Tb bin (3) × food visible (2) × water visible (2).
pub const N_BINS: usize = 108;
pub const ALPHA: f64 = 1.0;

fn third(x: f64) -> usize {
    if x < 1.0 / 3.0 { 0 } else if x < 2.0 / 3.0 { 1 } else { 2 }
}

pub fn bin_of(body: &[f64; 3], vis: &[bool; 2]) -> usize {
    let e = third(body[0]);
    let h = third(body[1]);
    let t = third(body[2]);
    (((e * 3 + h) * 3 + t) * 2 + vis[0] as usize) * 2 + vis[1] as usize
}

pub fn bin_label(bin: usize) -> String {
    let wv = bin % 2;
    let fv = (bin / 2) % 2;
    let t = (bin / 4) % 3;
    let h = (bin / 12) % 3;
    let e = bin / 36;
    format!("E{e}H{h}T{t}f{fv}w{wv}")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fingerprint {
    counts: Vec<[u32; N_ACT]>,
}

impl Default for Fingerprint {
    fn default() -> Self {
        Fingerprint { counts: vec![[0; N_ACT]; N_BINS] }
    }
}

impl Fingerprint {
    pub fn from_rows(rows: &[Row]) -> Fingerprint {
        let mut fp = Fingerprint::default();
        for r in rows {
            fp.add(r);
        }
        fp
    }

    pub fn add(&mut self, r: &Row) {
        self.counts[bin_of(&r.body, &r.vis)][r.action.index()] += 1;
    }

    pub fn merge(&mut self, other: &Fingerprint) {
        for bin in 0..N_BINS {
            for a in 0..N_ACT {
                self.counts[bin][a] += other.counts[bin][a];
            }
        }
    }

    pub fn counts(&self, bin: usize) -> &[u32; N_ACT] {
        &self.counts[bin]
    }

    pub fn occupancy(&self, bin: usize) -> u32 {
        self.counts[bin].iter().sum()
    }

    pub fn total(&self) -> u32 {
        (0..N_BINS).map(|b| self.occupancy(b)).sum()
    }

    /// Dirichlet(ALPHA)-smoothed P(a | bin).
    pub fn smoothed(&self, bin: usize) -> [f64; N_ACT] {
        let n = self.occupancy(bin) as f64;
        let mut p = [0.0; N_ACT];
        for a in 0..N_ACT {
            p[a] = (self.counts[bin][a] as f64 + ALPHA) / (n + ALPHA * N_ACT as f64);
        }
        p
    }

    /// Shannon entropy (nats) of the marginal action distribution.
    pub fn action_entropy(&self) -> f64 {
        let mut m = [0u32; N_ACT];
        for bin in 0..N_BINS {
            for a in 0..N_ACT {
                m[a] += self.counts[bin][a];
            }
        }
        entropy(&m)
    }

    /// Occupancy-weighted mean of per-bin action entropy (nats).
    pub fn conditional_entropy(&self) -> f64 {
        let total = self.total() as f64;
        if total == 0.0 {
            return 0.0;
        }
        (0..N_BINS).map(|b| self.occupancy(b) as f64 / total * entropy(&self.counts[b])).sum()
    }
}

fn entropy(counts: &[u32; N_ACT]) -> f64 {
    let n: u32 = counts.iter().sum();
    if n == 0 {
        return 0.0;
    }
    let mut h = 0.0;
    for &c in counts {
        if c > 0 {
            let p = c as f64 / n as f64;
            h -= p * p.ln();
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins_cover_range_and_are_unique() {
        let mut seen = [false; N_BINS];
        for e in [0.1, 0.5, 0.9] {
            for h in [0.1, 0.5, 0.9] {
                for t in [0.1, 0.5, 0.9] {
                    for fv in [false, true] {
                        for wv in [false, true] {
                            let b = bin_of(&[e, h, t], &[fv, wv]);
                            assert!(!seen[b]);
                            seen[b] = true;
                        }
                    }
                }
            }
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(bin_label(0), "E0H0T0f0w0");
        assert_eq!(bin_label(N_BINS - 1), "E2H2T2f1w1");
    }
}
