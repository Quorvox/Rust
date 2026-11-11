use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};

use quor_presence::config;
use quor_presence::presence::{Status, Worker, now_ms};

const EXAMPLE: &str = include_str!("../profiles.example.toml");

#[derive(Parser)]
#[command(version, about = "Свой статус в Discord (Rich Presence) с любым таймером")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    /// файл с профилями
    #[arg(long, global = true, default_value = "profiles.toml")]
    config: PathBuf,
}

#[derive(Subcommand)]
enum Cmd {
    /// создать profiles.toml из примера
    Init,
    /// показать профили
    List,
    /// поставить статус (держится, пока работает программа, стоп — Ctrl+C)
    Start {
        profile: String,
        /// сколько часов «уже играю», перебивает hours из профиля
        #[arg(long)]
        hours: Option<f64>,
        #[arg(long)]
        details: Option<String>,
        #[arg(long)]
        state: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init => {
            if cli.config.exists() {
                bail!("{} уже есть, ничего не трогаю", cli.config.display());
            }
            std::fs::write(&cli.config, EXAMPLE).context("не смог записать файл")?;
            println!("создал {}, впиши в него свой Application ID", cli.config.display());
        }
        Cmd::List => {
            for (name, p) in config::load(&cli.config)? {
                let hours = p.hours.map_or(String::from("-"), |h| format!("{h} ч"));
                println!("{name:<14} {:<22} {hours}", p.name.as_deref().unwrap_or("(имя приложения)"));
            }
        }
        Cmd::Start { profile, hours, details, state } => {
            let all = config::load(&cli.config)?;
            let mut p = all
                .get(&profile)
                .cloned()
                .ok_or_else(|| anyhow!("нет профиля '{profile}', смотри `quor-presence list`"))?;
            if details.is_some() {
                p.details = details;
            }
            if state.is_some() {
                p.state = state;
            }
            p.validate()?;
            let start = config::start_ms(now_ms(), hours.or(p.hours).unwrap_or(0.0))?;
            run(p, start);
        }
    }
    Ok(())
}

fn run(p: config::Profile, start: i64) -> ! {
    let worker = Worker::spawn(p, start);
    let mut last = None;
    loop {
        let s = worker.status();
        if last != Some(s) {
            match s {
                Status::Active => println!("подключено к Discord, статус стоит. Ctrl+C — убрать."),
                Status::Waiting => println!("Discord не отвечает, жду (он запущен?)..."),
            }
            last = Some(s);
        }
        sleep(Duration::from_secs(1));
    }
}
