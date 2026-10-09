//! Boards that describe themselves: listing where one could be, asking one
//! what it has, and keeping what it said in the user's own devices folder.
//!
//! **Development only for now.** Nothing here has driven a real board yet,
//! so every command refuses outside a development checkout, and the window
//! shows the section only there. Opening it to everyone is taking the gate
//! out of [`dev_only`] once a board has been flown.
//!
//! Asking a board who it is means opening it, unlike the panels, which are
//! only listed. On serial that resets the board. So nothing here asks on its
//! own: a board is asked only when the user adds it.

use dsc_config::paths::Paths;
use dsc_config::settings::Settings;
use dsc_config::user_devices;
use dsc_device::{hid_boards, scan, serial_ports, HidLink, Link, SerialLink};

use crate::{fail, Reply};

fn dev_only(paths: &Paths) -> Reply<()> {
    if paths.is_dev() {
        Ok(())
    } else {
        Err("Adding boards is not available in this release yet.".into())
    }
}

/// Somewhere a board could be.
#[derive(serde::Serialize)]
pub struct Place {
    /// A COM port's name, or a HID path; what `board_add` takes.
    pub place: String,
    pub serial: bool,
    /// What USB says is there, when it says.
    pub what: String,
    /// A serial port the converter may open.
    pub allowed: bool,
}

/// A board the user added.
#[derive(serde::Serialize)]
pub struct SavedBoard {
    pub key: String,
    pub display_name: String,
    pub lamps: usize,
}

/// A board just added, and whether one by the same key was there already.
#[derive(serde::Serialize)]
pub struct AddedBoard {
    #[serde(flatten)]
    pub board: SavedBoard,
    /// The same board added again, or a second board with the same identity:
    /// the editor cannot tell which, so it says both.
    pub replaced: bool,
}

#[derive(serde::Serialize)]
pub struct BoardsView {
    /// HID boards first, then every COM port.
    pub places: Vec<Place>,
    pub saved: Vec<SavedBoard>,
}

fn saved(spec: &dsc_config::DeviceSpec) -> SavedBoard {
    SavedBoard {
        key: spec.key.clone(),
        display_name: spec.display_name.clone(),
        lamps: spec.leds().count(),
    }
}

/// Where boards could be, and the boards already added. Opens nothing.
#[tauri::command]
pub fn boards_list() -> Reply<BoardsView> {
    let paths = Paths::resolve();
    dev_only(&paths)?;
    let allowed = Settings::load(&paths.settings)
        .map(|s| s.dsc_ports)
        .unwrap_or_default();
    let api = dsc_device::hidapi::HidApi::new().map_err(|e| fail("listing HID devices", e))?;
    let mut places: Vec<Place> = hid_boards(&api)
        .into_iter()
        .map(|(place, what)| Place {
            place,
            serial: false,
            what,
            allowed: false,
        })
        .collect();
    places.extend(serial_ports().into_iter().map(|(place, what)| Place {
        allowed: allowed.iter().any(|a| a.eq_ignore_ascii_case(&place)),
        place,
        serial: true,
        what,
    }));
    let saved = user_devices::read(&paths.user_devices)
        .map_err(|e| fail("reading the added boards", e))?
        .iter()
        .map(saved)
        .collect();
    Ok(BoardsView { places, saved })
}

/// Ask the board at `place` what it has and keep it, replacing an earlier
/// entry for the same board. A serial port is allowed for the converter too.
/// Off the main thread: a serial board can take three seconds to answer.
#[tauri::command]
pub async fn board_add(place: String) -> Reply<AddedBoard> {
    tauri::async_runtime::spawn_blocking(move || add(&place))
        .await
        .map_err(|e| fail("adding the board", e))?
}

fn add(place: &str) -> Reply<AddedBoard> {
    let paths = Paths::resolve();
    dev_only(&paths)?;
    let serial = !place.starts_with('\\') && place.to_ascii_uppercase().starts_with("COM");
    let (mut link, wait): (Box<dyn Link>, _) = if serial {
        let link = SerialLink::open(place).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied || e.to_string().contains("denied") {
                format!("{place} is in use. The converter holds it while DCS runs, and the Arduino IDE's serial monitor can too; close whichever has it and try again.")
            } else {
                fail(&format!("opening {place}"), e)
            }
        })?;
        (Box::new(link), dsc_device::SERIAL_WAIT)
    } else {
        let api = dsc_device::hidapi::HidApi::new().map_err(|e| fail("listing HID devices", e))?;
        let link = HidLink::open(&api, place).map_err(|e| fail("opening the board", e))?;
        (Box::new(link), dsc_device::HID_WAIT)
    };
    let description = scan(link.as_mut(), wait).map_err(|e| fail(place, e))?;
    let spec = description
        .spec()
        .map_err(|e| fail("this board cannot be added", e))?;
    let (shipped, _) = paths
        .inventory()
        .map_err(|e| fail("reading the inventory", e))?;
    if shipped
        .device(&spec.key)
        .is_some_and(|d| d.protocol != dsc_config::DSC_PROTOCOL)
    {
        return Err(format!(
            "This board calls itself {}, which is a device this release ships. Change its model in the sketch.",
            spec.key
        ));
    }
    let replaced = user_devices::read(&paths.user_devices)
        .map_err(|e| fail("reading the added boards", e))?
        .iter()
        .any(|d| d.key == spec.key);
    user_devices::save(&paths.user_devices, &spec).map_err(|e| fail("saving the board", e))?;
    if serial {
        let mut settings = Settings::load(&paths.settings).unwrap_or_default();
        if !settings
            .dsc_ports
            .iter()
            .any(|p| p.eq_ignore_ascii_case(place))
        {
            settings.dsc_ports.push(place.to_string());
            settings
                .save(&paths.settings)
                .map_err(|e| fail("allowing the port", e))?;
        }
    }
    Ok(AddedBoard {
        board: saved(&spec),
        replaced,
    })
}

/// Every profile in the active folder that names `key`, as its file and the
/// profile. A profile that will not read is skipped: it names nothing usable.
fn binding(paths: &Paths, key: &str) -> Vec<(std::path::PathBuf, dsc_config::Profile)> {
    let keys = std::collections::BTreeSet::from([key.to_string()]);
    let Ok(dir) = std::fs::read_dir(&paths.profiles.active) else {
        return Vec::new();
    };
    let mut out: Vec<_> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| {
            dsc_config::Profile::load(&p)
                .ok()
                .map(|profile| (p, profile))
        })
        .filter(|(_, profile)| profile.names_any(&keys))
        .collect();
    out.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    out
}

/// The profiles removing `key` would change, by name, for the window to ask
/// about first.
#[tauri::command]
pub fn board_remove_plan(key: String) -> Reply<Vec<String>> {
    let paths = Paths::resolve();
    dev_only(&paths)?;
    Ok(binding(&paths, &key)
        .into_iter()
        .map(|(_, p)| p.name)
        .collect())
}

/// Take a board out, and out of every profile that names it first, so none
/// is left binding a device that is no longer there. Returns the profiles
/// changed. A running converter picks them up as it does any saved profile.
#[tauri::command]
pub fn board_remove(key: String) -> Reply<Vec<String>> {
    let paths = Paths::resolve();
    dev_only(&paths)?;
    let keys = std::collections::BTreeSet::from([key.clone()]);
    let mut changed = Vec::new();
    for (file, profile) in binding(&paths, &key) {
        profile
            .without_devices(&keys)
            .save(&file)
            .map_err(|e| fail(&format!("saving {}", profile.name), e))?;
        changed.push(profile.name);
    }
    user_devices::remove(&paths.user_devices, &key).map_err(|e| fail("removing the board", e))?;
    Ok(changed)
}

/// Stop the converter opening a serial port.
#[tauri::command]
pub fn board_port_forget(port: String) -> Reply<()> {
    let paths = Paths::resolve();
    dev_only(&paths)?;
    let mut settings =
        Settings::load(&paths.settings).map_err(|e| fail("reading the settings", e))?;
    settings
        .dsc_ports
        .retain(|p| !p.eq_ignore_ascii_case(&port));
    settings
        .save(&paths.settings)
        .map_err(|e| fail("saving the settings", e))
}
