//! E4 — Component removal. Matched-null and deletion lesions against intact.

use std::path::Path;

use crate::AgentKind;
use crate::config::Config;
use crate::experiment::families::Family;
use crate::experiment::protocols::{Seed, analyze, cohort};
use crate::experiment::report::{Report, Table, Verdict, f3, opt3};
use crate::experiment::runner::{Arm, ArmKind, RunOpts, SeedResult, run_seed, write_csv};
use crate::experiment::{Context, held_out_env_seed};
use crate::metrics::cliffs_delta;
use crate::organism::lesion::{Lesion, Lesions};

pub const FAMILIES: [Family; 2] = [Family::Default, Family::Far];

type Setter = fn(&mut Lesions, Lesion);
/// (lesion name, family, per-metric Cliff's δ)
type DeficitRow = (String, Family, Vec<(String, f64)>);

fn lesion_arms() -> Vec<(String, Lesions, &'static str)> {
    let mut v = Vec::new();
    let setters: [(&str, Setter); 4] = [
        ("memory", |l, x| l.memory = x),
        ("world_model", |l, x| l.world_model = x),
        ("self_model", |l, x| l.self_model = x),
        ("exploration", |l, x| l.exploration = x),
    ];
    for (proc_name, set) in setters {
        for kind in [Lesion::Null, Lesion::Deleted] {
            let mut l = Lesions::default();
            set(&mut l, kind);
            v.push((format!("{proc_name}_{}", if kind == Lesion::Null { "null" } else { "deleted" }), l, proc_name));
        }
    }
    v.push(("drive_adaptation_off".into(), Lesions { drive_adaptation_off: true, ..Lesions::default() }, "drive_adaptation"));
    v
}

fn metric(r: &SeedResult, name: &str) -> Option<f64> {
    match name {
        "margin" => Some(r.margin),
        "intake" => Some(r.intake),
        "survival" => Some(r.ticks_alive as f64),
        "anticipation" => Some(r.anticipation),
        "prediction_mse" => r.prediction_mse.map(|m| -m), // higher is better, like the others
        "retention" => r.retention,
        "swap_recovery" => {
            // Faster is better: negative ticks; unrecovered = -(limit + 1).
            let s: Vec<f64> = r.adaptation.iter().filter(|(_, op, _)| op == "SwapPatches").map(|(_, _, t)| -(t.unwrap_or(4001) as f64)).collect();
            if s.is_empty() { None } else { Some(s.iter().sum::<f64>() / s.len() as f64) }
        }
        _ => None,
    }
}

const METRICS: [&str; 7] = ["margin", "intake", "survival", "anticipation", "prediction_mse", "retention", "swap_recovery"];

/// Predicted primary deficit per process (docs/03 H5).
fn primary(proc_name: &str) -> (&'static str, Family) {
    match proc_name {
        "memory" => ("intake", Family::Default),
        "world_model" => ("margin", Family::Default),
        "self_model" => ("anticipation", Family::Far),
        "exploration" => ("swap_recovery", Family::Default),
        _ => ("individuality", Family::Default),
    }
}

pub fn run(cfg: &Config, ctx: &Context, out: &Path) -> Result<Report, String> {
    let lesions = lesion_arms();
    let mut arms = vec![Arm::agent("intact", AgentKind::Epi)];
    for (name, l, _) in &lesions {
        arms.push(Arm { name: name.clone(), kind: ArmKind::Lesioned(l.clone()) });
    }
    let mut results = Vec::new();
    for family in FAMILIES {
        for arm in &arms {
            for i in 0..ctx.seeds {
                let env_seed = held_out_env_seed(ctx, i);
                let opts = RunOpts { ticks: ctx.ticks, window: ctx.window, log_path: None, keep_trajectory: false };
                let (r, _) = run_seed(cfg, family, arm, env_seed, i as u64, &opts)?;
                results.push(r);
            }
            eprintln!("e4 {} {} done", family.name(), arm.name);
        }
    }
    write_csv(&out.join("seeds.csv"), &results)?;

    // Individuality for the drive-adaptation lesion: the E2 protocol, intact vs lesioned.
    let intact_cohort = cohort(cfg, ctx, &Seed::Fresh, "e4 intact")?;
    let lesioned_cohort = cohort(cfg, ctx, &Seed::FreshWith(Lesions { drive_adaptation_off: true, ..Lesions::default() }), "e4 drive_adaptation_off")?;
    let si = analyze(&intact_cohort);
    let sl = analyze(&lesioned_cohort);
    let idiff = |s: &crate::experiment::protocols::CohortStats| match (s.individuality, s.individuality_null) { (Some(a), Some(b)) => Some(a - b), _ => None };

    let mut report = Report::new("E4 — Component removal");
    ctx.describe(&mut report);
    report.config("families", FAMILIES.iter().map(|f| f.name()).collect::<Vec<_>>().join(", "));
    report.config("lesions", lesions.iter().map(|l| l.0.as_str()).collect::<Vec<_>>().join(", "));
    let mut obs = String::new();
    // Deficit matrix: Cliff's δ(intact, lesion) per metric; positive = intact better.
    let mut deltas: Vec<DeficitRow> = Vec::new();
    for family in FAMILIES {
        let mut t = Table::new(&[&["lesion"], &METRICS[..]].concat());
        let intact: Vec<&SeedResult> = results.iter().filter(|r| r.family == family.name() && r.arm == "intact").collect();
        for (name, _, _) in &lesions {
            let les: Vec<&SeedResult> = results.iter().filter(|r| r.family == family.name() && &r.arm == name).collect();
            let mut row = vec![name.clone()];
            let mut ds = Vec::new();
            for m in METRICS {
                let xs: Vec<f64> = intact.iter().filter_map(|r| metric(r, m)).collect();
                let ys: Vec<f64> = les.iter().filter_map(|r| metric(r, m)).collect();
                if xs.len() < 3 || ys.len() < 3 {
                    row.push("n/a".into());
                    continue;
                }
                let d = cliffs_delta(&xs, &ys);
                row.push(f3(d));
                ds.push((m.to_string(), d));
            }
            t.row(row);
            deltas.push((name.clone(), family, ds));
        }
        obs.push_str(&format!("### Deficit matrix, family `{}` (Cliff's δ intact vs lesion; positive = intact better)\n\n{}\n", family.name(), t.markdown()));
        let mut raw = Table::new(&["arm", "median margin", "median intake", "survived", "anticipation", "retention", "swap recovery (median ticks)"]);
        for arm in &arms {
            let rs: Vec<&SeedResult> = results.iter().filter(|r| r.family == family.name() && r.arm == arm.name).collect();
            let s = crate::experiment::summarize(&rs);
            let mut ret: Vec<f64> = rs.iter().filter_map(|r| r.retention).collect();
            let mut rec: Vec<f64> = rs.iter().flat_map(|r| r.adaptation.iter()).filter(|(_, op, _)| op == "SwapPatches").map(|(_, _, t)| t.unwrap_or(4001) as f64).collect();
            raw.row(vec![arm.name.clone(), f3(s.margin), format!("{:.4}", s.intake), format!("{}/{}", s.survived, rs.len()), f3(s.anticipation), opt3(if ret.is_empty() { None } else { Some(crate::metrics::median(&mut ret)) }), opt3(if rec.is_empty() { None } else { Some(crate::metrics::median(&mut rec)) })]);
        }
        obs.push_str(&format!("Raw medians, family `{}`:\n\n{}\n", family.name(), raw.markdown()));
    }
    obs.push_str(&format!("Individuality (E2 protocol): intact I − I_null {}, drive_adaptation_off I − I_null {}.\n", opt3(idiff(&si)), opt3(idiff(&sl))));
    report.observations = obs;

    for proc_name in ["memory", "world_model", "self_model", "exploration", "drive_adaptation"] {
        let (pm, fam) = primary(proc_name);
        if proc_name == "drive_adaptation" {
            let (a, b) = (idiff(&si), idiff(&sl));
            let v = match (a, b) {
                (Some(a), Some(b)) if a - b >= 0.1 => Verdict::Supported,
                (Some(_), Some(_)) => Verdict::Refuted,
                _ => Verdict::Inconclusive,
            };
            report.verdict("H5 drive_adaptation", v, format!("predicted: individuality falls toward the Frozen level; intact I − I_null {}, lesioned {}", opt3(a), opt3(b)));
            continue;
        }
        let null_name = format!("{proc_name}_null");
        let Some((_, _, ds)) = deltas.iter().find(|(n, f, _)| *n == null_name && *f == fam) else { continue };
        let dp = ds.iter().find(|(m, _)| m == pm).map(|x| x.1);
        let others_ok = match dp {
            Some(dp) => ds.iter().filter(|(m, _)| m != pm).all(|(_, d)| *d <= dp - 0.2 || *d <= 0.5),
            None => false,
        };
        let v = match dp {
            Some(dp) if dp >= 0.5 && others_ok => Verdict::Supported,
            Some(dp) if dp >= 0.5 => Verdict::Inconclusive,
            Some(_) => Verdict::Refuted,
            None => Verdict::Inconclusive,
        };
        report.verdict(&format!("H5 {proc_name}"), v, format!("predicted primary deficit `{pm}` in family `{}` (matched null): δ {}; every other metric δ ≤ δ_primary − 0.2 or ≤ 0.5: {others_ok}", fam.name(), opt3(dp)));
    }
    report.interpretation = "A matched-null lesion keeps the process stepping and replaces its output with an empty value, so the deficit is attributable to the information the process carried, not to compute removed. Deletion rows are reported alongside; where they differ from the null rows, the difference is the side effect of not stepping. The world model lesion is expected to collapse survival because without predictions Eat, Drink and Rest are indistinguishable; that is the predicted outcome, not a confound. The 'others smaller' clause is relaxed to allow secondary deficits up to δ 0.5, since survival collapse drags every horizon-normalized metric with it.".to_string();
    Ok(report)
}
