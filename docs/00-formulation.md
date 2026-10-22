# EPI — Formulation

Status: Phase 0 draft, awaiting approval. No code exists.

## 1. Working definition

**Emergent Process Identity (EPI).** An individual is the self-maintaining
organization of a set of coupled processes (perception, memory, prediction,
drives, homeostatic regulation, arbitration, action, self-modeling). None of the processes is the
individual. The individual is the pattern of how they constrain each other over
time while keeping the whole viable. Identity is this pattern, measured; it is
reconstructed every tick, and it is history-dependent because several of the
processes carry learned state.

Three consequences follow and are the things this project can actually test:

1. **Organization, not component.** Behavioral coherence should come from the
   coupling, not from any one process. Removing or nulling a process should
   change the organism in specific, predictable ways rather than switch
   "identity" off.
2. **History makes individuals.** Two organisms with identical code, parameters,
   and initial state should become measurably and persistently different if
   their histories differ, and should stay measurably similar if their histories
   are shared (fork).
3. **Continuity under change.** An organism can change a lot and still be
   closer to its own past than to another organism's present.

Consciousness in the subjective sense is deliberately outside the project.
We measure organization, persistence, divergence, and function. Nothing in this
spec licenses a claim about experience.

## 2. Claims, decomposed

| # | Claim | Status | Testable here? |
|---|-------|--------|----------------|
| C1 | Coherent adaptive behavior can arise from interacting processes with no authored policy and no global utility. | Hypothesis | Yes (E1, E5) |
| C2 | Individuality (stable between-individual differences) can arise from history alone, with identical mechanism and parameters. | Hypothesis; has biological precedent (isogenic mice, clonal fish) | Yes (E2) |
| C3 | Identity is continuous under environmental change: the organism's signature drifts rather than resets. | Hypothesis | Yes (E2) |
| C4 | A copied state produces a new individual that shares history but diverges; shared history is measurable as kinship. | Hypothesis | Yes (E3) |
| C5 | Specific processes are necessary for specific capabilities (memory for retention, prediction for anticipation, drive adaptation for individuality). | Hypothesis | Yes (E4) |
| C6 | Competing drives produce context-dependent strategies, not a fixed priority order. | Hypothesis | Yes (E5) |
| C7 | The same organization on a different substrate is the same kind of individual. | Philosophical; substrate here is always "a Rust process" | No. Not addressed. |
| C8 | Any of this is or produces subjective experience. | Untestable | No. Explicitly excluded. |

C1–C6 are falsifiable with the metrics in `03-experiments.md`. C7 and C8 are
not part of the experimental program. Results will be reported against C1–C6
only.

## 3. Lineage

Established findings and frameworks the design borrows from. "Established" means
the empirical result or formal construct is well replicated or mathematically
standard; "framework" means a theoretical lens with contested empirical status.

**Homeostasis and ultrastability (established / framework).** Cannon (1932)
on physiological regulation; Ashby (1952) showed that a system with multiple
essential variables and a mechanism that reorganizes when those variables leave
their viable bounds exhibits adaptive behavior without a designed policy. Ashby's
homeostat is the closest ancestor of this project. Sterling & Eyer (1988) and
later Sterling (2012) extended this to allostasis: regulation by anticipation,
not just error correction. We use both: error-driven drives (homeostatic) and a
self-model that raises urgency before the error exists (allostatic).

**Autopoiesis and adaptivity (framework).** Maturana & Varela (1980) define a
living system as a network of processes that produce and maintain the network.
Di Paolo (2005) argues autopoiesis alone does not give you sense-making; you
also need *adaptivity*: the ability to monitor and regulate one's approach to
the viability boundary. Our self-model is a direct implementation of Di Paolo's
adaptivity. Beer (1995, 2004) showed how to treat agent–environment coupling as
one dynamical system and analyze it rather than narrate it.

**Action selection in ethology (established).** McFarland & Sibly (1975)
formalized the "behavioural final common path": multiple motivational systems
compete for a single motor output through a common currency. Tyrrell (1993),
Maes (1989), Blumberg (1994), Cañamero (1997), and Avila-García & Cañamero
(2004) built computational versions with explicit drive competition. Bach's
MicroPsi (2009), implementing Dörner's PSI theory, is a full multi-drive
cognitive architecture. **Our architecture is not novel in this respect**; the
contribution is in the measurement program (individuality, continuity, fork
kinship, ablation), not the agent design.

**Homeostatic reinforcement learning (established result).** Keramati & Gutkin
(2014) proved that drive reduction, defined as decrease in distance from a
homeostatic setpoint, is formally a reward signal, and that an agent maximizing
it performs homeostatic regulation. This cuts both ways: it gives us a
principled per-drive learning signal, and it is the main reason the "no global
utility" claim must be stated carefully (see `01-critique.md`, A4). Konidaris &
Barto (2006) and Yoshida (2017) are related. Man & Damasio (2019) argue that
homeostatic vulnerability is the precondition for anything feeling-like in
machines; we use their argument as motivation only.

**Predictive processing and active inference (framework; contested as a brain
theory, standard as a design pattern).** Friston (2010, 2013), Clark (2013),
Seth (2013), Pezzulo et al. (2015), Hesp et al. (2021). The useful imports are:
prediction error as the currency that couples perception, memory, and learning;
interoceptive prediction as the basis of a self-model; and the point that an
organism is defined by the states it keeps returning to. We do **not** adopt the
full variational free energy machinery. It would add a single global objective
and a lot of math for no experimental gain at this scale.

**Individuality from experience in isogenic animals (established).** Freund et
al. (2013) showed genetically identical mice in a shared enriched environment
develop stable, increasing individual differences in exploratory behavior.
Bierbach et al. (2017) found the same in clonal fish with near-identical
rearing. Sih et al. (2015) propose state–behavior positive feedback as a
general mechanism for stable "personality". This is the strongest empirical
support for C2. The architecture implements this feedback as a per-drive
*habit* term (an action that serves a drive becomes more likely to be chosen
for it). Gain adaptation, by contrast, is negative feedback with long
hysteresis and is not an instance of the Sih mechanism; `02-architecture.md`
§3.6 names which is which.

**Personal identity (philosophy).** Parfit (1984) on fission and psychological
continuity frames E3: after a fork, "which one is the original" is the wrong
question; what is measurable is degree of connectedness. Dennett (1991) and
Metzinger (2003) supply the stance that there is no inner observer, only a
self-model that is itself a process. Minsky (1986) and Brooks (1986) are the
engineering precedents for "mind as many agents, no central one". Global
Workspace (Baars 1988) is not used: the workspace is the kind of central stage
we are trying to do without. Integrated Information Theory (Tononi 2004) offers
a measure but requires full causal structure and does not scale; not used.

## 4. What this project can show

Positive result: a specific, inspectable architecture with no authored policy
produces, from history alone, individuals that are stable, distinguishable,
continuous under change, and degraded in predictable ways by lesions. That
would support C1–C6 in this model class.

Negative result: it does not, at this scale, and the metrics say so. That is
also a publishable outcome and is the more likely one on the first pass.

Neither result says anything about consciousness. The report template in
`03-experiments.md` §8 separates observation from interpretation to make this
hard to forget.

## 5. References

Verify before citing externally; these are from memory and should be checked
against the originals.

- Ashby, W. R. (1952). *Design for a Brain*. Chapman & Hall.
- Avila-García, O., Cañamero, L. (2004). Using hormonal feedback to modulate action selection in a competitive scenario. *SAB 2004*.
- Baars, B. (1988). *A Cognitive Theory of Consciousness*. Cambridge UP.
- Bach, J. (2009). *Principles of Synthetic Intelligence*. Oxford UP.
- Beer, R. D. (1995). A dynamical systems perspective on agent–environment interaction. *Artificial Intelligence* 72:173–215.
- Beer, R. D. (2004). Autopoiesis and cognition in the Game of Life. *Artificial Life* 10:309–326.
- Bierbach, D., Laskowski, K. L., Wolf, M. (2017). Behavioural individuality in clonal fish arises despite near-identical rearing conditions. *Nature Communications* 8:15361.
- Blumberg, B. (1994). Action-selection in Hamsterdam: lessons from ethology. *SAB 1994*.
- Brooks, R. A. (1986). A robust layered control system for a mobile robot. *IEEE J. Robotics and Automation* 2(1):14–23.
- Cañamero, L. (1997). Modeling motivations and emotions as a basis for intelligent behavior. *Proc. Autonomous Agents '97*.
- Cannon, W. B. (1932). *The Wisdom of the Body*. Norton.
- Clark, A. (2013). Whatever next? Predictive brains, situated agents, and the future of cognitive science. *Behavioral and Brain Sciences* 36:181–204.
- Dennett, D. (1991). *Consciousness Explained*. Little, Brown.
- Di Paolo, E. (2005). Autopoiesis, adaptivity, teaching and sense-making. *Phenomenology and the Cognitive Sciences* 4:429–452.
- Freund, J. et al. (2013). Emergence of individuality in genetically identical mice. *Science* 340:756–759.
- Friston, K. (2010). The free-energy principle: a unified brain theory? *Nature Reviews Neuroscience* 11:127–138.
- Friston, K. (2013). Life as we know it. *J. R. Soc. Interface* 10:20130475.
- Hesp, C. et al. (2021). Deeply felt affect: the emergence of valence in deep active inference. *Neural Computation* 33(2):398–446.
- Keramati, M., Gutkin, B. (2014). Homeostatic reinforcement learning for integrating reward collection and physiological stability. *eLife* 3:e04811.
- Konidaris, G., Barto, A. (2006). An adaptive robot motivational system. *SAB 2006*.
- Maes, P. (1989). How to do the right thing. *Connection Science* 1(3):291–323.
- Man, K., Damasio, A. (2019). Homeostasis and soft robotics in the design of feeling machines. *Nature Machine Intelligence* 1:446–452.
- Maturana, H. R., Varela, F. J. (1980). *Autopoiesis and Cognition*. Reidel.
- McFarland, D. J., Sibly, R. M. (1975). The behavioural final common path. *Phil. Trans. R. Soc. B* 270:265–293.
- Metzinger, T. (2003). *Being No One*. MIT Press.
- Minsky, M. (1986). *The Society of Mind*. Simon & Schuster.
- Parfit, D. (1984). *Reasons and Persons*. Oxford UP.
- Pezzulo, G., Rigoli, F., Friston, K. (2015). Active inference, homeostatic regulation and adaptive behavioural control. *Progress in Neurobiology* 134:17–35.
- Seth, A. K. (2013). Interoceptive inference, emotion, and the embodied self. *Trends in Cognitive Sciences* 17:565–573.
- Sih, A., Mathot, K. J., et al. (2015). Animal personality and state–behaviour feedbacks. *Trends in Ecology & Evolution* 30:50–60.
- Sterling, P., Eyer, J. (1988). Allostasis: a new paradigm to explain arousal pathology. In *Handbook of Life Stress, Cognition and Health*. Wiley.
- Sterling, P. (2012). Allostasis: a model of predictive regulation. *Physiology & Behavior* 106:5–15.
- Tononi, G. (2004). An information integration theory of consciousness. *BMC Neuroscience* 5:42.
- Tyrrell, T. (1993). *Computational Mechanisms for Action Selection*. PhD thesis, University of Edinburgh.
- Yoshida, N. (2017). Homeostatic agent for general environment. *J. Artificial General Intelligence* 8(1):1–22.
