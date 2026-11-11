use std::path::PathBuf;

use anyhow::Result;

use crate::state::State;

#[derive(Debug, PartialEq)]
pub enum Status {
    Applied,
    NotApplied,
    Unknown(String),
}

pub struct Ctx {
    pub dry_run: bool,
    pub data_dir: PathBuf,
    pub hosts_path: PathBuf,
}

impl Ctx {
    // единственное место, где что-то реально меняется. В dry-run только печатаем
    pub fn act<F: FnOnce() -> Result<()>>(&self, what: &str, f: F) -> Result<()> {
        if self.dry_run {
            println!("  [dry-run] {what}");
            Ok(())
        } else {
            println!("  {what}");
            f()
        }
    }

    pub fn save_state(&self, state: &State) -> Result<()> {
        if self.dry_run { Ok(()) } else { state.save(&self.data_dir) }
    }
}

pub trait Tweak {
    fn id(&self) -> &'static str;
    fn title(&self) -> &'static str;
    fn needs_admin(&self, _ctx: &Ctx) -> bool {
        true
    }
    fn check(&self, ctx: &Ctx) -> Result<Status>;
    fn apply(&self, ctx: &Ctx, state: &mut State) -> Result<()>;
    fn revert(&self, ctx: &Ctx, state: &mut State) -> Result<()>;
}
