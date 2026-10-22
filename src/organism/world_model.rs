//! Linear per-action, per-variable predictor of body deltas, learned online
//! by normalized LMS. Features include the destination cell, so the model can
//! learn what moving somewhere does without any rule naming it.

use serde::{Deserialize, Serialize};

use crate::body::{ACTIONS, Action, Deltas, N_ACTIONS, N_VARS};
use crate::organism::perception::Observation;
use crate::signals::{N_RESOURCES, Remembered};

pub const N_FEAT: usize = 7;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldModelParams {
    pub lr: f64,
    pub uncertainty_rate: f64,
    pub uncertainty_init: f64,
}

impl Default for WorldModelParams {
    fn default() -> Self {
        WorldModelParams { lr: 0.5, uncertainty_rate: 0.1, uncertainty_init: 0.01 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldModel {
    theta: Vec<[[f64; N_FEAT]; N_VARS]>, // [action][var][feat]
    uncertainty: [f64; N_ACTIONS],
    /// Feature vectors computed last tick, one per action.
    phis: Option<[[f64; N_FEAT]; N_ACTIONS]>,
}

pub struct WorldModelOut {
    pub predicted: [[f64; N_VARS]; N_ACTIONS],
    pub uncertainty: [f64; N_ACTIONS],
    pub prediction_error: f64,
    pub at_remembered: [f64; N_RESOURCES],
}

impl WorldModelOut {
    pub fn null(p: &WorldModelParams) -> WorldModelOut {
        WorldModelOut { predicted: [[0.0; N_VARS]; N_ACTIONS], uncertainty: [p.uncertainty_init; N_ACTIONS], prediction_error: 0.0, at_remembered: [0.0; N_RESOURCES] }
    }
}

impl WorldModel {
    pub fn new(p: &WorldModelParams) -> WorldModel {
        WorldModel {
            theta: vec![[[0.0; N_FEAT]; N_VARS]; N_ACTIONS],
            uncertainty: [p.uncertainty_init; N_ACTIONS],
            phis: None,
        }
    }

    pub fn theta(&self) -> &[[[f64; N_FEAT]; N_VARS]] {
        &self.theta
    }

    pub fn features(obs: &Observation, a: Action) -> [f64; N_FEAT] {
        let (r, c) = Observation::dest(a);
        [1.0, obs.food[r][c], obs.water[r][c], obs.ambient01[r][c], obs.v[0], obs.v[1], obs.v[2]]
    }

    fn predict_one(&self, a: Action, phi: &[f64; N_FEAT]) -> [f64; N_VARS] {
        let mut out = [0.0; N_VARS];
        for (d, slot) in out.iter_mut().enumerate() {
            *slot = dot(&self.theta[a.index()][d], phi);
        }
        out
    }

    /// Learn from the previous (features, action, realized deltas), then
    /// predict for every action from the current observation, and for
    /// consuming at the remembered targets (ambient taken from the current cell).
    pub fn step(&mut self, obs: &Observation, deltas: &Deltas, prev_action: Option<Action>, remembered: &[Remembered; N_RESOURCES], p: &WorldModelParams) -> WorldModelOut {
        let mut prediction_error = 0.0;
        if let (Some(phis), Some(a)) = (self.phis, prev_action) {
            let phi = phis[a.index()];
            let norm = 1.0 + dot(&phi, &phi);
            let predicted = self.predict_one(a, &phi);
            let mut sq = 0.0;
            for d in 0..N_VARS {
                let err = deltas.0[d] - predicted[d];
                prediction_error += err.abs();
                sq += err * err;
                let w = &mut self.theta[a.index()][d];
                for k in 0..N_FEAT {
                    w[k] += p.lr * err * phi[k] / norm;
                }
            }
            let u = &mut self.uncertainty[a.index()];
            *u += p.uncertainty_rate * (sq - *u);
        }
        let mut phis = [[0.0; N_FEAT]; N_ACTIONS];
        let mut predicted = [[0.0; N_VARS]; N_ACTIONS];
        for a in ACTIONS {
            let phi = Self::features(obs, a);
            predicted[a.index()] = self.predict_one(a, &phi);
            phis[a.index()] = phi;
        }
        self.phis = Some(phis);
        let mut at_remembered = [0.0; N_RESOURCES];
        let (cr, cc) = Observation::center();
        for (r, rem) in remembered.iter().enumerate() {
            if rem.conf > 0.0 {
                let phi = [1.0, rem.amounts[0], rem.amounts[1], obs.ambient01[cr][cc], obs.v[0], obs.v[1], obs.v[2]];
                let (a, d) = if r == 0 { (Action::Eat, 0) } else { (Action::Drink, 1) };
                at_remembered[r] = self.predict_one(a, &phi)[d];
            }
        }
        WorldModelOut { predicted, uncertainty: self.uncertainty, prediction_error, at_remembered }
    }
}

fn dot(a: &[f64; N_FEAT], b: &[f64; N_FEAT]) -> f64 {
    let mut s = 0.0;
    for k in 0..N_FEAT {
        s += a[k] * b[k];
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::organism::perception::WINDOW;

    fn obs(food_here: f64) -> Observation {
        let mut food = [[0.0; WINDOW]; WINDOW];
        food[2][2] = food_here;
        Observation { food, water: [[0.0; WINDOW]; WINDOW], ambient01: [[0.5; WINDOW]; WINDOW], v: [0.5, 0.5, 0.5], pos: (0, 0), prev_action: None }
    }

    #[test]
    fn learns_that_eating_on_food_raises_energy() {
        let p = WorldModelParams::default();
        let mut wm = WorldModel::new(&p);
        let o = obs(1.0);
        wm.step(&o, &Deltas::default(), None, &[Remembered::default(); 2], &p);
        for _ in 0..200 {
            wm.step(&o, &Deltas([0.15, -0.003, 0.0]), Some(Action::Eat), &[Remembered::default(); 2], &p);
        }
        let rem = [Remembered { target: (3, 3), dist: 5, conf: 1.0, amounts: [1.0, 0.0], toward: [false; 4] }, Remembered::default()];
        let out = wm.step(&o, &Deltas([0.15, -0.003, 0.0]), Some(Action::Eat), &rem, &p);
        assert!(out.predicted[Action::Eat.index()][0] > 0.1);
        assert!(out.at_remembered[0] > 0.1);
        assert_eq!(out.at_remembered[1], 0.0);
        assert!(out.predicted[Action::Rest.index()][0].abs() < 0.05);
        assert!(out.uncertainty[Action::Eat.index()] < p.uncertainty_init);
    }
}
