mod hosts;
mod registry;
mod state;
mod telemetry;
mod tweak;

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};

use state::State;
use tweak::{Ctx, Status, Tweak};

#[derive(Parser)]
#[command(version, about = "Обратимые твики Windows: телеметрия, с бэкапом и откатом")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    // эти два для тестов на копии, обычно не нужны
    #[arg(long, global = true, hide = true)]
    data_dir: Option<PathBuf>,
    #[arg(long, global = true, hide = true)]
    hosts_file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Cmd {
    /// список твиков
    List,
    /// что из твиков сейчас применено
    Status,
    /// применить твики
    Apply(Run),
    /// откатить твики
    Revert(Run),
}

#[derive(Args)]
struct Run {
    /// показать что будет, ничего не меняя
    #[arg(long)]
    dry_run: bool,
    /// только эти твики (id из list), можно несколько раз
    #[arg(long)]
    only: Vec<String>,
}

fn default_data_dir() -> PathBuf {
    let base = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    PathBuf::from(base).join("winlite-tuner")
}

fn default_hosts() -> PathBuf {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    PathBuf::from(root).join(r"System32\drivers\etc\hosts")
}

fn pick(tweaks: Vec<Box<dyn Tweak>>, only: &[String]) -> Result<Vec<Box<dyn Tweak>>> {
    if only.is_empty() {
        return Ok(tweaks);
    }
    for id in only {
        if !tweaks.iter().any(|t| t.id() == id) {
            bail!("нет твика '{id}', смотри `winlite-tuner list`");
        }
    }
    Ok(tweaks.into_iter().filter(|t| only.iter().any(|o| o == t.id())).collect())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut ctx = Ctx {
        dry_run: false,
        data_dir: cli.data_dir.unwrap_or_else(default_data_dir),
        hosts_path: cli.hosts_file.unwrap_or_else(default_hosts),
    };
    let all = telemetry::all();

    match cli.cmd {
        Cmd::List => {
            for t in &all {
                println!("{:<22} {}", t.id(), t.title());
            }
        }
        Cmd::Status => {
            for t in &all {
                let s = match t.check(&ctx) {
                    Ok(Status::Applied) => "применён".to_string(),
                    Ok(Status::NotApplied) => "не применён".to_string(),
                    Ok(Status::Unknown(why)) => format!("? {why}"),
                    Err(e) => format!("ошибка: {e:#}"),
                };
                println!("{:<22} {}", t.id(), s);
            }
        }
        Cmd::Apply(run) => go(&mut ctx, pick(all, &run.only)?, run.dry_run, true)?,
        Cmd::Revert(run) => go(&mut ctx, pick(all, &run.only)?, run.dry_run, false)?,
    }
    Ok(())
}

fn go(ctx: &mut Ctx, tweaks: Vec<Box<dyn Tweak>>, dry_run: bool, apply: bool) -> Result<()> {
    ctx.dry_run = dry_run;
    if !dry_run && tweaks.iter().any(|t| t.needs_admin(ctx)) && !registry::is_admin() {
        bail!("нужны права админа: запусти терминал «от имени администратора» (или добавь --dry-run)");
    }

    let mut state = State::load(&ctx.data_dir)?;
    for t in &tweaks {
        println!("{}", t.id());
        let cur = t.check(ctx)?;
        match (apply, cur) {
            (_, Status::Unknown(why)) => println!("  пропуск: {why}"),
            (true, Status::Applied) => println!("  уже применён"),
            (false, Status::NotApplied) => println!("  и так не применён"),
            (true, _) => t.apply(ctx, &mut state)?,
            (false, _) => t.revert(ctx, &mut state)?,
        }
    }
    if dry_run {
        println!("\ndry-run: ничего не изменено");
    }
    Ok(())
}
