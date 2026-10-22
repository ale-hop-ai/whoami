#![allow(clippy::needless_range_loop)] // indexed loops over parallel arrays are the clearer form here

pub mod agent;
pub mod body;
pub mod config;
pub mod env;
pub mod experiment;
pub mod hash;
pub mod log;
pub mod metrics;
pub mod organism;
pub mod rng;
pub mod signals;
pub mod sim;
pub mod snapshot;

use std::path::Path;

use std::path::PathBuf;

use agent::Agent;
use config::Config;
use env::World;
use experiment::baselines::{MixedForagerAgent, OracleAgent, RandomAgent};
use hash::Fnv;
use log::{Event, Logger};
use organism::Organism;
use rng::Rng;
use sim::{RunResult, Streams};
use snapshot::Snapshot;

/// Agents selectable from the CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentKind {
    Epi,
    Random,
    Oracle,
    MixedForager,
}

impl AgentKind {
    pub fn parse(s: &str) -> Result<AgentKind, String> {
        match s {
            "epi" => Ok(AgentKind::Epi),
            "random" => Ok(AgentKind::Random),
            "oracle" => Ok(AgentKind::Oracle),
            "mixed" => Ok(AgentKind::MixedForager),
            other => Err(format!("unknown agent '{other}' (expected: epi, random, oracle, mixed)")),
        }
    }
}

/// Starting position: the center of the mixed zone between the patches.
pub fn start_position(w: &World) -> (usize, usize) {
    (w.width() / 2, w.height() / 2)
}

pub fn build_agent(kind: AgentKind, cfg: &Config, w: &World) -> Box<dyn Agent> {
    let pos = start_position(w);
    match kind {
        AgentKind::Epi => Box::new(organism::Organism::new(cfg.organism.clone(), cfg.body.clone(), w, pos)),
        AgentKind::Random => Box::new(RandomAgent::new(cfg.body.clone(), pos)),
        AgentKind::Oracle => Box::new(OracleAgent::new(cfg.body.clone(), pos)),
        AgentKind::MixedForager => Box::new(MixedForagerAgent::new(cfg.body.clone(), pos)),
    }
}

/// Convenience used by tests and the CLI: one full run from a seed.
pub fn run_once(cfg: &Config, kind: AgentKind, seed: u64, ticks: u64, log_path: Option<&Path>, log_every: u64) -> Result<sim::RunResult, String> {
    let mut world = World::new(cfg.world.clone());
    let mut agent = build_agent(kind, cfg, &world);
    let mut streams = sim::Streams::from_seed(seed);
    let mut log = match log_path {
        Some(p) => Logger::to_file(p, log_every)?,
        None => Logger::null(),
    };
    let mut hash = Fnv::new();
    Ok(sim::run(&mut world, agent.as_mut(), &mut streams, ticks, &mut log, &mut hash))
}

/// An EPI organism in its world, with everything needed to snapshot, fork
/// and continue.
pub struct Session {
    pub world: World,
    pub organism: Organism,
    pub streams: Streams,
    pub hash: Fnv,
    pub config_hash: u64,
}

impl Session {
    pub fn new(cfg: &Config, config_hash: u64, seed: u64) -> Session {
        let world = World::new(cfg.world.clone());
        let pos = start_position(&world);
        let organism = Organism::new(cfg.organism.clone(), cfg.body.clone(), &world, pos);
        Session { world, organism, streams: Streams::from_seed(seed), hash: Fnv::new(), config_hash }
    }

    pub fn from_snapshot(s: Snapshot) -> Session {
        Session {
            hash: s.hash_state(),
            world: s.world,
            organism: s.organism,
            streams: Streams { env: s.rng_env, org: s.rng_org },
            config_hash: s.config_hash,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.world.tick,
            config_hash: self.config_hash,
            world: self.world.clone(),
            organism: self.organism.clone(),
            rng_env: self.streams.env.clone(),
            rng_org: self.streams.org.clone(),
            trajectory_hash: self.hash.finish(),
        }
    }

    /// Replace the environment stream: the fork's world takes a new path.
    pub fn reseed_env(&mut self, env_seed: u64) {
        self.streams.env = Rng::from_seed(env_seed).derive(1);
    }

    pub fn run(&mut self, ticks: u64, log: &mut Logger) -> RunResult {
        sim::run(&mut self.world, &mut self.organism, &mut self.streams, ticks, log, &mut self.hash)
    }

    /// Run in segments, writing `snapshot_<tick>.json` into `dir` every
    /// `every` ticks. Returns the combined result.
    pub fn run_with_snapshots(&mut self, ticks: u64, log: &mut Logger, dir: &std::path::Path, every: u64) -> Result<RunResult, String> {
        let every = every.max(1);
        let start = self.world.tick;
        let mut last = None;
        let mut alive_total = 0;
        while self.world.tick - start < ticks && self.organism.body().alive {
            let remaining = ticks - (self.world.tick - start);
            let r = self.run(remaining.min(every), log);
            alive_total += r.ticks_alive;
            last = Some(r);
            if self.organism.body().alive && (self.world.tick - start) < ticks {
                let path: PathBuf = dir.join(format!("snapshot_{}.json", self.world.tick));
                self.snapshot().save(&path)?;
                log.write(&Event::Snapshot { t: self.world.tick, path: path.display().to_string() });
            }
        }
        let mut r = last.ok_or("nothing ran")?;
        r.ticks = self.world.tick - start;
        r.ticks_alive = alive_total;
        Ok(r)
    }
}
