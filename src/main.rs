//! CLI. Hand-parsed; no framework.

use std::path::{Path, PathBuf};
use std::process::exit;

use epi::config::Config;
use epi::AgentKind;

const USAGE: &str = "usage:
  epi run  [--config C] [--seed S] [--ticks T] [--out DIR] [--agent random] [--log-every K]
  epi hash [--config C] [--seed S] [--ticks T] [--agent random]";

struct Args {
    config: Option<PathBuf>,
    seed: u64,
    ticks: u64,
    out: Option<PathBuf>,
    agent: AgentKind,
    log_every: u64,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args { config: None, seed: 0, ticks: 20_000, out: None, agent: AgentKind::Random, log_every: 1 };
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
            other => return Err(format!("unknown flag {other}")),
        }
        i += 2;
    }
    Ok(a)
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = argv.first() else {
        eprintln!("{USAGE}");
        exit(2);
    };
    let result = match cmd.as_str() {
        "run" => cmd_run(&argv[1..]),
        "hash" => cmd_hash(&argv[1..]),
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
    let r = epi::run_once(&cfg, a.agent, a.seed, a.ticks, Some(&log_path), a.log_every)?;
    let summary = serde_json::json!({
        "config_hash": format!("{cfg_hash:016x}"),
        "seed": a.seed,
        "agent": format!("{:?}", a.agent).to_lowercase(),
        "result": r,
    });
    let text = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    std::fs::write(out.join("result.json"), &text).map_err(|e| e.to_string())?;
    println!("{text}");
    Ok(())
}

fn cmd_hash(args: &[String]) -> Result<(), String> {
    let a = parse(args)?;
    let (cfg, _) = load(&a)?;
    let r = epi::run_once(&cfg, a.agent, a.seed, a.ticks, None::<&Path>, 1)?;
    println!("{} {}", r.trajectory_hash, r.ticks_alive);
    Ok(())
}
