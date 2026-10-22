//! Reproducible experiments: configuration, arms, seeds, reduction, report.

pub mod baselines;
pub mod e1;
pub mod families;
pub mod report;
pub mod runner;

use std::path::Path;

use crate::config::Config;
use crate::experiment::families::Family;
use crate::experiment::report::Report;
use crate::experiment::runner::{Arm, ArmKind, RunOpts, SeedResult, run_seed};
use crate::metrics::median;
use crate::organism::Organism;
use crate::AgentKind;

/// Shared settings for one experiment invocation.
#[derive(Clone, Debug)]
pub struct Context {
    pub seeds: usize,
    pub ticks: u64,
    pub window: usize,
    pub config_hash: u64,
    pub commit: String,
    /// First held-out environment seed (development seeds are below it).
    pub held_out_base: u64,
}

impl Context {
    pub fn describe(&self, report: &mut Report) {
        report.config("config_hash", format!("{:016x}", self.config_hash));
        report.config("commit", &self.commit);
        report.config("seeds", self.seeds);
        report.config("ticks", self.ticks);
        report.config("window", self.window);
        report.config("held_out_env_seeds_from", self.held_out_base);
    }
}

pub fn held_out_env_seed(ctx: &Context, i: usize) -> u64 {
    ctx.held_out_base + i as u64
}

/// The Frozen baseline: one organism developed in `default`, development
/// seed 0, for 10 000 ticks, then frozen. Deterministic, so every experiment
/// gets the same one.
pub fn pretrain_frozen(cfg: &Config, ctx: &Context) -> Result<Organism, String> {
    let arm = Arm::agent("pretrain", AgentKind::Epi);
    let opts = RunOpts { ticks: 10_000, window: ctx.window, log_path: None, keep_trajectory: false };
    let (r, org) = run_seed(cfg, Family::Default, &arm, 0, 0, &opts)?;
    let mut org = org.ok_or("pretraining produced no organism")?;
    if !r.alive {
        return Err(format!("Frozen pretraining died at tick {}", r.ticks_alive));
    }
    org.set_learning(false);
    Ok(org)
}

pub struct Summary {
    pub margin: f64,
    pub intake: f64,
    pub ticks_alive: f64,
    pub survived: usize,
    pub anticipation: f64,
    pub cond_entropy: f64,
    pub dangers: f64,
}

pub fn summarize(rs: &[&SeedResult]) -> Summary {
    let mut m: Vec<f64> = rs.iter().map(|r| r.margin).collect();
    let mut i: Vec<f64> = rs.iter().map(|r| r.intake).collect();
    let mut t: Vec<f64> = rs.iter().map(|r| r.ticks_alive as f64).collect();
    let mut a: Vec<f64> = rs.iter().map(|r| r.anticipation).collect();
    let mut c: Vec<f64> = rs.iter().map(|r| r.conditional_entropy).collect();
    let mut d: Vec<f64> = rs.iter().map(|r| r.dangers as f64).collect();
    Summary {
        margin: median(&mut m),
        intake: median(&mut i),
        ticks_alive: median(&mut t),
        survived: rs.iter().filter(|r| r.alive).count(),
        anticipation: median(&mut a),
        cond_entropy: median(&mut c),
        dangers: median(&mut d),
    }
}

pub fn run_named(name: &str, cfg: &Config, ctx: &Context, out: &Path) -> Result<Report, String> {
    std::fs::create_dir_all(out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
    let report = match name {
        "e1" => e1::run(cfg, ctx, out)?,
        other => return Err(format!("unknown experiment '{other}' (have: e1)")),
    };
    std::fs::write(out.join("report.md"), report.markdown()).map_err(|e| e.to_string())?;
    Ok(report)
}

pub fn git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

#[allow(dead_code)]
fn _arm_kind_is_used(_: ArmKind) {}
