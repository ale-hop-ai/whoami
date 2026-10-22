//! Environment families: named generator settings applied over the base
//! config. Explicit code, not JSON merging.

use crate::env::{RegimeOp, WorldParams};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Family {
    Default,
    Scarce,
    Far,
    Volatile,
}

pub const FAMILIES: [Family; 4] = [Family::Default, Family::Scarce, Family::Far, Family::Volatile];

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Default => "default",
            Family::Scarce => "scarce",
            Family::Far => "far",
            Family::Volatile => "volatile",
        }
    }

    pub fn parse(s: &str) -> Result<Family, String> {
        FAMILIES.iter().copied().find(|f| f.name() == s).ok_or_else(|| format!("unknown family '{s}'"))
    }

    pub fn apply(self, w: &mut WorldParams) {
        match self {
            Family::Default => {}
            Family::Scarce => {
                w.patch_peak *= 0.6;
                w.mixed_cap *= 0.6;
                w.regrowth *= 0.7;
            }
            Family::Far => {
                // Patches toward opposite corners and in opposite temperature
                // zones: the commute is twice the default (18 cells centre to
                // centre), and both patches are 10 cells from the start.
                // Corner placement (1,1)/(14,14) was tried first and killed
                // 60% of organisms before any learning; that is not a
                // competing-priorities test, it is a lottery.
                w.food_center = (3, 3);
                w.water_center = (12, 12);
                w.spread_food_center = (3, 8);
                w.spread_water_center = (12, 8);
                w.regime_ops = vec![RegimeOp::SwapPatches, RegimeOp::RotateGradient, RegimeOp::Scarcity];
            }
            Family::Volatile => {
                w.regime_every = 2000;
            }
        }
    }
}
