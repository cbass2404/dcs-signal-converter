//! Boards the user added: kept in their own folder, loaded after the shipped
//! inventory, and never able to stop it loading.

use std::path::{Path, PathBuf};

use dsc_config::{user_devices, DeviceInventory, DeviceSpec};

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let d =
            std::env::temp_dir().join(format!("dsc-user-devices-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        Dir(d)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn shipped() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/devices")
}

fn board(key: &str, lamps: &[&str]) -> DeviceSpec {
    let leds: Vec<String> = lamps
        .iter()
        .enumerate()
        .map(|(i, n)| format!(r#"{{"index": {i}, "name": "{n}", "kind": "indicator", "max": 1}}"#))
        .collect();
    serde_json::from_str(&format!(
        r#"{{"key": "{key}", "display_name": "{key}", "protocol": "dsc", "usb_pid": 0,
            "parts": [{{"part_id": 0, "leds": [{}]}}]}}"#,
        leds.join(",")
    ))
    .unwrap()
}

#[test]
fn a_board_is_saved_replaced_and_removed_by_key() {
    let dir = Dir::new("save");
    user_devices::save(&dir.0, &board("Arduino_Panel", &["FIRE"])).unwrap();
    user_devices::save(&dir.0, &board("Arduino_Other", &["GEAR"])).unwrap();
    user_devices::save(&dir.0, &board("Arduino_Panel", &["FIRE", "HOOK"])).unwrap();
    let read = user_devices::read(&dir.0).unwrap();
    assert_eq!(read.len(), 2, "the second save of a key replaces it");
    assert_eq!(read[0].leds().count(), 2);

    assert!(user_devices::remove(&dir.0, "Arduino_Panel").unwrap());
    assert!(!user_devices::remove(&dir.0, "Arduino_Panel").unwrap());
    assert_eq!(user_devices::read(&dir.0).unwrap().len(), 1);

    let text = std::fs::read_to_string(dir.0.join(user_devices::FILE)).unwrap();
    assert!(text.contains("\r\n"), "written CRLF like every other file");
}

#[test]
fn the_users_boards_come_after_the_shipped_inventory() {
    let dir = Dir::new("merge");
    let alone = DeviceInventory::load_dir(&shipped()).unwrap().devices.len();
    let (inv, notes) = DeviceInventory::load_with_user(&shipped(), &dir.0).unwrap();
    assert_eq!(
        (inv.devices.len(), notes.len()),
        (alone, 0),
        "no folder, no change"
    );

    user_devices::save(&dir.0, &board("Arduino_Panel", &["FIRE"])).unwrap();
    let (inv, notes) = DeviceInventory::load_with_user(&shipped(), &dir.0).unwrap();
    assert_eq!(inv.devices.len(), alone + 1);
    assert_eq!(inv.devices.last().unwrap().key, "Arduino_Panel");
    assert!(notes.is_empty());
}

#[test]
fn a_board_with_a_shipped_key_is_left_out_saying_so() {
    let dir = Dir::new("clash");
    user_devices::save(&dir.0, &board("TAKEOFF_PLANEL_2", &["FIRE"])).unwrap();
    let (inv, notes) = DeviceInventory::load_with_user(&shipped(), &dir.0).unwrap();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("left out"));
    assert_ne!(
        inv.device("TAKEOFF_PLANEL_2").unwrap().protocol,
        "dsc",
        "the shipped one stands"
    );
}

#[test]
fn a_broken_user_file_never_stops_the_shipped_inventory() {
    let dir = Dir::new("broken");
    std::fs::create_dir_all(&dir.0).unwrap();
    std::fs::write(dir.0.join(user_devices::FILE), "{ not json").unwrap();
    let (inv, notes) = DeviceInventory::load_with_user(&shipped(), &dir.0).unwrap();
    assert!(inv.device("TAKEOFF_PLANEL_2").is_some());
    assert!(notes[0].contains("could not be read"), "{notes:?}");
}
