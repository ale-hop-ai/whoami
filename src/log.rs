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
    /// Per resource; conf 0 = nothing remembered.
    pub remembered: [RememberedLog; 2],
}

/// Logged remembered target: (x, y, distance, confidence, remembered amount).
pub type RememberedLog = (usize, usize, usize, f64, f64);

/// A tick as kept in memory by the experiment runner.
#[derive(Clone, Debug)]
pub struct Row {
    pub t: u64,
    pub action: Action,
    pub pos: (usize, usize),
    pub body: [f64; N_VARS],
    pub here: [f64; 2],
    pub vis: [bool; 2],
    pub errors: Option<[f64; N_VARS]>,
    pub predicted: Option<[[f64; N_VARS]; crate::body::N_ACTIONS]>,
    pub values: Option<[[f64; crate::body::N_ACTIONS]; 4]>,
    pub remembered: Option<[RememberedLog; 2]>,
}

#[derive(Clone, Debug, Default)]
pub struct Collected {
    pub rows: Vec<Row>,
    pub regimes: Vec<(u64, RegimeOp)>,
    pub dangers: Vec<(u64, usize)>,
    pub death: Option<(u64, usize)>,
}

pub struct Logger {
    out: Option<BufWriter<File>>,
    every: u64,
    collect: Option<Collected>,
}

impl Logger {
    pub fn to_file(path: &Path, every: u64) -> Result<Logger, String> {
        let f = File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
        Ok(Logger { out: Some(BufWriter::with_capacity(1 << 16, f)), every: every.max(1), collect: None })
    }

    pub fn null() -> Logger {
        Logger { out: None, every: 1, collect: None }
    }

    /// Keep every tick in memory (and also on disk if `path` is given).
    pub fn memory(path: Option<&Path>) -> Result<Logger, String> {
        let mut l = match path {
            Some(p) => Logger::to_file(p, 1)?,
            None => Logger::null(),
        };
        l.collect = Some(Collected::default());
        Ok(l)
    }

    pub fn take(&mut self) -> Collected {
        self.collect.take().unwrap_or_default()
    }

    pub fn tick_wanted(&self, t: u64) -> bool {
        self.collect.is_some() || (self.out.is_some() && t.is_multiple_of(self.every))
    }

    pub fn write(&mut self, ev: &Event) {
        if let Some(c) = &mut self.collect {
            match ev {
                Event::Tick { t, action, pos, body, here, vis, org } => c.rows.push(Row {
                    t: *t,
                    action: *action,
                    pos: *pos,
                    body: *body,
                    here: *here,
                    vis: *vis,
                    errors: org.map(|o| o.errors),
                    predicted: org.map(|o| o.predicted),
                    values: org.map(|o| [o.values[0], o.values[1], o.values[2], o.values[3]]),
                    remembered: org.map(|o| o.remembered),
                }),
                Event::Regime { t, op } => c.regimes.push((*t, *op)),
                Event::Danger { t, var } => c.dangers.push((*t, var.index())),
                Event::Death { t, cause } => c.death = Some((*t, cause.index())),
                Event::Snapshot { .. } => {}
            }
        }
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
