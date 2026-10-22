# EPI — Rust Project Design

Single crate, single binary. No workspace until a concrete need appears.

## 1. Dependencies

| Crate | Why | Alternative rejected |
|-------|-----|----------------------|
| `serde`, `serde_json` (`float_roundtrip`) | snapshots, config, JSONL logs | hand-written JSON: more code, more bugs |

Nothing else:
- RNG: own `xoshiro256**` + `splitmix64` seeding, ~50 lines. No `rand`.
- CLI: hand-parsed `std::env::args`. No `clap`.
- Hashing: own FNV-1a. No logging framework; one `Logger` over a `BufWriter`.

## 2. Layout

```
epi/
  Cargo.toml
  src/
    main.rs            CLI dispatch: run | fork | experiment | metrics | hash
    rng.rs             Rng (xoshiro256**), seed derivation per stream
    config.rs          Config structs (serde), load, hash of raw bytes
    env/
      mod.rs           World, step(&mut self, rng_env), regime schedule, families
      field.rs         resource grid, temperature field
    body.rs            Body, Action, apply(action, &mut World) -> Deltas
    signals.rs         Signals: per-tick typed bus, written only by Organism::tick
    organism/
      mod.rs           Organism, tick() wiring in fixed order, lesion application
      homeostasis.rs
      perception.rs
      memory.rs
      world_model.rs
      self_model.rs
      drives.rs
      arbitration.rs   WinnerTakeAll
      lesion.rs        Lesion enum and null outputs
    snapshot.rs        Snapshot save/load
    log.rs             Event enum + JSONL writer
    metrics/
      mod.rs           Signature, distances, I, C, K, PV, noise floor
      policy.rs        fingerprint binning, smoothing
    experiment/
      mod.rs           runner: seeds × arms, results dir, config hash, commit
      baselines.rs     Random, Reactive, GreedyError, FixedPriority, Frozen, Oracle, MixedForager
      report.rs        markdown report from results
  configs/
    default.json, families/*.json, e1_emergence.json … e5_priorities.json
  docs/
  tests/
    determinism.rs     same seed → same hash; snapshot roundtrip; lesion changes hash
```

Target size for Phase 1: ≤ 3000 lines of Rust excluding tests.

## 3. Core types and interfaces

Sketches, not code. Names are binding; bodies are not.

```rust
// rng.rs
pub struct Rng { s: [u64; 4] }
impl Rng {
    pub fn from_seed(seed: u64) -> Rng;        // splitmix64 expansion
    pub fn derive(&self, stream: u64) -> Rng;  // independent stream per subsystem
    pub fn next_u64(&mut self) -> u64;
    pub fn f64(&mut self) -> f64;              // [0,1)
    pub fn below(&mut self, n: usize) -> usize;
}

// body.rs
pub enum Action { MoveN, MoveS, MoveE, MoveW, Eat, Drink, Rest }   // index 0..7
pub const N_ACTIONS: usize = 7;
pub enum Var { Energy, Hydration, Temp }                             // index 0..3
pub const N_VARS: usize = 3;
pub const N_DRIVES: usize = 4;                                       // vars + Exploration
pub const N_RESOURCES: usize = 2;                                    // food, water
pub struct Body { pub pos: (u8, u8), pub v: [f64; N_VARS], pub alive: bool }
pub struct Deltas(pub [f64; N_VARS]);
impl Body { pub fn apply(&mut self, a: Action, w: &mut World, p: &BodyParams) -> Deltas; }

// signals.rs — the whole inter-process interface. Each field is produced by
// one process (named) and written by Organism::tick after lesioning.
pub struct Signals {
    pub obs: Observation,                       // Perception
    pub deltas: Deltas,                         // Body (previous tick)
    pub prev_action: Option<Action>,            // Arbitration (previous tick)
    pub errors: [f64; N_VARS],                  // Homeostasis
    pub danger_events: [bool; N_VARS],          // Homeostasis, edge-triggered
    pub predicted: [[f64; N_VARS]; N_ACTIONS],  // WorldModel: Δ̂[action][var]
    pub uncertainty: [f64; N_ACTIONS],          // WorldModel
    pub prediction_error: f64,                  // WorldModel
    pub here: [f64; N_RESOURCES],               // Perception: cell amounts
    pub yield_: [f64; N_RESOURCES],             // SelfModel: gain per unit consumed
    pub novel_remembered: [bool; N_RESOURCES],  // SelfModel
    pub novel_here: [bool; N_RESOURCES],        // SelfModel
    pub remembered: [Remembered; N_RESOURCES],  // Memory (current tick)
    pub remembered_prev: [Remembered; N_RESOURCES], // Memory (previous tick; SelfModel reads this)
    pub neighbor_conf: [f64; 4],                // Memory, per move direction
    pub ttc: [f64; N_VARS],                     // SelfModel
    pub reach: [f64; N_RESOURCES],              // SelfModel
    pub urgency_prev: [f64; N_DRIVES],          // Drives (previous tick)
    pub bids: [[f64; N_ACTIONS]; N_DRIVES],     // Drives
    pub values: [[f64; N_ACTIONS]; N_DRIVES],   // Drives: value_d(a), logged for H6
}
pub struct Remembered { pub target: (usize, usize), pub dist: usize, pub conf: f64, pub amounts: [f64; N_RESOURCES], pub toward: [bool; 4] }

// Each process: private fields; `fn step(&mut self, s: &Signals, p: &Params) -> Output`.
// Processes never receive &mut Signals.
impl Memory {
    pub fn step(&mut self, s: &Signals, p: &MemoryParams) -> MemoryOut;   // Remembered ×2, neighbor_conf
}
impl Drives {
    pub fn step(&mut self, s: &Signals, p: &DriveParams) -> DrivesOut;   // bids, values, urgency
}

// organism/arbitration.rs — sees bids and prev action only.
pub struct WinnerTakeAll { pub persistence: f64 }
impl WinnerTakeAll {
    pub fn select(&self, bids: &[[f64; N_ACTIONS]; N_DRIVES], prev: Option<Action>) -> Action;
}

// organism/lesion.rs
pub enum Lesion { Intact, Null, Deleted }   // Shuffled dropped at M6: no hypothesis uses it
pub struct Lesions {
    pub memory: Lesion, pub world_model: Lesion, pub self_model: Lesion,
    pub exploration: Lesion, pub drive_adaptation: bool,   // bool: not an output
}
// Null outputs are the ones in 03-experiments H5. `Deleted` skips the process
// step entirely (deletion arm).

// Agent trait — organism and baselines share one shape.
pub trait Agent {
    fn step(&mut self, w: &mut World, rng_org: &mut Rng, log: &mut Logger) -> Action;
    fn body(&self) -> &Body;
}
// Baselines wrap an Organism with Drives+Arbitration replaced by a selector
// that may read `errors` (GreedyError, FixedPriority), or with learning
// disabled after load (Frozen), or are standalone (Random, Oracle, MixedForager).

// organism/mod.rs
pub struct Organism { /* processes, body, lesions, params */ }
impl Organism {
    /// Fixed order from 02-architecture §5. Builds Signals, steps each
    /// process, applies lesions at the write point, arbitrates, applies action.
    pub fn tick(&mut self, w: &mut World, rng_org: &mut Rng, log: &mut Logger) -> Action;
}

// Run loop (experiment/mod.rs):
//   loop { world.step(&mut rng_env); let a = agent.step(&mut world, &mut rng_org, &mut log); hash.update(...) }

// snapshot.rs
pub struct Snapshot { pub tick: u64, pub config_hash: u64, pub world: World,
                      pub agent: AgentState, pub rng_env: Rng, pub rng_org: Rng }

// log.rs
pub enum Event<'a> {
    Tick { t: u64, action: Action, body: [f64; 3], errors: [f64; 3],
           bids: &'a [[f64; 7]; 4], values: &'a [[f64; 7]; 4], predicted: &'a [[f64; 3]; 7],
           remembered: &'a [Remembered; 2], prediction_error: f64, gains: [f64; 4] },
    Danger { t: u64, var: Var }, Regime { t: u64, op: RegimeOp },
    Death { t: u64, cause: Var }, Snapshot { t: u64, path: String },
}
```

## 4. State ownership

- `World` is owned by the run loop; only `World::step` and `Body::apply` mutate it.
- Each process owns its parameters and learned state. Fields are private;
  outputs cross process boundaries only as return values that `Organism::tick`
  copies into `Signals`. Processes receive `&Signals`, never `&mut`.
- `Signals` is built fresh each tick inside `Organism::tick` from the previous
  tick's carried values (`deltas`, `prev_action`, `remembered_prev`,
  `urgency_prev`) and dropped at the end. No process holds a reference to it.
- RNG: `rng_env` (world regrowth, regime draws) and `rng_org` (Shuffled
  lesions, baseline random walks) are separate streams derived from the run
  seed. Phase 1 EPI organism itself uses no randomness (WTA ties by action
  order). Nothing uses global state.
- Lesions are applied by `Organism::tick` at the single write point for each
  output; the lagged copies are taken from the lesioned value.

## 5. Determinism and reproducibility

- All float math in fixed order; no `HashMap`/`HashSet` on any state path
  (arrays and `Vec`).
- No `powf`/`exp`/`ln` in the organism: `γ^dist` by repeated multiplication
  with `dist` an integer; Softmax is not in Phase 1. Metrics may use `ln`
  (PV) and are documented as deterministic **per toolchain and platform**, not
  bit-identical across platforms. The same caveat applies to the trajectory
  hash; CI compares hashes on one pinned toolchain.
- Every `Signals` field is asserted finite in debug builds. `ttc` is clamped
  to `[0, T_max]`, `reach` to `[1, ∞)` with `∞` encoded as `T_max`.
- Snapshot JSON: f64 through `serde_json` with the `float_roundtrip` feature
  (without it, parsing is not correctly rounded and a resumed run drifts by
  one ulp in derived fields; found by the roundtrip test). Non-finite values
  are a bug caught by the debug assertion, not a supported state.
- Trajectory hash: FNV-1a over `(t, action index, body.v[i].to_bits())`.
- Config hash: FNV-1a over the **raw config file bytes**, not a re-serialized value.
- Snapshot roundtrip test: run to N, snapshot, continue to M; equals direct run to M.
- Every results directory records config hash and git commit.

## 6. CLI

```
epi run        --config C --seed S --ticks T --out DIR [--snapshot-every K] [--agent epi|random|reactive|...]
epi fork       --snapshot FILE --env-seed S --ticks T --out DIR
epi experiment --name e1|e2|e3|e4|e5 --out DIR [--seeds N]
epi metrics    --log FILE [--window W]
epi hash       --config C --seed S --ticks T
```

Headless, exit code 0 on success, non-zero with a one-line error otherwise.

## 7. Testing

- Unit tests per process on hand-constructed `Signals`.
- Body dynamics: monotonicity (eating raises E; cold lowers E faster).
- Homeostasis: danger is edge-triggered (fires once per crossing).
- Arbitration: persistence works; ties deterministic.
- Memory: forgetting decays confidence; region prior survives cell forgetting; `Null` lesion yields `conf = 0`.
- Drives: habit increases only on credited success; gain relaxes with `τ_g`.
- Metrics: fingerprint distance 0 for identical logs; `I` on synthetic data with known between-variance; split-half floor on i.i.d. data ≈ distance.
- Integration: determinism, snapshot roundtrip, every lesion changes the hash, random policy dies within configured bounds.
