//! Physiology. Pure dynamics: no decisions live here.

use serde::{Deserialize, Serialize};

use crate::env::{Resource, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    MoveN = 0,
    MoveS = 1,
    MoveE = 2,
    MoveW = 3,
    Eat = 4,
    Drink = 5,
    Rest = 6,
}

pub const N_ACTIONS: usize = 7;
pub const ACTIONS: [Action; N_ACTIONS] = [
    Action::MoveN,
    Action::MoveS,
    Action::MoveE,
    Action::MoveW,
    Action::Eat,
    Action::Drink,
    Action::Rest,
];
pub const MOVES: [Action; 4] = [Action::MoveN, Action::MoveS, Action::MoveE, Action::MoveW];

impl Action {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Action {
        ACTIONS[i]
    }

    pub fn is_move(self) -> bool {
        self.index() < 4
    }

    /// Grid offset for moves; (0, 0) otherwise. North is -y.
    pub fn offset(self) -> (i32, i32) {
        match self {
            Action::MoveN => (0, -1),
            Action::MoveS => (0, 1),
            Action::MoveE => (1, 0),
            Action::MoveW => (-1, 0),
            _ => (0, 0),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Action::MoveN => "N",
            Action::MoveS => "S",
            Action::MoveE => "E",
            Action::MoveW => "W",
            Action::Eat => "eat",
            Action::Drink => "drink",
            Action::Rest => "rest",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Var {
    Energy = 0,
    Hydration = 1,
    Temp = 2,
}

pub const N_VARS: usize = 3;
pub const VARS: [Var; N_VARS] = [Var::Energy, Var::Hydration, Var::Temp];

impl Var {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Var::Energy => "energy",
            Var::Hydration => "hydration",
            Var::Temp => "temp",
        }
    }

    /// The resource that restores this variable, if any.
    pub fn resource(self) -> Option<Resource> {
        match self {
            Var::Energy => Some(Resource::Food),
            Var::Hydration => Some(Resource::Water),
            Var::Temp => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BodyParams {
    pub start: [f64; N_VARS],
    /// Per-tick drain of energy and hydration.
    pub base_energy: f64,
    pub base_hydration: f64,
    pub move_cost: f64,
    /// Body temperature added by a move.
    pub move_heat: f64,
    /// Rest multiplies base drains by this.
    pub rest_factor: f64,
    /// Body temperature relaxes toward ambient01 at this rate.
    pub temp_relax: f64,
    /// Temperature reference used by the penalties (body scale, [0, 1]).
    pub temp_ref: f64,
    /// Extra energy drain per unit of coldness below temp_ref.
    pub cold_penalty: f64,
    /// Extra hydration drain per unit of heat above temp_ref.
    pub heat_penalty: f64,
    /// Max resource taken from a cell per Eat/Drink.
    pub bite: f64,
    /// Energy or hydration gained per unit of resource consumed.
    pub yield_: f64,
}

impl Default for BodyParams {
    fn default() -> Self {
        BodyParams {
            start: [0.6, 0.6, 0.5],
            base_energy: 0.003,
            base_hydration: 0.003,
            move_cost: 0.001,
            move_heat: 0.01,
            rest_factor: 0.5,
            temp_relax: 0.03,
            temp_ref: 0.5,
            cold_penalty: 0.006,
            heat_penalty: 0.006,
            bite: 0.25,
            yield_: 0.6,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Deltas(pub [f64; N_VARS]);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Body {
    pub pos: (usize, usize),
    pub v: [f64; N_VARS],
    pub alive: bool,
    pub cause_of_death: Option<Var>,
}

impl Body {
    pub fn new(p: &BodyParams, pos: (usize, usize)) -> Body {
        Body { pos, v: p.start, alive: true, cause_of_death: None }
    }

    pub fn get(&self, var: Var) -> f64 {
        self.v[var.index()]
    }

    /// Apply one action. Mutates the world when consuming. Returns the
    /// realized change of each variable. Does nothing once dead.
    pub fn apply(&mut self, a: Action, w: &mut World, p: &BodyParams) -> Deltas {
        if !self.alive {
            return Deltas::default();
        }
        let before = self.v;
        let is_move = a.is_move();
        if is_move {
            let (dx, dy) = a.offset();
            self.pos = w.wrap(self.pos.0, self.pos.1, dx, dy);
        }
        let rest = if a == Action::Rest { p.rest_factor } else { 1.0 };
        let tb = self.v[Var::Temp.index()];
        let cold = (p.temp_ref - tb).max(0.0);
        let heat = (tb - p.temp_ref).max(0.0);

        let mut e = self.v[Var::Energy.index()];
        let mut h = self.v[Var::Hydration.index()];
        e -= p.base_energy * rest + p.cold_penalty * cold;
        h -= p.base_hydration * rest + p.heat_penalty * heat;
        if is_move {
            e -= p.move_cost;
        }
        match a {
            Action::Eat => e += consume(w, self.pos, Resource::Food, p),
            Action::Drink => h += consume(w, self.pos, Resource::Water, p),
            _ => {}
        }
        let ambient = w.cell(self.pos.0, self.pos.1).ambient01();
        let mut t = tb + p.temp_relax * (ambient - tb);
        if is_move {
            t += p.move_heat;
        }

        // Energy and hydration saturate at 1; temperature is lethal at both ends.
        self.v = [e.min(1.0), h.min(1.0), t];
        for var in VARS {
            let x = self.v[var.index()];
            let dead = match var {
                Var::Temp => !(0.0..=1.0).contains(&x),
                _ => x <= 0.0,
            };
            if dead && self.alive {
                self.alive = false;
                self.cause_of_death = Some(var);
            }
        }
        let mut d = [0.0; N_VARS];
        for i in 0..N_VARS {
            d[i] = self.v[i] - before[i];
        }
        Deltas(d)
    }
}

fn consume(w: &mut World, pos: (usize, usize), r: Resource, p: &BodyParams) -> f64 {
    let cell = w.cell_mut(pos.0, pos.1);
    let amount = cell.amount_mut(r);
    let taken = amount.min(p.bite);
    *amount -= taken;
    taken * p.yield_
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::WorldParams;

    fn world() -> World {
        World::new(WorldParams::default())
    }

    #[test]
    fn eating_on_food_raises_energy() {
        let mut w = world();
        let p = BodyParams::default();
        let (fc, _) = w.centers();
        let mut b = Body::new(&p, fc);
        let d = b.apply(Action::Eat, &mut w, &p);
        assert!(d.0[0] > 0.0);
        assert!(w.cell(fc.0, fc.1).food < 1.0);
    }

    #[test]
    fn cold_drains_energy_faster() {
        let mut w = world();
        let p = BodyParams::default();
        let mut warm = Body::new(&p, (8, 8));
        let mut cold = Body::new(&p, (8, 8));
        cold.v[2] = 0.2;
        let dw = warm.apply(Action::Rest, &mut w, &p);
        let dc = cold.apply(Action::Rest, &mut w, &p);
        assert!(dc.0[0] < dw.0[0]);
    }

    #[test]
    fn starvation_kills() {
        let mut w = world();
        let p = BodyParams::default();
        let mut b = Body::new(&p, (8, 8));
        let mut n = 0;
        while b.alive && n < 10_000 {
            b.apply(Action::Rest, &mut w, &p);
            n += 1;
        }
        assert!(!b.alive);
        assert!(b.cause_of_death.is_some());
        assert!(n > 100 && n < 2000, "died at {n}");
    }

    #[test]
    fn moves_wrap() {
        let mut w = world();
        let p = BodyParams::default();
        let mut b = Body::new(&p, (0, 0));
        b.apply(Action::MoveW, &mut w, &p);
        assert_eq!(b.pos, (15, 0));
        b.apply(Action::MoveN, &mut w, &p);
        assert_eq!(b.pos, (15, 15));
    }
}
