//! The device inventory as a folder: one file per maker, read as one.

use std::path::PathBuf;

use dsc_config::{DeviceInventory, Error};

/// A folder of inventory files, removed when dropped.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str, files: &[(&str, &[&str])]) -> Self {
        let dir =
            std::env::temp_dir().join(format!("dsc-device-folder-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, keys) in files {
            let devices: Vec<String> = keys
                .iter()
                .map(|k| {
                    format!(r#"{{"key": "{k}", "display_name": "{k}", "usb_pid": 1, "parts": []}}"#)
                })
                .collect();
            std::fs::write(
                dir.join(file),
                format!(r#"{{"devices": [{}]}}"#, devices.join(",")),
            )
            .unwrap();
        }
        Folder(dir)
    }

    fn load(&self) -> dsc_config::Result<DeviceInventory> {
        DeviceInventory::load_dir(&self.0)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn keys(inv: &DeviceInventory) -> Vec<&str> {
    inv.devices.iter().map(|d| d.key.as_str()).collect()
}

#[test]
fn files_are_read_in_name_order_and_keep_their_own() {
    // Whatever order the folder lists in, the inventory comes out the same.
    let folder = Folder::new(
        "order",
        &[("zeta.json", &["Z2", "Z1"]), ("alpha.json", &["A2", "A1"])],
    );
    assert_eq!(keys(&folder.load().unwrap()), ["A2", "A1", "Z2", "Z1"]);
}

#[test]
fn a_key_in_two_files_is_refused_naming_both() {
    // Bindings name a device by key, so neither file can quietly win.
    let folder = Folder::new("twice", &[("one.json", &["P"]), ("two.json", &["P"])]);
    match folder.load() {
        Err(Error::DeviceTwice(key, first, second)) => {
            assert_eq!(
                (key.as_str(), first.as_str(), second.as_str()),
                ("P", "one.json", "two.json")
            );
        }
        other => panic!("expected DeviceTwice, got {other:?}"),
    }
}

#[test]
fn only_json_files_are_read() {
    // A note or a backup left in the folder is not an inventory.
    let folder = Folder::new("json-only", &[("winctrl.json", &["W"])]);
    std::fs::write(folder.0.join("notes.txt"), "not json").unwrap();
    std::fs::write(folder.0.join("winctrl.json.bak"), "{").unwrap();
    assert_eq!(keys(&folder.load().unwrap()), ["W"]);
}

#[test]
fn the_shipped_folder_holds_each_maker_once() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/devices");
    let inv = DeviceInventory::load_dir(&dir).expect("the inventory loads");
    assert!(inv.device("TAKEOFF_PLANEL_2").is_some(), "winctrl.json");
    assert!(inv.device("CDU_Kneeboard").is_some(), "virtual.json");
}
