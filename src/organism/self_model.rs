//! The organism's model of its own condition and capability: how fast each
//! variable is drifting, how long until it is critical, how far the
//! remembered resource is, how much an Eat or Drink yields.

use serde::{Deserialize, Serialize};

use crate::body::{Action, Deltas, N_VARS};
use crate::signals::{N_RESOURCES, Remembered};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SelfModelParams {
    pub drift_rate: f64,
    pub drift_eps: f64,
    pub t_max: f64,
    /// EMA rate of the gain-per-unit estimate.
    pub yield_rate: f64,
    /// A cell amount above `novelty_ratio` × the largest amount ever
    /// consumed at is outside experience: novel.
    pub novelty_ratio: f64,
}

impl Default for SelfModelParams {
    fn default() -> Self {
        SelfModelParams { drift_rate: 0.02, drift_eps: 1e-4, t_max: 20_000.0, yield_rate: 0.3, novelty_ratio: 2.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfModel {
    drift: [f64; N_VARS],
    /// Gain per unit of resource on the cell when consuming. 0 until the
    /// first successful Eat / Drink.
    yield_: [f64; N_RESOURCES],
    /// Largest amount at which Eat / Drink was ever tried.
    max_consumed_at: [f64; N_RESOURCES],
}

pub struct SelfModelOut {
    pub ttc: [f64; N_VARS],
    pub reach: [f64; N_RESOURCES],
    pub yield_: [f64; N_RESOURCES],
    pub novel_remembered: [bool; N_RESOURCES],
    pub novel_here: [bool; N_RESOURCES],
}

impl SelfModel {
    pub fn new(p: &SelfModelParams) -> SelfModel {
        let _ = p;
        SelfModel { drift: [0.0; N_VARS], yield_: [0.0; N_RESOURCES], max_consumed_at: [0.0; N_RESOURCES] }
    }

    pub fn null_out(p: &SelfModelParams) -> SelfModelOut {
        SelfModelOut { ttc: [p.t_max; N_VARS], reach: [p.t_max; N_RESOURCES], yield_: [0.0; N_RESOURCES], novel_remembered: [false; N_RESOURCES], novel_here: [false; N_RESOURCES] }
    }

    pub fn yields(&self) -> [f64; N_RESOURCES] {
        self.yield_
    }

    /// `prev_action` and `here_prev` (cell amounts before that action) teach
    /// the yield; `here` is the current cell for novelty.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        v: &[f64; N_VARS],
        deltas: &Deltas,
        prev_action: Option<Action>,
        here_prev: &[f64; N_RESOURCES],
        here: &[f64; N_RESOURCES],
        remembered_prev: &[Remembered; N_RESOURCES],
        danger_lo: &[f64; N_VARS],
        p: &SelfModelParams,
    ) -> SelfModelOut {
        for d in 0..N_VARS {
            self.drift[d] += p.drift_rate * (deltas.0[d] - self.drift[d]);
        }
        let consumed = match prev_action {
            Some(Action::Eat) => Some(0),
            Some(Action::Drink) => Some(1),
            _ => None,
        };
        if let Some(r) = consumed {
            let amount = here_prev[r];
            self.max_consumed_at[r] = self.max_consumed_at[r].max(amount);
            if amount > 0.01 {
                let sample = (deltas.0[r].max(0.0)) / amount;
                if self.yield_[r] == 0.0 { self.yield_[r] = sample } else { self.yield_[r] += p.yield_rate * (sample - self.yield_[r]) }
            }
        }
        let mut novel_remembered = [false; N_RESOURCES];
        let mut novel_here = [false; N_RESOURCES];
        for r in 0..N_RESOURCES {
            let limit = p.novelty_ratio * self.max_consumed_at[r] + 0.02;
            novel_here[r] = here[r] > limit;
            novel_remembered[r] = remembered_prev[r].conf > 0.0 && remembered_prev[r].amounts[r] > limit;
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
        SelfModelOut { ttc, reach, yield_: self.yield_, novel_remembered, novel_here }
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
        let slow = sm.step(&[0.6, 0.6, 0.5], &Deltas([-0.001, 0.0, 0.0]), None, &[0.0; 2], &[0.0; 2], &rem, &lo, &p).ttc[0];
        let mut sm2 = SelfModel::new(&p);
        let mut fast = 0.0;
        for _ in 0..100 {
            fast = sm2.step(&[0.6, 0.6, 0.5], &Deltas([-0.01, 0.0, 0.0]), None, &[0.0; 2], &[0.0; 2], &rem, &lo, &p).ttc[0];
        }
        assert!(fast < slow);
        assert!(fast > 0.0);
    }

    #[test]
    fn learns_yield_and_novelty() {
        let p = SelfModelParams::default();
        let mut sm = SelfModel::new(&p);
        let rem = [Remembered { target: (1, 1), dist: 4, conf: 1.0, amounts: [1.0, 0.0], toward: [false; 4] }, Remembered::default()];
        let lo = [0.15; 3];
        let o = sm.step(&[0.5; 3], &Deltas::default(), None, &[0.0; 2], &[0.25, 0.0], &rem, &lo, &p);
        assert!(o.novel_here[0] && o.novel_remembered[0]);
        // Ate on a 0.25 cell and gained 0.15: yield 0.6 per unit.
        let o = sm.step(&[0.5; 3], &Deltas([0.15, 0.0, 0.0]), Some(Action::Eat), &[0.25, 0.0], &[0.0, 0.0], &rem, &lo, &p);
        assert!((o.yield_[0] - 0.6).abs() < 1e-9);
        assert!(!o.novel_here[0]);
        assert!(o.novel_remembered[0]); // 1.0 > 2 × 0.25
    }
}
