//! Authored policies. They share Body and Perception with the organism and
//! exist to bound what the organism's machinery buys.

use crate::agent::Agent;
use crate::body::{ACTIONS, Action, Body, BodyParams, MOVES, Var};
use crate::env::{Resource, World};
use crate::log::Logger;
use crate::rng::Rng;

/// Uniform random action. The floor.
pub struct RandomAgent {
    body: Body,
    params: BodyParams,
}

impl RandomAgent {
    pub fn new(params: BodyParams, pos: (usize, usize)) -> RandomAgent {
        RandomAgent { body: Body::new(&params, pos), params }
    }
}

impl Agent for RandomAgent {
    fn step(&mut self, w: &mut World, rng: &mut Rng, _log: &mut Logger) -> Action {
        let a = ACTIONS[rng.below(ACTIONS.len())];
        self.body.apply(a, w, &self.params);
        a
    }

    fn body(&self) -> &Body {
        &self.body
    }
}

/// Hand-written commuter with the full map: the ceiling, and half of the
/// degenerate-optima check (A5). Serves the lower of energy/hydration,
/// with hysteresis, by walking to the richest cell of that resource.
pub struct OracleAgent {
    body: Body,
    params: BodyParams,
    target: Option<Resource>,
    prev: Option<Action>,
}

impl OracleAgent {
    pub const SWITCH_AT: f64 = 0.85;
    pub const CONSUME_MIN: f64 = 0.1;

    pub fn new(params: BodyParams, pos: (usize, usize)) -> OracleAgent {
        OracleAgent { body: Body::new(&params, pos), params, target: None, prev: None }
    }
}

fn step_toward(w: &World, from: (usize, usize), to: (usize, usize), prev: Option<Action>) -> Action {
    let (dx, dy) = w.offset(from, to);
    let mut options = Vec::new();
    if dx > 0 { options.push(Action::MoveE) }
    if dx < 0 { options.push(Action::MoveW) }
    if dy > 0 { options.push(Action::MoveS) }
    if dy < 0 { options.push(Action::MoveN) }
    if options.is_empty() {
        return Action::Rest;
    }
    // Keep going the same way when both axes remain; otherwise the first.
    match prev {
        Some(p) if options.contains(&p) => p,
        _ => options[0],
    }
}

fn richest_cell(w: &World, r: Resource, from: (usize, usize)) -> Option<(usize, usize)> {
    let mut best: Option<((usize, usize), f64)> = None;
    for y in 0..w.height() {
        for x in 0..w.width() {
            let amount = w.cell(x, y).amount(r);
            if amount < OracleAgent::CONSUME_MIN {
                continue;
            }
            // Prefer closer cells among rich ones: value discounted by distance.
            let score = amount - 0.02 * w.distance(from, (x, y)) as f64;
            if best.is_none_or(|b| score > b.1) {
                best = Some(((x, y), score));
            }
        }
    }
    best.map(|b| b.0)
}

impl Agent for OracleAgent {
    fn step(&mut self, w: &mut World, _rng: &mut Rng, _log: &mut Logger) -> Action {
        let e = self.body.get(Var::Energy);
        let h = self.body.get(Var::Hydration);
        // Hysteresis: keep the target until its variable is high.
        let done = match self.target {
            Some(Resource::Food) => e >= Self::SWITCH_AT,
            Some(Resource::Water) => h >= Self::SWITCH_AT,
            None => true,
        };
        if done {
            self.target = if e.min(h) >= Self::SWITCH_AT { None } else if e <= h { Some(Resource::Food) } else { Some(Resource::Water) };
        }
        let a = match self.target {
            None => Action::Rest,
            Some(r) => {
                let here = w.cell(self.body.pos.0, self.body.pos.1).amount(r);
                if here >= Self::CONSUME_MIN {
                    if r == Resource::Food { Action::Eat } else { Action::Drink }
                } else {
                    match richest_cell(w, r, self.body.pos) {
                        Some(to) => step_toward(w, self.body.pos, to, self.prev),
                        None => Action::Rest,
                    }
                }
            }
        };
        self.body.apply(a, w, &self.params);
        self.prev = Some(a);
        a
    }

    fn body(&self) -> &Body {
        &self.body
    }
}

/// Stays in the mixed zone (cells outside both patches) and grazes: the
/// other half of the degenerate-optima check (A5).
pub struct MixedForagerAgent {
    body: Body,
    params: BodyParams,
    prev: Option<Action>,
}

impl MixedForagerAgent {
    pub const GRAZE_MIN: f64 = 0.012;

    pub fn new(params: BodyParams, pos: (usize, usize)) -> MixedForagerAgent {
        MixedForagerAgent { body: Body::new(&params, pos), params, prev: None }
    }
}

fn in_mixed_zone(w: &World, pos: (usize, usize)) -> bool {
    let c = w.cell(pos.0, pos.1);
    let cap = w.params.mixed_cap * (if w.layout.scarce { 0.5 } else { 1.0 }) + 1e-9;
    c.cap_food <= cap && c.cap_water <= cap
}

impl Agent for MixedForagerAgent {
    fn step(&mut self, w: &mut World, rng: &mut Rng, _log: &mut Logger) -> Action {
        let e = self.body.get(Var::Energy);
        let h = self.body.get(Var::Hydration);
        let r = if e <= h { Resource::Food } else { Resource::Water };
        let here = w.cell(self.body.pos.0, self.body.pos.1);
        let a = if here.amount(r) >= Self::GRAZE_MIN {
            if r == Resource::Food { Action::Eat } else { Action::Drink }
        } else if e.min(h) >= 0.9 {
            Action::Rest
        } else {
            // Best adjacent mixed-zone cell for the needed resource; random
            // mixed-zone step when nothing stands out.
            let mut best: Option<(Action, f64)> = None;
            for m in MOVES {
                let (dx, dy) = m.offset();
                let next = w.wrap(self.body.pos.0, self.body.pos.1, dx, dy);
                if !in_mixed_zone(w, next) {
                    continue;
                }
                let amount = w.cell(next.0, next.1).amount(r);
                if best.is_none_or(|b| amount > b.1) {
                    best = Some((m, amount));
                }
            }
            match best {
                Some((m, amount)) if amount >= Self::GRAZE_MIN => m,
                Some(_) => {
                    let legal: Vec<Action> = MOVES.iter().copied().filter(|m| {
                        let (dx, dy) = m.offset();
                        in_mixed_zone(w, w.wrap(self.body.pos.0, self.body.pos.1, dx, dy))
                    }).collect();
                    match self.prev {
                        Some(p) if legal.contains(&p) && rng.f64() < 0.8 => p,
                        _ => legal[rng.below(legal.len())],
                    }
                }
                None => Action::Rest,
            }
        };
        self.body.apply(a, w, &self.params);
        self.prev = Some(a);
        a
    }

    fn body(&self) -> &Body {
        &self.body
    }
}
