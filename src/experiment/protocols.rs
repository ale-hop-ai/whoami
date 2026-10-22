//! Protocol pieces shared by E2, E3 and E4: development runs, transplants,
//! post-burn-in windows and the individuality analysis.

use std::path::Path;

use crate::config::Config;
use crate::experiment::families::{FAMILIES, Family};
use crate::experiment::report::{Table, f3, opt3};
use crate::experiment::runner::{Arm, ArmKind, RunOpts, SeedResult, run_seed};
use crate::experiment::Context;
use crate::metrics::{
    Fingerprint, continuity, fingerprint_distance, individuality, individuality_null, median, pooled_occupancy,
    predictive_validity, split_half_floor, N_BINS,
};
use crate::organism::Organism;
use crate::AgentKind;

pub const DEV_TICKS: u64 = 10_000;
pub const TEST_TICKS: u64 = 5_000;
pub const BURN_IN: usize = 1_000;
pub const PER_FAMILY: usize = 10;

/// One individual through development and test.
pub struct Individual {
    pub family: Family,
    pub env_seed: u64,
    pub dev: SeedResult,
    pub test: SeedResult,
    /// Test windows after burn-in.
    pub windows: Vec<Fingerprint>,
    pub floor: f64,
}

/// How to make the organism that enters development.
#[derive(Clone)]
pub enum Seed {
    Fresh,
    FreshWith(crate::organism::lesion::Lesions),
    Given(Box<Organism>),
}

/// Develop one organism in `family` with `env_seed`, then transplant it into
/// the held-out test world. Returns None if it died in development.
pub fn develop_and_test(cfg: &Config, ctx: &Context, seed: &Seed, family: Family, env_seed: u64, test_env_seed: u64) -> Result<Option<Individual>, String> {
    let dev_arm = match seed {
        Seed::Fresh => Arm::agent("dev", AgentKind::Epi),
        Seed::FreshWith(l) => Arm { name: "dev".into(), kind: ArmKind::Lesioned(l.clone()) },
        Seed::Given(o) => Arm { name: "dev".into(), kind: ArmKind::Frozen(o.clone()) },
    };
    let opts = RunOpts { ticks: DEV_TICKS, window: ctx.window, log_path: None, keep_trajectory: false };
    let (dev, org) = run_seed(cfg, family, &dev_arm, env_seed, 0, &opts)?;
    let Some(org) = org else { return Err("development produced no organism".into()) };
    if !dev.alive {
        return Ok(None);
    }
    let test_arm = Arm { name: "test".into(), kind: ArmKind::Continue(Box::new(org)) };
    let opts = RunOpts { ticks: TEST_TICKS, window: ctx.window, log_path: None, keep_trajectory: true };
    let (test, _) = run_seed(cfg, Family::Default, &test_arm, test_env_seed, 0, &opts)?;
    let tr = test.trajectory.as_ref().ok_or("no trajectory")?;
    if tr.rows.len() < BURN_IN + ctx.window {
        return Ok(None);
    }
    let post = &tr.rows[BURN_IN..];
    let windows: Vec<Fingerprint> = post.chunks_exact(ctx.window).map(Fingerprint::from_rows).collect();
    let w0 = pooled_occupancy(&windows.iter().collect::<Vec<_>>());
    let floor = post.chunks_exact(ctx.window).map(|w| split_half_floor(w, &w0)).sum::<f64>() / windows.len().max(1) as f64;
    Ok(Some(Individual { family, env_seed, dev, test, windows, floor }))
}

pub struct Cohort {
    pub individuals: Vec<Individual>,
    pub died_in_development: usize,
}

/// 4 families × PER_FAMILY development seeds, one shared held-out test world.
pub fn cohort(cfg: &Config, ctx: &Context, seed: &Seed, label: &str) -> Result<Cohort, String> {
    let test_env_seed = ctx.held_out_base + 100;
    let mut individuals = Vec::new();
    let mut died = 0;
    for family in FAMILIES {
        for j in 0..PER_FAMILY {
            let env_seed = j as u64; // development seeds: below the held-out base
            match develop_and_test(cfg, ctx, seed, family, env_seed, test_env_seed)? {
                Some(ind) => {
                    eprintln!("{label} {} dev-seed {j}: test alive {} windows {}", family.name(), ind.test.ticks_alive, ind.windows.len());
                    individuals.push(ind);
                }
                None => {
                    eprintln!("{label} {} dev-seed {j}: died in development or test", family.name());
                    died += 1;
                }
            }
        }
    }
    Ok(Cohort { individuals, died_in_development: died })
}

pub struct CohortStats {
    pub n: usize,
    pub windows_per_individual: usize,
    pub weights: [f64; N_BINS],
    /// Within-family ICC, mean over families.
    pub individuality: Option<f64>,
    pub individuality_null: Option<f64>,
    pub pv_w: Vec<f64>,
    pub pv_5w: Vec<f64>,
    /// Continuity across the transplant at Δ = W and Δ = 5W.
    pub c_w: Vec<Option<f64>>,
    pub c_5w: Vec<Option<f64>>,
    /// Fraction of individuals whose post-transplant window is closer to their own
    /// last development window than to any other individual's post-transplant window.
    pub own_past_closest: f64,
    pub family_accuracy: Option<f64>,
    pub identical_trajectories: bool,
}

pub fn analyze(c: &Cohort) -> CohortStats {
    let inds = &c.individuals;
    let n = inds.len();
    let k = inds.iter().map(|i| i.windows.len()).min().unwrap_or(0);
    let all: Vec<&Fingerprint> = inds.iter().flat_map(|i| i.windows.iter().take(k)).collect();
    let weights = pooled_occupancy(&all);
    let mut hashes: Vec<&str> = inds.iter().map(|i| i.test.trajectory_hash.as_str()).collect();
    hashes.dedup();
    let identical = n > 1 && hashes.len() == 1;

    // Within-family individuality.
    let mut fam_i = Vec::new();
    let mut fam_null = Vec::new();
    for family in FAMILIES {
        let members: Vec<Vec<Fingerprint>> = inds.iter().filter(|i| i.family == family).map(|i| i.windows[..k].to_vec()).collect();
        if members.len() >= 2 && k >= 2 {
            if let Some(v) = individuality(&members, &weights) {
                fam_i.push(v);
            }
            if let Some(v) = individuality_null(&members, &weights, 7, 20) {
                fam_null.push(v);
            }
        }
    }
    let mean = |v: &[f64]| if v.is_empty() { None } else { Some(v.iter().sum::<f64>() / v.len() as f64) };

    // Predictive validity at Δ = W and Δ = 5W. The null model is a stranger's
    // window of the same age and the same size (mean over strangers), so
    // smoothing and data volume are matched (docs/03 §2, amendment 2026-10-22).
    let mut pv_w = Vec::new();
    let mut pv_5w = Vec::new();
    for (i, ind) in inds.iter().enumerate() {
        for (delta, out) in [(1usize, &mut pv_w), (5usize, &mut pv_5w)] {
            let mut acc = 0.0;
            let mut cnt = 0;
            for t in 0..k.saturating_sub(delta) {
                let mut stranger_sum = 0.0;
                let mut m = 0;
                for (j, other) in inds.iter().enumerate() {
                    if j != i {
                        stranger_sum += predictive_validity(&ind.windows[t], &other.windows[t], &ind.windows[t + delta]);
                        m += 1;
                    }
                }
                if m > 0 {
                    acc += stranger_sum / m as f64;
                    cnt += 1;
                }
            }
            if cnt > 0 {
                out.push(acc / cnt as f64);
            }
        }
    }

    // Continuity across the transplant: own last development window vs own
    // test window, against strangers' last development windows vs own test
    // window (the same kind of pair; docs/03 §2, amendment 2026-10-22).
    let mut c_w = Vec::new();
    let mut c_5w = Vec::new();
    let mut own_closest = 0usize;
    let pasts: Vec<Option<&Fingerprint>> = inds.iter().map(|o| o.dev.fingerprints.last()).collect();
    for (i, ind) in inds.iter().enumerate() {
        let Some(earlier) = pasts[i] else { continue };
        let strangers: Vec<&Fingerprint> = pasts.iter().enumerate().filter(|(j, p)| *j != i && p.is_some()).map(|(_, p)| p.unwrap()).collect();
        c_w.push(continuity(earlier, &ind.windows[0], &strangers, &weights, ind.floor));
        if k > 5 {
            c_5w.push(continuity(earlier, &ind.windows[5], &strangers, &weights, ind.floor));
        }
        let d_own = fingerprint_distance(earlier, &ind.windows[0], &weights);
        if strangers.iter().all(|s| fingerprint_distance(&ind.windows[0], s, &weights) > d_own) {
            own_closest += 1;
        }
    }

    // Family classification, leave-one-out nearest centroid on merged test windows.
    let merged: Vec<Fingerprint> = inds.iter().map(|i| { let mut f = Fingerprint::default(); for w in &i.windows[..k] { f.merge(w) } f }).collect();
    let mut correct = 0;
    for (i, ind) in inds.iter().enumerate() {
        let mut best: Option<(Family, f64)> = None;
        for family in FAMILIES {
            let mut cen = Fingerprint::default();
            let mut any = false;
            for (j, o) in inds.iter().enumerate() {
                if j != i && o.family == family {
                    cen.merge(&merged[j]);
                    any = true;
                }
            }
            if !any {
                continue;
            }
            let d = fingerprint_distance(&merged[i], &cen, &weights);
            if best.is_none_or(|b| d < b.1) {
                best = Some((family, d));
            }
        }
        if best.map(|b| b.0) == Some(ind.family) {
            correct += 1;
        }
    }

    CohortStats {
        n,
        windows_per_individual: k,
        weights,
        individuality: mean(&fam_i),
        individuality_null: mean(&fam_null),
        pv_w,
        pv_5w,
        c_w,
        c_5w,
        own_past_closest: if n > 0 { own_closest as f64 / n as f64 } else { 0.0 },
        family_accuracy: if n > 0 { Some(correct as f64 / n as f64) } else { None },
        identical_trajectories: identical,
    }
}

pub fn frac_positive(v: &[f64]) -> f64 {
    if v.is_empty() { 0.0 } else { v.iter().filter(|x| **x > 0.0).count() as f64 / v.len() as f64 }
}

pub fn median_opt(v: &[Option<f64>]) -> Option<f64> {
    let mut xs: Vec<f64> = v.iter().flatten().copied().collect();
    if xs.is_empty() { None } else { Some(median(&mut xs)) }
}

pub fn cohort_table(label: &str, s: &CohortStats) -> String {
    let mut t = Table::new(&["cohort", "n", "windows", "I (within family)", "I_null", "I − I_null", "PV(W) > 0", "PV(5W) > 0", "median PV(5W)", "median C(W)", "median C(5W)", "own past closest", "family acc."]);
    let diff = match (s.individuality, s.individuality_null) { (Some(a), Some(b)) => Some(a - b), _ => None };
    let mut pv5 = s.pv_5w.clone();
    t.row(vec![
        label.to_string(), s.n.to_string(), s.windows_per_individual.to_string(),
        opt3(s.individuality), opt3(s.individuality_null), opt3(diff),
        f3(frac_positive(&s.pv_w)), f3(frac_positive(&s.pv_5w)), opt3(if pv5.is_empty() { None } else { Some(median(&mut pv5)) }),
        opt3(median_opt(&s.c_w)), opt3(median_opt(&s.c_5w)), f3(s.own_past_closest), opt3(s.family_accuracy),
    ]);
    let mut out = t.markdown();
    if s.identical_trajectories {
        out.push_str("\nAll test trajectories in this cohort are bit-identical (same trajectory hash).\n");
    }
    out
}

pub fn write_cohort_csv(path: &Path, c: &Cohort) -> Result<(), String> {
    let mut s = String::from("family,dev_env_seed,dev_ticks_alive,test_ticks_alive,test_margin,test_intake,windows,floor,test_hash\n");
    for i in &c.individuals {
        s.push_str(&format!("{},{},{},{},{:.4},{:.5},{},{:.5},{}\n", i.family.name(), i.env_seed, i.dev.ticks_alive, i.test.ticks_alive, i.test.margin, i.test.intake, i.windows.len(), i.floor, i.test.trajectory_hash));
    }
    std::fs::write(path, s).map_err(|e| e.to_string())
}
