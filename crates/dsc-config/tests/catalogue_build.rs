//! When the catalogue is rebuilt from the installed DCS-BIOS, and when not.
//!
//! Each test builds a small fake DCS-BIOS install in its own temp folder, so
//! nothing here depends on DCS-BIOS being on the machine running the tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use dsc_config::catalogue_build::{ensure, rebuild, Freshness};
use dsc_config::Catalogue;

/// A fake `Saved Games/DCS/Scripts` with DCS-BIOS in it, and a catalogue
/// folder beside it that does not exist yet.
struct Install {
    root: PathBuf,
}

impl Install {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("dsc-build-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("DCS-BIOS").join("doc").join("json")).unwrap();
        let install = Install { root };
        install.set_version("2026.09.18-nightly");
        install.write(
            "AircraftAliases.json",
            r#"{"TestJet": ["CommonData", "TestJet"], "TestJet_2": ["CommonData", "TestJet"], "": ["x"]}"#,
        );
        install.write(
            "CommonData.json",
            r#"{"Metadata": {"VERSION": {"category": "Metadata", "control_type": "metadata",
                "outputs": [{"address": 1126, "max_length": 24, "type": "string"}]}}}"#,
        );
        install.write("TestJet.json", &jet("MASTER_CAUTION"));
        install
    }

    fn bios_json(&self) -> PathBuf {
        self.root.join("DCS-BIOS").join("doc").join("json")
    }

    fn catalogue(&self) -> PathBuf {
        self.root.join("catalogue")
    }

    fn set_version(&self, version: &str) {
        fs::write(
            self.root.join("DCS-BIOS").join("BIOSConfig.lua"),
            format!("BIOSConfig = {{\n\tversion = \"{version}\", -- set automatically\n}}\n"),
        )
        .unwrap();
    }

    fn write(&self, name: &str, text: &str) {
        fs::write(self.bios_json().join(name), text).unwrap();
    }
}

impl Drop for Install {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A module with one lamp and one input-only control.
fn jet(lamp: &str) -> String {
    format!(
        r#"{{"Caution Panel": {{
            "{lamp}": {{"category": "Caution Panel", "control_type": "led", "description": "lamp",
                "outputs": [{{"address": 4096, "mask": 1, "shift_by": 0, "max_value": 1,
                              "type": "integer", "description": "0 = Off, 1 = On"}}]}},
            "PRESS_TO_TEST": {{"category": "Caution Panel", "control_type": "action", "outputs": []}}
        }}}}"#
    )
}

fn built(fresh: &Freshness) -> bool {
    matches!(fresh, Freshness::Built { .. })
}

fn signal_ids(catalogue: &Path) -> Vec<String> {
    let cat = Catalogue::load_dir(catalogue).unwrap();
    cat.module("TestJet")
        .unwrap()
        .signals
        .iter()
        .map(|s| s.id.clone())
        .collect()
}

#[test]
fn a_missing_catalogue_is_built_and_reads_back() {
    let install = Install::new("missing");
    let fresh = ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert_eq!(
        fresh,
        Freshness::Built {
            was: None,
            now: "2026.09.18-nightly".into(),
            modules: 1
        }
    );

    let cat = Catalogue::load_dir(&install.catalogue()).unwrap();
    assert_eq!(cat.bios_version(), Some("2026.09.18-nightly"));
    assert_eq!(cat.version_signal(), Some((1126, 24)));
    // Both runtime names reach the module; the nameless alias does not.
    assert_eq!(cat.for_aircraft("TestJet_2").unwrap().module, "TestJet");
    let module = cat.module("TestJet").unwrap();
    assert_eq!(module.aircraft, ["TestJet", "TestJet_2"]);
    // The input-only control has nothing to read and is left out.
    assert_eq!(signal_ids(&install.catalogue()), ["MASTER_CAUTION"]);
    let output = module.signals[0].primary().unwrap();
    assert!(output.discrete);
    assert_eq!(output.values[1].label, "On");
}

#[test]
fn the_same_version_is_not_built_twice() {
    // What the second of the daemon and the editor to start sees.
    let install = Install::new("same");
    assert!(built(
        &ensure(&install.bios_json(), &install.catalogue()).unwrap()
    ));
    let again = ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert_eq!(
        again,
        Freshness::Current {
            version: "2026.09.18-nightly".into()
        }
    );
}

#[test]
fn a_new_version_replaces_the_whole_catalogue() {
    let install = Install::new("update");
    ensure(&install.bios_json(), &install.catalogue()).unwrap();

    // DCS-BIOS updates, and the lamp is renamed along the way.
    install.set_version("2026.10.01");
    install.write("TestJet.json", &jet("MASTER_CAUTION_LT"));
    let fresh = ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert_eq!(
        fresh,
        Freshness::Built {
            was: Some("2026.09.18-nightly".into()),
            now: "2026.10.01".into(),
            modules: 1
        }
    );
    // Replaced, not layered over: the old name is gone.
    assert_eq!(signal_ids(&install.catalogue()), ["MASTER_CAUTION_LT"]);
    // And nothing is left beside it from the swap.
    for leftover in ["catalogue.building", "catalogue.old", "catalogue.lock"] {
        assert!(
            !install.root.join(leftover).exists(),
            "{leftover} was left behind"
        );
    }
}

#[test]
fn changed_files_under_the_same_version_are_rebuilt() {
    // What a build part-way through an install leaves: the new version over
    // the old files. The files land after it, and the next start must notice
    // even though the version already matches.
    let install = Install::new("files");
    ensure(&install.bios_json(), &install.catalogue()).unwrap();
    install.write("TestJet.json", &jet("MASTER_CAUTION_LT"));
    let fresh = ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert_eq!(
        fresh,
        Freshness::Built {
            was: Some("2026.09.18-nightly".into()),
            now: "2026.09.18-nightly".into(),
            modules: 1
        }
    );
    assert_eq!(fresh.to_string(), "catalogue rebuilt: the DCS-BIOS 2026.09.18-nightly files changed since the last build: 1 modules");
    assert_eq!(signal_ids(&install.catalogue()), ["MASTER_CAUTION_LT"]);
    assert!(!built(
        &ensure(&install.bios_json(), &install.catalogue()).unwrap()
    ));
}

#[test]
fn a_module_dcs_bios_dropped_goes_with_the_rebuild() {
    let install = Install::new("dropped");
    install.write("OtherJet.json", &jet("GEAR_LT"));
    ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert!(install.catalogue().join("OtherJet.json").is_file());

    fs::remove_file(install.bios_json().join("OtherJet.json")).unwrap();
    install.set_version("2026.10.01");
    ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert!(!install.catalogue().join("OtherJet.json").exists());
}

#[test]
fn a_catalogue_from_the_python_builder_is_rebuilt_once() {
    // Same version, but no record of where the stream reports it, so the
    // daemon could not check the running DCS-BIOS, and no stamp of the files.
    // Rebuilt to gain both.
    let install = Install::new("python");
    fs::create_dir_all(install.catalogue()).unwrap();
    fs::write(
        install.catalogue().join("index.json"),
        r#"{"bios_version": "2026.09.18-nightly", "source": "x", "modules": {}}"#,
    )
    .unwrap();
    assert!(built(
        &ensure(&install.bios_json(), &install.catalogue()).unwrap()
    ));
    assert!(!built(
        &ensure(&install.bios_json(), &install.catalogue()).unwrap()
    ));
}

#[test]
fn a_catalogue_from_an_older_builder_is_rebuilt_once() {
    // Same version and files, but built before lamp colours and retired
    // signals were kept, so it is rebuilt to gain them.
    let install = Install::new("format");
    ensure(&install.bios_json(), &install.catalogue()).unwrap();
    let index = install.catalogue().join("index.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&index).unwrap()).unwrap();
    doc.as_object_mut().unwrap().remove("format");
    fs::write(&index, doc.to_string()).unwrap();
    assert!(built(
        &ensure(&install.bios_json(), &install.catalogue()).unwrap()
    ));
    assert!(!built(
        &ensure(&install.bios_json(), &install.catalogue()).unwrap()
    ));
}

#[test]
fn lamp_colours_and_retired_signals_are_kept() {
    let install = Install::new("extras");
    install.write(
        "TestJet.json",
        r#"{"Lamps": {
            "GEAR_LT": {"category": "Lamps", "control_type": "led", "description": "Gear",
                "color": "Green",
                "outputs": [{"address": 4096, "mask": 1, "shift_by": 0, "max_value": 1,
                              "type": "integer", "description": "0 if light is off, 1 if light is on"}]},
            "OLD_LT": {"category": "Lamps", "control_type": "led", "description": "Old",
                "deprecated": {"description": "lacks granular control", "since": "0.11.1",
                               "use_instead": "GEAR_LT"},
                "outputs": [{"address": 4096, "mask": 2, "shift_by": 1, "max_value": 1,
                              "type": "integer", "description": "0 if light is off, 1 if light is on"}]},
            "GONE_LT": {"category": "Lamps", "control_type": "led", "description": "Gone",
                "deprecated": {"since": "0.11.6"},
                "outputs": [{"address": 4096, "mask": 4, "shift_by": 2, "max_value": 1,
                              "type": "integer", "description": "0 if light is off, 1 if light is on"}]}
        }}"#,
    );
    ensure(&install.bios_json(), &install.catalogue()).unwrap();
    let cat = Catalogue::load_dir(&install.catalogue()).unwrap();
    let module = cat.module("TestJet").unwrap();
    let get = |id: &str| module.signals.iter().find(|s| s.id == id).unwrap();

    assert_eq!(get("GEAR_LT").color, "green");
    assert!(get("GEAR_LT").deprecated.is_none());
    let old = get("OLD_LT").deprecated.as_ref().unwrap();
    assert_eq!(old.use_instead, "GEAR_LT");
    assert_eq!(old.why, "lacks granular control");
    // Retired with nothing named in its place.
    let gone = get("GONE_LT").deprecated.as_ref().unwrap();
    assert_eq!((gone.use_instead.as_str(), gone.why.as_str()), ("", ""));
    assert_eq!(get("GONE_LT").color, "");
}

#[test]
fn no_dcs_bios_says_so_and_keeps_what_is_there() {
    let install = Install::new("nobios");
    ensure(&install.bios_json(), &install.catalogue()).unwrap();
    fs::remove_dir_all(install.root.join("DCS-BIOS")).unwrap();

    let fresh = ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert!(
        matches!(
            fresh,
            Freshness::NoBios {
                have_catalogue: true,
                ..
            }
        ),
        "{fresh:?}"
    );
    assert!(install.catalogue().join("TestJet.json").is_file());

    fs::remove_dir_all(install.catalogue()).unwrap();
    let fresh = ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert!(
        matches!(
            fresh,
            Freshness::NoBios {
                have_catalogue: false,
                ..
            }
        ),
        "{fresh:?}"
    );
    assert!(rebuild(&install.bios_json(), &install.catalogue()).is_err());
}

#[test]
fn a_lock_left_by_a_crashed_build_does_not_block_forever() {
    let install = Install::new("stalelock");
    let lock = install.root.join("catalogue.lock");
    fs::write(&lock, "12345").unwrap();
    let old = SystemTime::now() - Duration::from_secs(600);
    fs::File::options()
        .write(true)
        .open(&lock)
        .unwrap()
        .set_modified(old)
        .unwrap();

    assert!(built(
        &ensure(&install.bios_json(), &install.catalogue()).unwrap()
    ));
    assert!(!lock.exists());
}

#[test]
fn a_half_finished_build_is_cleared_before_the_next() {
    let install = Install::new("leftover");
    let leftover = install.root.join("catalogue.building");
    fs::create_dir_all(&leftover).unwrap();
    fs::write(leftover.join("Stale.json"), "{}").unwrap();

    ensure(&install.bios_json(), &install.catalogue()).unwrap();
    assert!(!install.catalogue().join("Stale.json").exists());
    assert!(!leftover.exists());
}

#[test]
fn a_forced_rebuild_runs_even_when_current() {
    let install = Install::new("force");
    ensure(&install.bios_json(), &install.catalogue()).unwrap();
    let fresh = rebuild(&install.bios_json(), &install.catalogue()).unwrap();
    assert_eq!(
        fresh,
        Freshness::Built {
            was: Some("2026.09.18-nightly".into()),
            now: "2026.09.18-nightly".into(),
            modules: 1
        }
    );
}
