//! Measurement. Identity is not a struct anywhere in the organism; it is
//! computed here, after the fact, from logs and snapshots.
//!
//! Definitions follow docs/03-experiments.md. Every quantity here is a
//! function of recorded data; nothing feeds back into the simulation.

pub mod policy;
pub mod trajectory;

use crate::rng::Rng;
pub use policy::{Fingerprint, N_BINS};
pub use trajectory::{Row, Trajectory, WindowStats};

/// Occupancy-weighted Jensen–Shannon distance between two fingerprints.
/// `weights` are the pooled bin occupancies (they are normalized here).
pub fn fingerprint_distance(a: &Fingerprint, b: &Fingerprint, weights: &[f64; N_BINS]) -> f64 {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    let mut d = 0.0;
    for bin in 0..N_BINS {
        if weights[bin] > 0.0 {
            d += weights[bin] / total * js_divergence(&a.smoothed(bin), &b.smoothed(bin));
        }
    }
    d
}

/// Jensen–Shannon divergence in nats, in [0, ln 2].
pub fn js_divergence(p: &[f64], q: &[f64]) -> f64 {
    let mut d = 0.0;
    for (pi, qi) in p.iter().zip(q) {
        let m = 0.5 * (pi + qi);
        if *pi > 0.0 {
            d += 0.5 * pi * (pi / m).ln();
        }
        if *qi > 0.0 {
            d += 0.5 * qi * (qi / m).ln();
        }
    }
    d.max(0.0)
}

/// Pooled occupancy over a set of fingerprints.
pub fn pooled_occupancy(fps: &[&Fingerprint]) -> [f64; N_BINS] {
    let mut w = [0.0; N_BINS];
    for fp in fps {
        for bin in 0..N_BINS {
            w[bin] += fp.occupancy(bin) as f64;
        }
    }
    w
}

/// Split-half noise floor: distance between the even-tick and odd-tick
/// halves of one window. A distance is "above noise" if it exceeds this.
pub fn split_half_floor(rows: &[Row], weights: &[f64; N_BINS]) -> f64 {
    let (mut even, mut odd) = (Fingerprint::default(), Fingerprint::default());
    for (i, r) in rows.iter().enumerate() {
        if i % 2 == 0 { even.add(r) } else { odd.add(r) }
    }
    fingerprint_distance(&even, &odd, weights)
}

/// Continuity: 1 - d_self / d_stranger, where d_self is the distance between
/// an individual's own earlier and later windows and d_stranger is the mean
/// distance from the later window to other individuals' windows at the same
/// age. Both are noise-corrected by subtracting `floor`. `None` when strangers
/// are not distinguishable from noise.
pub fn continuity(earlier: &Fingerprint, later: &Fingerprint, strangers: &[&Fingerprint], weights: &[f64; N_BINS], floor: f64) -> Option<f64> {
    if strangers.is_empty() {
        return None;
    }
    let d_self = (fingerprint_distance(earlier, later, weights) - floor).max(0.0);
    let d_str: f64 = strangers.iter().map(|s| fingerprint_distance(later, s, weights)).sum::<f64>() / strangers.len() as f64;
    let d_str = d_str - floor;
    if d_str <= 0.0 {
        return None;
    }
    Some(1.0 - d_self / d_str)
}

/// Fork kinship: d(A, B) / mean d(A, stranger). Below 1 means the forks are
/// closer to each other than to strangers of the same age.
pub fn kinship(a: &Fingerprint, b: &Fingerprint, strangers: &[&Fingerprint], weights: &[f64; N_BINS]) -> Option<f64> {
    if strangers.is_empty() {
        return None;
    }
    let d_ab = fingerprint_distance(a, b, weights);
    let d_str: f64 = strangers.iter().map(|s| fingerprint_distance(a, s, weights)).sum::<f64>() / strangers.len() as f64;
    if d_str <= 0.0 { None } else { Some(d_ab / d_str) }
}

/// Predictive validity: log-likelihood of `later`'s actions under `own`
/// minus under `population` (a leave-one-out pooled fingerprint). Positive
/// means the individual's own past predicts its future better than the
/// population does.
pub fn predictive_validity(own: &Fingerprint, population: &Fingerprint, later: &Fingerprint) -> f64 {
    log_likelihood(own, later) - log_likelihood(population, later)
}

pub fn log_likelihood(model: &Fingerprint, data: &Fingerprint) -> f64 {
    let mut ll = 0.0;
    for bin in 0..N_BINS {
        let p = model.smoothed(bin);
        for (a, &n) in data.counts(bin).iter().enumerate() {
            if n > 0 {
                ll += n as f64 * p[a].ln();
            }
        }
    }
    ll
}

/// Individuality index: one-way ICC(1) of smoothed P(a|b) across
/// individuals with windows as repeated measures, averaged over (bin,
/// action) with pooled occupancy weights. `windows[i]` holds individual i's
/// windows; every individual must have the same number of windows.
pub fn individuality(windows: &[Vec<Fingerprint>], weights: &[f64; N_BINS]) -> Option<f64> {
    let n = windows.len();
    if n < 2 {
        return None;
    }
    let k = windows[0].len();
    if k < 2 || windows.iter().any(|w| w.len() != k) {
        return None;
    }
    let total_w: f64 = weights.iter().sum();
    if total_w <= 0.0 {
        return None;
    }
    let mut icc_sum = 0.0;
    let mut w_sum = 0.0;
    for bin in 0..N_BINS {
        if weights[bin] <= 0.0 {
            continue;
        }
        for a in 0..policy::N_ACT {
            let x: Vec<Vec<f64>> = windows.iter().map(|ws| ws.iter().map(|fp| fp.smoothed(bin)[a]).collect()).collect();
            if let Some(icc) = icc1(&x) {
                icc_sum += weights[bin] * icc;
                w_sum += weights[bin];
            }
        }
    }
    if w_sum <= 0.0 { None } else { Some(icc_sum / w_sum) }
}

/// Null for `individuality`: windows reassigned to individuals at random
/// (seeded), averaged over `permutations`.
pub fn individuality_null(windows: &[Vec<Fingerprint>], weights: &[f64; N_BINS], seed: u64, permutations: usize) -> Option<f64> {
    let n = windows.len();
    if n < 2 {
        return None;
    }
    let k = windows[0].len();
    let mut pool: Vec<&Fingerprint> = windows.iter().flatten().collect();
    let mut rng = Rng::from_seed(seed);
    let mut acc = 0.0;
    let mut count = 0;
    for _ in 0..permutations {
        for i in (1..pool.len()).rev() {
            let j = rng.below(i + 1);
            pool.swap(i, j);
        }
        let shuffled: Vec<Vec<Fingerprint>> = (0..n).map(|i| (0..k).map(|w| pool[i * k + w].clone()).collect()).collect();
        if let Some(v) = individuality(&shuffled, weights) {
            acc += v;
            count += 1;
        }
    }
    if count == 0 { None } else { Some(acc / count as f64) }
}

/// One-way random-effects ICC(1): (MSB - MSW) / (MSB + (k-1) MSW).
pub fn icc1(groups: &[Vec<f64>]) -> Option<f64> {
    let n = groups.len();
    let k = groups[0].len();
    if n < 2 || k < 2 {
        return None;
    }
    let grand: f64 = groups.iter().flatten().sum::<f64>() / (n * k) as f64;
    let mut ssb = 0.0;
    let mut ssw = 0.0;
    for g in groups {
        let m = g.iter().sum::<f64>() / k as f64;
        ssb += k as f64 * (m - grand) * (m - grand);
        for x in g {
            ssw += (x - m) * (x - m);
        }
    }
    let msb = ssb / (n - 1) as f64;
    let msw = ssw / (n * (k - 1)) as f64;
    let denom = msb + (k as f64 - 1.0) * msw;
    if denom <= 0.0 { None } else { Some((msb - msw) / denom) }
}

/// Cliff's delta: P(x > y) - P(x < y).
pub fn cliffs_delta(xs: &[f64], ys: &[f64]) -> f64 {
    if xs.is_empty() || ys.is_empty() {
        return 0.0;
    }
    let mut more = 0i64;
    let mut less = 0i64;
    for x in xs {
        for y in ys {
            if x > y { more += 1 } else if x < y { less += 1 }
        }
    }
    (more - less) as f64 / (xs.len() * ys.len()) as f64
}

pub fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { 0.5 * (v[n / 2 - 1] + v[n / 2]) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Action;

    fn row(e: f64, a: Action) -> Row {
        Row { t: 0, action: a, pos: (0, 0), body: [e, 0.5, 0.5], here: [0.0; 2], vis: [false; 2], errors: None, predicted: None, values: None, remembered: None }
    }

    #[test]
    fn identical_logs_have_zero_distance() {
        let rows: Vec<Row> = (0..200).map(|i| row(0.5, if i % 2 == 0 { Action::Eat } else { Action::Rest })).collect();
        let mut a = Fingerprint::default();
        for r in &rows { a.add(r) }
        let b = a.clone();
        let w = pooled_occupancy(&[&a, &b]);
        assert_eq!(fingerprint_distance(&a, &b, &w), 0.0);
    }

    #[test]
    fn different_policies_have_positive_distance() {
        let mut a = Fingerprint::default();
        let mut b = Fingerprint::default();
        for _ in 0..200 { a.add(&row(0.5, Action::Eat)); b.add(&row(0.5, Action::Rest)); }
        let w = pooled_occupancy(&[&a, &b]);
        assert!(fingerprint_distance(&a, &b, &w) > 0.3);
    }

    #[test]
    fn icc_separates_individuals_with_known_variance() {
        // Two individuals, each consistent; between-variance dominates.
        let tight = vec![vec![0.1, 0.11, 0.09], vec![0.9, 0.91, 0.89]];
        assert!(icc1(&tight).unwrap() > 0.9);
        let noisy = vec![vec![0.1, 0.9, 0.5], vec![0.5, 0.1, 0.9]];
        assert!(icc1(&noisy).unwrap() < 0.3);
    }

    #[test]
    fn split_half_floor_is_small_for_iid_data() {
        let mut rng = Rng::from_seed(1);
        let rows: Vec<Row> = (0..2000).map(|_| row(0.5, if rng.f64() < 0.5 { Action::Eat } else { Action::Rest })).collect();
        let mut fp = Fingerprint::default();
        for r in &rows { fp.add(r) }
        let w = pooled_occupancy(&[&fp]);
        assert!(split_half_floor(&rows, &w) < 0.01);
    }

    #[test]
    fn predictive_validity_favors_own_model_when_consistent() {
        let mut own = Fingerprint::default();
        let mut pop = Fingerprint::default();
        let mut later = Fingerprint::default();
        for _ in 0..100 { own.add(&row(0.5, Action::Eat)); later.add(&row(0.5, Action::Eat)); }
        for _ in 0..100 { pop.add(&row(0.5, Action::Rest)); pop.add(&row(0.5, Action::Eat)); }
        assert!(predictive_validity(&own, &pop, &later) > 0.0);
    }

    #[test]
    fn cliffs_delta_extremes() {
        assert_eq!(cliffs_delta(&[2.0, 3.0], &[0.0, 1.0]), 1.0);
        assert_eq!(cliffs_delta(&[0.0, 1.0], &[2.0, 3.0]), -1.0);
        assert_eq!(cliffs_delta(&[1.0], &[1.0]), 0.0);
    }
}
