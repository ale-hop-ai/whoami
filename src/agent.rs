//! One shape for the organism and every baseline.

use crate::body::{Action, Body};
use crate::env::World;
use crate::log::{Logger, OrgTick};
use crate::rng::Rng;

pub trait Agent {
    /// Choose and apply one action. The implementation calls `Body::apply`.
    fn step(&mut self, w: &mut World, rng: &mut Rng, log: &mut Logger) -> Action;
    fn body(&self) -> &Body;
    /// Internal state for the tick log, if the agent has any.
    fn org_tick(&self) -> Option<&OrgTick> {
        None
    }
}
