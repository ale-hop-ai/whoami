# Phase 1 exit review: spec vs code

Date: 2026-10-22. Branch `feat/phase1-minimal-organism`. All of M1–M4 done.
Each row maps a spec statement to the code that implements it. Deviations
are listed at the end; each has a docs amendment in the same commit.

## 02-architecture.md

| Spec | Code |
|------|------|
| §1 toroidal 16×16, patches on opposite temperature sides, regrowth with env RNG | `src/env/mod.rs:210` |
| §1 regime ops are toggles | `src/env/mod.rs:198` |
| §1 relocated patch arrives full (amendment M2) | `src/env/mod.rs:181` |
| §2 body dynamics; E/H saturate at 1, Tb lethal at both ends | `src/body.rs:167`, `src/body.rs:200` |
| §3 processes return outputs; only `Organism::tick` writes `Signals` | `src/signals.rs:27`, `src/organism/mod.rs:110` |
| §3.1 edge-triggered danger | `src/organism/homeostasis.rs:45` |
| §3.2 5×5 window, no learning | `src/organism/perception.rs:46` |
| §3.3 record with drive-modulated confidence | `src/organism/memory.rs:127` |
| §3.3 forget, relax toward region prior | `src/organism/memory.rs:112` |
| §3.3 consolidate quadrant priors | `src/organism/memory.rs:170` |
| §3.3 nearest remembered target, `toward[]` | `src/organism/memory.rs:134` |
| §3.4 destination-cell features | `src/organism/world_model.rs:61` |
| §3.4 normalized LMS | `src/organism/world_model.rs:81` |
| §3.4 `at_remembered` (amendment M2) | `src/organism/world_model.rs:104` |
| §3.5 time-to-critical, reach | `src/organism/self_model.rs:58` |
| §3.6 urgency with anticipation | `src/organism/drives.rs:131` |
| §3.6 F1 priced by world model | `src/organism/drives.rs:148` |
| §3.6 F2 for a drive without a target (amendment M2) | `src/organism/drives.rs:157` |
| §3.6 bid = gain · urgency · value · (1 + habit) | `src/organism/drives.rs:173` |
| §3.6 gain up on danger, capped, slow relax | `src/organism/drives.rs:123` |
| §3.6 habit credit on success | `src/organism/drives.rs:112` |
| §3.7 arbitration sees bids and previous action only | `src/organism/arbitration.rs:25` |
| §5 lesions applied at the `Signals` write point | `src/organism/mod.rs:119`, `src/organism/lesion.rs:7` |
| §6.1 arbitration cannot see state | `src/organism/arbitration.rs:25` (signature takes `bids`, `prev`, params) |

## 03-experiments.md

| Spec | Code |
|------|------|
| §1 margin, viability, intake, anticipation, entropies, prediction MSE | `src/metrics/trajectory.rs:107` |
| §1 adaptation speed | `src/metrics/trajectory.rs:176` |
| §2 fingerprint: 108 fixed bins, α = 1 | `src/metrics/policy.rs:18` |
| §2 occupancy-weighted JS distance | `src/metrics/mod.rs:16` |
| §2 split-half floor | `src/metrics/mod.rs:58` |
| §2 continuity | `src/metrics/mod.rs:71` |
| §2 fork kinship | `src/metrics/mod.rs:86` |
| §2 predictive validity (caller passes leave-one-out population) | `src/metrics/mod.rs:99` |
| §2 individuality (one-way ICC) and shuffled null | `src/metrics/mod.rs:120`, `src/metrics/mod.rs:152` |
| §3 Oracle, MixedForager | `src/experiment/baselines.rs:37`, `src/experiment/baselines.rs:127` |
| §6 A5 check | run 2026-10-22, static default geometry, 3 seeds: Oracle margin 0.435 / intake 0.0040; MixedForager 0.405 / 0.0043 |

Not yet implemented (Phase 2): nested ICC, Reactive, GreedyError,
FixedPriority, Frozen, memory retention (needs live world access), runner,
reports, Shuffled lesion, fork null arms.

## 04-rust-design.md

| Spec | Code |
|------|------|
| §1 two dependencies | `Cargo.toml` |
| §4 per-subsystem RNG streams | `src/rng.rs:33` |
| §5 trajectory hash over `to_bits` | `src/sim.rs:63` |
| §5 snapshot carries both streams and the hash | `src/snapshot.rs:14` |
| §5 roundtrip test | `tests/determinism.rs:56` |
| §6 `run`, `fork`, `hash`, `metrics` | `src/main.rs` |
| fork reseeds the environment stream only | `src/lib.rs:122` |

## 05-plan.md acceptance

| Milestone | Criterion | Result |
|-----------|-----------|--------|
| M1 | same seed → same hash | test passes |
| M1 | random median lifetime 200–600 over 30 seeds | ~345 after tuning (`mixed_cap` 0.03, base drains 0.003) |
| M2 | EPI median ≥ 10× random, static world, 30 seeds | `tests/determinism.rs:42` passes |
| M2 | no non-finite `Signals` value in debug | `src/signals.rs:49`; debug test runs clean |
| M3 | snapshot roundtrip bit-identical | passes after enabling `serde_json/float_roundtrip` |
| M3 | fork from one snapshot with two env seeds diverges | test passes |
| M4 | metric unit tests | 34 lib tests pass |
| M4 | A5: Oracle and MixedForager within 20% on margin and intake | 7% and 8% |

## Deviations from the Phase 0 spec, all amended in docs

1. Self-model no longer estimates intake; the world model prices consumption
   at the remembered cell (`at_remembered`). One learned model, one currency.
2. F2 applies to a homeostatic drive that has no remembered target, scaled by
   that drive's urgency. Without it the organism starved in place after a
   patch swap.
3. Gains are capped (`gain_max`).
4. A relocated patch arrives full.
5. Energy and hydration saturate at 1 instead of being lethal above 1.

## Observations (not interpretations)

From one 20 000-tick EPI run (seed 3, shifting world, window 1000):
viability 1.00 throughout, margin 0.34–0.40, anticipation rate 0.85–0.95,
window-to-window fingerprint distance 0.03–0.10 against a split-half floor of
0.002–0.007. Over 20 seeds in the shifting world, 18 reach 20 000 ticks; the
two deaths follow regime shifts. Nothing here is a hypothesis verdict; those
need the Phase 2 runner, baselines and held-out seeds.
