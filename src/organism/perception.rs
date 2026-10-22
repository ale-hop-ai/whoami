//! Local window of the world plus interoception. No learning.

use serde::{Deserialize, Serialize};

use crate::body::{Action, Body, N_VARS};
use crate::env::World;

pub const RADIUS: i32 = 2;
pub const WINDOW: usize = 5;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    /// Indexed [row][col] with the organism at the center (2, 2). Row 0 is north.
    pub food: [[f64; WINDOW]; WINDOW],
    pub water: [[f64; WINDOW]; WINDOW],
    pub ambient01: [[f64; WINDOW]; WINDOW],
    pub v: [f64; N_VARS],
    pub pos: (usize, usize),
    pub prev_action: Option<Action>,
}

impl Observation {
    pub fn center() -> (usize, usize) {
        (RADIUS as usize, RADIUS as usize)
    }

    /// Window coordinates of the cell an action lands on.
    pub fn dest(a: Action) -> (usize, usize) {
        let (dx, dy) = a.offset();
        ((RADIUS + dy) as usize, (RADIUS + dx) as usize)
    }

    pub fn food_visible(&self) -> bool {
        self.food.iter().flatten().any(|&f| f > 0.05)
    }

    pub fn water_visible(&self) -> bool {
        self.water.iter().flatten().any(|&f| f > 0.05)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Perception;

impl Perception {
    pub fn observe(&self, w: &World, body: &Body, prev_action: Option<Action>) -> Observation {
        let mut food = [[0.0; WINDOW]; WINDOW];
        let mut water = [[0.0; WINDOW]; WINDOW];
        let mut ambient01 = [[0.0; WINDOW]; WINDOW];
        for dy in -RADIUS..=RADIUS {
            for dx in -RADIUS..=RADIUS {
                let (x, y) = w.wrap(body.pos.0, body.pos.1, dx, dy);
                let c = w.cell(x, y);
                let r = (dy + RADIUS) as usize;
                let col = (dx + RADIUS) as usize;
                food[r][col] = c.food;
                water[r][col] = c.water;
                ambient01[r][col] = c.ambient01();
            }
        }
        Observation { food, water, ambient01, v: body.v, pos: body.pos, prev_action }
    }
}
