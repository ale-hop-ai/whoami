use epi::agent::Agent;
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

#[test]
fn snapshot_roundtrip_is_bit_identical() {
    use epi::log::Logger;
    use epi::snapshot::Snapshot;
    use epi::Session;
    let cfg = Config::default();
    let mut direct = Session::new(&cfg, 0, 11);
    let r = direct.run(4000, &mut Logger::null());

    let mut first = Session::new(&cfg, 0, 11);
    first.run(1500, &mut Logger::null());
    let snap = first.snapshot();
    // Through JSON, as the CLI does it.
    let text = serde_json::to_string(&snap).unwrap();
    let snap: Snapshot = serde_json::from_str(&text).unwrap();
    let mut second = Session::from_snapshot(snap);
    let r2 = second.run(2500, &mut Logger::null());

    assert_eq!(r.trajectory_hash, r2.trajectory_hash);
    assert_eq!(direct.organism.body().v, second.organism.body().v);
    assert_eq!(direct.world.cells(), second.world.cells());
}

#[test]
fn forks_share_a_past_and_diverge() {
    use epi::log::Logger;
    use epi::Session;
    let cfg = Config::default();
    let mut parent = Session::new(&cfg, 0, 5);
    parent.run(3000, &mut Logger::null());
    let snap = parent.snapshot();
    let mut a = Session::from_snapshot(snap.clone());
    let mut b = Session::from_snapshot(snap);
    a.reseed_env(100);
    b.reseed_env(101);
    let ra = a.run(3000, &mut Logger::null());
    let rb = b.run(3000, &mut Logger::null());
    assert_ne!(ra.trajectory_hash, rb.trajectory_hash);
}
