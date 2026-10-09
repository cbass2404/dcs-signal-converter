//! Boards the user added, kept in one file in the user devices folder.
//!
//! Each entry is what a board said about itself (see the `dsc-device` crate),
//! written as an ordinary inventory entry so it loads the way the shipped
//! ones do. Adding a board that is already there replaces its entry, which is
//! how a board whose lamps changed is described again.

use std::path::Path;

use crate::{DeviceInventory, DeviceSpec, Error, Result};

/// The file, in [`Paths::user_devices`](crate::paths::Paths::user_devices).
pub const FILE: &str = "boards.json";

/// Every board in the folder's file, none if there is no file yet.
pub fn read(dir: &Path) -> Result<Vec<DeviceSpec>> {
    let path = dir.join(FILE);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    Ok(DeviceInventory::load(&path)?.devices)
}

/// Add a board, or replace the one with its key.
pub fn save(dir: &Path, spec: &DeviceSpec) -> Result<()> {
    let mut boards = read(dir)?;
    match boards.iter_mut().find(|d| d.key == spec.key) {
        Some(d) => *d = spec.clone(),
        None => boards.push(spec.clone()),
    }
    write(dir, boards)
}

/// Take a board out. False when there was none by that key.
pub fn remove(dir: &Path, key: &str) -> Result<bool> {
    let mut boards = read(dir)?;
    let before = boards.len();
    boards.retain(|d| d.key != key);
    if boards.len() == before {
        return Ok(false);
    }
    write(dir, boards)?;
    Ok(true)
}

/// In one step, CRLF, as the settings are written.
fn write(dir: &Path, devices: Vec<DeviceSpec>) -> Result<()> {
    let path = dir.join(FILE);
    let text = serde_json::to_string_pretty(&DeviceInventory { devices })
        .map_err(|e| Error::Json(e, path.display().to_string()))?
        .replace('\n', "\r\n");
    std::fs::create_dir_all(dir)?;
    let temp = path.with_extension("json.saving");
    let written = std::fs::write(&temp, text).and_then(|()| std::fs::rename(&temp, &path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written?;
    Ok(())
}
