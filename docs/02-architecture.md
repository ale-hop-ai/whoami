# EPI — Minimal Experimental Architecture

Everything below is the smallest configuration that can run E1–E5. All numbers
are initial defaults, exposed in config, and frozen before held-out runs.

## 1. Environment

**World.** Toroidal grid 16×16. Each cell holds `food ∈ [0,1]`, `water ∈ [0,1]`,
`ambient ∈ [-1,1]` (temperature, −1 cold, +1 hot). The temperature field is
static within a regime.

**Resource layout.** Food concentrates in one patch, water in another, with a
sparse mixed zone between. Regrowth is logistic toward a per-cell cap with a
stochastic term from the environment RNG stream.

**Temperature field.** Linear gradient across the grid. Default layout puts the
food patch on the cold side and the water patch on the hot side, so every need
costs something in another need. This is the source of competing requirements.
No day/night cycle in Phase 1 (cut: the static gradient already creates the
conflict, and a cycle aliases with the metric window).

**Regimes.** Every 5000 ticks the regime changes by one scheduled operation
drawn from the environment seed. Operations are toggles, so the schedule
cannot compound into an unviable world: `SwapPatches`, `RotateGradient` (90°),
`Scarcity` (halve caps ↔ restore), `Spread` (move patches to opposite corners
↔ restore). A relocated patch arrives full: the organism's problem is to find
it, not to wait for regrowth. The schedule is deterministic given the seed and
logged.

**Environment families.** A family is a named set of generator parameters
(patch size, gradient strength, regrowth rate, regime op set). Phase 2 defines
four: `default`; `scarce` (caps and regrowth reduced); `far` (patch centres
at (3,3) and (12,12), both ten cells from the start, commute twice the
default; corner placement was tried first and killed 60% of organisms before
any learning, which tests luck, not priorities); `volatile` (regime every
2000 ticks). Seeds 0–99 per family are development, 100+ held-out.

**Degenerate optima (required, critique A5).** The `default` geometry must
admit two strategies with similar payoff: *commute* (shuttle between patches)
and *mixed-zone forage* (stay between them). Verified in M4 with two authored
fixed policies on uncensored metrics (margin, efficiency, strategy occupancy),
not survival.

Out of scope for Phase 1: predators, other organisms, terrain, tools, cycles.

## 2. Body (physiology)

Internal variables, each in [0,1]. Energy and hydration saturate at 1 and are
lethal at 0; body temperature is lethal at both ends (amendment M1: eating to
death is not a failure mode worth modeling).

| Var | Meaning | Decay per tick | Restored by |
|-----|---------|----------------|-------------|
| E | energy | base + move cost + cold penalty(Tb) | Eat (bite from cell food) |
| H | hydration | base + heat penalty(Tb) | Drink (bite from cell water) |
| Tb | body temperature | relaxes toward ambient at rate k; moving adds heat | Position and activity |

Actions: `MoveN, MoveS, MoveE, MoveW, Eat, Drink, Rest`. Rest halves base decay.
The body is plain dynamics, no decisions. Tuning target: a random policy dies
in 200–600 ticks; the oracle keeps all variables inside the viable band for
the full run but not at the setpoints (so margin and efficiency stay
informative when survival saturates).

## 3. Internal processes

Each process is a struct that owns its state. It reads a `Signals` snapshot
and **returns** its outputs; only `Organism::tick` writes `Signals`. No process
reads another's private state. Order of execution per tick is fixed (§5).

Eight processes: Homeostasis, Perception, Memory, World model, Self-model,
Drives, Arbitration, Action. Adaptation is not a ninth process; it is the
learning inside Memory, World model, Self-model and Drives.

### 3.1 Homeostasis (interoception)
Owns: setpoints `s_d`, widths `w_d`, danger thresholds. Input: body variables.
Output: error vector `e_d = (s_d − v_d)/w_d`; danger events, **edge-triggered**
(fires once when `v_d` crosses into the danger band, not every tick inside it).
Phase 1: fixed setpoints.

### 3.2 Perception
Owns: window radius 2 (5×5). Input: environment, body. Output: `Observation` =
local food/water/ambient grid, interoceptive vector, previous action. No
learning.

### 3.3 Memory
Owns: spatial map (per cell, per resource: estimate `v̂`, confidence `c`) and
region priors (per quadrant mean, recomputed every K ticks from
high-confidence cells; used only as the forgetting target).
Input: observation, previous-tick drive urgencies, prediction error.
- **Record.** Observed cells: `v̂ ← obs`, `c ← min(1, 1 + α·urgency_prev[r])`
  for the matching drive. Hungry organisms encode food more strongly.
- **Forget.** `c ← ρ·c` for unobserved cells; `v̂` relaxes toward the region prior.
Output: per resource r ∈ {food, water}: `Remembered { target, dist, conf,
amounts, toward }` for the cell maximizing `c·v̂·γ_m^dist` among cells with
`c·v̂` above threshold (`conf = 0` if none; amendment M5: the nearest
qualifying cell kept the organism grazing a patch edge); mean confidence of
the four neighbor cells (for Exploration).
Cut from Phase 1: episodic event ring (nothing read it; danger and surprise
events go to the log instead).

### 3.4 World model (prediction)
Owns: per-action, per-variable linear weights `θ_{a,d}` over
`φ(obs, a) = [1, dest food, dest water, dest ambient, E, H, Tb]` where *dest* is
the cell the action lands on (the current cell for non-moves). Running squared
error `u_{a,d}` (uncertainty).
Learn: normalized LMS on `Δ̂_d = θ_{a,d}·φ` against the realized body delta for
the action taken. Output: `Δ̂[a][d]`, `uncertainty[a]`, scalar prediction
error. The world model prices costs and dynamics (moves, rest, temperature);
consumption gains are priced by the self-model's yield (§3.5, amendment M5).
Because `φ` carries destination features, the model can learn that moving
toward the hot side raises Tb and that eating on a food cell raises E, without
any rule naming those relations.

### 3.5 Self-model
Owns: EMA drift rate per variable `ḋ_d`; **yield per unit** `ŷ_r` (EMA of
realized gain ÷ amount on the cell, over Eat/Drink ticks); the largest
amount ever consumed at, `max_r`.
Input: realized deltas, previous action and the cell amounts before it,
current cell amounts, previous-tick `Remembered`.
Output: `ttc_d = clamp((v_d − danger_d)/max(−ḋ_d, ε), 0, T_max)`;
`reach_r = Remembered[r].dist` in ticks; `ŷ_r`; novelty flags
`novel_here[r] = here_r > 2·max_r + 0.02` and likewise `novel_remembered[r]`
for the remembered amount.
Reads Memory's previous-tick output (documented lag, see §5).
Amendment history. M2: the intake EMA was dropped in favor of the world
model's prediction at the remembered cell. M5: that was reversed. The linear
world model shares seven features across all actions; normalized LMS spreads
one or two patch-edge samples across the bias and the interoceptive
features, so the predicted gain at a remembered patch stayed near zero and in
the `far` family every organism starved between two patches it remembered.
Gain per unit consumed is a capability of the body, which is what the
self-model is for; it extrapolates to richer cells by construction, which is
a declared inductive bias ("gain is proportional to amount").

### 3.6 Drives
Four drives: Energy, Hydration, Temp, Exploration. Each owns gain `g_d`,
anticipation `λ_d` (λ_Temp = 0: no resource to reach), habit `h_{d,a}`.
A drive sees only its own variable's error, ttc, reach, intake, and `Δ̂[·][d]`.

Urgency: `u_d = e_d·|e_d| + λ_d · max(0, 1 − ttc_d / reach_d)` (reach = ∞ when
nothing remembered → anticipation term 0).

Bid: `bid_d(a) = g_d · u_d · value_d(a) · (1 + h_{d,a})`, where
- for Eat (Energy) and Drink (Hydration): `value = ŷ_r · here_r` (self-model
  yield times what is on the cell); for Rest, the other consumption, and all
  actions of the Temp drive: `value_d(a) = Δ̂[a][d]` from the world model;
- for moves of Energy/Hydration with a remembered target: **authored
  fragment F1**: `value = conf · ŷ_r · amount_r · γ^dist` if `a` reduces
  distance to the remembered resource, else `Δ̂[a][d]`. This is a hand-written
  rule mapping memory to movement; the price is learned. It is declared here,
  listed in the frozen config, and is a Phase 4 replacement target
  (per-variable multi-step prediction). Critique A1 is weakened accordingly:
  E1 tests memory, learned consumption, and arbitration, not emergent
  navigation.
- for moves of Energy/Hydration with **no** remembered target: **authored
  fragment F2**: `value = seek_unknown · (1 − neighbor_conf[a])`. A drive that
  has lost its resource searches where memory is weakest, scaled by its own
  urgency. (Amendment M2: without this, a drive whose target vanished after a
  regime shift had no positive-valued action and the organism starved in place.)
- Exploration: urgency = `explore_scale · sqrt(mean uncertainty)`; F2 again
  (least-known neighbor) plus a term favoring the action with the highest
  relative `uncertainty[a]`, plus **novelty** (amendment 2026-10-22, M5):
  value for moving toward a remembered cell whose amount exceeds twice the
  largest amount the organism has ever consumed at (`novel_remembered`, from
  the self-model), and, standing on such a cell (`novel_here`), value for
  consuming there that outranks any pull elsewhere. Before the first
  successful consumption `ŷ_r = 0`, so novelty is what takes the organism to
  its first patch and makes it try.

**Adaptation (three carriers, each pre-registered).**
- *Gain, fast up / slow down.* On an edge-triggered danger event for d:
  `g_d ← min(g_d + η, g_max)`. Otherwise `g_d` relaxes toward 1 with time
  constant `τ_g = 20 000` ticks (the full run), not quickly. This is negative feedback
  with long hysteresis, and is named as such.
- *Habit, positive feedback.* When drive d's bid for the chosen action was the
  largest contributor and `e_d` decreased on the next tick:
  `h_{d,a} ← h_{d,a} + η_h · (1 − h_{d,a})`; all `h` decay with
  `τ_h = 50 000` ticks. A drive that gets served by an action makes that
  action more likely to be chosen for it, which serves it more. This is the
  state→behavior positive feedback of Sih et al. (2015) and the intended
  engine of divergence. `η_h` is small; lock-in is a measurable outcome, not a
  bug.
- *Learned parameters* `θ` (world model) and the memory map carry history by
  construction.

### 3.7 Arbitration
Content-free: input is the `4 × 7` bid matrix and the previous action, nothing
else. Phase 1 has one variant:
- `WinnerTakeAll { persistence, drive_persistence }`:
  `argmax_a Σ_d w_d·bid_d(a) + π·[a = prev]` with `w_d = 1 + ρ` for the drive
  that contributed most to the previous winning action and 1 otherwise; ties
  by fixed action order (no RNG). Amendment 2026-10-22 (M5): `ρ` was added
  after the `far` family showed dithering: with both patches ten cells away
  the hungrier drive won until its variable improved slightly, the other
  drive then won and the organism turned around, and it starved between the
  patches. Motivation-level hysteresis is the ethological remedy (McFarland &
  Sibly 1975). Arbitration still sees only bids and its own last winner.
`Softmax` and `LateralInhibition` are cut from Phase 1 (nothing in E1–E5 needs
them); Phase 4 candidates if WTA shows dithering.
Baseline selectors (`FixedPriority`, `GreedyError`) are **not** arbitration
variants: they need errors, so they live in `experiment/baselines.rs` and
replace the Drives+Arbitration stage wholesale. Arbitration proper never sees
state.

### 3.8 Action
Applies the chosen action to body and environment. Pure dynamics.

## 4. Coupling graph

Every arrow is a field in `Signals`, written by `Organism::tick` from the
producing process's return value:

```
Body ──v, deltas──▶ Homeostasis ──e, danger(edge)──▶ Drives
Body ──deltas──▶ WorldModel (learn), SelfModel
Env  ──obs──▶ Perception ──obs──▶ Memory, WorldModel (φ, prev obs kept by WM)
WorldModel ──Δ̂, uncertainty──▶ Drives
WorldModel ──prediction error──▶ Memory (salience)
Memory ──Remembered, neighbor conf──▶ Drives, SelfModel (lagged one tick)
SelfModel ──ttc, reach, yield, novelty──▶ Drives
Perception ──here (cell amounts)──▶ Drives (consumption value), SelfModel (yield)
Drives ──urgency──▶ Memory (lagged one tick)
Drives ──bids──▶ Arbitration ──action──▶ Body/Env
Arbitration ──prev action──▶ Arbitration (persistence), Drives (habit credit)
```

Two documented one-tick lags: Memory reads previous-tick urgency; Self-model
reads previous-tick `Remembered`. Both avoid within-tick cycles.

## 5. Tick order

1. Environment step (regrowth, regime schedule).
2. Perception → `obs`.
3. World model: learn from (prev obs, prev action, realized deltas); predict for `obs`.
4. Self-model update (deltas, prev action, prev-tick `Remembered`).
5. Homeostasis → errors, edge-triggered danger events.
6. Memory update (obs, prev-tick urgency, prediction error) → `Remembered`.
7. Drives: habit credit from prev action and error change; gain adaptation from danger events; → bids, urgency.
8. Arbitration → action.
9. Action applied → deltas, env mutation.
10. Lesions are applied by `Organism::tick` at the single point where each
    output is written into `Signals`, so every downstream reader (including
    lagged ones) sees the lesioned value.
11. Log event, update metrics.

## 6. Why there is no hidden central agent

1. Arbitration cannot see state. Inputs: bids and the previous action.
2. No process has read access to another's private state; processes return
   outputs and only `Organism::tick` assembles `Signals`.
3. No scalar over the whole state is computed anywhere except the bid sum,
   which has no parameters.
4. **Two authored policy fragments exist and are declared:** F1 (move toward
   remembered resource, distance-discounted) and F2 (explore toward low memory
   confidence and high model uncertainty). They are listed in the frozen
   config, shared verbatim with the `Reactive` baseline so E1 does not credit
   them to EPI, and are Phase 4 replacement targets. No other rule refers to a
   situation.
5. What remains central: one body, one action per tick. That is the "final
   common path" and is biological, not a cheat.

## 7. Identity as measurement

Identity is not a struct. It is `Signature(t)`, computed by the metrics module
from logs and snapshots (`03-experiments.md` §2). The organism never reads it.

## 8. Explicitly not in the architecture

- No personality parameters, no traits, no "character".
- No global reward, no planner, no value function over full state.
- No neural networks, no LLMs. No other organisms. No visualization in Phase 1.
