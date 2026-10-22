# EPI — Critique

The hypothesis as stated in the project brief is attractive and under-specified.
This file lists the assumptions it rests on, the ways the project can fool
itself, and the design constraint each one imposes. Every constraint here is
carried into `02-architecture.md` or `03-experiments.md`.

## 1. Strongest assumptions

**A1. "Emergent" is a claim about where behavior content lives, not about
magic.** Everything in the system is designed. What can be non-designed is the
*content* of behavior: which actions in which contexts, which places are
remembered, which drive dominates when. The claim must be stated as: *the
behavioral policy is not written down anywhere; it is a product of learned
state plus a content-free arbitration rule.* Constraint: the arbitration rule
may not inspect environment or internal variables, only bids; and the
designers may not tune per-situation behavior. Where an authored rule is
unavoidable in Phase 1 (movement toward a remembered resource, exploration),
it is declared as a named fragment, shared with the baselines so it earns EPI
no credit, and scheduled for replacement (`02-architecture.md` §6.4).

**A2. Individuality is cheap unless the null model is strong, and it decays
unless something carries it.** Adaptation rules that relax toward a default
are negative feedback: whatever history writes, time erases. At least one
carrier must be slow or self-reinforcing (the habit term, `02-architecture.md`
§3.6), and its time constants are pre-registered. Two seeds of a
random walk are "different individuals". So are two chaotic trajectories.
Constraint: individuality counts only if (a) differences are stable across
time within an individual, (b) they are larger between individuals than within,
(c) they are caused by history, not parameters or seed, and (d) the signature
at time t predicts the individual's own behavior at t+Δ better than the
population average does. (d) is the non-circular test: identity must be
predictive, not just descriptive.

**A3. Identity-as-measurement is circular if we pick one signature.** We define
a pattern, then find it. Constraint: pre-register several signature components
(policy fingerprint, operating point, learned parameters, memory map) and
require that conclusions hold on the policy fingerprint alone, which is
observable from behavior without reading internal state.

**A4. "No global utility function" is not fully achievable.** Keramati & Gutkin
(2014) show that per-drive error reduction is a reward; summing bids at
arbitration is a common currency; McFarland & Sibly (1975) argue a common
currency is unavoidable for a single body. What we can honestly claim:
no process computes a value over the whole state; each drive sees only its
own variable; the summation is explicit, parameter-free, and swappable; and
Experiment 5 tests whether the result differs from a fixed priority order.
Any report must say "no authored global objective", not "no objective".

**A5. A toy environment may have one optimum.** If the world admits a single
best strategy, every organism converges to it and individuality is just
noise around convergence. Constraint: the environment must have at least two
viable strategies of similar payoff (degenerate optima) so that history can
select between them. This is a design requirement, not an outcome, and must
be verified with baselines before E2 is run.

**A6. Continuity needs a threshold decided in advance.** "Changed a lot but still
itself" is unfalsifiable without numbers. Constraint: continuity is defined as
self-distance at lag Δ divided by stranger-distance at the same age, both
noise-corrected by a split-half floor, with pre-registered thresholds, and it
is only defined when strangers are distinguishable (H3 conditional on H2).

**A7. Forks always diverge.** Sensitive dependence guarantees two forks differ
eventually. The interesting question is whether divergence is *structured*:
do forks stay closer to each other than to strangers, and for how long?
Constraint: E3 measures kinship decay, not divergence, against two nulls: a
frozen-parameter fork (no learning) and a body-only fork (same position and
body, learned state reset). Kinship must be carried by learned state.

**A8. Ablation confounds capacity with function.** Deleting a process also
removes its compute, its noise, and its side effects. Constraint: lesion by
*matched null* (replace outputs with a semantically empty value or a shuffled
stream of real outputs), never by deletion alone; report both; measure the
deficit on logged signals, not on internal state the null leaves running.

## 2. Weaknesses of the proposed model

**W1. It is a known architecture.** Multi-drive homeostatic agents with
competitive action selection exist (Tyrrell, Maes, Cañamero, Bach). Claiming
novelty for the agent would be wrong. Novelty, if any, is the measurement
program for individuality, continuity and fork kinship. Say so.

**W2. The processes are chosen by us.** The eight "responsibilities" are a
human taxonomy. An organism that needs them to survive will use them; that
does not show they are the right decomposition. E4 can show which ones matter
here. It cannot show they are necessary in general.

**W3. Learning algorithms smuggle in structure.** A tabular or linear learner
with hand-picked features is a strong prior on what the organism can learn.
The features are designer choices. Mitigation: keep features generic (local
resource presence, interoceptive state) and list them as part of the frozen
pre-registered configuration.

**W4. Single lineage, no evolution, no population.** Biological individuality
sits inside a population with selection. We have one organism per run. That is
a deliberate simplification; conclusions are about an architecture, not about
life.

**W5. Survivorship bias.** Organisms that die early leave the sample. Metrics
must be computed at fixed ages and report censoring.

**W6. Designer-in-the-loop tuning.** If we iterate on parameters until E2 passes,
E2 proves nothing. Mitigation: a development environment set and a held-out
environment set; the configuration is frozen (hash logged) before held-out runs.

**W7. Small worlds, short horizons.** Results may not hold at scale, and we
will not know. Phase 4 exists for this; no claims beyond the tested scale.

**W8. Anthropomorphic reading.** "The organism prefers warmth" is a description
of a policy fingerprint, not a mental state. Report template enforces an
observation/interpretation split.

**W9. Determinism as a false comfort.** One seed is one sample. Every
hypothesis test runs ≥30 seeds; effect sizes and intervals, not single runs.

**W11. Survival saturates.** A competent agent lives the whole run, so
survival cannot rank competent agents. Margin and efficiency are primary.

**W12. Trivial context-dependence.** Any bid sum reverses between "hungrier
than thirsty" and "thirstier than hungry". That is not a strategy. H6 demands
reversal within bins of equal error rank order and a `GreedyError` baseline.

**W10. Snapshot ≠ experiential continuity.** Nothing in E3 bears on whether
anything "transfers". The brief already says this; the report must repeat it.

## 3. Where the hypothesis would be refuted here

The project is a failure of the hypothesis (in this model class) if, after
Phase 2 with frozen configuration and held-out environments:

- EPI does not beat the reactive baseline on survival in shifting environments
  (then the interacting-process machinery buys nothing), **and/or**
- the individuality index is not above the matched-null (reseeded, same
  history) level, **and/or**
- the policy fingerprint has no predictive validity beyond the population
  average.

Any of these is a reportable negative result and halts Phase 4.
