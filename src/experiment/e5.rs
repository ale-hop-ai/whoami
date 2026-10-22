//! E5 — Competing priorities: does arbitration produce context-dependent
//! strategies that a largest-error rule or a fixed order does not?

use std::collections::BTreeMap;
use std::path::Path;

use crate::AgentKind;
use crate::body::ACTIONS;
use crate::config::Config;
use crate::experiment::baselines::Selector;
use crate::experiment::families::Family;
use crate::experiment::report::{Report, Table, Verdict, f3};
use crate::experiment::runner::{Arm, ArmKind, RunOpts, SeedResult, run_seed, write_csv};
use crate::experiment::{Context, held_out_env_seed, summarize};
use crate::log::Row;
use crate::metrics::{cliffs_delta, policy::bin_of};

const MIN_TICKS: usize = 20;
const MIN_WINDOWS: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Served {
    Energy,
    Hydration,
}

/// Which single drive the chosen action served this tick, if exactly one.
fn served(r: &Row) -> Option<Served> {
    let (Some(e), Some(v)) = (r.errors, r.values) else { return None };
    if e[0] <= 0.0 || e[1] <= 0.0 {
        return None; // both must be in deficit
    }
    let best = |d: usize| ACTIONS.iter().copied().max_by(|a, b| v[d][a.index()].partial_cmp(&v[d][b.index()]).unwrap()).unwrap();
    let se = r.action == best(0);
    let sh = r.action == best(1);
    match (se, sh) {
        (true, false) => Some(Served::Energy),
        (false, true) => Some(Served::Hydration),
        _ => None,
    }
}

/// Key: (state bin, energy error larger than hydration error).
type Key = (usize, bool);

fn window_majorities(rows: &[Row]) -> BTreeMap<Key, Served> {
    let mut tally: BTreeMap<Key, (usize, usize)> = BTreeMap::new();
    for r in rows {
        let Some(s) = served(r) else { continue };
        let e = r.errors.unwrap();
        let key = (bin_of(&r.body, &r.vis), e[0].abs() > e[1].abs());
        let t = tally.entry(key).or_insert((0, 0));
        if s == Served::Energy { t.0 += 1 } else { t.1 += 1 }
    }
    tally.into_iter().filter(|(_, (a, b))| a + b >= MIN_TICKS).map(|(k, (a, b))| (k, if a >= b { Served::Energy } else { Served::Hydration })).collect()
}

/// Does this individual show a consistent reversal: two keys with the same
/// error rank order where it serves different drives, in ≥ 80% of the
/// windows where both keys are occupied (at least MIN_WINDOWS such windows)?
fn consistent_reversal(windows: &[BTreeMap<Key, Served>]) -> bool {
    let mut keys: Vec<Key> = windows.iter().flat_map(|w| w.keys().copied()).collect();
    keys.sort();
    keys.dedup();
    for (i, k1) in keys.iter().enumerate() {
        for k2 in &keys[i + 1..] {
            if k1.1 != k2.1 {
                continue;
            }
            let mut both = 0;
            let mut reversed = 0;
            let mut sign: Option<(Served, Served)> = None;
            let mut same_sign = 0;
            for w in windows {
                if let (Some(a), Some(b)) = (w.get(k1), w.get(k2)) {
                    both += 1;
                    if a != b {
                        reversed += 1;
                        match sign {
                            None => { sign = Some((*a, *b)); same_sign += 1 }
                            Some(s) if s == (*a, *b) => same_sign += 1,
                            _ => {}
                        }
                    }
                }
            }
            if both >= MIN_WINDOWS && reversed == both && same_sign as f64 / both as f64 >= 0.8 {
                return true;
            }
        }
    }
    false
}

pub fn run(cfg: &Config, ctx: &Context, out: &Path) -> Result<Report, String> {
    let arms = vec![
        Arm::agent("epi", AgentKind::Epi),
        Arm { name: "greedy_error".into(), kind: ArmKind::Selector(Selector::GreedyError) },
        Arm { name: "fixed_priority".into(), kind: ArmKind::Selector(Selector::FixedPriority { order: [0, 1, 2], threshold: Selector::FIXED_THRESHOLD }) },
    ];
    let family = Family::Far;
    let mut results: Vec<SeedResult> = Vec::new();
    let mut reversal: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    // Diagnostics per arm: ticks with a served drive, occupied keys per window,
    // and how often the served drive is the one with the larger error.
    let mut diag: BTreeMap<String, (usize, usize, usize, usize, usize)> = BTreeMap::new();
    for arm in &arms {
        for i in 0..ctx.seeds {
            let env_seed = held_out_env_seed(ctx, i);
            let opts = RunOpts { ticks: ctx.ticks, window: ctx.window, log_path: None, keep_trajectory: true };
            let (mut r, _) = run_seed(cfg, family, arm, env_seed, i as u64, &opts)?;
            let tr = r.trajectory.take().ok_or("no trajectory")?;
            let d = diag.entry(arm.name.clone()).or_insert((0, 0, 0, 0, 0));
            for row in &tr.rows {
                d.0 += 1;
                if let Some(sv) = served(row) {
                    d.1 += 1;
                    let e = row.errors.unwrap();
                    let larger = if e[0].abs() > e[1].abs() { Served::Energy } else { Served::Hydration };
                    if sv == larger {
                        d.2 += 1;
                    }
                }
            }
            let windows: Vec<BTreeMap<Key, Served>> = tr.rows.chunks_exact(ctx.window).map(window_majorities).collect();
            d.3 += windows.iter().map(|w| w.len()).sum::<usize>();
            d.4 += windows.len();
            let e = reversal.entry(arm.name.clone()).or_insert((0, 0));
            e.1 += 1;
            if consistent_reversal(&windows) {
                e.0 += 1;
            }
            results.push(r);
        }
        eprintln!("e5 {} done: reversal {:?}", arm.name, reversal[&arm.name]);
    }
    write_csv(&out.join("seeds.csv"), &results)?;

    let mut report = Report::new("E5 — Competing priorities");
    ctx.describe(&mut report);
    report.config("family", family.name());
    report.config("reversal definition", format!("state bin × error rank order; a key needs ≥ {MIN_TICKS} ticks in a window with both variables in deficit; a reversal is two same-rank keys served by different drives in every one of ≥ {MIN_WINDOWS} co-occupied windows, same direction in ≥ 80%"));
    let mut t = Table::new(&["arm", "consistent reversal", "median margin", "median intake", "survived", "H(a|b)"]);
    let mut frac = BTreeMap::new();
    for arm in &arms {
        let rs: Vec<&SeedResult> = results.iter().filter(|r| r.arm == arm.name).collect();
        let s = summarize(&rs);
        let (k, n) = reversal[&arm.name];
        frac.insert(arm.name.clone(), k as f64 / n.max(1) as f64);
        t.row(vec![arm.name.clone(), format!("{k}/{n}"), f3(s.margin), format!("{:.4}", s.intake), format!("{}/{}", s.survived, rs.len()), f3(s.cond_entropy)]);
    }
    let m = |name: &str| results.iter().filter(|r| r.arm == name).map(|r| r.margin).collect::<Vec<_>>();
    let d_greedy = cliffs_delta(&m("epi"), &m("greedy_error"));
    let d_fixed = cliffs_delta(&m("epi"), &m("fixed_priority"));
    let mut dt = Table::new(&["arm", "ticks", "ticks serving one drive", "served the larger error", "occupied keys per window", "windows"]);
    for arm in &arms {
        let (n, srv, larger, keys, wins) = diag[&arm.name];
        dt.row(vec![arm.name.clone(), n.to_string(), f3(srv as f64 / n.max(1) as f64), f3(larger as f64 / srv.max(1) as f64), f3(keys as f64 / wins.max(1) as f64), wins.to_string()]);
    }
    report.observations = format!("{}\nCliff's δ on margin: EPI vs GreedyError {}, EPI vs FixedPriority {}.\n\nDetector diagnostics:\n\n{}", t.markdown(), f3(d_greedy), f3(d_fixed), dt.markdown());
    let fe = frac["epi"];
    let v = if fe < 0.3 || d_greedy <= 0.0 {
        Verdict::Refuted
    } else if fe >= 0.7 && d_greedy >= 0.3 && d_fixed >= 0.3 {
        Verdict::Supported
    } else {
        Verdict::Inconclusive
    };
    report.verdict("H6", v, format!("pre-registered: consistent reversal in ≥ 70% of EPI individuals and margin δ ≥ 0.3 vs both selectors; measured reversal EPI {} greedy {} fixed {}, δ greedy {}, δ fixed {}", f3(fe), f3(frac["greedy_error"]), f3(frac["fixed_priority"]), f3(d_greedy), f3(d_fixed)));
    report.interpretation = "A reversal within one error rank order means the organism serves the smaller deficit in some contexts and the larger in others, consistently; a largest-error rule cannot do that and a fixed order can only do it through its threshold. The selectors share every other process with EPI, so the margin gap, if any, is the value of competitive arbitration in this family.".to_string();
    Ok(report)
}
