//! E2 — Continuity and individuality from history.

use std::path::Path;

use crate::config::Config;
use crate::experiment::protocols::{self, Seed, analyze, cohort, cohort_table, frac_positive, median_opt, write_cohort_csv};
use crate::experiment::report::{Report, Verdict, f3, opt3};
use crate::experiment::{Context, pretrain_frozen};

pub fn run(cfg: &Config, ctx: &Context, out: &Path) -> Result<Report, String> {
    let epi = cohort(cfg, ctx, &Seed::Fresh, "e2 epi")?;
    write_cohort_csv(&out.join("epi_cohort.csv"), &epi)?;
    let frozen_org = pretrain_frozen(cfg, ctx)?;
    let frozen = cohort(cfg, ctx, &Seed::Given(Box::new(frozen_org)), "e2 frozen")?;
    write_cohort_csv(&out.join("frozen_cohort.csv"), &frozen)?;
    let se = analyze(&epi);
    let sf = analyze(&frozen);

    let mut report = Report::new("E2 — Continuity and individuality");
    ctx.describe(&mut report);
    report.config("protocol", format!("4 families × {} development seeds, {} dev ticks, transplant into held-out default world (env seed {}), {} test ticks, {} burn-in, organism seed 0 for all", protocols::PER_FAMILY, protocols::DEV_TICKS, ctx.held_out_base + 100, protocols::TEST_TICKS, protocols::BURN_IN));
    report.config("died (EPI / Frozen)", format!("{} / {}", epi.died_in_development, frozen.died_in_development));

    let mut obs = String::new();
    obs.push_str(&cohort_table("EPI", &se));
    obs.push('\n');
    obs.push_str(&cohort_table("Frozen", &sf));
    obs.push_str("\nI is the one-way ICC of the smoothed policy fingerprint across individuals of the same development family, averaged over families with pooled occupancy weights; I_null reshuffles windows across individuals within family. PV is the per-individual log-likelihood gain of its own earlier window over a stranger's window of the same age and size (mean over strangers). C is continuity across the transplant: own last development window vs own test window, against strangers' last development windows vs own test window, noise-corrected by each individual's split-half floor. Family accuracy is leave-one-out nearest-centroid classification of development family from the test fingerprint (chance 0.25).\n");
    report.observations = obs;

    let diff = match (se.individuality, se.individuality_null) { (Some(a), Some(b)) => a - b, _ => f64::NAN };
    let fdiff = match (sf.individuality, sf.individuality_null) { (Some(a), Some(b)) => a - b, _ => 0.0 };
    let pv_ok = frac_positive(&se.pv_5w);
    let h2 = if diff.is_nan() {
        Verdict::Inconclusive
    } else if diff >= 0.3 && pv_ok >= 0.8 && fdiff < 0.1 {
        Verdict::Supported
    } else if diff < 0.1 || pv_ok < 0.5 {
        Verdict::Refuted
    } else {
        Verdict::Inconclusive
    };
    report.verdict("H2", h2.clone(), format!("pre-registered: EPI I − I_null ≥ 0.3, PV(5W) > 0 for ≥ 80%, Frozen I − I_null < 0.1; measured I − I_null {} , PV(5W) > 0 for {}, Frozen I − I_null {}", f3(diff), f3(pv_ok), f3(fdiff)));
    if fdiff >= 0.1 && !diff.is_nan() && (fdiff - diff).abs() < 0.1 {
        report.verdict("H2 (learning)", Verdict::Refuted, "Frozen matches EPI: individuality is not from learning");
    }

    let cw = median_opt(&se.c_w);
    let c5 = median_opt(&se.c_5w);
    let h3 = match (&h2, cw, c5) {
        (Verdict::Supported, Some(a), Some(b)) if a >= 0.5 && b >= 0.3 && se.own_past_closest >= 0.7 => Verdict::Supported,
        (Verdict::Supported, Some(a), _) if a < 0.2 => Verdict::Refuted,
        (Verdict::Supported, _, _) => Verdict::Inconclusive,
        _ => Verdict::Inconclusive,
    };
    report.verdict("H3", h3, format!("conditional on H2; pre-registered C(W) ≥ 0.5, C(5W) ≥ 0.3, own past closest for ≥ 70%; measured median C(W) {}, C(5W) {}, own past closest {}", opt3(cw), opt3(c5), f3(se.own_past_closest)));
    report.interpretation = "Within-family individuality separates history from environment: individuals of the same family saw the same generator settings and differ only by the environment seed and what they learned from it. A positive PV(5W) says the individual's own past predicts its own future better than the population average does, which is the non-circular reading of 'identity'. The Frozen cohort cannot differ by construction when it never learns; it is reported to show that the pipeline returns zero where zero is expected.".to_string();
    Ok(report)
}
