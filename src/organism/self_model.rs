//! The organism's model of its own condition and capability: how fast each
//! variable is drifting, how long until it is critical, how far the
//! remembered resource is, how much an Eat or Drink yields.

use serde::{Deserialize, Serialize};

use crate::body::{Deltas, N_VARS};
use crate::signals::{N_RESOURCES, Remembered};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SelfModelParams {
    pub drift_rate: f64,
    pub drift_eps: f64,
    pub t_max: f64,
}

impl Default for SelfModelParams {
    fn default() -> Self {
        SelfModelParams { drift_rate: 0.02, drift_eps: 1e-4, t_max: 20_000.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfModel {
    drift: [f64; N_VARS],
}

pub struct SelfModelOut {
    pub ttc: [f64; N_VARS],
    pub reach: [f64; N_RESOURCES],
}

impl SelfModel {
    pub fn new(p: &SelfModelParams) -> SelfModel {
        let _ = p;
        SelfModel { drift: [0.0; N_VARS] }
    }

    pub fn null_out(p: &SelfModelParams) -> SelfModelOut {
        SelfModelOut { ttc: [p.t_max; N_VARS], reach: [p.t_max; N_RESOURCES] }
    }

    pub fn step(
        &mut self,
        v: &[f64; N_VARS],
        deltas: &Deltas,
        remembered_prev: &[Remembered; N_RESOURCES],
        danger_lo: &[f64; N_VARS],
        p: &SelfModelParams,
    ) -> SelfModelOut {
        for d in 0..N_VARS {
            self.drift[d] += p.drift_rate * (deltas.0[d] - self.drift[d]);
        }
        let mut ttc = [0.0; N_VARS];
        for d in 0..N_VARS {
            let fall = (-self.drift[d]).max(p.drift_eps);
            ttc[d] = ((v[d] - danger_lo[d]) / fall).clamp(0.0, p.t_max);
        }
        let mut reach = [p.t_max; N_RESOURCES];
        for r in 0..N_RESOURCES {
            if remembered_prev[r].conf > 0.0 {
                reach[r] = (remembered_prev[r].dist as f64).max(1.0);
            }
        }
        SelfModelOut { ttc, reach }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttc_shrinks_as_drift_grows() {
        let p = SelfModelParams::default();
        let mut sm = SelfModel::new(&p);
        let rem = [Remembered::default(); 2];
        let lo = [0.15; 3];
        let slow = sm.step(&[0.6, 0.6, 0.5], &Deltas([-0.001, 0.0, 0.0]), &rem, &lo, &p).ttc[0];
        let mut sm2 = SelfModel::new(&p);
        let mut fast = 0.0;
        for _ in 0..100 {
            fast = sm2.step(&[0.6, 0.6, 0.5], &Deltas([-0.01, 0.0, 0.0]), &rem, &lo, &p).ttc[0];
        }
        assert!(fast < slow);
        assert!(fast > 0.0);
    }
}
