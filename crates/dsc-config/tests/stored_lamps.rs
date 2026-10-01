//! Stored signals of lamp conditions, and stored numbers deciding switches.
//!
//! The CH-47's master caution repeats on every CDU and on the PTO2, and which
//! cockpit lamp it follows depends on the seat. These hold the promises that
//! make one set of conditions drive all of them: a lamp lit by a signal lights
//! exactly as if the conditions were its own, at its own brightness; the
//! engine watches what the signal reads; and a signal of the wrong kind, or
//! one the module lacks, is refused where it is used. Then the screen side:
//! a switch decided by a stored number, open-ended bands, and a stored number
//! worked out again only when a part has moved.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use dsc_config::{
    DeviceInventory, DisplayCatalogue, Module, Page, PageLibrary, Profile, Reading, Readout,
    StoredCache, StoredSignal, ValueBand,
};

fn r(p: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

/// A seat, a caution lamp per seat, and a fuel needle.
fn module() -> Module {
    serde_json::from_str(
        r#"{
          "module": "TEST",
          "aircraft": ["TEST"],
          "signals": [
            {"id": "SEAT_POSITION", "control_type": "metadata",
             "outputs": [{"address": 100, "mask": 3, "shift": 0, "max_value": 1, "max_length": null}]},
            {"id": "PLT_MC", "control_type": "led",
             "outputs": [{"address": 102, "mask": 1, "shift": 0, "max_value": 1, "max_length": null}]},
            {"id": "CPLT_MC", "control_type": "led",
             "outputs": [{"address": 102, "mask": 2, "shift": 1, "max_value": 1, "max_length": null}]},
            {"id": "FUEL", "control_type": "analog_gauge",
             "outputs": [{"address": 104, "mask": 65535, "shift": 0, "max_value": 65535, "max_length": null}]}
          ]
        }"#,
    )
    .expect("the fixture module parses")
}

/// The pilot's lamp in the pilot's seat, the copilot's in the copilot's.
fn caution() -> StoredSignal {
    serde_json::from_str(
        r#"{"id": "mc0001", "name": "Master caution", "note": "Whichever seat you are in",
            "any_of": [
              {"conditions": [{"source": "SEAT_POSITION", "on_when": {"equals": 0}},
                              {"source": "PLT_MC", "on_when": {"equals": 1}}]},
              {"conditions": [{"source": "SEAT_POSITION", "on_when": {"equals": 1}},
                              {"source": "CPLT_MC", "on_when": {"equals": 1}}]}
            ]}"#,
    )
    .unwrap()
}

/// Fuel in pounds, 0 to 10000 across the needle.
fn fuel() -> StoredSignal {
    serde_json::from_str(
        r#"{"id": "fuel01", "name": "Fuel", "terms": [{"source": "FUEL", "reads": [0, 10000]}]}"#,
    )
    .unwrap()
}

fn lib(pages: Vec<Page>) -> PageLibrary {
    PageLibrary::with_signals("TEST", pages, vec![caution(), fuel()])
}

/// A profile on TEST with these lamp rows.
fn profile(bindings: &str) -> Profile {
    serde_json::from_str(&format!(
        r#"{{"schema_version": 2, "name": "T", "aircraft": ["TEST"], "module": "TEST",
             "bindings": [{bindings}]}}"#
    ))
    .unwrap()
}

fn devices() -> DeviceInventory {
    DeviceInventory::load(&r("data/devices.json")).expect("devices")
}

/// What each lamp row reads with the seat and both cockpit lamps as given.
fn lit(p: &Profile, seat: u32, plt: u32, cplt: u32) -> Vec<Option<u8>> {
    let devices = devices();
    p.bindings
        .iter()
        .map(|b| {
            let (_, led) = devices.device(&b.device).unwrap().led(&b.led).unwrap();
            p.resolve_binding(b, led, |id| match id {
                "SEAT_POSITION" => Some(seat),
                "PLT_MC" => Some(plt),
                "CPLT_MC" => Some(cplt),
                _ => None,
            })
        })
        .collect()
}

const INDICATOR: &str = r#"{"device": "Orion_Throttle_Base_II", "led": "A/A", "signal": "mc0001"}"#;

#[test]
fn a_lamp_lights_by_the_signal_as_if_its_conditions_were_its_own() {
    let p = profile(INDICATOR).with_pages(&lib(Vec::new()));
    assert_eq!(
        lit(&p, 0, 1, 0),
        vec![Some(1)],
        "the pilot's lamp in the pilot's seat"
    );
    assert_eq!(
        lit(&p, 1, 1, 0),
        vec![Some(0)],
        "the pilot's lamp, but the copilot's seat"
    );
    assert_eq!(
        lit(&p, 1, 0, 1),
        vec![Some(1)],
        "the copilot's lamp in the copilot's seat"
    );
}

#[test]
fn each_lamp_keeps_its_own_brightness() {
    let p = profile(
        r#"{"device": "TAKEOFF_PLANEL_2", "led": "SL", "signal": "mc0001"},
           {"device": "TAKEOFF_PLANEL_2", "led": "Landing_gear_lights", "signal": "mc0001", "on": 40, "off": 5}"#,
    )
    .with_pages(&lib(Vec::new()));
    assert_eq!(lit(&p, 0, 1, 0), vec![Some(255), Some(40)]);
    assert_eq!(lit(&p, 0, 0, 0), vec![Some(0), Some(5)]);
}

#[test]
fn the_engine_watches_what_the_signal_reads() {
    let p = profile(INDICATOR).with_pages(&lib(Vec::new()));
    let mut read = p.sources_of(&p.bindings[0]);
    read.sort_unstable();
    read.dedup();
    assert_eq!(read, vec!["CPLT_MC", "PLT_MC", "SEAT_POSITION"]);
}

#[test]
fn a_lamp_row_lit_by_a_signal_is_not_a_placeholder() {
    let p = profile(INDICATOR);
    assert!(!p.bindings[0].is_placeholder());
}

fn refusals(p: &Profile, lib: &PageLibrary) -> Vec<String> {
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    p.problems(&module(), &devices(), &displays, lib)
        .iter()
        .map(|e| e.to_string())
        .collect()
}

#[test]
fn a_lamp_is_checked_against_the_library_before_pages_are_attached() {
    // The daemon validates a profile before it runs it with its pages.
    assert!(refusals(&profile(INDICATOR), &lib(Vec::new())).is_empty());
    let gone = refusals(&profile(INDICATOR), &PageLibrary::default());
    assert!(gone.iter().any(|e| e.contains("mc0001")), "{gone:?}");
}

#[test]
fn a_lamp_cannot_light_by_a_number() {
    let found = refusals(
        &profile(r#"{"device": "Orion_Throttle_Base_II", "led": "A/A", "signal": "fuel01"}"#),
        &lib(Vec::new()),
    );
    assert!(
        found
            .iter()
            .any(|e| e.contains("a shared result, not shared conditions")),
        "{found:?}"
    );
}

#[test]
fn a_lamp_lit_by_a_signal_has_no_conditions_of_its_own() {
    let found = refusals(
        &profile(
            r#"{"device": "Orion_Throttle_Base_II", "led": "A/A", "signal": "mc0001",
                "conditions": [{"source": "PLT_MC", "on_when": {"equals": 1}}]}"#,
        ),
        &lib(Vec::new()),
    );
    assert!(
        found.iter().any(|e| e.contains("it can have one")),
        "{found:?}"
    );
}

#[test]
fn a_signal_of_lamp_conditions_is_held_to_what_a_lamp_is() {
    let mut bad = caution();
    bad.any_of[1].conditions[1].source.clear();
    let found: Vec<String> = bad
        .problems(&module())
        .iter()
        .map(|e| e.to_string())
        .collect();
    assert!(
        found.iter().any(|e| e.contains("no signal chosen")),
        "{found:?}"
    );
}

/// A field of one switch on the MCDU's top row, loaded as a page file loads.
fn switch_field(content: &str) -> Readout {
    let page: Page = serde_json::from_str(&format!(
        r#"{{"id": "page01", "name": "P", "display": "MCDU",
             "fields": [{{"cells": "0-9", "content": [{content}]}}]}}"#
    ))
    .unwrap();
    let lib = lib(vec![page]);
    lib.on_module("TEST")[0].fields[0].clone()
}

fn drawn(field: &Readout, fuel: u16) -> String {
    let glyphs = field
        .compose(|id| {
            (id == "FUEL").then_some(Reading::Number {
                value: fuel,
                max: 65535,
            })
        })
        .unwrap();
    glyphs
        .iter()
        .map(|g| g.text.as_str())
        .collect::<String>()
        .trim()
        .to_string()
}

const LOW_FUEL: &str = r#"{"switch_signal": "fuel01",
    "cases": {"..999": {"text": "LOW"}, "1000..": {"text": "FUEL"}}}"#;

#[test]
fn a_stored_number_decides_a_switch_in_its_own_units() {
    let field = switch_field(LOW_FUEL);
    assert_eq!(drawn(&field, 6000), "LOW", "about 915 lb");
    assert_eq!(drawn(&field, 30000), "FUEL", "about 4578 lb");
    assert_eq!(field.sources(), vec!["FUEL"]);
}

#[test]
fn a_switch_on_a_signal_of_lamp_conditions_is_refused() {
    let field = switch_field(
        r#"{"switch_signal": "mc0001", "cases": {"1": {"text": "MC"}, "else": {"text": ""}}}"#,
    );
    let mut p = profile("");
    let mut field = field;
    field.device = "MCDU_Captain".into();
    field.display = "MCDU".into();
    field.page = Some("page01".into());
    p.font = Some("../mcdu/f14bu-font-21x31.json".into());
    p.readouts = vec![field];
    let found = refusals(&p, &PageLibrary::default());
    assert!(
        found.iter().any(|e| e.contains("lamp conditions")),
        "{found:?}"
    );
}

#[test]
fn open_bands_read_and_write_as_they_are_typed() {
    for written in ["1000..", "..999", "-5..5", "3"] {
        let band: ValueBand = written.parse().unwrap();
        assert_eq!(band.to_string(), written);
    }
    let at_least: ValueBand = "1000..".parse().unwrap();
    assert!(at_least.matches(1e9, 0.5) && !at_least.matches(999.0, 0.0));
    assert!("..".parse::<ValueBand>().is_err());
}

#[test]
fn a_stored_number_is_worked_out_again_only_when_a_part_moves() {
    let page: Page = serde_json::from_str(
        r#"{"id": "page01", "name": "P", "display": "MCDU",
            "fields": [{"cells": "0-9", "signal": "fuel01"}]}"#,
    )
    .unwrap();
    let field = lib(vec![page]).on_module("TEST")[0].fields[0].clone();
    let cache = StoredCache::default();
    let worked = Cell::new(0);
    let paint = |fuel: u16| {
        field
            .compose_cached(
                |id| {
                    (id == "FUEL").then_some(Reading::Number {
                        value: fuel,
                        max: 65535,
                    })
                },
                &cache,
                |_, _, _| worked.set(worked.get() + 1),
            )
            .unwrap();
    };
    paint(30000);
    paint(30000);
    assert_eq!(worked.get(), 1, "the needle stood still");
    paint(30001);
    assert_eq!(worked.get(), 2, "the needle moved");
}
