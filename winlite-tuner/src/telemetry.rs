use std::fs;
use std::process::Command;

use anyhow::{Context, Result};

use crate::hosts;
use crate::registry;
use crate::state::{Prev, State};
use crate::tweak::{Ctx, Status, Tweak};

const DOMAINS: &str = include_str!("../data/telemetry_domains.txt");

const DIAG_KEY: &str = r"SYSTEM\CurrentControlSet\Services\DiagTrack";
const POLICY_KEY: &str = r"SOFTWARE\Policies\Microsoft\Windows\DataCollection";

pub fn all() -> Vec<Box<dyn Tweak>> {
    vec![Box::new(HostsBlock), Box::new(DiagTrack), Box::new(TelemetryPolicy)]
}

fn sc(action: &str) {
    // ошибку игнорим: служба может быть уже остановлена
    let _ = Command::new("sc.exe").args([action, "DiagTrack"]).output();
}

pub struct HostsBlock;

impl Tweak for HostsBlock {
    fn id(&self) -> &'static str {
        "telemetry-hosts"
    }
    fn title(&self) -> &'static str {
        "hosts: блок доменов телеметрии на 0.0.0.0"
    }
    fn needs_admin(&self, ctx: &Ctx) -> bool {
        // для тестов на копии hosts админ не нужен
        ctx.hosts_path.starts_with(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
    }

    fn check(&self, ctx: &Ctx) -> Result<Status> {
        let c = fs::read_to_string(&ctx.hosts_path)
            .with_context(|| format!("читаю {}", ctx.hosts_path.display()))?;
        Ok(if hosts::has_block(&c) { Status::Applied } else { Status::NotApplied })
    }

    fn apply(&self, ctx: &Ctx, _state: &mut State) -> Result<()> {
        let old = fs::read_to_string(&ctx.hosts_path)
            .with_context(|| format!("читаю {}", ctx.hosts_path.display()))?;
        let new = hosts::apply_block(&old, &hosts::parse_domains(DOMAINS))?;

        let backup = ctx.data_dir.join("hosts.original");
        if !backup.exists() {
            ctx.act(&format!("бэкап hosts → {}", backup.display()), || {
                fs::create_dir_all(&ctx.data_dir)?;
                fs::write(&backup, &old)?;
                Ok(())
            })?;
        }
        ctx.act("пишу блок в hosts", || {
            fs::write(&ctx.hosts_path, &new).context("запись hosts (нужен админ?)")
        })
    }

    fn revert(&self, ctx: &Ctx, _state: &mut State) -> Result<()> {
        let old = fs::read_to_string(&ctx.hosts_path)
            .with_context(|| format!("читаю {}", ctx.hosts_path.display()))?;
        let new = hosts::remove_block(&old)?;
        ctx.act("убираю блок из hosts", || {
            fs::write(&ctx.hosts_path, &new).context("запись hosts (нужен админ?)")
        })
    }
}

pub struct DiagTrack;

impl Tweak for DiagTrack {
    fn id(&self) -> &'static str {
        "telemetry-diagtrack"
    }
    fn title(&self) -> &'static str {
        "служба DiagTrack (Connected User Experiences and Telemetry): отключить"
    }

    fn check(&self, _ctx: &Ctx) -> Result<Status> {
        Ok(match registry::read_dword(DIAG_KEY, "Start")? {
            Some(4) => Status::Applied,
            Some(_) => Status::NotApplied,
            None => Status::Unknown("службы DiagTrack нет".into()),
        })
    }

    fn apply(&self, ctx: &Ctx, state: &mut State) -> Result<()> {
        let cur = registry::read_dword(DIAG_KEY, "Start")?;
        state.diagtrack_start = Some(Prev::from_opt(cur));
        ctx.save_state(state)?;
        ctx.act("DiagTrack: тип запуска → отключена", || registry::write_dword(DIAG_KEY, "Start", 4))?;
        ctx.act("DiagTrack: останавливаю", || {
            sc("stop");
            Ok(())
        })
    }

    fn revert(&self, ctx: &Ctx, state: &mut State) -> Result<()> {
        // нет сохранённого значения → ставим дефолт винды (авто)
        let back = match state.diagtrack_start {
            Some(Prev::Value(v)) => v,
            _ => 2,
        };
        ctx.act(&format!("DiagTrack: тип запуска → {back}"), || {
            registry::write_dword(DIAG_KEY, "Start", back)
        })?;
        if back != 4 {
            ctx.act("DiagTrack: запускаю", || {
                sc("start");
                Ok(())
            })?;
        }
        state.diagtrack_start = None;
        ctx.save_state(state)
    }
}

pub struct TelemetryPolicy;

impl Tweak for TelemetryPolicy {
    fn id(&self) -> &'static str {
        "telemetry-policy"
    }
    fn title(&self) -> &'static str {
        "политика AllowTelemetry = 0 (на Home/Pro винда считает это минимумом)"
    }

    fn check(&self, _ctx: &Ctx) -> Result<Status> {
        Ok(match registry::read_dword(POLICY_KEY, "AllowTelemetry")? {
            Some(0) => Status::Applied,
            _ => Status::NotApplied,
        })
    }

    fn apply(&self, ctx: &Ctx, state: &mut State) -> Result<()> {
        let cur = registry::read_dword(POLICY_KEY, "AllowTelemetry")?;
        state.allow_telemetry = Some(Prev::from_opt(cur));
        ctx.save_state(state)?;
        ctx.act("AllowTelemetry → 0", || registry::write_dword(POLICY_KEY, "AllowTelemetry", 0))
    }

    fn revert(&self, ctx: &Ctx, state: &mut State) -> Result<()> {
        match state.allow_telemetry {
            Some(Prev::Value(v)) => ctx.act(&format!("AllowTelemetry → {v}"), || {
                registry::write_dword(POLICY_KEY, "AllowTelemetry", v)
            })?,
            _ => ctx.act("AllowTelemetry: удаляю значение (раньше его не было)", || {
                registry::delete_value(POLICY_KEY, "AllowTelemetry")
            })?,
        }
        state.allow_telemetry = None;
        ctx.save_state(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(dir: &std::path::Path, dry: bool) -> Ctx {
        Ctx { dry_run: dry, data_dir: dir.join("data"), hosts_path: dir.join("hosts") }
    }

    #[test]
    fn domains_list_sane() {
        let d = hosts::parse_domains(DOMAINS);
        assert!(d.len() > 5);
        assert!(d.iter().all(|x| !x.contains("windowsupdate") && !x.contains("delivery")));
        let mut s = d.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), d.len(), "дубли в списке доменов");
    }

    #[test]
    fn hosts_apply_revert_on_copy() {
        let dir = tempfile::tempdir().unwrap();
        let orig = "127.0.0.1 localhost\r\n";
        fs::write(dir.path().join("hosts"), orig).unwrap();
        let c = ctx(dir.path(), false);
        let mut st = State::default();

        assert_eq!(HostsBlock.check(&c).unwrap(), Status::NotApplied);
        HostsBlock.apply(&c, &mut st).unwrap();
        assert_eq!(HostsBlock.check(&c).unwrap(), Status::Applied);
        assert!(c.data_dir.join("hosts.original").exists());

        HostsBlock.revert(&c, &mut st).unwrap();
        assert_eq!(fs::read_to_string(&c.hosts_path).unwrap(), orig);
    }

    #[test]
    fn dry_run_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("hosts"), "127.0.0.1 localhost\n").unwrap();
        let c = ctx(dir.path(), true);
        HostsBlock.apply(&c, &mut State::default()).unwrap();
        assert_eq!(fs::read_to_string(&c.hosts_path).unwrap(), "127.0.0.1 localhost\n");
        assert!(!c.data_dir.exists());
    }
}
