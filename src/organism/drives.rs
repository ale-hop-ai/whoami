//! Four drives. Each sees only its own variable's error, time-to-critical
//! and reach, and the world model's predictions for its variable.
//! Two authored fragments live here and are named: F1 (move toward a
//! remembered resource, priced by the world model) and F2 (seek the least
//! known neighbor: used by the Exploration drive, and by a homeostatic drive
//! that has no remembered target). Everything else is learned or predicted.

use serde::{Deserialize, Serialize};

use crate::body::{ACTIONS, N_ACTIONS};
use crate::signals::{EXPLORE, N_DRIVES, Signals};
#[cfg(test)]
use crate::body::Action;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DriveParams {
    /// Gain increment on an edge-triggered danger event for the drive's variable.
    pub gain_eta: f64,
    /// Ticks for the gain to relax toward 1.
    pub gain_tau: f64,
    pub gain_max: f64,
    /// Habit increment on credited success.
    pub habit_eta: f64,
    /// Ticks for habits to decay.
    pub habit_tau: f64,
    /// Anticipation weight per drive (Temp and Exploration are 0).
    pub lambda: [f64; N_DRIVES],
    /// F1 distance discount per cell.
    pub gamma: f64,
    /// Exploration urgency = explore_scale * sqrt(mean uncertainty).
    pub explore_scale: f64,
    /// F2 weights: unknown-neighbor term and model-uncertainty term.
    pub explore_unknown: f64,
    pub explore_uncertainty: f64,
    /// F2 for a homeostatic drive with no remembered target: value of moving
    /// toward the least-known neighbor, scaled by that drive's own urgency.
    pub seek_unknown: f64,
}

impl Default for DriveParams {
    fn default() -> Self {
        DriveParams {
            gain_eta: 0.2,
            gain_tau: 20_000.0,
            gain_max: 5.0,
            habit_eta: 0.05,
            habit_tau: 50_000.0,
            lambda: [0.5, 0.5, 0.0, 0.0],
            gamma: 0.97,
            explore_scale: 0.2,
            explore_unknown: 0.3,
            explore_uncertainty: 0.5,
            seek_unknown: 0.05,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Drives {
    gain: [f64; N_DRIVES],
    habit: [[f64; N_ACTIONS]; N_DRIVES],
    prev_bids: Option<[[f64; N_ACTIONS]; N_DRIVES]>,
    /// |error| per drive last tick (mean uncertainty for Exploration).
    prev_abs: [f64; N_DRIVES],
}

pub struct DrivesOut {
    pub bids: [[f64; N_ACTIONS]; N_DRIVES],
    pub values: [[f64; N_ACTIONS]; N_DRIVES],
    pub urgency: [f64; N_DRIVES],
}

impl Default for Drives {
    fn default() -> Self {
        Drives { gain: [1.0; N_DRIVES], habit: [[0.0; N_ACTIONS]; N_DRIVES], prev_bids: None, prev_abs: [0.0; N_DRIVES] }
    }
}

impl Drives {
    pub fn gains(&self) -> [f64; N_DRIVES] {
        self.gain
    }

    pub fn habits(&self) -> &[[f64; N_ACTIONS]; N_DRIVES] {
        &self.habit
    }

    /// `adapt` false freezes gains and habits (the drive-adaptation lesion).
    /// `explore` false zeroes the Exploration drive (the exploration lesion).
    pub fn step(&mut self, s: &Signals, p: &DriveParams, adapt: bool, explore: bool) -> DrivesOut {
        let mean_unc = s.uncertainty.iter().sum::<f64>() / N_ACTIONS as f64;
        let mut abs_now = [0.0; N_DRIVES];
        for d in 0..3 {
            abs_now[d] = s.errors[d].abs();
        }
        abs_now[EXPLORE] = mean_unc;

        if adapt {
            // Habit credit: the drive that contributed most to the previous
            // action's bid, if its own error then decreased.
            if let (Some(a), Some(pb)) = (s.prev_action, self.prev_bids) {
                let ai = a.index();
                let mut top = 0;
                for d in 1..N_DRIVES {
                    if pb[d][ai] > pb[top][ai] {
                        top = d;
                    }
                }
                if pb[top][ai] > 0.0 && abs_now[top] < self.prev_abs[top] {
                    let h = &mut self.habit[top][ai];
                    *h += p.habit_eta * (1.0 - *h);
                }
            }
            for d in 0..N_DRIVES {
                for a in 0..N_ACTIONS {
                    self.habit[d][a] -= self.habit[d][a] / p.habit_tau;
                }
                self.gain[d] += (1.0 - self.gain[d]) / p.gain_tau;
            }
            for d in 0..3 {
                if s.danger_events[d] {
                    self.gain[d] = (self.gain[d] + p.gain_eta).min(p.gain_max);
                }
            }
        }

        let mut urgency = [0.0; N_DRIVES];
        for d in 0..3 {
            let e = s.errors[d];
            let mut u = e * e.abs();
            if p.lambda[d] > 0.0 {
                let r = d; // resource index equals drive index for Energy/Hydration
                let anticipation = (1.0 - s.ttc[d] / s.reach[r]).max(0.0);
                u += p.lambda[d] * anticipation;
            }
            urgency[d] = u;
        }
        urgency[EXPLORE] = if explore { p.explore_scale * mean_unc.sqrt() } else { 0.0 };

        let mut values = [[0.0; N_ACTIONS]; N_DRIVES];
        for a in ACTIONS {
            let ai = a.index();
            for d in 0..3 {
                values[d][ai] = s.predicted[ai][d];
            }
            if a.is_move() {
                // F1: value of moving toward a remembered resource, priced by
                // the world model's predicted gain of consuming there.
                for r in 0..2 {
                    let rem = &s.remembered[r];
                    if rem.conf > 0.0 {
                        if rem.toward[ai] {
                            values[r][ai] = rem.conf * s.at_remembered[r] * pow_int(p.gamma, rem.dist);
                        }
                    } else {
                        // F2 for a drive that has lost its target: seek the unknown.
                        values[r][ai] = p.seek_unknown * (1.0 - s.neighbor_conf[ai]);
                    }
                }
                // F2: explore toward unknown cells.
                values[EXPLORE][ai] = p.explore_unknown * (1.0 - s.neighbor_conf[ai]);
            }
            values[EXPLORE][ai] += p.explore_uncertainty * s.uncertainty[ai] / (mean_unc + 1e-12);
            if !explore {
                values[EXPLORE][ai] = 0.0;
            }
        }

        let mut bids = [[0.0; N_ACTIONS]; N_DRIVES];
        for d in 0..N_DRIVES {
            for a in 0..N_ACTIONS {
                bids[d][a] = self.gain[d] * urgency[d] * values[d][a] * (1.0 + self.habit[d][a]);
            }
        }
        self.prev_bids = Some(bids);
        self.prev_abs = abs_now;
        DrivesOut { bids, values, urgency }
    }
}

/// gamma^n by repeated multiplication: no libm, bit-identical everywhere.
fn pow_int(base: f64, n: usize) -> f64 {
    let mut r = 1.0;
    for _ in 0..n {
        r *= base;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Deltas;
    use crate::organism::perception::{Observation, WINDOW};
    use crate::signals::Remembered;

    fn signals() -> Signals {
        Signals {
            obs: Observation { food: [[0.0; WINDOW]; WINDOW], water: [[0.0; WINDOW]; WINDOW], ambient01: [[0.5; WINDOW]; WINDOW], v: [0.5; 3], pos: (8, 8), prev_action: None },
            deltas: Deltas::default(),
            prev_action: None,
            errors: [0.5, 0.0, 0.0],
            danger_events: [false; 3],
            predicted: [[0.0; 3]; N_ACTIONS],
            uncertainty: [0.01; N_ACTIONS],
            prediction_error: 0.0,
            remembered: [Remembered::default(); 2],
            remembered_prev: [Remembered::default(); 2],
            neighbor_conf: [0.0; 4],
            ttc: [1000.0; 3],
            reach: [20_000.0; 2],
            at_remembered: [0.15; 2],
            urgency_prev: [0.0; N_DRIVES],
        }
    }

    #[test]
    fn hungry_drive_bids_for_predicted_energy_gain() {
        let p = DriveParams::default();
        let mut d = Drives::default();
        let mut s = signals();
        s.predicted[Action::Eat.index()][0] = 0.1;
        let out = d.step(&s, &p, true, true);
        assert!(out.bids[0][Action::Eat.index()] > out.bids[0][Action::Rest.index()]);
    }

    #[test]
    fn f1_moves_toward_remembered_food() {
        let p = DriveParams::default();
        let mut d = Drives::default();
        let mut s = signals();
        s.remembered[0] = Remembered { target: (8, 3), dist: 5, conf: 1.0, amounts: [1.0, 0.0], toward: [true, false, false, false] };
        let out = d.step(&s, &p, true, true);
        assert!(out.bids[0][Action::MoveN.index()] > 0.0);
        assert_eq!(out.bids[0][Action::MoveS.index()], 0.0);
    }

    #[test]
    fn gain_rises_on_danger_and_relaxes() {
        let p = DriveParams { gain_tau: 10.0, ..DriveParams::default() };
        let mut d = Drives::default();
        let mut s = signals();
        s.danger_events[0] = true;
        d.step(&s, &p, true, true);
        assert!(d.gains()[0] > 1.0);
        s.danger_events[0] = false;
        for _ in 0..200 {
            d.step(&s, &p, true, true);
        }
        assert!((d.gains()[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn habit_credits_only_on_success() {
        let p = DriveParams::default();
        let mut d = Drives::default();
        let mut s = signals();
        s.predicted[Action::Eat.index()][0] = 0.1;
        d.step(&s, &p, true, true);
        s.prev_action = Some(Action::Eat);
        s.errors[0] = 0.6; // got worse
        d.step(&s, &p, true, true);
        assert_eq!(d.habits()[0][Action::Eat.index()], 0.0);
        s.errors[0] = 0.3; // got better
        d.step(&s, &p, true, true);
        assert!(d.habits()[0][Action::Eat.index()] > 0.0);
    }

    #[test]
    fn frozen_drives_do_not_adapt() {
        let p = DriveParams::default();
        let mut d = Drives::default();
        let mut s = signals();
        s.danger_events[0] = true;
        d.step(&s, &p, false, true);
        assert_eq!(d.gains()[0], 1.0);
    }
}
