//! Configuration. Every parameter the experiments freeze lives here.
//! JSON files override fields; missing fields take the defaults below.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::env::{RegimeOp, WorldParams};
use crate::body::BodyParams;
use crate::organism::OrganismParams;
use crate::hash::hash_bytes;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub world: WorldParams,
    pub body: BodyParams,
    pub organism: OrganismParams,
}

pub struct Loaded {
    pub config: Config,
    /// FNV-1a over the raw file bytes (0 for the built-in default).
    pub hash: u64,
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Loaded, String> {
        match path {
            None => Ok(Loaded { config: Config::default(), hash: 0 }),
            Some(p) => {
                let bytes = std::fs::read(p).map_err(|e| format!("read {}: {e}", p.display()))?;
                let config: Config = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("parse {}: {e}", p.display()))?;
                Ok(Loaded { config, hash: hash_bytes(&bytes) })
            }
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let w = &self.world;
        if w.width < 4 || w.height < 4 {
            return Err("world must be at least 4x4".into());
        }
        if w.regime_every == 0 {
            return Err("regime_every must be > 0".into());
        }
        if w.regime_ops.is_empty() {
            return Err("regime_ops must not be empty".into());
        }
        for op in &w.regime_ops {
            let _: &RegimeOp = op;
        }
        Ok(())
    }
}
