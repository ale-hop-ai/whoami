# Emergent Process Identity — Phase 0 package

Status: draft for approval, revised after one adversarial review (25 findings,
all addressed or explicitly declared). No implementation exists; Phase 1 does
not start until this package is approved.

| File | Deliverable |
|------|-------------|
| `00-formulation.md` | Scientific and philosophical formulation; claims decomposed into testable / untestable; lineage with sources |
| `01-critique.md` | Strongest assumptions, weaknesses, refutation conditions |
| `02-architecture.md` | Minimal environment, body, the eight processes, coupling graph, arbitration, why there is no hidden central agent |
| `03-experiments.md` | Metrics, behavioral signature, baselines, hypotheses H1–H6 with pre-registered thresholds, experiments E1–E5, confounds, failure criteria, report template |
| `04-rust-design.md` | Crate layout, dependencies, interfaces, state ownership, determinism, CLI, tests |
| `05-plan.md` | Milestones M1–M7 with acceptance criteria; Phase 3/4 gates |

Reading order is numeric. The one-paragraph version:

> One organism, one 16×16 world with food, water and a temperature gradient
> that puts them in conflict. Three internal variables that kill it if they
> leave range. Eight processes that each own their state and talk only through
> a typed per-tick bus. Drives bid; a content-free arbitration rule picks one
> action. Identity is never represented; it is measured afterwards as a
> behavioral signature, and the hypotheses are about whether that signature is
> stable, history-dependent, continuous under change, structured after a fork,
> and degraded in specific ways by lesions. Thresholds are fixed before any
> code is written. Nothing here is about subjective experience.
