use std::io::ErrorKind;

use anyhow::{Context, Result};
use winreg::RegKey;
use winreg::enums::*;

fn hklm() -> RegKey {
    RegKey::predef(HKEY_LOCAL_MACHINE)
}

pub fn read_dword(path: &str, name: &str) -> Result<Option<u32>> {
    let key = match hklm().open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => k,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("открыть HKLM\\{path}")),
    };
    match key.get_value::<u32, _>(name) {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("прочитать {name} в HKLM\\{path}")),
    }
}

pub fn write_dword(path: &str, name: &str, value: u32) -> Result<()> {
    let (key, _) = hklm()
        .create_subkey_with_flags(path, KEY_WRITE)
        .with_context(|| format!("открыть на запись HKLM\\{path}"))?;
    key.set_value(name, &value)
        .with_context(|| format!("записать {name} в HKLM\\{path}"))
}

pub fn delete_value(path: &str, name: &str) -> Result<()> {
    let key = match hklm().open_subkey_with_flags(path, KEY_SET_VALUE) {
        Ok(k) => k,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("открыть HKLM\\{path}")),
    };
    match key.delete_value(name) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("удалить {name} в HKLM\\{path}")),
    }
}

// без админа HKLM\SOFTWARE на запись не откроется
pub fn is_admin() -> bool {
    hklm().open_subkey_with_flags("SOFTWARE", KEY_WRITE).is_ok()
}
