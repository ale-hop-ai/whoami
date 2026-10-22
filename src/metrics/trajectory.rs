//! Reading JSONL logs back into rows and computing per-window statistics.

use serde::Deserialize;
use std::path::Path;

use crate::body::{Action, N_ACTIONS, N_VARS};
use crate::env::RegimeOp;

#[derive(Clone, Debug)]
pub struct Row {
    pub t: u64,
    pub action: Action,
    pub pos: (usize, usize),
    pub body: [f64; N_VARS],
    pub here: [f64; 2],
    pub vis: [bool; 2],
    pub errors: Option<[f64; N_VARS]>,
    pub predicted: Option<[[f64; N_VARS]; N_ACTIONS]>,
    pub remembered: Option<[(usize, usize, usize, f64); 2]>,
}

#[derive(Clone, Debug)]
pub struct Trajectory {
    pub rows: Vec<Row>,
    pub regimes: Vec<(u64, RegimeOp)>,
    pub dangers: Vec<(u64, usize)>,
    pub death: Option<(u64, usize)>,
}

#[derive(Deserialize)]
struct OrgIn {
    errors: [f64; N_VARS],
    predicted: [[f64; N_VARS]; N_ACTIONS],
    remembered: [(usize, usize, usize, f64); 2],
}

#[derive(Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
enum EventIn {
    Tick { t: u64, action: Action, pos: (usize, usize), body: [f64; N_VARS], here: [f64; 2], vis: [bool; 2], org: Option<Box<OrgIn>> },
    Regime { t: u64, op: RegimeOp },
    Danger { t: u64, var: crate::body::Var },
    Death { t: u64, cause: crate::body::Var },
    Snapshot {},
}

impl Trajectory {
    pub fn load(path: &Path) -> Result<Trajectory, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Trajectory, String> {
        let mut tr = Trajectory { rows: Vec::new(), regimes: Vec::new(), dangers: Vec::new(), death: None };
        for (i, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let ev: EventIn = serde_json::from_str(line).map_err(|e| format!("line {}: {e}", i + 1))?;
            match ev {
                EventIn::Tick { t, action, pos, body, here, vis, org } => {
                    let (errors, predicted, remembered) = match org {
                        Some(o) => (Some(o.errors), Some(o.predicted), Some(o.remembered)),
                        None => (None, None, None),
                    };
                    tr.rows.push(Row { t, action, pos, body, here, vis, errors, predicted, remembered });
                }
                EventIn::Regime { t, op } => tr.regimes.push((t, op)),
                EventIn::Danger { t, var } => tr.dangers.push((t, var.index())),
                EventIn::Death { t, cause } => tr.death = Some((t, cause.index())),
                EventIn::Snapshot {} => {}
            }
        }
        Ok(tr)
    }

    pub fn ticks_alive(&self) -> u64 {
        match self.death {
            Some((t, _)) => t.saturating_sub(1),
            None => self.rows.len() as u64,
        }
    }

    /// Consecutive windows of `w` rows (the last partial window is dropped).
    pub fn windows(&self, w: usize) -> Vec<&[Row]> {
        self.rows.chunks_exact(w).collect()
    }
}

/// Per-window scalar metrics from docs/03-experiments.md §1.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct WindowStats {
    pub start: u64,
    pub margin: f64,
    pub viability: f64,
    pub operating_point: [f64; N_VARS],
    pub intake_per_tick: f64,
    pub moves_per_intake: f64,
    pub anticipation_rate: f64,
    pub action_entropy: f64,
    pub conditional_entropy: f64,
    pub prediction_mse: Option<f64>,
}

/// `setpoint`/`width` are the homeostasis parameters, needed for viability
/// and anticipation. `prev` is the row before the window (for the first delta).
pub fn window_stats(rows: &[Row], prev: Option<&Row>, setpoint: &[f64; N_VARS], width: &[f64; N_VARS]) -> WindowStats {
    let n = rows.len() as f64;
    if rows.is_empty() {
        return WindowStats::default();
    }
    let mut margin = 0.0;
    let mut viable = 0.0;
    let mut op = [0.0; N_VARS];
    let mut intake = 0.0;
    let mut moves = 0.0;
    let mut consume = 0.0;
    let mut anticip = 0.0;
    let mut mse_sum = 0.0;
    let mut mse_n = 0.0;
    let mut last = prev;
    for r in rows {
        let m = r.body[0].min(r.body[1]).min(r.body[2].min(1.0 - r.body[2]));
        margin += m;
        let ok = (0..N_VARS).all(|d| ((setpoint[d] - r.body[d]) / width[d]).abs() < 1.0);
        viable += ok as u8 as f64;
        for d in 0..N_VARS {
            op[d] += r.body[d];
        }
        if r.action.is_move() {
            moves += 1.0;
        }
        if let Some(p) = last {
            let delta = [r.body[0] - p.body[0], r.body[1] - p.body[1], r.body[2] - p.body[2]];
            match r.action {
                Action::Eat | Action::Drink => {
                    let d = if r.action == Action::Eat { 0 } else { 1 };
                    intake += delta[d].max(0.0);
                    consume += 1.0;
                    // Anticipation: consuming while the error is still small. Uses the
                    // state before the action.
                    if ((setpoint[d] - p.body[d]) / width[d]).abs() < 0.5 {
                        anticip += 1.0;
                    }
                }
                _ => {}
            }
            if let Some(pred) = p.predicted {
                let row = pred[r.action.index()];
                for d in 0..N_VARS {
                    mse_sum += (row[d] - delta[d]) * (row[d] - delta[d]);
                }
                mse_n += N_VARS as f64;
            }
        }
        last = Some(r);
    }
    let fp = crate::metrics::Fingerprint::from_rows(rows);
    WindowStats {
        start: rows[0].t,
        margin: margin / n,
        viability: viable / n,
        operating_point: [op[0] / n, op[1] / n, op[2] / n],
        intake_per_tick: intake / n,
        moves_per_intake: if intake > 0.0 { moves / intake } else { f64::INFINITY },
        anticipation_rate: if consume > 0.0 { anticip / consume } else { 0.0 },
        action_entropy: fp.action_entropy(),
        conditional_entropy: fp.conditional_entropy(),
        prediction_mse: if mse_n > 0.0 { Some(mse_sum / mse_n) } else { None },
    }
}

/// Adaptation speed after a regime shift at `shift`: ticks until the rolling
/// viability (window `w`) returns to `fraction` of its pre-shift mean.
/// `None` if it never does before `limit` ticks or the trajectory ends.
pub fn adaptation_speed(tr: &Trajectory, shift: u64, w: usize, fraction: f64, limit: u64, setpoint: &[f64; N_VARS], width: &[f64; N_VARS]) -> Option<u64> {
    let idx = tr.rows.iter().position(|r| r.t >= shift)?;
    if idx < w {
        return None;
    }
    let before = window_stats(&tr.rows[idx - w..idx], None, setpoint, width).viability;
    let target = fraction * before;
    let mut i = idx;
    while i + w <= tr.rows.len() && (tr.rows[i].t - shift) <= limit {
        let v = window_stats(&tr.rows[i..i + w], None, setpoint, width).viability;
        if v >= target {
            return Some(tr.rows[i].t - shift);
        }
        i += (w / 4).max(1);
    }
    None
}
