pub mod agent;
pub mod body;
pub mod config;
pub mod env;
pub mod experiment;
pub mod hash;
pub mod log;
pub mod rng;
pub mod sim;

use std::path::Path;

use agent::Agent;
use config::Config;
use env::World;
use experiment::baselines::RandomAgent;

/// Agents selectable from the CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentKind {
    Random,
}

impl AgentKind {
    pub fn parse(s: &str) -> Result<AgentKind, String> {
        match s {
            "random" => Ok(AgentKind::Random),
            other => Err(format!("unknown agent '{other}' (expected: random)")),
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
        AgentKind::Random => Box::new(RandomAgent::new(cfg.body.clone(), pos)),
    }
}

/// Convenience used by tests and the CLI: one full run from a seed.
pub fn run_once(cfg: &Config, kind: AgentKind, seed: u64, ticks: u64, log_path: Option<&Path>, log_every: u64) -> Result<sim::RunResult, String> {
    let mut world = World::new(cfg.world.clone());
    let mut agent = build_agent(kind, cfg, &world);
    let mut streams = sim::Streams::from_seed(seed);
    let mut log = match log_path {
        Some(p) => log::Logger::to_file(p, log_every)?,
        None => log::Logger::null(),
    };
    Ok(sim::run(&mut world, agent.as_mut(), &mut streams, ticks, &mut log))
}
