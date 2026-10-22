//! The whole inter-process interface. Built fresh each tick by
//! `Organism::tick` from process return values; processes read it, never
//! write it. Every field names its producer.

use serde::{Deserialize, Serialize};

use crate::body::{Action, Deltas, N_ACTIONS, N_VARS};
use crate::organism::perception::Observation;

pub const N_DRIVES: usize = 4; // Energy, Hydration, Temp, Exploration
pub const EXPLORE: usize = 3;
pub const N_RESOURCES: usize = 2;

/// Memory's answer for one resource. `conf == 0` means nothing remembered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Remembered {
    pub target: (usize, usize),
    pub dist: usize,
    pub conf: f64,
    /// Remembered food and water at `target`.
    pub amounts: [f64; N_RESOURCES],
    /// Which of the four moves reduce the distance to `target`.
    pub toward: [bool; 4],
}

#[derive(Clone, Debug)]
pub struct Signals {
    pub obs: Observation,                          // Perception
    pub deltas: Deltas,                            // Body, previous tick
    pub prev_action: Option<Action>,               // Arbitration, previous tick
    pub errors: [f64; N_VARS],                     // Homeostasis
    pub danger_events: [bool; N_VARS],             // Homeostasis, edge-triggered
    pub predicted: [[f64; N_VARS]; N_ACTIONS],     // WorldModel: Δ̂[action][var]
    pub uncertainty: [f64; N_ACTIONS],             // WorldModel
    pub prediction_error: f64,                     // WorldModel
    /// Food and water on the current cell.
    pub here: [f64; N_RESOURCES],                  // Perception
    /// Learned gain per unit of resource consumed (a body capability).
    pub yield_: [f64; N_RESOURCES],                // SelfModel
    /// The remembered amount is beyond anything ever consumed at: the price
    /// is an extrapolation and Exploration treats the target as novel (F2).
    pub novel_remembered: [bool; N_RESOURCES],     // SelfModel
    /// The amount on the current cell is beyond anything ever consumed at.
    pub novel_here: [bool; N_RESOURCES],           // SelfModel
    pub remembered: [Remembered; N_RESOURCES],     // Memory, this tick
    pub remembered_prev: [Remembered; N_RESOURCES],// Memory, previous tick (SelfModel reads this)
    pub neighbor_conf: [f64; 4],                   // Memory, per move direction
    pub ttc: [f64; N_VARS],                        // SelfModel
    pub reach: [f64; N_RESOURCES],                 // SelfModel
    pub urgency_prev: [f64; N_DRIVES],             // Drives, previous tick
}

impl Signals {
    /// Debug-build guard: no non-finite value may cross a process boundary.
    pub fn assert_finite(&self) {
        if cfg!(debug_assertions) {
            let mut all: Vec<f64> = Vec::new();
            all.extend(self.errors);
            all.extend(self.predicted.iter().flatten());
            all.extend(self.uncertainty);
            all.push(self.prediction_error);
            all.extend(self.remembered.iter().map(|r| r.conf));
            all.extend(self.neighbor_conf);
            all.extend(self.ttc);
            all.extend(self.reach);
            all.extend(self.yield_);
            all.extend(self.here);
            all.extend(self.urgency_prev);
            for x in all {
                assert!(x.is_finite(), "non-finite value in Signals");
            }
        }
    }
}
