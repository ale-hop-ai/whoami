use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resource {
    Food = 0,
    Water = 1,
}

pub const RESOURCES: [Resource; 2] = [Resource::Food, Resource::Water];

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub food: f64,
    pub water: f64,
    pub cap_food: f64,
    pub cap_water: f64,
    /// Ambient temperature in [-1, 1].
    pub ambient: f64,
}

impl Cell {
    pub fn amount(&self, r: Resource) -> f64 {
        match r {
            Resource::Food => self.food,
            Resource::Water => self.water,
        }
    }

    pub fn amount_mut(&mut self, r: Resource) -> &mut f64 {
        match r {
            Resource::Food => &mut self.food,
            Resource::Water => &mut self.water,
        }
    }

    /// Ambient mapped to [0, 1], the scale body temperature uses.
    pub fn ambient01(&self) -> f64 {
        (self.ambient + 1.0) * 0.5
    }
}
