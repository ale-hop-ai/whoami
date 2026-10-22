# EPI — Staged Plan and Acceptance Criteria

Gate: nothing in Phase 1 starts until this Phase 0 package is approved.
Each milestone ends with a conventional commit on a feature branch in a
worktree under `~/worktrees/whoami/<branch>`; nothing is pushed or merged
automatically.

## Phase 1 — Minimal organism

**M1 — Skeleton.** `rng`, `config`, `env`, `body`, `log`, `Random` policy,
`epi run`, `epi hash`.
Acceptance:
- Two runs, same config and seed → identical trajectory hash (test).
- `Random` policy median survival in 200–600 ticks over 30 seeds in `default.json`.
- JSONL log parses; every tick has a `Tick` event.

**M2 — Processes and arbitration.** All eight processes, `Signals`,
`WinnerTakeAll`, gain and habit adaptation, F1/F2 as named config entries.
No lesions yet.
Acceptance:
- Median survival ≥ 10× `Random` over 30 seeds in the static environment.
- Debug build: no non-finite value in any `Signals` field over 30 seeds.
- Unit tests per process pass.
- `Tick` events include bids, errors, gains, prediction error.
- `cargo clippy` clean; no `HashMap` on state paths.

**M3 — Snapshots and forks.** `snapshot.rs`, `epi fork`, roundtrip test.
Acceptance:
- Snapshot roundtrip hash equality test passes.
- `epi fork` from one snapshot with two env seeds yields two diverging logs.

**M4 — Metrics and A5 check.** Signature, distances, split-half floor, nested
`I`, `C`, `K`, `PV`, `epi metrics`; `Oracle` and `MixedForager` (mandatory).
Acceptance:
- Metric unit tests on synthetic logs pass.
- Degenerate-optima check: `Oracle` and `MixedForager` within 20% of each
  other on margin and efficiency in `default.json` (uncensored metrics, not
  survival); if not, geometry is retuned and the change logged before M5.

Phase 1 exit: all of the above; Phase 1 spec section (`02-architecture.md`)
validated against code with `path:line` citations in a short review note.

## Phase 2 — Experimental framework

**M5 — Runner, baselines, configs.** `experiment/`, `Reactive`,
`GreedyError`, `FixedPriority`, `Frozen` (pretrained snapshot); `e1`–`e5`
configs; environment families; results directories with config hash and commit.
Acceptance: `epi experiment --name e1` runs end to end and emits the report
template sections.

**M6 — Lesions and fork arms.** `lesion.rs`, matched-null and deletion
variants wired into `Organism::tick` at the write point; `Frozen` fork and
body-only fork arms. Acceptance: each lesion changes the trajectory hash;
matched-null leaves the process stepping; lagged readers see lesioned values.

**M7 — Freeze and held-out runs.** Freeze `configs/*.json`, record hash, run
E1–E5 on held-out environment seeds (≥100). Acceptance: all five reports
generated; each hypothesis has a verdict against its pre-registered threshold;
observations and interpretations separated.

Phase 2 exit: a results summary document with verdicts for H1–H6 and a
decision on Phase 4 per `03-experiments.md` §7.

## Phase 3 — Visualization

Only if Phase 2 produced something worth looking at. Minimal candidate: `epi
render` emits SVG frames or a single static HTML file that reads a JSONL log
(no new Rust dependencies). Decision deferred to a short Phase 3 spec.

## Phase 4 — Advanced mechanisms (conditional)

Candidates, each requiring its own spec and a concrete deficit from Phase 2
that motivates it:
- Replace F1 with learned multi-step per-variable prediction (removes the
  authored navigation rule).
- Setpoint adaptation (allostatic homeostasis).
- `Softmax` / `LateralInhibition` arbitration if WTA dithers.
- Per-drive model-free learners (Q_d) alongside the shared world model.
- Richer environment (hazards, terrain, multiple patches).
- Memory consolidation with replay.
- Learned representations only if the linear world model is shown to be the
  bottleneck for adaptation speed.

## Budget and scope guards

- Phase 1 ≤ 3000 lines of Rust; two dependencies.
- No multi-organism, evolution, networks, or GUI before Phase 4.
- Any new dependency or module outside `04-rust-design.md` §2 requires a spec amendment.
