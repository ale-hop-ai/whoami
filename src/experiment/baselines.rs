//! Authored policies. They share Body and Perception with the organism and
//! exist to bound what the organism's machinery buys.

use crate::agent::Agent;
use crate::body::{ACTIONS, Action, Body, BodyParams, MOVES, Var};
use crate::env::{Resource, World};
use crate::log::Logger;
use crate::organism::homeostasis::HomeostasisParams;
use crate::organism::perception::{Observation, Perception, RADIUS, WINDOW};
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

/// Replaces the Drives+Arbitration stage inside an otherwise complete
/// organism. `Competitive` is EPI proper. The others read errors, which
/// arbitration proper never does; that is why they live here.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Selector {
    #[default]
    Competitive,
    /// Serve the drive with the largest |error|, by its best-valued action.
    GreedyError,
    /// Serve the first drive in `order` whose |error| exceeds `threshold`.
    FixedPriority { order: [usize; 3], threshold: f64 },
}

impl Selector {
    pub const FIXED_THRESHOLD: f64 = 0.25;

    pub fn parse(s: &str) -> Result<Selector, String> {
        match s {
            "competitive" => Ok(Selector::Competitive),
            "greedy" => Ok(Selector::GreedyError),
            "fixed" => Ok(Selector::FixedPriority { order: [0, 1, 2], threshold: Self::FIXED_THRESHOLD }),
            other => Err(format!("unknown selector '{other}'")),
        }
    }

    /// Best action for drive `d`: the one whose value moves the variable in
    /// the direction the error asks for. Ties by action order.
    pub fn best_for(d: usize, errors: &[f64; 3], values: &[[f64; crate::body::N_ACTIONS]; crate::signals::N_DRIVES]) -> Action {
        let sign = if errors[d] >= 0.0 { 1.0 } else { -1.0 };
        let mut best = ACTIONS[0];
        let mut best_v = f64::NEG_INFINITY;
        for a in ACTIONS {
            let v = sign * values[d][a.index()];
            if v > best_v {
                best_v = v;
                best = a;
            }
        }
        best
    }

    pub fn select(&self, errors: &[f64; 3], values: &[[f64; crate::body::N_ACTIONS]; crate::signals::N_DRIVES]) -> Option<Action> {
        match self {
            Selector::Competitive => None,
            Selector::GreedyError => {
                let mut d = 0;
                for i in 1..3 {
                    if errors[i].abs() > errors[d].abs() {
                        d = i;
                    }
                }
                Some(Self::best_for(d, errors, values))
            }
            Selector::FixedPriority { order, threshold } => {
                for &d in order {
                    if errors[d].abs() > *threshold {
                        return Some(Self::best_for(d, errors, values));
                    }
                }
                Some(Action::Rest)
            }
        }
    }
}

/// No memory, no learning: serve the largest error if its remedy is in
/// sight, else wander. Shares Perception with the organism and the spirit
/// of F1/F2 so E1 does not credit those fragments to EPI.
pub struct ReactiveAgent {
    body: Body,
    params: BodyParams,
    setpoint: [f64; 3],
    width: [f64; 3],
    perception: Perception,
    prev: Option<Action>,
}

impl ReactiveAgent {
    pub const THRESHOLD: f64 = 0.25;
    pub const VISIBLE_MIN: f64 = 0.05;
    pub const GRAZE_MIN: f64 = 0.012;

    pub fn new(params: BodyParams, homeo: &HomeostasisParams, pos: (usize, usize)) -> ReactiveAgent {
        ReactiveAgent { body: Body::new(&params, pos), params, setpoint: homeo.setpoint, width: homeo.width, perception: Perception, prev: None }
    }
}

/// Window cell (row, col) → the first move that heads there; None at center.
fn move_toward_window(row: usize, col: usize, prev: Option<Action>) -> Option<Action> {
    let dr = row as i32 - RADIUS;
    let dc = col as i32 - RADIUS;
    let mut options = Vec::new();
    if dc > 0 { options.push(Action::MoveE) }
    if dc < 0 { options.push(Action::MoveW) }
    if dr > 0 { options.push(Action::MoveS) }
    if dr < 0 { options.push(Action::MoveN) }
    if options.is_empty() {
        return None;
    }
    Some(match prev {
        Some(p) if options.contains(&p) => p,
        _ => options[0],
    })
}

impl Agent for ReactiveAgent {
    fn step(&mut self, w: &mut World, rng: &mut Rng, _log: &mut Logger) -> Action {
        let obs = self.perception.observe(w, &self.body, self.prev);
        let mut errors = [0.0; 3];
        for d in 0..3 {
            errors[d] = (self.setpoint[d] - self.body.v[d]) / self.width[d];
        }
        let mut d = 0;
        for i in 1..3 {
            if errors[i].abs() > errors[d].abs() {
                d = i;
            }
        }
        let a = if errors[d].abs() < Self::THRESHOLD {
            Action::Rest
        } else if d < 2 {
            let grid = if d == 0 { &obs.food } else { &obs.water };
            let (cr, cc) = Observation::center();
            if grid[cr][cc] >= Self::GRAZE_MIN {
                if d == 0 { Action::Eat } else { Action::Drink }
            } else {
                // Nearest visible cell with the resource.
                let mut best: Option<(usize, usize, i32)> = None;
                for r in 0..WINDOW {
                    for c in 0..WINDOW {
                        if grid[r][c] >= Self::VISIBLE_MIN {
                            let dist = (r as i32 - RADIUS).abs() + (c as i32 - RADIUS).abs();
                            if best.is_none_or(|b| dist < b.2) {
                                best = Some((r, c, dist));
                            }
                        }
                    }
                }
                match best.and_then(|(r, c, _)| move_toward_window(r, c, self.prev)) {
                    Some(m) => m,
                    None => match self.prev {
                        Some(p) if p.is_move() && rng.f64() < 0.8 => p,
                        _ => MOVES[rng.below(4)],
                    },
                }
            }
        } else {
            // Temperature: head for the visible cell whose ambient is closest to the setpoint.
            let (cr, cc) = Observation::center();
            let mut best = (cr, cc, (obs.ambient01[cr][cc] - self.setpoint[2]).abs());
            for r in 0..WINDOW {
                for c in 0..WINDOW {
                    let gap = (obs.ambient01[r][c] - self.setpoint[2]).abs();
                    if gap < best.2 {
                        best = (r, c, gap);
                    }
                }
            }
            move_toward_window(best.0, best.1, self.prev).unwrap_or(Action::Rest)
        };
        self.body.apply(a, w, &self.params);
        self.prev = Some(a);
        a
    }

    fn body(&self) -> &Body {
        &self.body
    }
}
