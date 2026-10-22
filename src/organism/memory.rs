//! Spatial memory with drive-modulated encoding, decay, and consolidation
//! into quadrant priors that survive cell-level forgetting.

use serde::{Deserialize, Serialize};

use crate::body::MOVES;
use crate::organism::perception::{Observation, RADIUS};
use crate::signals::{N_DRIVES, N_RESOURCES, Remembered};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryParams {
    /// Confidence given to an observed cell with zero urgency.
    pub conf_base: f64,
    /// Extra confidence per unit of the matching drive's urgency.
    pub salience: f64,
    /// Per-tick confidence multiplier for unobserved cells.
    pub forget: f64,
    /// Per-tick relaxation of unobserved estimates toward the region prior.
    pub relax: f64,
    pub consolidate_every: u64,
    /// Cells with confidence above this feed consolidation.
    pub consolidate_conf: f64,
    /// A cell counts as a remembered target when conf * estimate exceeds this.
    pub target_threshold: f64,
}

impl Default for MemoryParams {
    fn default() -> Self {
        MemoryParams {
            conf_base: 0.5,
            salience: 1.0,
            forget: 0.995,
            relax: 0.002,
            consolidate_every: 500,
            consolidate_conf: 0.5,
            target_threshold: 0.05,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory {
    width: usize,
    height: usize,
    tick: u64,
    estimate: Vec<[f64; N_RESOURCES]>,
    conf: Vec<[f64; N_RESOURCES]>,
    /// Quadrant priors: [quadrant][resource].
    prior: [[f64; N_RESOURCES]; 4],
}

pub struct MemoryOut {
    pub remembered: [Remembered; N_RESOURCES],
    pub neighbor_conf: [f64; 4],
}

impl MemoryOut {
    pub fn null() -> MemoryOut {
        MemoryOut { remembered: [Remembered::default(); N_RESOURCES], neighbor_conf: [0.0; 4] }
    }
}

impl Memory {
    pub fn new(width: usize, height: usize) -> Memory {
        let n = width * height;
        Memory { width, height, tick: 0, estimate: vec![[0.0; N_RESOURCES]; n], conf: vec![[0.0; N_RESOURCES]; n], prior: [[0.0; N_RESOURCES]; 4] }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn wrap(&self, x: usize, y: usize, dx: i32, dy: i32) -> (usize, usize) {
        let w = self.width as i32;
        let h = self.height as i32;
        ((x as i32 + dx).rem_euclid(w) as usize, (y as i32 + dy).rem_euclid(h) as usize)
    }

    fn distance(&self, a: (usize, usize), b: (usize, usize)) -> usize {
        let dx = a.0.abs_diff(b.0);
        let dy = a.1.abs_diff(b.1);
        dx.min(self.width - dx) + dy.min(self.height - dy)
    }

    fn quadrant(&self, x: usize, y: usize) -> usize {
        let qx = if x < self.width / 2 { 0 } else { 1 };
        let qy = if y < self.height / 2 { 0 } else { 1 };
        qy * 2 + qx
    }

    pub fn conf_at(&self, x: usize, y: usize) -> [f64; N_RESOURCES] {
        self.conf[self.idx(x, y)]
    }

    pub fn estimate_at(&self, x: usize, y: usize) -> [f64; N_RESOURCES] {
        self.estimate[self.idx(x, y)]
    }

    pub fn priors(&self) -> &[[f64; N_RESOURCES]; 4] {
        &self.prior
    }

    /// `learn` false (the Frozen baseline) computes outputs from the map as
    /// it is and writes nothing.
    pub fn step(&mut self, obs: &Observation, urgency_prev: &[f64; N_DRIVES], p: &MemoryParams, learn: bool) -> MemoryOut {
        if learn {
            self.record(obs, urgency_prev, p);
        }
        self.outputs(obs, p)
    }

    fn record(&mut self, obs: &Observation, urgency_prev: &[f64; N_DRIVES], p: &MemoryParams) {
        self.tick += 1;
        // Forget and relax everywhere; observed cells are overwritten below.
        for i in 0..self.estimate.len() {
            let x = i % self.width;
            let y = i / self.width;
            let q = self.quadrant(x, y);
            for r in 0..N_RESOURCES {
                self.conf[i][r] *= p.forget;
                self.estimate[i][r] += p.relax * (self.prior[q][r] - self.estimate[i][r]);
            }
        }
        // Record the window. Drive index equals resource index for food/water.
        for dy in -RADIUS..=RADIUS {
            for dx in -RADIUS..=RADIUS {
                let (x, y) = self.wrap(obs.pos.0, obs.pos.1, dx, dy);
                let i = self.idx(x, y);
                let row = (dy + RADIUS) as usize;
                let col = (dx + RADIUS) as usize;
                let seen = [obs.food[row][col], obs.water[row][col]];
                for r in 0..N_RESOURCES {
                    self.estimate[i][r] = seen[r];
                    let u = urgency_prev[r].max(0.0);
                    self.conf[i][r] = (p.conf_base + p.salience * u).min(1.0);
                }
            }
        }
        if self.tick.is_multiple_of(p.consolidate_every) {
            self.consolidate(p);
        }
    }

    fn outputs(&self, obs: &Observation, p: &MemoryParams) -> MemoryOut {
        // Nearest remembered target per resource, excluding the current cell.
        let mut remembered = [Remembered::default(); N_RESOURCES];
        for r in 0..N_RESOURCES {
            let mut best: Option<(usize, usize, usize)> = None; // (dist, x, y)
            for i in 0..self.estimate.len() {
                if self.conf[i][r] * self.estimate[i][r] < p.target_threshold {
                    continue;
                }
                let x = i % self.width;
                let y = i / self.width;
                let d = self.distance(obs.pos, (x, y));
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, x, y));
                }
            }
            if let Some((d, x, y)) = best {
                let i = self.idx(x, y);
                let mut toward = [false; 4];
                for (k, m) in MOVES.iter().enumerate() {
                    let (dx, dy) = m.offset();
                    let next = self.wrap(obs.pos.0, obs.pos.1, dx, dy);
                    toward[k] = self.distance(next, (x, y)) < d;
                }
                remembered[r] = Remembered { target: (x, y), dist: d, conf: self.conf[i][r], amounts: self.estimate[i], toward };
            }
        }
        let mut neighbor_conf = [0.0; 4];
        for (k, m) in MOVES.iter().enumerate() {
            let (dx, dy) = m.offset();
            let (x, y) = self.wrap(obs.pos.0, obs.pos.1, dx, dy);
            let c = self.conf[self.idx(x, y)];
            neighbor_conf[k] = (c[0] + c[1]) * 0.5;
        }
        MemoryOut { remembered, neighbor_conf }
    }

    fn consolidate(&mut self, p: &MemoryParams) {
        let mut sum = [[0.0; N_RESOURCES]; 4];
        let mut n = [[0usize; N_RESOURCES]; 4];
        for i in 0..self.estimate.len() {
            let q = self.quadrant(i % self.width, i / self.width);
            for r in 0..N_RESOURCES {
                if self.conf[i][r] >= p.consolidate_conf {
                    sum[q][r] += self.estimate[i][r];
                    n[q][r] += 1;
                }
            }
        }
        for q in 0..4 {
            for r in 0..N_RESOURCES {
                if n[q][r] > 0 {
                    self.prior[q][r] = sum[q][r] / n[q][r] as f64;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organism::perception::WINDOW;

    fn obs(pos: (usize, usize), food_here: f64) -> Observation {
        let mut food = [[0.0; WINDOW]; WINDOW];
        food[2][2] = food_here;
        Observation { food, water: [[0.0; WINDOW]; WINDOW], ambient01: [[0.5; WINDOW]; WINDOW], v: [0.5; 3], pos, prev_action: None }
    }

    #[test]
    fn remembers_then_forgets() {
        let p = MemoryParams::default();
        let mut m = Memory::new(16, 16);
        m.step(&obs((3, 3), 1.0), &[0.0; 4], &p, true);
        let out = m.step(&obs((8, 8), 0.0), &[0.0; 4], &p, true);
        assert_eq!(out.remembered[0].target, (3, 3));
        assert_eq!(out.remembered[0].dist, 10);
        assert!(out.remembered[0].toward[0] && out.remembered[0].toward[3]); // N and W
        for _ in 0..2000 {
            m.step(&obs((8, 8), 0.0), &[0.0; 4], &p, true);
        }
        let out = m.step(&obs((8, 8), 0.0), &[0.0; 4], &p, true);
        assert_eq!(out.remembered[0].conf, 0.0);
    }

    #[test]
    fn urgency_raises_encoding_confidence() {
        let p = MemoryParams::default();
        let mut calm = Memory::new(16, 16);
        let mut hungry = Memory::new(16, 16);
        calm.step(&obs((3, 3), 1.0), &[0.0; 4], &p, true);
        hungry.step(&obs((3, 3), 1.0), &[0.4; 4], &p, true);
        assert!(hungry.conf_at(3, 3)[0] > calm.conf_at(3, 3)[0]);
    }

    #[test]
    fn prior_survives_cell_forgetting() {
        let p = MemoryParams { consolidate_every: 10, ..MemoryParams::default() };
        let mut m = Memory::new(16, 16);
        for _ in 0..10 {
            m.step(&obs((3, 3), 1.0), &[0.0; 4], &p, true);
        }
        assert!(m.priors()[0][0] > 0.0);
        for _ in 0..3000 {
            m.step(&obs((12, 12), 0.0), &[0.0; 4], &p, true);
        }
        assert!(m.conf_at(3, 3)[0] < 1e-3);
        assert!(m.priors()[0][0] > 0.0);
    }

    #[test]
    fn frozen_memory_writes_nothing() {
        let p = MemoryParams::default();
        let mut m = Memory::new(16, 16);
        m.step(&obs((3, 3), 1.0), &[0.0; 4], &p, true);
        let before = m.conf_at(3, 3)[0];
        m.step(&obs((8, 8), 0.0), &[0.0; 4], &p, false);
        assert_eq!(m.conf_at(3, 3)[0], before);
        assert_eq!(m.conf_at(8, 8)[0], 0.0);
    }
}
