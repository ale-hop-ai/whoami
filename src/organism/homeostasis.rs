//! Interoception: normalized error per variable and edge-triggered danger.

use serde::{Deserialize, Serialize};

use crate::body::{N_VARS, VARS, Var};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeostasisParams {
    pub setpoint: [f64; N_VARS],
    pub width: [f64; N_VARS],
    pub danger_lo: [f64; N_VARS],
    pub danger_hi: [f64; N_VARS],
}

impl Default for HomeostasisParams {
    fn default() -> Self {
        HomeostasisParams {
            setpoint: [0.6, 0.6, 0.5],
            width: [0.4, 0.4, 0.3],
            danger_lo: [0.15, 0.15, 0.15],
            danger_hi: [2.0, 2.0, 0.85],
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Homeostasis {
    in_danger: [bool; N_VARS],
}

pub struct HomeostasisOut {
    pub errors: [f64; N_VARS],
    pub danger_events: [bool; N_VARS],
}

impl Homeostasis {
    pub fn step(&mut self, v: &[f64; N_VARS], p: &HomeostasisParams) -> HomeostasisOut {
        let mut errors = [0.0; N_VARS];
        let mut danger_events = [false; N_VARS];
        for var in VARS {
            let i = var.index();
            errors[i] = (p.setpoint[i] - v[i]) / p.width[i];
            let now = v[i] < p.danger_lo[i] || v[i] > p.danger_hi[i];
            danger_events[i] = now && !self.in_danger[i];
            self.in_danger[i] = now;
        }
        HomeostasisOut { errors, danger_events }
    }

    pub fn is_in_danger(&self, var: Var) -> bool {
        self.in_danger[var.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn danger_is_edge_triggered() {
        let p = HomeostasisParams::default();
        let mut h = Homeostasis::default();
        let a = h.step(&[0.1, 0.6, 0.5], &p);
        assert!(a.danger_events[0]);
        let b = h.step(&[0.1, 0.6, 0.5], &p);
        assert!(!b.danger_events[0]);
        h.step(&[0.5, 0.6, 0.5], &p);
        let c = h.step(&[0.1, 0.6, 0.5], &p);
        assert!(c.danger_events[0]);
    }

    #[test]
    fn error_sign_is_deficit_positive() {
        let p = HomeostasisParams::default();
        let mut h = Homeostasis::default();
        let o = h.step(&[0.2, 0.9, 0.5], &p);
        assert!(o.errors[0] > 0.0);
        assert!(o.errors[1] < 0.0);
        assert_eq!(o.errors[2], 0.0);
    }
}
