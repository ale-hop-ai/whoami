//! Structured JSONL event log. One line per event.

use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::body::{Action, Var, N_VARS};
use crate::env::RegimeOp;

#[derive(Serialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum Event<'a> {
    Tick {
        t: u64,
        action: Action,
        pos: (usize, usize),
        body: [f64; N_VARS],
        /// Food and water on the current cell.
        here: [f64; 2],
        /// Food / water visible anywhere in the perception window.
        vis: [bool; 2],
        #[serde(skip_serializing_if = "Option::is_none")]
        org: Option<&'a OrgTick>,
    },
    Regime { t: u64, op: RegimeOp },
    Danger { t: u64, var: Var },
    Death { t: u64, cause: Var },
    Snapshot { t: u64, path: String },
}

/// Organism internals logged per tick. Filled in from M2 onward.
#[derive(Serialize, Default, Clone, Debug)]
pub struct OrgTick {
    pub errors: [f64; N_VARS],
    pub bids: Vec<[f64; crate::body::N_ACTIONS]>,
    pub values: Vec<[f64; crate::body::N_ACTIONS]>,
    pub predicted: [[f64; N_VARS]; crate::body::N_ACTIONS],
    pub prediction_error: f64,
    pub gains: Vec<f64>,
    /// Per resource: remembered target (x, y, dist, conf); conf 0 = nothing.
    pub remembered: [(usize, usize, usize, f64); 2],
}

pub struct Logger {
    out: Option<BufWriter<File>>,
    every: u64,
}

impl Logger {
    pub fn to_file(path: &Path, every: u64) -> Result<Logger, String> {
        let f = File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
        Ok(Logger { out: Some(BufWriter::with_capacity(1 << 16, f)), every: every.max(1) })
    }

    pub fn null() -> Logger {
        Logger { out: None, every: 1 }
    }

    pub fn tick_wanted(&self, t: u64) -> bool {
        self.out.is_some() && t.is_multiple_of(self.every)
    }

    pub fn write(&mut self, ev: &Event) {
        if let Some(out) = &mut self.out {
            // Tick events honour the sampling interval; everything else is always written.
            if let Event::Tick { t, .. } = ev
                && !t.is_multiple_of(self.every)
            {
                return;
            }
            serde_json::to_writer(&mut *out, ev).expect("serialize event");
            out.write_all(b"\n").expect("write event");
        }
    }

    pub fn flush(&mut self) {
        if let Some(out) = &mut self.out {
            out.flush().expect("flush log");
        }
    }
}
