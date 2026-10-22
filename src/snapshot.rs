//! Full state capture: world, organism, RNG streams and the running
//! trajectory hash. Loading a snapshot and continuing is bit-identical to
//! never having stopped (tests/determinism.rs).

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::env::World;
use crate::hash::Fnv;
use crate::organism::Organism;
use crate::rng::Rng;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub config_hash: u64,
    pub world: World,
    pub organism: Organism,
    pub rng_env: Rng,
    pub rng_org: Rng,
    pub trajectory_hash: u64,
}

impl Snapshot {
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string(self).map_err(|e| format!("serialize snapshot: {e}"))?;
        std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
    }

    pub fn load(path: &Path) -> Result<Snapshot, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("parse {}: {e}", path.display()))
    }

    pub fn hash_state(&self) -> Fnv {
        Fnv::from_state(self.trajectory_hash)
    }
}
