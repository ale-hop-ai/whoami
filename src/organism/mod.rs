//! The organism: eight processes wired in a fixed order through `Signals`.
//! There is no decision-maker here; `tick` only moves values between
//! processes and applies the arbitration result to the body.

pub mod arbitration;
pub mod drives;
pub mod homeostasis;
pub mod lesion;
pub mod memory;
pub mod perception;
pub mod self_model;
pub mod world_model;

use serde::{Deserialize, Serialize};

use crate::agent::Agent;
use crate::body::{Action, Body, BodyParams, Deltas, N_ACTIONS, VARS};
use crate::env::World;
use crate::experiment::baselines::Selector;
use crate::log::{Event, Logger, OrgTick};
use crate::rng::Rng;
use crate::signals::{EXPLORE, N_DRIVES, N_RESOURCES, Remembered, Signals};
use arbitration::{ArbitrationParams, WinnerTakeAll};
use drives::{DriveParams, Drives};
use homeostasis::{Homeostasis, HomeostasisParams};
use lesion::Lesions;
use memory::{Memory, MemoryOut, MemoryParams};
use perception::Perception;
use self_model::{SelfModel, SelfModelParams};
use world_model::{WorldModel, WorldModelOut, WorldModelParams};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OrganismParams {
    pub homeostasis: HomeostasisParams,
    pub memory: MemoryParams,
    pub world_model: WorldModelParams,
    pub self_model: SelfModelParams,
    pub drives: DriveParams,
    pub arbitration: ArbitrationParams,
    pub lesions: Lesions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Organism {
    params: OrganismParams,
    body_params: BodyParams,
    body: Body,
    perception: Perception,
    homeostasis: Homeostasis,
    memory: Memory,
    world_model: WorldModel,
    self_model: SelfModel,
    drives: Drives,
    arbitration: WinnerTakeAll,
    // Carried across ticks.
    deltas_prev: Deltas,
    prev_action: Option<Action>,
    remembered_prev: [Remembered; N_RESOURCES],
    urgency_prev: [f64; N_DRIVES],
    /// Cell amounts before the previous action (teaches the yield).
    #[serde(default)]
    here_prev: [f64; N_RESOURCES],
    /// Learning switch used by the Frozen baseline.
    learning: bool,
    /// Baseline selectors replace Drives+Arbitration; Competitive is EPI.
    selector: Selector,
    #[serde(skip)]
    last: OrgTick,
}

impl Organism {
    pub fn new(params: OrganismParams, body_params: BodyParams, world: &World, pos: (usize, usize)) -> Organism {
        Organism {
            body: Body::new(&body_params, pos),
            perception: Perception,
            homeostasis: Homeostasis::default(),
            memory: Memory::new(world.width(), world.height()),
            world_model: WorldModel::new(&params.world_model),
            self_model: SelfModel::new(&params.self_model),
            drives: Drives::default(),
            arbitration: WinnerTakeAll::default(),
            deltas_prev: Deltas::default(),
            prev_action: None,
            remembered_prev: [Remembered::default(); N_RESOURCES],
            urgency_prev: [0.0; N_DRIVES],
            here_prev: [0.0; N_RESOURCES],
            learning: true,
            selector: Selector::Competitive,
            last: OrgTick::default(),
            params,
            body_params,
        }
    }

    pub fn set_learning(&mut self, on: bool) {
        self.learning = on;
    }

    pub fn learning(&self) -> bool {
        self.learning
    }

    pub fn set_selector(&mut self, s: Selector) {
        self.selector = s;
    }

    pub fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }

    /// Move the organism into another world: body and position reset to the
    /// start, carried tick values cleared, everything learned kept.
    pub fn transplant(&mut self, pos: (usize, usize)) {
        self.body = Body::new(&self.body_params, pos);
        self.deltas_prev = Deltas::default();
        self.prev_action = None;
        self.remembered_prev = [Remembered::default(); N_RESOURCES];
        self.urgency_prev = [0.0; N_DRIVES];
        self.here_prev = [0.0; N_RESOURCES];
    }

    /// A fresh organism that inherits only `donor`'s body and position.
    pub fn body_only_from(donor: &Organism, world: &World) -> Organism {
        let mut o = Organism::new(donor.params.clone(), donor.body_params.clone(), world, donor.body.pos);
        o.body = donor.body.clone();
        o
    }

    pub fn params(&self) -> &OrganismParams {
        &self.params
    }

    pub fn drives(&self) -> &Drives {
        &self.drives
    }

    pub fn world_model(&self) -> &WorldModel {
        &self.world_model
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// One tick in the order fixed by 02-architecture §5. Lesions are applied
    /// at the single point where each output enters `Signals`.
    pub fn tick(&mut self, world: &mut World, log: &mut Logger) -> Action {
        let p = &self.params;
        let les = &p.lesions;

        // 2. Perception.
        let obs = self.perception.observe(world, &self.body, self.prev_action);
        let (cr, cc) = perception::Observation::center();
        let here = [obs.food[cr][cc], obs.water[cr][cc]];

        // 3. World model: learn from the previous tick, predict for this one.
        let wm_params = if self.learning { p.world_model.clone() } else { WorldModelParams { lr: 0.0, uncertainty_rate: 0.0, ..p.world_model.clone() } };
        let wm = if les.world_model.runs() {
            let out = self.world_model.step(&obs, &self.deltas_prev, self.prev_action, &wm_params);
            if les.world_model.passes() { out } else { WorldModelOut::null(&p.world_model) }
        } else {
            WorldModelOut::null(&p.world_model)
        };

        // 4. Self-model.
        let sm = if les.self_model.runs() {
            let out = self.self_model.step(&self.body.v, &self.deltas_prev, self.prev_action, &self.here_prev, &here, &self.remembered_prev, &p.homeostasis.danger_lo, &p.self_model);
            if les.self_model.passes() { out } else { SelfModel::null_out(&p.self_model) }
        } else {
            SelfModel::null_out(&p.self_model)
        };

        // 5. Homeostasis.
        let ho = self.homeostasis.step(&self.body.v, &p.homeostasis);

        // 6. Memory (reads previous-tick urgency).
        let mem = if les.memory.runs() {
            let out = self.memory.step(&obs, &self.urgency_prev, &p.memory, self.learning);
            if les.memory.passes() { out } else { MemoryOut::null() }
        } else {
            MemoryOut::null()
        };

        let signals = Signals {
            obs,
            deltas: self.deltas_prev,
            prev_action: self.prev_action,
            errors: ho.errors,
            danger_events: ho.danger_events,
            predicted: wm.predicted,
            uncertainty: wm.uncertainty,
            prediction_error: wm.prediction_error,
            here,
            yield_: sm.yield_,
            novel_remembered: sm.novel_remembered,
            novel_here: sm.novel_here,
            remembered: mem.remembered,
            remembered_prev: self.remembered_prev,
            neighbor_conf: mem.neighbor_conf,
            ttc: sm.ttc,
            reach: sm.reach,
            urgency_prev: self.urgency_prev,
        };
        signals.assert_finite();

        // 7. Drives.
        let adapt = self.learning && !les.drive_adaptation_off;
        let explore = les.exploration.passes();
        let dr = self.drives.step(&signals, &p.drives, adapt, explore);
        let mut bids = dr.bids;
        if !explore {
            bids[EXPLORE] = [0.0; N_ACTIONS];
        }

        // 8. Arbitration (or a baseline selector that replaces it).
        let action = match self.selector.select(&ho.errors, &dr.values) {
            Some(a) => a,
            None => self.arbitration.select(&bids, self.prev_action, &p.arbitration),
        };

        // 9. Action.
        let deltas = self.body.apply(action, world, &self.body_params);

        // Carry and log.
        for var in VARS {
            if ho.danger_events[var.index()] {
                log.write(&Event::Danger { t: world.tick, var });
            }
        }
        self.deltas_prev = deltas;
        self.prev_action = Some(action);
        self.remembered_prev = mem.remembered;
        self.urgency_prev = dr.urgency;
        self.here_prev = here;
        self.last = OrgTick {
            errors: ho.errors,
            bids: bids.to_vec(),
            values: dr.values.to_vec(),
            predicted: wm.predicted,
            prediction_error: wm.prediction_error,
            gains: self.drives.gains().to_vec(),
            remembered: [
                (mem.remembered[0].target.0, mem.remembered[0].target.1, mem.remembered[0].dist, mem.remembered[0].conf, mem.remembered[0].amounts[0]),
                (mem.remembered[1].target.0, mem.remembered[1].target.1, mem.remembered[1].dist, mem.remembered[1].conf, mem.remembered[1].amounts[1]),
            ],
        };
        action
    }
}

impl Agent for Organism {
    fn step(&mut self, w: &mut World, _rng: &mut Rng, log: &mut Logger) -> Action {
        self.tick(w, log)
    }

    fn body(&self) -> &Body {
        &self.body
    }

    fn org_tick(&self) -> Option<&OrgTick> {
        Some(&self.last)
    }

    fn into_organism(self: Box<Self>) -> Option<Organism> {
        Some(*self)
    }
}
