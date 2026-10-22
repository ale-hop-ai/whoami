use epi::config::Config;
use epi::AgentKind;

#[test]
fn same_seed_same_trajectory() {
    let cfg = Config::default();
    let a = epi::run_once(&cfg, AgentKind::Random, 42, 2000, None, 1).unwrap();
    let b = epi::run_once(&cfg, AgentKind::Random, 42, 2000, None, 1).unwrap();
    assert_eq!(a.trajectory_hash, b.trajectory_hash);
    assert_eq!(a.ticks_alive, b.ticks_alive);
}

#[test]
fn different_seed_different_trajectory() {
    let cfg = Config::default();
    let a = epi::run_once(&cfg, AgentKind::Random, 1, 500, None, 1).unwrap();
    let b = epi::run_once(&cfg, AgentKind::Random, 2, 500, None, 1).unwrap();
    assert_ne!(a.trajectory_hash, b.trajectory_hash);
}

#[test]
fn random_agent_dies_within_tuned_band() {
    let cfg = Config::default();
    let mut lifetimes: Vec<u64> = (0..30)
        .map(|s| epi::run_once(&cfg, AgentKind::Random, s, 20_000, None, 1).unwrap().ticks_alive)
        .collect();
    lifetimes.sort_unstable();
    let median = lifetimes[15];
    assert!((200..=600).contains(&median), "median random lifetime {median}, all {lifetimes:?}");
}

#[test]
fn epi_same_seed_same_trajectory() {
    let cfg = Config::default();
    let a = epi::run_once(&cfg, AgentKind::Epi, 7, 3000, None, 1).unwrap();
    let b = epi::run_once(&cfg, AgentKind::Epi, 7, 3000, None, 1).unwrap();
    assert_eq!(a.trajectory_hash, b.trajectory_hash);
}

#[test]
fn epi_outlives_random_tenfold_in_static_world() {
    let mut cfg = Config::default();
    cfg.world.regime_every = 1_000_000;
    let median = |kind: AgentKind| {
        let mut v: Vec<u64> = (0..30).map(|s| epi::run_once(&cfg, kind, s, 20_000, None, 1).unwrap().ticks_alive).collect();
        v.sort_unstable();
        (v[15], v)
    };
    let (r, _) = median(AgentKind::Random);
    let (e, all) = median(AgentKind::Epi);
    assert!(e >= 10 * r, "epi median {e} vs random median {r}; epi lifetimes {all:?}");
}
