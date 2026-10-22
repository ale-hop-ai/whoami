//! E3 — Forking. Two copies of one snapshot take different environment paths.

use std::path::Path;

use crate::config::Config;
use crate::experiment::families::Family;
use crate::experiment::report::{Report, Table, Verdict, f3, opt3};
use crate::experiment::runner::{Arm, RunId, RunOpts, SeedResult, reduce};
use crate::experiment::{Context, pretrain_frozen};
use crate::hash::Fnv;
use crate::log::Logger;
use crate::metrics::{Fingerprint, Trajectory, kinship, median, pooled_occupancy};
use crate::organism::Organism;
use crate::sim::{self, Streams};
use crate::{AgentKind, Session, start_position};

pub const DEV_TICKS: u64 = 5_000;
pub const FORK_TICKS: u64 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForkArm {
    Epi,
    Frozen,
    BodyOnly,
}

impl ForkArm {
    fn name(self) -> &'static str {
        match self {
            ForkArm::Epi => "epi",
            ForkArm::Frozen => "frozen",
            ForkArm::BodyOnly => "body_only",
        }
    }
}

struct Pair {
    a: Vec<Fingerprint>,
    b: Vec<Fingerprint>,
}

fn run_session(mut s: Session, cfg: &Config, ticks: u64, window: usize, label: &str) -> Result<(SeedResult, Session), String> {
    let mut log = Logger::memory(None)?;
    let r = s.run(ticks, &mut log);
    let tr = Trajectory::from_collected(log.take());
    let arm = Arm::agent(label, AgentKind::Epi);
    let opts = RunOpts { ticks, window, log_path: None, keep_trajectory: false };
    let res = reduce(&tr, cfg, &RunId { family: Family::Default, arm: &arm, env_seed: 0, org_seed: 0 }, &r, &opts);
    Ok((res, s))
}

fn fork_pair(cfg: &Config, ctx: &Context, arm: ForkArm, i: usize, frozen: &Organism) -> Result<Option<Pair>, String> {
    let env_seed = ctx.held_out_base + i as u64;
    let streams = Streams::new(env_seed, i as u64);
    let session = match arm {
        ForkArm::Frozen => Session::adopt(cfg, ctx.config_hash, frozen.clone(), streams),
        _ => Session::with_streams(cfg, ctx.config_hash, streams),
    };
    let (dev, session) = run_session(session, cfg, DEV_TICKS, ctx.window, "dev")?;
    if !dev.alive {
        return Ok(None);
    }
    let snap = session.snapshot();
    let mut fps = Vec::new();
    for (k, env_seed) in [ctx.held_out_base + 200 + 2 * i as u64, ctx.held_out_base + 201 + 2 * i as u64].into_iter().enumerate() {
        let mut s = Session::from_snapshot(snap.clone());
        s.reseed_env(env_seed);
        if arm == ForkArm::BodyOnly {
            let fresh = Organism::body_only_from(&s.organism, &s.world);
            s.organism = fresh;
        }
        let (r, _) = run_session(s, cfg, FORK_TICKS, ctx.window, if k == 0 { "fork_a" } else { "fork_b" })?;
        fps.push(r.fingerprints);
    }
    let b = fps.pop().unwrap();
    let a = fps.pop().unwrap();
    Ok(Some(Pair { a, b }))
}

/// Median kinship per window across the pairs alive in that window, plus
/// the per-organism curves (None where a fork is no longer alive or fewer
/// than two strangers remain).
fn kinship_curves(pairs: &[Pair]) -> (Vec<Option<f64>>, Vec<Vec<Option<f64>>>) {
    let k = pairs.iter().map(|p| p.a.len().min(p.b.len())).max().unwrap_or(0);
    let all: Vec<&Fingerprint> = pairs.iter().flat_map(|p| p.a.iter().chain(p.b.iter())).collect();
    let weights = pooled_occupancy(&all);
    let mut per: Vec<Vec<Option<f64>>> = vec![Vec::new(); pairs.len()];
    let mut medians = Vec::new();
    for w in 0..k {
        let mut col = Vec::new();
        for (i, p) in pairs.iter().enumerate() {
            let alive = p.a.len() > w && p.b.len() > w;
            let strangers: Vec<&Fingerprint> = pairs.iter().enumerate().filter(|(j, o)| *j != i && o.a.len() > w).map(|(_, o)| &o.a[w]).collect();
            let kv = if alive && strangers.len() >= 2 { kinship(&p.a[w], &p.b[w], &strangers, &weights) } else { None };
            per[i].push(kv);
            if let Some(v) = kv {
                col.push(v);
            }
        }
        medians.push(if col.is_empty() { None } else { Some(median(&mut col)) });
    }
    (medians, per)
}

/// One-sided sign test that consecutive kinship differences are ≥ 0:
/// returns (ups, downs, p-value of seeing at most `downs` decreases under p = 0.5).
fn sign_test(per: &[Vec<Option<f64>>]) -> (usize, usize, f64) {
    let mut ups = 0usize;
    let mut downs = 0usize;
    for curve in per {
        for pair in curve.windows(2) {
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                if b > a { ups += 1 } else if b < a { downs += 1 }
            }
        }
    }
    let n = ups + downs;
    let p = if n == 0 { 1.0 } else { binom_cdf(downs, n) };
    (ups, downs, p)
}

fn binom_cdf(k: usize, n: usize) -> f64 {
    let mut p = 0.0;
    for i in 0..=k {
        p += choose(n, i) * 0.5f64.powi(n as i32);
    }
    p.min(1.0)
}

fn choose(n: usize, k: usize) -> f64 {
    let mut r = 1.0;
    for i in 0..k {
        r *= (n - i) as f64 / (i + 1) as f64;
    }
    r
}

pub fn run(cfg: &Config, ctx: &Context, out: &Path) -> Result<Report, String> {
    let frozen = pretrain_frozen(cfg, ctx)?;
    let mut report = Report::new("E3 — Forking");
    ctx.describe(&mut report);
    report.config("protocol", format!("{} organisms, {} dev ticks in default family (held-out env seeds), snapshot, two forks with distinct env seeds, {} ticks each; strangers = other organisms' fork A at the same window", ctx.seeds, DEV_TICKS, FORK_TICKS));
    let mut obs = String::new();
    let mut summary = Vec::new();
    let mut csv = String::from("arm,organism,window,kinship\n");
    for arm in [ForkArm::Epi, ForkArm::Frozen, ForkArm::BodyOnly] {
        let mut pairs = Vec::new();
        let mut died = 0;
        for i in 0..ctx.seeds {
            match fork_pair(cfg, ctx, arm, i, &frozen)? {
                Some(p) => pairs.push(p),
                None => died += 1,
            }
            eprintln!("e3 {} organism {i}: {}", arm.name(), if pairs.len() + died == i + 1 { "ok" } else { "?" });
        }
        let (medians, per) = kinship_curves(&pairs);
        for (i, curve) in per.iter().enumerate() {
            for (w, k) in curve.iter().enumerate() {
                if let Some(k) = k {
                    csv.push_str(&format!("{},{},{},{:.4}\n", arm.name(), i, w, k));
                }
            }
        }
        let (ups, downs, p) = sign_test(&per);
        let mut t = Table::new(&["window (Δ ticks)", "median K", "pairs alive"]);
        for (w, m) in medians.iter().enumerate() {
            let alive = per.iter().filter(|c| c[w].is_some()).count();
            t.row(vec![format!("{}", (w + 1) * ctx.window), opt3(*m), alive.to_string()]);
        }
        obs.push_str(&format!("### Arm `{}` ({} pairs, {} died in development)\n\n{}\nSign test on consecutive ΔK: {ups} increases, {downs} decreases, p(≤{downs} decreases | fair) = {}.\n\n", arm.name(), pairs.len(), died, t.markdown(), f3(p)));
        summary.push((arm, medians, p));
    }
    std::fs::write(out.join("kinship.csv"), csv).map_err(|e| e.to_string())?;
    report.observations = obs;

    let epi = &summary[0].1;
    let body = &summary[2].1;
    let first = epi.first().copied().flatten();
    let early_ok = epi.iter().take(4).all(|k| k.is_some_and(|v| v < 0.7));
    let below_body = epi.iter().zip(body).all(|(e, b)| match (e, b) { (Some(e), Some(b)) => e < b, _ => false });
    let p = summary[0].2;
    let v = if first.is_some_and(|k| k >= 0.9) || (epi.iter().zip(body).all(|(e, b)| e == b) && !epi.is_empty()) {
        Verdict::Refuted
    } else if early_ok && below_body && p < 0.05 {
        Verdict::Supported
    } else {
        Verdict::Inconclusive
    };
    report.verdict("H4", v, format!("pre-registered: EPI median K < 0.7 for Δ ≤ 2000, EPI K below body-only K at every window, non-decreasing K by sign test p < 0.05; measured K(W) {}, early windows < 0.7: {early_ok}, below body-only everywhere: {below_body}, sign test p {}", opt3(first), f3(p)));
    report.interpretation = "Kinship below 1 means two forks of one snapshot behave more alike than two unrelated organisms of the same age. The body-only arm carries position and physiology but no learned state; if EPI forks are closer than body-only forks, the shared resemblance is carried by what was learned. The frozen arm has no learning before or after the fork. None of this bears on whether anything is transferred by the copy.".to_string();
    let _ = (Fnv::new(), sim::visible, start_position);
    Ok(report)
}
