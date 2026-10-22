//! E1 — Emergent behavior. EPI against every baseline in the default and
//! volatile families, identical environment seed across arms.

use std::path::Path;

use crate::AgentKind;
use crate::config::Config;
use crate::experiment::baselines::Selector;
use crate::experiment::families::Family;
use crate::experiment::report::{Report, Table, Verdict, f3, opt3};
use crate::experiment::runner::{Arm, ArmKind, RunOpts, SeedResult, run_seed, write_csv};
use crate::experiment::{Context, held_out_env_seed, pretrain_frozen, summarize};
use crate::metrics::{cliffs_delta, median};

pub const FAMILIES: [Family; 2] = [Family::Default, Family::Volatile];

pub fn arms(cfg: &Config, ctx: &Context) -> Result<Vec<Arm>, String> {
    Ok(vec![
        Arm::agent("epi", AgentKind::Epi),
        Arm::agent("random", AgentKind::Random),
        Arm::agent("reactive", AgentKind::Reactive),
        Arm { name: "greedy_error".into(), kind: ArmKind::Selector(Selector::GreedyError) },
        Arm { name: "fixed_priority".into(), kind: ArmKind::Selector(Selector::FixedPriority { order: [0, 1, 2], threshold: Selector::FIXED_THRESHOLD }) },
        Arm { name: "frozen".into(), kind: ArmKind::Frozen(Box::new(pretrain_frozen(cfg, ctx)?)) },
        Arm::agent("oracle", AgentKind::Oracle),
        Arm::agent("mixed_forager", AgentKind::MixedForager),
    ])
}

pub fn run(cfg: &Config, ctx: &Context, out: &Path) -> Result<Report, String> {
    let arms = arms(cfg, ctx)?;
    let mut results: Vec<SeedResult> = Vec::new();
    for family in FAMILIES {
        for arm in &arms {
            for i in 0..ctx.seeds {
                let env_seed = held_out_env_seed(ctx, i);
                let log_path = (i == 0).then(|| out.join(format!("log_{}_{}_{}.jsonl", family.name(), arm.name, env_seed)));
                let opts = RunOpts { ticks: ctx.ticks, window: ctx.window, log_path: log_path.as_deref(), keep_trajectory: false };
                let (r, _) = run_seed(cfg, family, arm, env_seed, i as u64, &opts)?;
                eprintln!("e1 {} {} seed {i}: alive {} margin {:.3}", family.name(), arm.name, r.ticks_alive, r.margin);
                results.push(r);
            }
        }
    }
    write_csv(&out.join("seeds.csv"), &results)?;

    let mut report = Report::new("E1 — Emergent behavior");
    ctx.describe(&mut report);
    report.config("families", FAMILIES.iter().map(|f| f.name()).collect::<Vec<_>>().join(", "));
    report.config("arms", arms.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "));

    let mut obs = String::new();
    let mut h1_parts = Vec::new();
    for family in FAMILIES {
        let mut t = Table::new(&["arm", "median margin", "median intake", "median ticks alive", "survived", "anticipation", "H(a|b)", "dangers"]);
        for arm in &arms {
            let rs: Vec<&SeedResult> = results.iter().filter(|r| r.family == family.name() && r.arm == arm.name).collect();
            let s = summarize(&rs);
            t.row(vec![arm.name.clone(), f3(s.margin), format!("{:.4}", s.intake), format!("{:.0}", s.ticks_alive), format!("{}/{}", s.survived, rs.len()), f3(s.anticipation), f3(s.cond_entropy), format!("{:.1}", s.dangers)]);
        }
        obs.push_str(&format!("### Family `{}`\n\n{}\n", family.name(), t.markdown()));

        let epi: Vec<&SeedResult> = results.iter().filter(|r| r.family == family.name() && r.arm == "epi").collect();
        let rea: Vec<&SeedResult> = results.iter().filter(|r| r.family == family.name() && r.arm == "reactive").collect();
        let d_margin = cliffs_delta(&epi.iter().map(|r| r.margin).collect::<Vec<_>>(), &rea.iter().map(|r| r.margin).collect::<Vec<_>>());
        let d_intake = cliffs_delta(&epi.iter().map(|r| r.intake).collect::<Vec<_>>(), &rea.iter().map(|r| r.intake).collect::<Vec<_>>());
        let d_surv = cliffs_delta(&epi.iter().map(|r| r.ticks_alive as f64).collect::<Vec<_>>(), &rea.iter().map(|r| r.ticks_alive as f64).collect::<Vec<_>>());
        obs.push_str(&format!("Cliff's δ EPI vs Reactive: margin {}, intake {}, survival {}.\n\n", f3(d_margin), f3(d_intake), f3(d_surv)));
        let mut speeds = Table::new(&["arm", "shifts", "recovered", "median recovery ticks"]);
        for arm in &arms {
            let rs: Vec<&SeedResult> = results.iter().filter(|r| r.family == family.name() && r.arm == arm.name).collect();
            let all: Vec<&(u64, String, Option<u64>)> = rs.iter().flat_map(|r| r.adaptation.iter()).collect();
            let mut rec: Vec<f64> = all.iter().filter_map(|a| a.2.map(|x| x as f64)).collect();
            let n_rec = rec.len();
            speeds.row(vec![arm.name.clone(), all.len().to_string(), n_rec.to_string(), opt3(if rec.is_empty() { None } else { Some(median(&mut rec)) })]);
        }
        obs.push_str(&format!("Adaptation after regime shifts (`{}`):\n\n{}\n", family.name(), speeds.markdown()));
        h1_parts.push((family, d_margin, d_intake, d_surv));
    }
    report.observations = obs;

    for (family, d_margin, d_intake, d_surv) in h1_parts {
        let v = if d_margin >= 0.5 && d_intake >= 0.5 && d_surv >= -0.1 {
            Verdict::Supported
        } else if d_margin <= 0.0 {
            Verdict::Refuted
        } else {
            Verdict::Inconclusive
        };
        report.verdict(&format!("H1 ({})", family.name()), v, format!("pre-registered: δ(margin) ≥ 0.5 and δ(intake) ≥ 0.5 vs Reactive, survival not worse; measured δ margin {} intake {} survival {}", f3(d_margin), f3(d_intake), f3(d_surv)));
    }
    report.interpretation = "Interpretation is limited to what the arms isolate. EPI above Reactive \
shows that memory, the learned world model and competitive arbitration together add value over a \
stateless policy that shares the same perception and the same authored movement fragments. The \
Oracle row is a ceiling with a full map, not a competitor. Frozen shows what the learned state \
from one development run is worth without further learning. GreedyError and FixedPriority use \
every EPI process except competitive arbitration; their gap to EPI, if any, is the value of \
competition itself and is examined properly in E5.".to_string();
    Ok(report)
}
