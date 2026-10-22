//! Authored policies. They share Body and Perception with the organism and
//! exist to bound what the organism's machinery buys.

use crate::agent::Agent;
use crate::body::{Action, Body, BodyParams, ACTIONS};
use crate::env::World;
use crate::log::Logger;
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
