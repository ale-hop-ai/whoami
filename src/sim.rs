//! The run loop and the trajectory hash.

use serde::Serialize;

use crate::agent::Agent;
use crate::body::Var;
use crate::env::World;
use crate::hash::Fnv;
use crate::log::{Event, Logger};
use crate::rng::Rng;

#[derive(Clone, Debug, Serialize)]
pub struct RunResult {
    pub ticks: u64,
    pub ticks_alive: u64,
    pub alive: bool,
    pub cause_of_death: Option<Var>,
    pub trajectory_hash: String,
}

pub struct Streams {
    pub env: Rng,
    pub org: Rng,
}

impl Streams {
    pub fn from_seed(seed: u64) -> Streams {
        let root = Rng::from_seed(seed);
        Streams { env: root.derive(1), org: root.derive(2) }
    }
}

/// Run `ticks` ticks or until death. The trajectory hash covers
/// (tick, action, body) each tick, so any divergence in dynamics shows up.
/// `hash` is carried by the caller so it survives snapshots.
pub fn run(world: &mut World, agent: &mut dyn Agent, streams: &mut Streams, ticks: u64, log: &mut Logger, hash: &mut Fnv) -> RunResult {
    let start = world.tick;
    let mut ticks_alive = 0;
    while world.tick - start < ticks && agent.body().alive {
        if let Some(op) = world.step(&mut streams.env) {
            log.write(&Event::Regime { t: world.tick, op });
        }
        let action = agent.step(world, &mut streams.org, log);
        let body = agent.body();
        hash.write_u64(world.tick);
        hash.write_u64(action.index() as u64);
        for &x in &body.v {
            hash.write_f64(x);
        }
        let cell = world.cell(body.pos.0, body.pos.1);
        log.write(&Event::Tick {
            t: world.tick,
            action,
            pos: body.pos,
            body: body.v,
            here: [cell.food, cell.water],
            org: agent.org_tick(),
        });
        if body.alive {
            ticks_alive += 1;
        } else if let Some(cause) = body.cause_of_death {
            log.write(&Event::Death { t: world.tick, cause });
        }
    }
    log.flush();
    let body = agent.body();
    RunResult {
        ticks: world.tick - start,
        ticks_alive,
        alive: body.alive,
        cause_of_death: body.cause_of_death,
        trajectory_hash: format!("{:016x}", hash.finish()),
    }
}
