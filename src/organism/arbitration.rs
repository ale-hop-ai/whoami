//! Content-free action selection: bids in, action out. Never sees state.

use serde::{Deserialize, Serialize};

use crate::body::{ACTIONS, Action, N_ACTIONS};
use crate::signals::N_DRIVES;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ArbitrationParams {
    /// Bonus added to the previous action's summed bid.
    pub persistence: f64,
    /// The drive that contributed most to the previous winning action has
    /// its bids multiplied by (1 + drive_persistence) this tick. Hysteresis
    /// at the motivation level, so a commute is not abandoned halfway
    /// (amendment 2026-10-22: without it the organism dithered between two
    /// distant patches and starved between them).
    pub drive_persistence: f64,
}

impl Default for ArbitrationParams {
    fn default() -> Self {
        ArbitrationParams { persistence: 0.002, drive_persistence: 0.5 }
    }
}

/// Content-free: sees bids and its own record of which drive row won.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WinnerTakeAll {
    last_winner: Option<usize>,
}

impl WinnerTakeAll {
    pub fn last_winner(&self) -> Option<usize> {
        self.last_winner
    }

    pub fn select(&mut self, bids: &[[f64; N_ACTIONS]; N_DRIVES], prev: Option<Action>, p: &ArbitrationParams) -> Action {
        let mut best = ACTIONS[0];
        let mut best_score = f64::NEG_INFINITY;
        for a in ACTIONS {
            let mut s = 0.0;
            for (d, row) in bids.iter().enumerate() {
                let w = if self.last_winner == Some(d) { 1.0 + p.drive_persistence } else { 1.0 };
                s += w * row[a.index()];
            }
            if prev == Some(a) {
                s += p.persistence;
            }
            if s > best_score {
                best_score = s;
                best = a;
            }
        }
        // Record the largest positive contributor to the chosen action.
        let ai = best.index();
        let mut top: Option<usize> = None;
        for d in 0..N_DRIVES {
            if bids[d][ai] > 0.0 && top.is_none_or(|t| bids[d][ai] > bids[t][ai]) {
                top = Some(d);
            }
        }
        self.last_winner = top;
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_highest_sum_and_breaks_ties_by_order() {
        let p = ArbitrationParams { persistence: 0.0, drive_persistence: 0.0 };
        let mut w = WinnerTakeAll::default();
        let mut bids = [[0.0; N_ACTIONS]; N_DRIVES];
        bids[0][Action::Eat.index()] = 0.3;
        bids[1][Action::Drink.index()] = 0.2;
        bids[2][Action::Drink.index()] = 0.2;
        assert_eq!(w.select(&bids, None, &p), Action::Drink);
        let zero = [[0.0; N_ACTIONS]; N_DRIVES];
        assert_eq!(w.select(&zero, None, &p), Action::MoveN);
    }

    #[test]
    fn persistence_breaks_near_ties() {
        let p = ArbitrationParams { persistence: 0.01, drive_persistence: 0.0 };
        let mut w = WinnerTakeAll::default();
        let mut bids = [[0.0; N_ACTIONS]; N_DRIVES];
        bids[0][Action::Eat.index()] = 0.105;
        bids[0][Action::Drink.index()] = 0.1;
        assert_eq!(w.select(&bids, Some(Action::Drink), &p), Action::Drink);
    }

    #[test]
    fn winning_drive_keeps_the_floor_until_clearly_outbid() {
        let p = ArbitrationParams { persistence: 0.0, drive_persistence: 0.5 };
        let mut w = WinnerTakeAll::default();
        let mut bids = [[0.0; N_ACTIONS]; N_DRIVES];
        bids[0][Action::MoveN.index()] = 0.10; // energy drive heads north
        bids[1][Action::MoveS.index()] = 0.08;
        assert_eq!(w.select(&bids, None, &p), Action::MoveN);
        assert_eq!(w.last_winner(), Some(0));
        bids[1][Action::MoveS.index()] = 0.12; // hydration now slightly higher
        assert_eq!(w.select(&bids, None, &p), Action::MoveN); // 0.15 vs 0.12: keeps going
        bids[1][Action::MoveS.index()] = 0.20;
        assert_eq!(w.select(&bids, None, &p), Action::MoveS); // clearly outbid
        assert_eq!(w.last_winner(), Some(1));
    }
}
