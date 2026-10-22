//! CLI. Hand-parsed; no framework.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::exit;

use epi::config::Config;
use epi::log::Logger;
use epi::snapshot::Snapshot;
use epi::{AgentKind, Session};

const USAGE: &str = "usage:
  epi run  [--config C] [--seed S] [--ticks T] [--out DIR] [--agent epi|random|oracle|mixed] [--log-every K] [--snapshot-every K]
  epi fork --snapshot FILE --env-seed S [--ticks T] [--out DIR] [--log-every K]
  epi metrics --log FILE [--config C] [--window W]
  epi hash [--config C] [--seed S] [--ticks T] [--agent epi|random|oracle|mixed]";

struct Args {
    config: Option<PathBuf>,
    seed: u64,
    ticks: u64,
    out: Option<PathBuf>,
    agent: AgentKind,
    log_every: u64,
    snapshot_every: Option<u64>,
    snapshot: Option<PathBuf>,
    env_seed: Option<u64>,
    log: Option<PathBuf>,
    window: usize,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args { config: None, seed: 0, ticks: 20_000, out: None, agent: AgentKind::Epi, log_every: 1, snapshot_every: None, snapshot: None, env_seed: None, log: None, window: 500 };
    let mut i = 0;
    while i < args.len() {
        let key = &args[i];
        let val = args.get(i + 1).ok_or_else(|| format!("{key} needs a value"))?;
        match key.as_str() {
            "--config" => a.config = Some(PathBuf::from(val)),
            "--seed" => a.seed = val.parse().map_err(|_| format!("bad seed {val}"))?,
            "--ticks" => a.ticks = val.parse().map_err(|_| format!("bad ticks {val}"))?,
            "--out" => a.out = Some(PathBuf::from(val)),
            "--agent" => a.agent = AgentKind::parse(val)?,
            "--log-every" => a.log_every = val.parse().map_err(|_| format!("bad log-every {val}"))?,
            "--snapshot-every" => a.snapshot_every = Some(val.parse().map_err(|_| format!("bad snapshot-every {val}"))?),
            "--snapshot" => a.snapshot = Some(PathBuf::from(val)),
            "--env-seed" => a.env_seed = Some(val.parse().map_err(|_| format!("bad env-seed {val}"))?),
            "--log" => a.log = Some(PathBuf::from(val)),
            "--window" => a.window = val.parse().map_err(|_| format!("bad window {val}"))?,
            other => return Err(format!("unknown flag {other}")),
        }
        i += 2;
    }
    Ok(a)
}

/// Print a line; a closed pipe (e.g. `| head`) ends the program quietly.
fn out(line: std::fmt::Arguments) {
    let stdout = std::io::stdout();
    let mut h = stdout.lock();
    if h.write_fmt(line).and_then(|_| h.write_all(b"\n")).is_err() {
        exit(0);
    }
}

macro_rules! outln {
    ($($arg:tt)*) => { out(format_args!($($arg)*)) };
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = argv.first() else {
        eprintln!("{USAGE}");
        exit(2);
    };
    let result = match cmd.as_str() {
        "run" => cmd_run(&argv[1..]),
        "fork" => cmd_fork(&argv[1..]),
        "hash" => cmd_hash(&argv[1..]),
        "metrics" => cmd_metrics(&argv[1..]),
        _ => Err(USAGE.to_string()),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        exit(1);
    }
}

fn load(a: &Args) -> Result<(Config, u64), String> {
    let loaded = Config::load(a.config.as_deref())?;
    loaded.config.validate()?;
    Ok((loaded.config, loaded.hash))
}

fn cmd_run(args: &[String]) -> Result<(), String> {
    let a = parse(args)?;
    let (cfg, cfg_hash) = load(&a)?;
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from("results/run"));
    std::fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
    let log_path = out.join("log.jsonl");
    let r = match (a.agent, a.snapshot_every) {
        (AgentKind::Epi, Some(every)) => {
            let mut session = Session::new(&cfg, cfg_hash, a.seed);
            let mut log = Logger::to_file(&log_path, a.log_every)?;
            session.run_with_snapshots(a.ticks, &mut log, &out, every)?
        }
        (_, Some(_)) => return Err("--snapshot-every is only supported for --agent epi".into()),
        (_, None) => epi::run_once(&cfg, a.agent, a.seed, a.ticks, Some(&log_path), a.log_every)?,
    };
    write_summary(&out, serde_json::json!({
        "config_hash": format!("{cfg_hash:016x}"),
        "seed": a.seed,
        "agent": format!("{:?}", a.agent).to_lowercase(),
        "result": r,
    }))
}

fn cmd_fork(args: &[String]) -> Result<(), String> {
    let a = parse(args)?;
    let path = a.snapshot.clone().ok_or("fork needs --snapshot FILE")?;
    let env_seed = a.env_seed.ok_or("fork needs --env-seed S")?;
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from("results/fork"));
    std::fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
    let snap = Snapshot::load(&path)?;
    let config_hash = snap.config_hash;
    let from_tick = snap.tick;
    let mut session = Session::from_snapshot(snap);
    session.reseed_env(env_seed);
    let mut log = Logger::to_file(&out.join("log.jsonl"), a.log_every)?;
    let r = session.run(a.ticks, &mut log);
    write_summary(&out, serde_json::json!({
        "config_hash": format!("{config_hash:016x}"),
        "forked_from": path.display().to_string(),
        "from_tick": from_tick,
        "env_seed": env_seed,
        "agent": "epi",
        "result": r,
    }))
}

fn cmd_metrics(args: &[String]) -> Result<(), String> {
    use epi::metrics::{Fingerprint, fingerprint_distance, pooled_occupancy, split_half_floor, trajectory::{adaptation_speed, window_stats}, Trajectory};
    let a = parse(args)?;
    let (cfg, _) = load(&a)?;
    let path = a.log.clone().ok_or("metrics needs --log FILE")?;
    let tr = Trajectory::load(&path)?;
    let sp = &cfg.organism.homeostasis.setpoint;
    let wd = &cfg.organism.homeostasis.width;
    outln!("ticks_alive {}  death {:?}  regimes {}  dangers {}", tr.ticks_alive(), tr.death, tr.regimes.len(), tr.dangers.len());
    let windows = tr.windows(a.window);
    let fps: Vec<Fingerprint> = windows.iter().map(|w| Fingerprint::from_rows(w)).collect();
    let refs: Vec<&Fingerprint> = fps.iter().collect();
    let weights = pooled_occupancy(&refs);
    outln!("{:>7} {:>6} {:>5} {:>18} {:>7} {:>7} {:>6} {:>6} {:>6} {:>8} {:>8}", "start", "margin", "viab", "operating_point", "intake", "mv/int", "antic", "H(a)", "H(a|b)", "d_prev", "floor");
    let mut prev_row: Option<&epi::metrics::Row> = None;
    for (i, w) in windows.iter().enumerate() {
        let st = window_stats(w, prev_row, sp, wd);
        let d_prev = if i > 0 { fingerprint_distance(&fps[i - 1], &fps[i], &weights) } else { 0.0 };
        let floor = split_half_floor(w, &weights);
        outln!("{:>7} {:>6.3} {:>5.2} {:>5.2} {:>5.2} {:>5.2} {:>7.4} {:>7.1} {:>6.2} {:>6.2} {:>6.2} {:>8.4} {:>8.4}",
            st.start, st.margin, st.viability, st.operating_point[0], st.operating_point[1], st.operating_point[2],
            st.intake_per_tick, st.moves_per_intake, st.anticipation_rate, st.action_entropy, st.conditional_entropy, d_prev, floor);
        prev_row = w.last();
    }
    for (t, op) in &tr.regimes {
        let speed = adaptation_speed(&tr, *t, a.window, 0.9, 4000, sp, wd);
        outln!("regime {t} {op:?}: adaptation_speed {speed:?}");
    }
    Ok(())
}

fn write_summary(out: &Path, summary: serde_json::Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    std::fs::write(out.join("result.json"), &text).map_err(|e| e.to_string())?;
    outln!("{text}");
    Ok(())
}

fn cmd_hash(args: &[String]) -> Result<(), String> {
    let a = parse(args)?;
    let (cfg, _) = load(&a)?;
    let r = epi::run_once(&cfg, a.agent, a.seed, a.ticks, None::<&Path>, 1)?;
    outln!("{} {}", r.trajectory_hash, r.ticks_alive);
    Ok(())
}
