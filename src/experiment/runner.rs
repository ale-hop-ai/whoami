//! Runs one arm for one seed and reduces the trajectory to the numbers the
//! experiments compare. Full JSONL logs are kept only where asked.

use std::path::Path;

use serde::Serialize;

use crate::agent::Agent;
use crate::body::N_VARS;
use crate::config::Config;
use crate::env::World;
use crate::experiment::baselines::Selector;
use crate::experiment::families::Family;
use crate::hash::Fnv;
use crate::log::Logger;
use crate::metrics::{Fingerprint, Trajectory, trajectory::{adaptation_speed, window_stats}};
use crate::organism::Organism;
use crate::organism::lesion::Lesions;
use crate::sim::{self, Streams};
use crate::{AgentKind, build_agent, start_position};

#[derive(Clone, Debug)]
pub enum ArmKind {
    Agent(AgentKind),
    /// Full organism with a baseline selector in place of arbitration.
    Selector(Selector),
    /// A pretrained organism with learning off, transplanted at tick 0.
    Frozen(Box<Organism>),
    /// EPI with lesions.
    Lesioned(Lesions),
    /// A specific organism to continue (transplants in E2; forks in E3).
    Continue(Box<Organism>),
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub name: String,
    pub kind: ArmKind,
}

impl Arm {
    pub fn agent(name: &str, kind: AgentKind) -> Arm {
        Arm { name: name.to_string(), kind: ArmKind::Agent(kind) }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SeedResult {
    pub arm: String,
    pub family: String,
    pub env_seed: u64,
    pub org_seed: u64,
    pub ticks: u64,
    pub ticks_alive: u64,
    pub alive: bool,
    pub cause: Option<usize>,
    pub margin: f64,
    pub viability: f64,
    pub intake: f64,
    pub moves_per_intake: f64,
    pub anticipation: f64,
    pub action_entropy: f64,
    pub conditional_entropy: f64,
    pub prediction_mse: Option<f64>,
    pub retention: Option<f64>,
    pub dangers: usize,
    /// (shift tick, op, ticks to recover or None)
    pub adaptation: Vec<(u64, String, Option<u64>)>,
    pub trajectory_hash: String,
    #[serde(skip)]
    pub fingerprints: Vec<Fingerprint>,
    #[serde(skip)]
    pub trajectory: Option<Trajectory>,
}

/// Identity of one run within an experiment.
#[derive(Clone, Debug)]
pub struct RunId<'a> {
    pub family: Family,
    pub arm: &'a Arm,
    pub env_seed: u64,
    pub org_seed: u64,
}

pub struct RunOpts<'a> {
    pub ticks: u64,
    pub window: usize,
    /// Keep the full JSONL log here.
    pub log_path: Option<&'a Path>,
    /// Keep the parsed trajectory on the result (E5 needs it).
    pub keep_trajectory: bool,
}

/// Build the world for a family and the agent for an arm.
pub fn build(cfg: &Config, family: Family, arm: &Arm) -> (World, Box<dyn Agent>) {
    let mut cfg = cfg.clone();
    family.apply(&mut cfg.world);
    let world = World::new(cfg.world.clone());
    let pos = start_position(&world);
    let agent: Box<dyn Agent> = match &arm.kind {
        ArmKind::Agent(k) => build_agent(*k, &cfg, &world),
        ArmKind::Selector(s) => {
            let mut o = Organism::new(cfg.organism.clone(), cfg.body.clone(), &world, pos);
            o.set_selector(*s);
            Box::new(o)
        }
        ArmKind::Frozen(o) => {
            let mut o = (**o).clone();
            o.set_learning(false);
            o.transplant(pos);
            Box::new(o)
        }
        ArmKind::Lesioned(l) => {
            let mut params = cfg.organism.clone();
            params.lesions = l.clone();
            Box::new(Organism::new(params, cfg.body.clone(), &world, pos))
        }
        ArmKind::Continue(o) => {
            let mut o = (**o).clone();
            o.transplant(pos);
            Box::new(o)
        }
    };
    (world, agent)
}

/// Run and reduce. The organism, if any, is returned for protocols that
/// continue it (E2 development, E3 snapshots).
pub fn run_seed(cfg: &Config, family: Family, arm: &Arm, env_seed: u64, org_seed: u64, opts: &RunOpts) -> Result<(SeedResult, Option<Organism>), String> {
    let (mut world, mut agent) = build(cfg, family, arm);
    let mut streams = Streams::new(env_seed, org_seed);
    let mut log = Logger::memory(opts.log_path)?;
    let mut hash = Fnv::new();
    let r = sim::run(&mut world, agent.as_mut(), &mut streams, opts.ticks, &mut log, &mut hash);
    let tr = Trajectory::from_collected(log.take());
    let organism = organism_of(agent);
    let result = reduce(&tr, cfg, &RunId { family, arm, env_seed, org_seed }, &r, opts);
    Ok((result, organism))
}

fn organism_of(agent: Box<dyn Agent>) -> Option<Organism> {
    agent.into_organism()
}

pub fn reduce(tr: &Trajectory, cfg: &Config, id: &RunId, r: &sim::RunResult, opts: &RunOpts) -> SeedResult {
    let RunId { family, arm, env_seed, org_seed } = *id;
    let sp = &cfg.organism.homeostasis.setpoint;
    let wd = &cfg.organism.homeostasis.width;
    let windows = tr.windows(opts.window);
    let mut acc = Acc::default();
    let mut prev = None;
    let mut fingerprints = Vec::new();
    for w in &windows {
        let st = window_stats(w, prev, sp, wd);
        acc.add(&st);
        fingerprints.push(Fingerprint::from_rows(w));
        prev = w.last();
    }
    // A short life still gets one partial window so every seed has numbers.
    if windows.is_empty() && !tr.rows.is_empty() {
        let st = window_stats(&tr.rows, None, sp, wd);
        acc.add(&st);
        fingerprints.push(Fingerprint::from_rows(&tr.rows));
    }
    let adaptation = tr.regimes.iter().map(|(t, op)| (*t, format!("{op:?}"), adaptation_speed(tr, *t, opts.window, 0.9, 4000, sp, wd))).collect();
    let n = acc.n.max(1) as f64;
    // Survivorship control: margin, viability and intake are averaged over
    // the full horizon with dead ticks counted as 0 (docs/03 §1, amendment
    // 2026-10-22). Anticipation and entropies describe behavior while alive.
    let alive_frac = (r.ticks_alive as f64 / opts.ticks.max(1) as f64).min(1.0);
    SeedResult {
        arm: arm.name.clone(),
        family: family.name().to_string(),
        env_seed,
        org_seed,
        ticks: r.ticks,
        ticks_alive: r.ticks_alive,
        alive: r.alive,
        cause: r.cause_of_death.map(|v| v.index()),
        margin: acc.margin / n * alive_frac,
        viability: acc.viability / n * alive_frac,
        intake: acc.intake / n * alive_frac,
        moves_per_intake: acc.moves / n,
        anticipation: acc.anticipation / n,
        action_entropy: acc.action_entropy / n,
        conditional_entropy: acc.cond_entropy / n,
        prediction_mse: if acc.mse_n > 0 { Some(acc.mse / acc.mse_n as f64) } else { None },
        retention: retention(tr),
        dangers: tr.dangers.len(),
        adaptation,
        trajectory_hash: r.trajectory_hash.clone(),
        fingerprints,
        trajectory: if opts.keep_trajectory { Some(tr.clone()) } else { None },
    }
}

#[derive(Default)]
struct Acc {
    n: usize,
    margin: f64,
    viability: f64,
    intake: f64,
    moves: f64,
    anticipation: f64,
    action_entropy: f64,
    cond_entropy: f64,
    mse: f64,
    mse_n: usize,
}

impl Acc {
    fn add(&mut self, st: &crate::metrics::WindowStats) {
        self.n += 1;
        self.margin += st.margin;
        self.viability += st.viability;
        self.intake += st.intake_per_tick;
        self.moves += if st.moves_per_intake.is_finite() { st.moves_per_intake } else { 0.0 };
        self.anticipation += st.anticipation_rate;
        self.action_entropy += st.action_entropy;
        self.cond_entropy += st.conditional_entropy;
        if let Some(m) = st.prediction_mse {
            self.mse += m;
            self.mse_n += 1;
        }
    }
}

/// Memory retention from the logged remembered targets. A target the
/// organism has pointed at for at least `K` ticks is checked when the
/// organism stands on it: does the cell hold at least half of what memory
/// said when the target was first adopted? Retention is the fraction of
/// such arrivals that do. None when no long-held target was ever reached.
pub fn retention(tr: &Trajectory) -> Option<f64> {
    const K: u64 = 200;
    // Per resource: (target, tick adopted, amount remembered then).
    type Held = Option<((usize, usize), u64, f64)>;
    let mut held: [Held; 2] = [None, None];
    let mut checked = 0usize;
    let mut ok = 0usize;
    for r in &tr.rows {
        let Some(rem) = r.remembered else { continue };
        for res in 0..2 {
            let (x, y, dist, conf, amount) = rem[res];
            if conf <= 0.0 {
                held[res] = None;
                continue;
            }
            match held[res] {
                Some((target, since, recorded)) if target == (x, y) => {
                    if dist == 0 && r.t - since >= K {
                        checked += 1;
                        if r.here[res] >= 0.5 * recorded {
                            ok += 1;
                        }
                        held[res] = None;
                    }
                }
                _ => held[res] = Some(((x, y), r.t, amount)),
            }
        }
    }
    if checked == 0 { None } else { Some(ok as f64 / checked as f64) }
}

/// Simple CSV of per-seed results.
pub fn write_csv(path: &Path, results: &[SeedResult]) -> Result<(), String> {
    let mut s = String::from("arm,family,env_seed,org_seed,ticks_alive,alive,cause,margin,viability,intake,moves_per_intake,anticipation,action_entropy,conditional_entropy,prediction_mse,retention,dangers,trajectory_hash\n");
    for r in results {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{:.5},{:.4},{:.6},{:.3},{:.4},{:.4},{:.4},{},{},{},{}\n",
            r.arm, r.family, r.env_seed, r.org_seed, r.ticks_alive, r.alive,
            r.cause.map(|c| c.to_string()).unwrap_or_default(),
            r.margin, r.viability, r.intake, r.moves_per_intake, r.anticipation, r.action_entropy, r.conditional_entropy,
            r.prediction_mse.map(|m| format!("{m:.3e}")).unwrap_or_default(),
            r.retention.map(|m| format!("{m:.3}")).unwrap_or_default(),
            r.dangers, r.trajectory_hash
        ));
    }
    std::fs::write(path, s).map_err(|e| format!("write {}: {e}", path.display()))
}

pub fn vars_name(i: usize) -> &'static str {
    crate::body::VARS[i.min(N_VARS - 1)].name()
}
