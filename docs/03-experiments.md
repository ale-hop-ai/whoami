# EPI — Metrics, Hypotheses, Experiments

Everything in this file is pre-registered: thresholds are fixed here before
implementation and may only change by an explicit, dated amendment at the top of
this file.

Amendments: none.

## 1. Primary metrics

Computed from the JSONL event log and snapshots by the `metrics` module.
`T` = run length (default 20 000 ticks). `W` = window (default 500 ticks).
Statistics over seeds use ≥30 seeds; report median, IQR, and effect size
(Cliff's δ pairwise; nested ICC for individuality). Survival saturates at `T`
for competent agents, so **margin and efficiency are the primary comparison
metrics; survival is secondary** and analyzed with censoring.

| Metric | Definition |
|--------|-----------|
| Margin `M(w)` | mean over window of `min_d` distance of `v_d` to its nearest bound |
| Viability `V(w)` | fraction of ticks in window with all `|e_d| < 1` |
| Resource efficiency | intake per tick alive; moves per unit intake |
| Survival `S` | ticks alive, censored at `T` (Kaplan–Meier) |
| Adaptation speed `A` | ticks after a regime shift until rolling `V` returns to 90% of its pre-shift mean (censored at next shift) |
| Prediction accuracy | MSE between the **logged** `Δ̂` and realized deltas (post-lesion signal) |
| Behavioral diversity | entropy of action distribution per window; conditional entropy given state bin |
| Memory retention | fraction of **logged** `Remembered` targets that, k=200 ticks after last observation, still point to a cell holding ≥ half the remembered value |
| Anticipation rate | fraction of Eat/Drink taken while `|e_d| < 0.5` |
| Strategy occupancy | fraction of ticks in food patch / water patch / mixed zone |

## 2. Behavioral signature

`Signature(t)` over window `[t−W, t)`, three components, reported separately
and combined:

1. **Policy fingerprint** `P(a | b)`: action distribution conditioned on state
   bin `b` = (E bin × H bin × Tb bin × food visible × water visible) with
   **fixed absolute bins** at thirds of [0,1] (not terciles of a population),
   108 bins, Dirichlet-smoothed with `α = 1`. Observable from behavior alone.
   Distance: Jensen–Shannon divergence per bin, weighted by **pooled
   occupancy over all individuals and windows in the experiment**.
2. **Operating point**: mean E, H, Tb over window. Euclidean, normalized by
   pooled std at the same age.
3. **Learned parameters**: gains `g`, habits `h`, world model `θ`. Normalized
   Euclidean.

Combined distance = mean of normalized components. **All hypotheses must pass
on component 1 alone** (critique A3).

**Noise correction.** Every within-individual distance is reported alongside
its split-half floor: distance between the two halves of the same window.
A distance is "above noise" if it exceeds the split-half floor.

Derived quantities:

- **Individuality index** `I`: per-bin ICC of `P(a|b)` across individuals with
  successive windows as repeated measures, averaged with pooled occupancy
  weights. In E2 the ICC is **nested** (individual within development family)
  and the within-family component is the one tested. Null `I_null`: window
  labels shuffled across individuals. Report `I − I_null`.
- **Continuity** `C(t, Δ)` = `1 − d(S(t), S(t+Δ)) / d̄_stranger(t+Δ)`, with both
  distances noise-corrected. Only defined when `d̄_stranger` is above noise;
  H3 is therefore conditional on H2.
- **Fork kinship** `K(t)` = `d(A_t, B_t) / d̄(A_t, stranger_t)`.
- **Predictive validity** `PV(Δ)`: log-likelihood of an individual's actions in
  window t+Δ under its own smoothed fingerprint from window t, minus
  log-likelihood under the **leave-one-out** population fingerprint. Reported
  at `Δ = W` and `Δ = 5W`; the `5W` value is the one tested (short-lag
  autocorrelation inflates `Δ = W`).

## 3. Baselines

All baselines share Perception, Body and (where listed) Memory with EPI, and
the authored fragments F1/F2 verbatim.

| Name | Description | Isolates |
|------|-------------|----------|
| `Random` | uniform action | floor |
| `Reactive` | no memory, no learning. Lowest-margin variable chosen; if a matching resource is visible, F1 toward it; for Tb, step toward the visible cell with ambient closest to the Tb setpoint; else F2-style random walk | memory + learning + competitive arbitration |
| `GreedyError` | full EPI processes; selector = serve `argmax_d |e_d|` with that drive's best-valued action | competitive arbitration vs largest-error rule (E5) |
| `FixedPriority` | full EPI processes; selector = first drive in fixed order E > H > Tb with `|e_d| > 0` (threshold zero, so no reversal is possible) | competitive arbitration vs fixed order (E5) |
| `Frozen` | one common organism pretrained 10 000 ticks in `default` seed 0, snapshotted; all of `g`, `h`, `θ`, memory map frozen afterwards | value of *learning* for individuality and continuity |
| `Oracle` (mandatory) | authored commuter with full map | ceiling; A5 check |
| `MixedForager` (mandatory) | authored policy that never leaves the mixed zone | A5 check |

## 4. Hypotheses (falsifiable, pre-registered thresholds)

**H1 — Emergent adaptive behavior (E1).**
Median margin `M` and efficiency of EPI exceed `Reactive` with Cliff's
δ ≥ 0.5, and EPI survival is not worse than `Reactive` (Kaplan–Meier).
Supported if both hold; refuted if δ ≤ 0 on margin; **inconclusive**
otherwise. `Oracle` is reported as ceiling, not as a threshold.

**H2 — Individuality from history (E2).**
Organisms with identical config and identical organism seed, exposed to
different developmental environments for 10 000 ticks, then placed in the
same held-out test environment for 5000 ticks (first 1000 discarded as entry
transient), show within-family `I − I_null ≥ 0.3` on the policy fingerprint,
and `PV(5W) > 0` for ≥ 80% of individuals. `Frozen` under the same protocol
must show `I − I_null < 0.1`. Refuted if EPI `I − I_null < 0.1` or `PV(5W) ≤ 0`
for the majority; **not from learning** if `Frozen` matches EPI.

**H3 — Continuity (E2), conditional on H2 supported.**
Across each regime shift at tick t: `C(t−W, t+W) ≥ 0.5` and
`C(t−W, t+5W) ≥ 0.3`, with post-shift signature closer to own pre-shift than
to any other individual's post-shift for ≥ 70% of individuals.
Refuted if `C(t−W, t+W) < 0.2` while `d̄_stranger` is above noise.

**H4 — Fork kinship (E3).**
Arms: EPI fork, `Frozen` fork (null: identical parameters, no learning),
body-only fork (copy position and body, reset learned state). Supported if
EPI `K(t+Δ) < 0.7` for Δ ≤ 2000 **and** EPI `K` is below the body-only arm's
`K` at every Δ (kinship is carried by learned state, not by starting
position) **and** a sign test on consecutive `ΔK` shows non-decreasing `K`
(p < 0.05). Refuted if `K(t+W) ≥ 0.9` or if body-only kinship equals EPI
kinship.

**H5 — Component necessity (E4).** Lesion = matched null (process runs, output
replaced), compared to intact, Cliff's δ ≥ 0.5 on the primary deficit, and
every other listed metric has `δ_other ≤ δ_primary − 0.2`:

| Lesion | Null output | Predicted primary deficit |
|--------|-------------|---------------------------|
| Memory | `conf = 0` for all `Remembered`; neighbor conf = 0 | efficiency ↓; retention (logged) → 0 |
| World model | `Δ̂ = 0`, uncertainty = const | Eat/Drink/Rest indistinguishable → margin collapse; this is the predicted outcome, not a confound |
| Self-model | `ttc = T_max`, `reach = ∞` | anticipation rate ↓; margin ↓ in the `far` family |
| Drive adaptation (off) | `g = 1`, `h = 0` fixed | `I − I_null` ↓ toward `Frozen` level |
| Exploration | bids = 0 | adaptation speed ↓ after `SwapPatches` |

Deletion variants (process not stepped) are run and reported alongside.
Refuted per process if its predicted deficit does not appear.

**H6 — Context-dependent strategy (E5).**
Define "serves d" for a tick as the action with maximal `value_d(a)` as
logged. Reversal: two state bins with the **same rank order of errors**
(e.g. `|e_E| > |e_H|` in both) where the individual serves E in one bin and H
in the other, consistently (same sign in ≥ 80% of windows). Supported if
reversal bins exist for ≥ 70% of EPI individuals and EPI margin exceeds both
`GreedyError` and `FixedPriority` (δ ≥ 0.3). Refuted if no consistent
reversal bins exist or `GreedyError` matches EPI on margin.

## 5. Experiments

Each experiment is one config file plus a runner command. Outputs: config
hash, git commit, per-seed CSV, summary table, markdown report (§8).

**E1 — Emergent behavior.** `default` and `volatile` families, 20 000 ticks,
EPI vs all baselines, 30 seeds, identical environment seed across arms.

**E2 — Continuity and individuality.** 40 organisms, same config, same
organism seed. Development: 4 families × 10 environment seeds, 10 000 ticks.
Test: all 40 in one held-out `default` environment, 5000 ticks, 1000 burn-in.
Plus the `Frozen` arm. Outputs: nested `I`, `PV(W)`, `PV(5W)`, `C` at every
regime shift, family classification accuracy from test fingerprint.

**E3 — Forking.** 30 organisms, 5000 ticks, snapshot; each snapshot loaded
twice with **distinct** environment seeds (no seed reused anywhere in the
experiment), 5000 more ticks. Strangers = other organisms at the same age.
Three arms per H4. Report states that no claim about experience is made.

**E4 — Component removal.** E1 protocol in `default` and `far` families with
each lesion from H5, matched-null and deletion. Output: deficit matrix.

**E5 — Competing priorities.** `far` family. EPI vs `GreedyError` vs
`FixedPriority`. Output: reversal bins per individual, consistency, margin.

## 6. Confounds and controls

| Confound | Control |
|----------|---------|
| Seed variance | ≥30 seeds, effect sizes, intervals |
| Chaotic divergence masquerading as individuality | `Frozen` arm; `I_null`; `PV(5W)`; split-half noise floor |
| Family variance counted as individuality | nested ICC, within-family component tested |
| Entry transient in shared test env | 1000-tick burn-in |
| Parameter differences | identical config hash across individuals in E2/E3 |
| Designer tuning | frozen config, held-out environment seeds ≥100, config hash in every result |
| Metric circularity | all hypotheses must pass on policy fingerprint alone |
| Compute/noise removal in ablation | matched-null lesions, deletion reported alongside |
| Internal-state metrics unaffected by lesion | retention and accuracy computed from logged (post-lesion) signals |
| Survivorship | metrics at fixed ages; Kaplan–Meier for `S` |
| Survival ceiling | margin/efficiency primary |
| Environment luck | identical environment seed across arms within a comparison |
| Starting-state kinship in forks | body-only fork arm |
| Single optimum (A5) | `Oracle` and `MixedForager` within 20% on margin and efficiency, and EPI individuals spread across strategy occupancy; else retune geometry before E2 |

## 7. Failure criteria

Stop and report negative if any of:
- H1 refuted in held-out environments.
- H2 refuted, or `Frozen` matches EPI on `I` (individuality not from learning).
- `PV(5W) ≤ 0` for the majority.

Partial results are reported as such and guide Phase 4 priorities.

## 8. Report template

1. **Configuration** — config hash, seeds, families, commit.
2. **Observations** — numbers, tables, plots. No adjectives.
3. **Hypothesis verdicts** — supported / refuted / inconclusive, threshold and measured value.
4. **Interpretation** — labeled; what it may mean in the model; what it does not mean.
5. **Not claimed** — fixed paragraph: no claim about subjective experience; no claim about transfer of experience under snapshot; no claim beyond the tested scale; F1/F2 are authored.
