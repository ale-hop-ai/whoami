//! Content-free action selection: bids in, action out. Never sees state.

use serde::{Deserialize, Serialize};

use crate::body::{ACTIONS, Action, N_ACTIONS};
use crate::signals::N_DRIVES;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ArbitrationParams {
    /// Bonus added to the previous action's summed bid.
    pub persistence: f64,
}

impl Default for ArbitrationParams {
    fn default() -> Self {
        ArbitrationParams { persistence: 0.002 }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WinnerTakeAll;

impl WinnerTakeAll {
    pub fn select(&self, bids: &[[f64; N_ACTIONS]; N_DRIVES], prev: Option<Action>, p: &ArbitrationParams) -> Action {
        let mut best = ACTIONS[0];
        let mut best_score = f64::NEG_INFINITY;
        for a in ACTIONS {
            let mut s: f64 = bids.iter().map(|row| row[a.index()]).sum();
            if prev == Some(a) {
                s += p.persistence;
            }
            if s > best_score {
                best_score = s;
                best = a;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_highest_sum_and_breaks_ties_by_order() {
        let p = ArbitrationParams { persistence: 0.0 };
        let mut bids = [[0.0; N_ACTIONS]; N_DRIVES];
        bids[0][Action::Eat.index()] = 0.3;
        bids[1][Action::Drink.index()] = 0.2;
        bids[2][Action::Drink.index()] = 0.2;
        assert_eq!(WinnerTakeAll.select(&bids, None, &p), Action::Drink);
        let zero = [[0.0; N_ACTIONS]; N_DRIVES];
        assert_eq!(WinnerTakeAll.select(&zero, None, &p), Action::MoveN);
    }

    #[test]
    fn persistence_breaks_near_ties() {
        let p = ArbitrationParams { persistence: 0.01 };
        let mut bids = [[0.0; N_ACTIONS]; N_DRIVES];
        bids[0][Action::Eat.index()] = 0.105;
        bids[0][Action::Drink.index()] = 0.1;
        assert_eq!(WinnerTakeAll.select(&bids, Some(Action::Drink), &p), Action::Drink);
    }
}
