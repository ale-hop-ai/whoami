//! Component removal for E4. `Null` keeps the process stepping and replaces
//! its output with a semantically empty value; `Deleted` skips the step.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lesion {
    #[default]
    Intact,
    Null,
    Deleted,
}

impl Lesion {
    pub fn runs(self) -> bool {
        self != Lesion::Deleted
    }

    pub fn passes(self) -> bool {
        self == Lesion::Intact
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Lesions {
    pub memory: Lesion,
    pub world_model: Lesion,
    pub self_model: Lesion,
    pub exploration: Lesion,
    /// Gains and habits frozen at their current values.
    pub drive_adaptation_off: bool,
}
