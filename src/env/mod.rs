//! The world: a toroidal grid of food, water and ambient temperature, with a
//! deterministic regime schedule. Only `World::step` and `Body::apply` mutate it.

pub mod field;

use serde::{Deserialize, Serialize};

use crate::rng::Rng;
pub use field::{Cell, Resource};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegimeOp {
    /// Exchange the food and water patch centers.
    SwapPatches,
    /// Rotate the temperature gradient by 90 degrees.
    RotateGradient,
    /// Halve all resource caps.
    Scarcity,
    /// Move the patches to opposite corners.
    Spread,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldParams {
    pub width: usize,
    pub height: usize,
    pub food_center: (usize, usize),
    pub water_center: (usize, usize),
    pub spread_food_center: (usize, usize),
    pub spread_water_center: (usize, usize),
    pub patch_radius: f64,
    pub patch_peak: f64,
    /// Baseline cap for both resources everywhere (the mixed zone).
    pub mixed_cap: f64,
    pub regrowth: f64,
    pub regrowth_floor: f64,
    pub regrowth_noise: f64,
    /// Ambient runs from -strength to +strength across the gradient axis.
    pub gradient_strength: f64,
    pub regime_every: u64,
    pub regime_ops: Vec<RegimeOp>,
}

impl Default for WorldParams {
    fn default() -> Self {
        WorldParams {
            width: 16,
            height: 16,
            food_center: (3, 8),
            water_center: (12, 8),
            spread_food_center: (1, 1),
            spread_water_center: (14, 14),
            patch_radius: 4.0,
            patch_peak: 1.0,
            mixed_cap: 0.03,
            regrowth: 0.02,
            regrowth_floor: 0.05,
            regrowth_noise: 0.002,
            gradient_strength: 0.6,
            regime_every: 5000,
            regime_ops: vec![
                RegimeOp::SwapPatches,
                RegimeOp::RotateGradient,
                RegimeOp::Scarcity,
                RegimeOp::Spread,
            ],
        }
    }
}

/// Toggle state of the regime. Every op is a toggle so the schedule cannot
/// compound into an unviable world.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Layout {
    pub swapped: bool,
    pub rotated: bool,
    pub scarce: bool,
    pub spread: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub params: WorldParams,
    pub tick: u64,
    pub layout: Layout,
    cells: Vec<Cell>,
}

impl World {
    pub fn new(params: WorldParams) -> World {
        let n = params.width * params.height;
        let mut w = World { params, tick: 0, layout: Layout::default(), cells: vec![Cell::default(); n] };
        w.recompute_caps();
        for c in &mut w.cells {
            c.food = c.cap_food;
            c.water = c.cap_water;
        }
        w
    }

    pub fn width(&self) -> usize {
        self.params.width
    }

    pub fn height(&self) -> usize {
        self.params.height
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.params.width + x
    }

    pub fn cell(&self, x: usize, y: usize) -> &Cell {
        &self.cells[self.idx(x, y)]
    }

    pub fn cell_mut(&mut self, x: usize, y: usize) -> &mut Cell {
        let i = self.idx(x, y);
        &mut self.cells[i]
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Wrap a signed offset from (x, y) onto the torus.
    pub fn wrap(&self, x: usize, y: usize, dx: i32, dy: i32) -> (usize, usize) {
        let w = self.params.width as i32;
        let h = self.params.height as i32;
        let nx = (x as i32 + dx).rem_euclid(w) as usize;
        let ny = (y as i32 + dy).rem_euclid(h) as usize;
        (nx, ny)
    }

    /// Toroidal Manhattan distance.
    pub fn distance(&self, a: (usize, usize), b: (usize, usize)) -> usize {
        let w = self.params.width;
        let h = self.params.height;
        let dx = a.0.abs_diff(b.0);
        let dy = a.1.abs_diff(b.1);
        dx.min(w - dx) + dy.min(h - dy)
    }

    /// Signed toroidal offset from a to b, each component in the shortest direction.
    pub fn offset(&self, a: (usize, usize), b: (usize, usize)) -> (i32, i32) {
        let w = self.params.width as i32;
        let h = self.params.height as i32;
        let mut dx = b.0 as i32 - a.0 as i32;
        let mut dy = b.1 as i32 - a.1 as i32;
        if dx > w / 2 { dx -= w } else if dx < -w / 2 { dx += w }
        if dy > h / 2 { dy -= h } else if dy < -h / 2 { dy += h }
        (dx, dy)
    }

    pub fn centers(&self) -> ((usize, usize), (usize, usize)) {
        let p = &self.params;
        let (f, wt) = if self.layout.spread {
            (p.spread_food_center, p.spread_water_center)
        } else {
            (p.food_center, p.water_center)
        };
        if self.layout.swapped { (wt, f) } else { (f, wt) }
    }

    fn cap_at(&self, pos: (usize, usize), center: (usize, usize)) -> f64 {
        let p = &self.params;
        let d = self.distance(pos, center) as f64;
        let cone = if d < p.patch_radius { p.patch_peak * (1.0 - d / p.patch_radius) } else { 0.0 };
        let cap = cone.max(p.mixed_cap);
        if self.layout.scarce { cap * 0.5 } else { cap }
    }

    fn ambient_at(&self, pos: (usize, usize)) -> f64 {
        let p = &self.params;
        let (axis, n) = if self.layout.rotated { (pos.1, p.height) } else { (pos.0, p.width) };
        let t = if n > 1 { axis as f64 / (n - 1) as f64 } else { 0.5 };
        (-1.0 + 2.0 * t) * p.gradient_strength
    }

    fn recompute_caps(&mut self) {
        let (fc, wc) = self.centers();
        for y in 0..self.params.height {
            for x in 0..self.params.width {
                let cap_food = self.cap_at((x, y), fc);
                let cap_water = self.cap_at((x, y), wc);
                let ambient = self.ambient_at((x, y));
                let c = self.cell_mut(x, y);
                c.cap_food = cap_food;
                c.cap_water = cap_water;
                c.ambient = ambient;
                c.food = c.food.min(cap_food);
                c.water = c.water.min(cap_water);
            }
        }
    }

    pub fn apply_regime(&mut self, op: RegimeOp) {
        match op {
            RegimeOp::SwapPatches => self.layout.swapped = !self.layout.swapped,
            RegimeOp::RotateGradient => self.layout.rotated = !self.layout.rotated,
            RegimeOp::Scarcity => self.layout.scarce = !self.layout.scarce,
            RegimeOp::Spread => self.layout.spread = !self.layout.spread,
        }
        self.recompute_caps();
    }

    /// Advance one tick: regime schedule, then regrowth. Returns the regime
    /// op applied this tick, if any.
    pub fn step(&mut self, rng: &mut Rng) -> Option<RegimeOp> {
        self.tick += 1;
        let p = &self.params;
        let op = if self.tick.is_multiple_of(p.regime_every) {
            let op = p.regime_ops[rng.below(p.regime_ops.len())];
            self.apply_regime(op);
            Some(op)
        } else {
            None
        };
        let r = self.params.regrowth;
        let floor = self.params.regrowth_floor;
        let noise = self.params.regrowth_noise;
        for c in &mut self.cells {
            c.food = regrow(c.food, c.cap_food, r, floor, noise * rng.signed());
            c.water = regrow(c.water, c.cap_water, r, floor, noise * rng.signed());
        }
        op
    }
}

/// Logistic regrowth toward `cap` with a floor so empty cells recover.
fn regrow(v: f64, cap: f64, r: f64, floor: f64, noise: f64) -> f64 {
    if cap <= 0.0 {
        return 0.0;
    }
    let base = v.max(floor * cap);
    let next = v + r * base * (1.0 - v / cap) + noise;
    next.clamp(0.0, cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torus_distance_wraps() {
        let w = World::new(WorldParams::default());
        assert_eq!(w.distance((0, 0), (15, 15)), 2);
        assert_eq!(w.distance((3, 3), (3, 3)), 0);
        assert_eq!(w.offset((0, 0), (15, 0)), (-1, 0));
    }

    #[test]
    fn patches_sit_on_opposite_temperature_sides() {
        let w = World::new(WorldParams::default());
        let (fc, wc) = w.centers();
        assert!(w.cell(fc.0, fc.1).ambient < 0.0);
        assert!(w.cell(wc.0, wc.1).ambient > 0.0);
        assert!(w.cell(fc.0, fc.1).cap_food > w.cell(wc.0, wc.1).cap_food);
    }

    #[test]
    fn regime_ops_toggle() {
        let mut w = World::new(WorldParams::default());
        let before = w.cells.clone();
        w.apply_regime(RegimeOp::Scarcity);
        assert!(w.cell(3, 8).cap_food < before[8 * 16 + 3].cap_food);
        w.apply_regime(RegimeOp::Scarcity);
        assert_eq!(w.cell(3, 8).cap_food, before[8 * 16 + 3].cap_food);
    }

    #[test]
    fn regrowth_recovers_and_is_capped() {
        let mut v = 0.0;
        for _ in 0..2000 {
            v = regrow(v, 1.0, 0.02, 0.05, 0.0);
        }
        assert!(v > 0.99 && v <= 1.0);
    }
}
