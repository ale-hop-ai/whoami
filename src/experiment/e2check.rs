//! E2 sensitivity check. Two cohorts with deliberately different drive
//! parameters go through the E2 protocol. If the individuality index cannot
//! separate them, H2's refutation says nothing about the organism.

use std::path::Path;

use crate::config::Config;
use crate::experiment::families::FAMILIES;
use crate::experiment::protocols::{Individual, Seed, cohort};
use crate::experiment::report::{Report, Table, Verdict, f3, opt3};
use crate::experiment::Context;
use crate::metrics::{Fingerprint, fingerprint_distance, individuality, individuality_null, pooled_occupancy};

fn windows_of(inds: &[&Individual], k: usize) -> Vec<Vec<Fingerprint>> {
    inds.iter().map(|i| i.windows[..k].to_vec()).collect()
}

pub fn run(cfg: &Config, ctx: &Context, out: &Path) -> Result<Report, String> {
    let mut alt = cfg.clone();
    alt.organism.drives.gamma = 0.85;
    alt.organism.drives.explore_scale = 0.4;
    alt.organism.drives.lambda = [0.0, 0.0, 0.0, 0.0];
    alt.organism.arbitration.drive_persistence = 0.0;
    let a = cohort(cfg, ctx, &Seed::Fresh, "e2check A")?;
    let b = cohort(&alt, ctx, &Seed::Fresh, "e2check B")?;
    let k = a.individuals.iter().chain(&b.individuals).map(|i| i.windows.len()).min().unwrap_or(0);
    let all: Vec<&Individual> = a.individuals.iter().chain(&b.individuals).collect();
    let weights = pooled_occupancy(&all.iter().flat_map(|i| i.windows.iter().take(k)).collect::<Vec<_>>());
    let wa = windows_of(&a.individuals.iter().collect::<Vec<_>>(), k);
    let wb = windows_of(&b.individuals.iter().collect::<Vec<_>>(), k);
    let wall = windows_of(&all, k);
    let i_all = individuality(&wall, &weights);
    let i_a = individuality(&wa, &weights);
    let i_b = individuality(&wb, &weights);
    let null = individuality_null(&wall, &weights, 7, 20);

    // Leave-one-out nearest-centroid classification A vs B on merged windows.
    let merged: Vec<Fingerprint> = all.iter().map(|i| { let mut f = Fingerprint::default(); for w in &i.windows[..k] { f.merge(w) } f }).collect();
    let n_a = a.individuals.len();
    let mut correct = 0;
    for i in 0..all.len() {
        let mut ca = Fingerprint::default();
        let mut cb = Fingerprint::default();
        for (j, m) in merged.iter().enumerate() {
            if j == i { continue; }
            if j < n_a { ca.merge(m) } else { cb.merge(m) }
        }
        let da = fingerprint_distance(&merged[i], &ca, &weights);
        let db = fingerprint_distance(&merged[i], &cb, &weights);
        if (i < n_a) == (da < db) { correct += 1; }
    }
    let acc = correct as f64 / all.len().max(1) as f64;

    let mut report = Report::new("E2 sensitivity check — planted individuality");
    ctx.describe(&mut report);
    report.config("cohort A", "frozen default parameters");
    report.config("cohort B", "gamma 0.85, explore_scale 0.4, lambda 0, drive_persistence 0");
    report.config("families", FAMILIES.iter().map(|f| f.name()).collect::<Vec<_>>().join(", "));
    let mut t = Table::new(&["quantity", "value"]);
    t.row(vec!["n A / n B".into(), format!("{} / {}", a.individuals.len(), b.individuals.len())]);
    t.row(vec!["windows per individual".into(), k.to_string()]);
    t.row(vec!["I over A ∪ B (one-way ICC)".into(), opt3(i_all)]);
    t.row(vec!["I within A".into(), opt3(i_a)]);
    t.row(vec!["I within B".into(), opt3(i_b)]);
    t.row(vec!["I_null (A ∪ B shuffled)".into(), opt3(null)]);
    t.row(vec!["A-vs-B classification accuracy (LOO, chance 0.5)".into(), f3(acc)]);
    report.observations = t.markdown();
    let diff = match (i_all, null) { (Some(x), Some(n)) => x - n, _ => f64::NAN };
    let v = if diff.is_nan() { Verdict::Inconclusive } else if diff >= 0.3 && acc >= 0.9 { Verdict::Supported } else if diff < 0.1 { Verdict::Refuted } else { Verdict::Inconclusive };
    report.verdict("Metric sensitivity", v, format!("planted parameter difference should give I − I_null ≥ 0.3 and classification ≥ 0.9; measured I − I_null {} , accuracy {}", f3(diff), f3(acc)));
    report.interpretation = "Supported means the individuality index detects a difference of the size a parameter change produces; H2's refutation then says the organism's history produces less than that. Refuted means the index is blind at this window size and bin resolution, and H2 is untested.".to_string();
    let _ = out;
    Ok(report)
}
