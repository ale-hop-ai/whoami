# Phase 2 results: held-out runs, frozen configuration

Date: 2026-10-22. Branch `feat/phase2-experiments`. Config
`configs/default.json` (hash in every report), 30 held-out environment seeds
(100+) per arm, 20 000 ticks unless the protocol says otherwise. Full reports
and per-seed data: `docs/results/`. Verdicts are against the thresholds
pre-registered in `03-experiments.md`, including its dated amendments.

## 1. Verdicts

| Hypothesis | Verdict | Measured (pre-registered threshold) |
|------------|---------|-------------------------------------|
| H1 emergent adaptive behavior, `default` | **supported** | δ vs Reactive: margin 0.996, intake 0.996, survival 0.991 (≥ 0.5, ≥ 0.5, ≥ −0.1) |
| H1, `volatile` | **supported** | margin 0.980, intake 0.980, survival 0.971 |
| H2 individuality from history | **refuted on PV; untested on I** | within-family I − I_null 0.034 (≥ 0.3, but see sensitivity check: the index tops out near 0.09 for a planted parameter difference); PV(5W) > 0 for 53% (≥ 80%); Frozen 0 by construction |
| H3 continuity | inconclusive (conditional on H2) | median C(W) 0.038, C(5W) 0.023, own past closest 3% |
| H4 fork kinship | inconclusive | K(W) 0.49 (< 0.9 ✓), early K < 0.7 ✓, below body-only at every window ✗, sign test p 0.17 (< 0.05 ✗) |
| H5 memory | inconclusive | primary deficit δ 1.00 ✓; not specific: every metric collapses |
| H5 world model | inconclusive | primary deficit δ 0.98 ✓; not specific |
| H5 self-model | inconclusive | primary deficit δ 1.00 ✓; not specific, and confounded (see §3) |
| H5 exploration | **refuted** | predicted swap-recovery deficit δ 0.04 (≥ 0.5) |
| H5 drive adaptation | **refuted** | intact I − I_null 0.034 vs lesioned −0.007: no individuality to lose |
| H6 context-dependent priorities | inconclusive | reversal detector starved (0.25 occupied keys per window); margin δ vs GreedyError 1.00, vs FixedPriority 1.00 |

**Sensitivity check (`docs/results/e2check-report.md`).** Two cohorts with
deliberately different drive parameters (γ 0.85 vs 0.97, no anticipation, no
drive persistence) went through the E2 protocol. Leave-one-out classification
separates them with 98.5% accuracy; the one-way ICC index reaches only
0.088 against a null of 0.002. The pre-registered I threshold of 0.3 is
therefore not reachable by the index as defined (occupancy-weighted average
over 756 bin×action cells, most of them near-empty in a 500-tick window),
and H2's I clause is untested. The PV clause is the sensitive one and it
fails (53% against 80%). H2 stands refuted on predictive validity only.

Per `03-experiments.md` §7 that is still a stop condition for Phase 4 as
planned. See §5.

## 2. Observations

**E1.** In `default`, EPI survives the full run in 24 of 30 seeds with median
margin 0.390 and intake 0.0062 per tick; Reactive (same perception, same
authored movement fragments, no memory, no learning) lives a median 1806
ticks; Random 334. The Oracle with a full map reaches margin 0.410. Frozen
(one pretrained organism, learning off) dies at 396 ticks in every seed.
GreedyError and FixedPriority, which share every process with EPI except
competitive arbitration, reach margins 0.257 and 0.353 with 14 and 20
survivors; FixedPriority logs a median 23 danger events per run against 0 for
EPI. In `volatile` (shift every 2000 ticks) EPI survives 15 of 30; the
MixedForager strategy collapses (0 of 30). The adaptation-speed metric did
not discriminate: viability stays near 1 across shifts for every surviving
arm, so recovery is "immediate" by definition.

**E2.** 32 of 40 organisms survived development and test. Within-family
ICC of the policy fingerprint is 0.034 against a shuffled null of 0.000.
Own-past prediction beats a same-size stranger window in 53% of cases at
Δ = 5 windows (median gain +1.5 nats, i.e. small). Leave-one-out family
classification from the test fingerprint reaches 47% against 25% chance:
development family leaves a readable trace, individual history within a
family does not, at this resolution. Continuity across the transplant is
indistinguishable from zero.

**E3.** Two forks of one snapshot start at kinship 0.49 (strangers = 1.0)
and drift to about 0.65 over 5000 ticks; body-only forks (same position and
physiology, learned state reset) start at 0.66. Forks share a past, but at
the first window most of the resemblance is also carried by position and
physiology, and the EPI advantage over body-only is not sustained at every
window. The Frozen fork arm is empty: the Frozen organism dies before the
fork point in every seed.

**E4.** Memory null or deleted: survival collapses (δ 1.00 on margin, intake,
survival). World-model null: margin δ 0.98, survival δ 0.55; prediction MSE
rises (δ 1.00) as predicted. Self-model null: everything collapses.
Exploration null: mild deficits (margin δ 0.32, survival δ 0.02); the
predicted swap-recovery deficit is absent. Drive adaptation off: no deficit
(margin δ 0.12), individuality unchanged at zero. Null and deleted variants
give identical trajectories for every process, so stepping a nulled process
has no side effects here.

**E5.** In `far` EPI reaches margin 0.359 with 16 of 30 survivors; the two
selectors that read errors directly survive in 0 of 30 each (margin ≤ 0.007).
The reversal detector found 0.25 occupied keys per window for EPI: both
variables are rarely in deficit at once.

## 3. Interpretation (labeled as such)

- **What works.** The coupled processes with competitive arbitration keep a
  body alive in shifting worlds where stateless, greedy and fixed-order
  policies do not, in both families, with large effect sizes. This supports
  C1 in `00-formulation.md`: coherent adaptive behavior from interacting
  processes with no authored policy beyond the two declared fragments.
- **What does not.** History does not carve stable, distinguishable
  individuals at this scale. Individuals of one family are as alike as
  shuffled windows say they should be. The habit term, the only positive
  feedback carrier, is too weak or too slow to matter within 15 000 ticks,
  and removing drive adaptation changes nothing. This is the negative result
  the critique (A2, W7) anticipated: a competent organism in a world with
  one or two good strategies converges, and convergence erases history.
- **E5's margin gap is real, its mechanism is not what H6 named.** EPI beats
  the greedy selector by keeping both deficits small so that a reversal
  situation rarely arises; the selectors that serve the largest error
  oscillate between two distant patches and die between them. The
  pre-registered reversal clause assumed the interesting behavior happens
  inside joint deficits; the organism's actual strategy is to avoid them.
- **E4's self-model lesion is confounded by an M6 design change.** Consumption
  yield moved into the self-model (`02-architecture.md` §3.5 amendment), so
  nulling it removes both anticipation and the price of eating. The
  anticipation deficit cannot be isolated from that. A finer lesion
  (time-to-critical only) is required before any claim about anticipation.
- **E4's exploration result is informative.** The organism recovers from a
  patch swap without the exploration drive, because the homeostatic drives
  carry their own unknown-seeking fragment (F2) once a target is lost.

## 4. Not claimed

No claim is made about subjective experience. No claim is made about
transfer of experience under snapshot or fork. No claim extends beyond
16×16 worlds, four families, 30 seeds and 20 000 ticks. Two authored policy
fragments (F1, F2) exist, are shared with the baselines, and are not
credited to the organism.

## 5. Decision

Phase 4 as planned ("richer environments, learned representations") is not
justified by these results. What the results justify instead, in order:

1. Done: the metric check above. Consequence for any further H2 work: the
   individuality clause must be re-registered on a sensitive statistic
   (leave-one-out identification accuracy, or pairwise own-vs-stranger
   distance), with its threshold calibrated on the planted-difference
   cohorts (98.5% there; chance 50%). In E2 proper, family classification
   reached 47% against 25% chance, so development *family* already leaves a
   detectable trace; individual history within a family is the open
   question.
2. A lesion of time-to-critical alone, to unconfound the self-model result.
3. Only then, one mechanism experiment aimed at H2: a positive-feedback
   carrier with a time constant inside the run (the habit term at
   `τ_h = 50 000` cannot act within a 15 000-tick protocol), pre-registered
   as an amendment, run on held-out seeds.

Everything above is on `feat/phase2-experiments`, unmerged.
