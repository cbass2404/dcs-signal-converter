//! What the editor asks for: every fault at once, named where it is.
//!
//! `validate` answers the daemon's question, which is whether to load the file
//! at all, so it stops at the first fault. The editor asks a different one: it
//! is showing the user a list to work through, and reporting one fault at a
//! time would mean fixing something only to be told the same bad news again.
//!
//! These cover `problems` specifically: that it finds what `validate` finds,
//! that it keeps going, and that it says each thing once.

use std::path::Path;

use dsc_config::{DeviceInventory, DisplayCatalogue, Module, Profile};

fn r(p: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn module() -> Module {
    serde_json::from_str(
        r#"{
          "module": "TEST",
          "aircraft": ["TEST"],
          "signals": [
            {
              "id": "GEAR",
              "control_type": "led",
              "outputs": [{"address": 100, "mask": 1, "shift": 0, "max_value": 1, "max_length": null}]
            },
            {
              "id": "CHAN",
              "control_type": "display",
              "outputs": [{"address": 200, "mask": null, "max_value": null, "max_length": 2, "type": "string"}]
            },
            {
              "id": "KNOB",
              "control_type": "selector",
              "outputs": [{"address": 300, "mask": 224, "shift": 5, "max_value": 5, "max_length": null}]
            },
            {
              "id": "NEEDLE",
              "control_type": "analog_gauge",
              "outputs": [{"address": 400, "mask": 65535, "shift": 0, "max_value": 65535, "max_length": null}]
            }
          ]
        }"#,
    )
    .expect("the fixture module parses")
}

/// A profile of version 2 on TEST. Every screen takes its fields only from
/// a page, so each field here is marked as a page's, which is what the
/// field checks see once a start page is resolved.
fn profile(body: &str) -> Profile {
    let mut p: Profile = serde_json::from_str(&format!(
        r#"{{"schema_version": 2, "name": "T", "aircraft": ["TEST"], "module": "TEST", {body}}}"#
    ))
    .expect("the fixture profile parses");
    for r in &mut p.readouts {
        r.page = Some("fixture".into());
    }
    p
}

fn found(p: &Profile) -> Vec<String> {
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    p.problems(
        &module(),
        &devices,
        &displays,
        &dsc_config::PageLibrary::default(),
    )
    .iter()
    .map(|e| e.to_string())
    .collect()
}

#[test]
fn a_clean_profile_has_nothing_to_report() {
    let p = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0,
             "conditions": [{"source": "GEAR", "on_when": {"equals": 1}}]}
        ]"#,
    );
    assert!(found(&p).is_empty(), "{:?}", found(&p));
}

#[test]
fn every_fault_is_reported_not_just_the_first() {
    // The point of the whole exercise. A user given one of these, who fixes it
    // and is handed the next, has been made to do the work three times.
    let p = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "NOT_A_LAMP", "off": 0,
             "conditions": [{"source": "GEAR", "on_when": {"equals": 1}}]},
            {"device": "TAKEOFF_PLANEL_2", "led": "SL", "off": 0, "always": true,
             "conditions": [{"source": "GEAR", "on_when": {"equals": 1}}]},
            {"device": "TAKEOFF_PLANEL_2", "led": "Master_Caution", "off": 0, "on": 200,
             "conditions": [{"source": "GEAR", "on_when": {"equals": 1}}]}
        ]"#,
    );
    let problems = found(&p);
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert!(
        problems.iter().any(|m| m.contains("NOT_A_LAMP")),
        "{problems:?}"
    );
    assert!(problems.iter().any(|m| m.contains("SL")), "{problems:?}");
    assert!(problems.iter().any(|m| m.contains("200")), "{problems:?}");
}

#[test]
fn validate_still_stops_at_the_first_one() {
    // The daemon's contract is unchanged: one error, and the profile is skipped.
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    let p = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "NOT_A_LAMP", "off": 0,
             "conditions": [{"source": "GEAR", "on_when": {"equals": 1}}]}
        ]"#,
    );
    p.validate(
        &module(),
        &devices,
        &displays,
        &dsc_config::PageLibrary::default(),
    )
    .expect_err("a bad profile is still an error");

    // A signal this DCS-BIOS lacks is not one: the profile loads, flagged.
    let other_release = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0,
             "conditions": [{"source": "NOT_A_SIGNAL", "on_when": {"equals": 1}}]}
        ]"#,
    );
    other_release
        .validate(
            &module(),
            &devices,
            &displays,
            &dsc_config::PageLibrary::default(),
        )
        .expect("a missing signal flags the row rather than refusing the profile");

    let clean = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0,
             "conditions": [{"source": "GEAR", "on_when": {"equals": 1}}]}
        ]"#,
    );
    clean
        .validate(
            &module(),
            &devices,
            &displays,
            &dsc_config::PageLibrary::default(),
        )
        .expect("a clean profile still loads");
}

#[test]
fn a_condition_with_no_signal_chosen_is_said_in_those_terms() {
    // The editor creates one of these the moment "Add condition" is clicked, so
    // it is the most common thing a half-finished profile carries. Reporting it
    // as `unknown signal ""` would be true and useless.
    let p = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0,
             "conditions": [
                {"source": "", "on_when": {"equals": 1}},
                {"source": "", "on_when": {"equals": 1}}
             ]}
        ]"#,
    );
    let problems = found(&p);
    // Twice unfinished is still one lamp to go and finish.
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("Backlight"), "{:?}", problems[0]);
    assert!(
        problems[0].contains("no signal chosen"),
        "{:?}",
        problems[0]
    );
}

#[test]
fn a_field_with_no_signal_chosen_names_where_it_is() {
    // Adding a field lands on a run of cells before it has anything in it,
    // and the cells are the only thing on screen that identifies it.
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": ""}
        ]"#,
    );
    let problems = found(&p);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("30-33"), "{:?}", problems[0]);
    assert!(problems[0].contains("nothing in it"), "{:?}", problems[0]);
}

#[test]
fn an_overlap_is_one_problem_not_two() {
    // Both fields are at fault and either one could be moved, but there is one
    // thing wrong. Listing it from each end would read as two conflicts.
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": "CHAN"},
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "32-35", "source": "CHAN"}
        ]"#,
    );
    let problems = found(&p);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("overlap"), "{:?}", problems[0]);
}

#[test]
fn a_mirror_chain_is_found_without_the_target_hiding_it() {
    // Two dimmers pointed at each other, which is what a two-dimmer panel
    // produces if the editor lets both lamps mirror the other one.
    let p = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0, "same_as": "FLAG"},
            {"device": "TAKEOFF_PLANEL_2", "led": "FLAG", "off": 0, "same_as": "Backlight"}
        ]"#,
    );
    let problems = found(&p);
    assert_eq!(problems.len(), 2, "both ends are a chain: {problems:?}");
    assert!(
        problems.iter().all(|m| m.contains("mirrors")),
        "{problems:?}"
    );
}

#[test]
fn a_backlight_can_match_one_on_another_device() {
    // One knob for the pit: the MFD bezel follows the PTO2's backlight rather
    // than carrying a copy of its condition.
    let p = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0,
             "conditions": [{"source": "GEAR", "on_when": {"equals": 1}}]},
            {"device": "CarrierAce_MFD_L", "led": "INST_PNL_Backlight", "off": 0,
             "same_as": "Backlight", "same_as_device": "TAKEOFF_PLANEL_2"}
        ]"#,
    );
    assert!(found(&p).is_empty(), "{:?}", found(&p));
}

#[test]
fn a_mirror_on_another_device_is_checked_there() {
    // The lamp is looked for on the device named, not on the mirroring one:
    // the MFD has no `Backlight`, and the Orion's `A/A` is an indicator.
    let missing = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0,
             "same_as": "Backlight", "same_as_device": "CarrierAce_MFD_L"}
        ]"#,
    );
    let problems = found(&missing);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("CarrierAce_MFD_L"),
        "{:?}",
        problems[0]
    );

    let indicator = profile(
        r#""bindings": [
            {"device": "Orion_Throttle_Base_II", "led": "A/A", "off": 0,
             "conditions": [{"source": "GEAR", "on_when": {"equals": 1}}]},
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0,
             "same_as": "A/A", "same_as_device": "Orion_Throttle_Base_II"}
        ]"#,
    );
    let problems = found(&indicator);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("only lamps that dim"),
        "{:?}",
        problems[0]
    );
}

#[test]
fn a_loop_across_two_panels_is_still_a_chain() {
    // Across devices the rule is the same: the target must read signals of its
    // own, so two panels pointed at each other are refused at both ends.
    let p = profile(
        r#""bindings": [
            {"device": "TAKEOFF_PLANEL_2", "led": "Backlight", "off": 0,
             "same_as": "INST_PNL_Backlight", "same_as_device": "CarrierAce_MFD_L"},
            {"device": "CarrierAce_MFD_L", "led": "INST_PNL_Backlight", "off": 0,
             "same_as": "Backlight", "same_as_device": "TAKEOFF_PLANEL_2"}
        ]"#,
    );
    let problems = found(&p);
    assert_eq!(problems.len(), 2, "both ends are a chain: {problems:?}");
    assert!(
        problems
            .iter()
            .all(|m| m.contains("mirrors something itself")),
        "{problems:?}"
    );
}

#[test]
fn a_lamp_cannot_reach_itself_through_a_panel_that_follows() {
    // The MFD_R takes the MFD_L's setup, so its backlight is the MFD_L's, and
    // pointing the MFD_L at it is pointing the lamp at itself.
    let p = profile(
        r#""follows": {"CarrierAce_MFD_R": "CarrierAce_MFD_L"},
        "bindings": [
            {"device": "CarrierAce_MFD_L", "led": "INST_PNL_Backlight", "off": 0,
             "same_as": "INST_PNL_Backlight", "same_as_device": "CarrierAce_MFD_R"}
        ]"#,
    );
    let problems = found(&p);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("mirrors something itself"),
        "{:?}",
        problems[0]
    );
}

#[test]
fn a_number_shown_as_sent_needs_no_range() {
    // A six position knob drawn as the digit DCS-BIOS sends. The editor
    // offers that, so the check has to let it through.
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": "KNOB"}
        ]"#,
    );
    assert!(found(&p).is_empty(), "{:?}", found(&p));
}

#[test]
fn a_number_can_be_shown_by_its_aliases() {
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": "KNOB",
             "value_aliases": {"0": "OFF", "3": "SEMI"}}
        ]"#,
    );
    assert!(found(&p).is_empty(), "{:?}", found(&p));
}

#[test]
fn a_reading_can_be_banded_across_a_converted_face() {
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": "NEEDLE",
             "reads": [-1.5, 1.5], "decimals": 1, "abs": true,
             "value_aliases": {"-1.5..-0.1": "ND", "0": " ", "0.1..1.5": "NU"}}
        ]"#,
    );
    assert!(found(&p).is_empty(), "{:?}", found(&p));
}

#[test]
fn two_bands_claiming_one_reading_are_a_caution() {
    // Ambiguous but not undefined: bands are held in order of where they
    // start, so the lower one draws. A profile is refused whole, so refusing
    // this would take every lamp and screen dark over one row drawing the
    // first of two words the user wrote.
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": "NEEDLE",
             "reads": [-1.5, 1.5], "decimals": 1,
             "value_aliases": {"-1.5..0": "ND", "-0.5..0.5": "NEAR"}}
        ]"#,
    );
    assert!(found(&p).is_empty(), "{:?}", found(&p));
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    let cautions = p.field_cautions(&module(), &devices, &displays);
    assert_eq!(cautions.len(), 1, "{cautions:?}");
    assert!(
        cautions[0].1.contains("the lower one draws it"),
        "{:?}",
        cautions[0]
    );
}

#[test]
fn a_band_nothing_can_reach_is_a_caution() {
    // The likeliest way to get this wrong: band a converted face in the
    // numbers DCS-BIOS sends instead of the ones the dial is marked with. A
    // band past the top of the face can never draw, and nothing else can tell
    // the user so.
    //
    // Only a band wholly outside the face counts. One that reaches into it
    // draws for part of its travel, which is a band written loosely rather
    // than a band written for the wrong numbers.
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": "NEEDLE",
             "reads": [-1.5, 1.5], "decimals": 1,
             "value_aliases": {"32768..65535": "NU"}}
        ]"#,
    );
    // A caution, so the profile still loads and the panel settles it.
    assert!(found(&p).is_empty(), "{:?}", found(&p));
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    let cautions = p.field_cautions(&module(), &devices, &displays);
    assert_eq!(cautions.len(), 1, "{cautions:?}");
    assert!(
        cautions[0].1.contains("outside everything the face reads"),
        "{:?}",
        cautions[0]
    );
}

/// A field on NEEDLE converted by the Mosquito's inner tank table, with the
/// stretches given, for the conversion checks.
fn tank(stretches: &str, extra: &str) -> Profile {
    profile(&format!(
        r#""readouts": [
            {{"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": "NEEDLE",
             "conversions": {stretches}{extra}}}
        ]"#
    ))
}

/// The inner tank table without its styling, which the UFC cannot draw.
const TANK: &str = r#"[
    {"raw": [0, 12910], "reads": [0, 20]},
    {"raw": [12911, 31785], "reads": [20, 60]},
    {"raw": [31786, 48561], "reads": [60, 100]},
    {"raw": [48562, 56819], "reads": [100, 120]},
    {"raw": [56820, 61603], "reads": [120, 146]},
    {"raw": [61604, 65535], "reads": [146, 160]}
]"#;

fn cautions(p: &Profile) -> Vec<String> {
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    p.field_cautions(&module(), &devices, &displays)
        .into_iter()
        .map(|c| c.1)
        .collect()
}

#[test]
fn a_dial_converted_stretch_by_stretch_is_clean() {
    let p = tank(TANK, "");
    assert!(found(&p).is_empty(), "{:?}", found(&p));
    assert!(cautions(&p).is_empty(), "{:?}", cautions(&p));
}

#[test]
fn counts_no_stretch_claims_are_a_caution_naming_them() {
    // How a half finished table looks, and what the editor offers a stretch
    // for. It still loads: those counts read as the nearest stretch's end.
    let short = TANK.replace(
        r#",
    {"raw": [61604, 65535], "reads": [146, 160]}"#,
        "",
    );
    let p = tank(&short, "");
    assert!(found(&p).is_empty(), "{:?}", found(&p));
    let said = cautions(&p);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("counts 61604 to 65535") && said[0].contains("no conversion"),
        "{said:?}"
    );
}

#[test]
fn stretches_claiming_the_same_counts_are_a_caution() {
    let p = tank(
        r#"[{"raw": [0, 40000], "reads": [0, 50]}, {"raw": [30000, 65535], "reads": [40, 160]}]"#,
        "",
    );
    assert!(found(&p).is_empty(), "{:?}", found(&p));
    let said = cautions(&p);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("both claim some of the same counts"),
        "{said:?}"
    );
}

#[test]
fn a_range_beside_conversions_or_a_backwards_stretch_is_refused() {
    let both = found(&tank(TANK, r#", "reads": [0, 160]"#));
    assert_eq!(both.len(), 1, "{both:?}");
    assert!(both[0].contains("converted two ways"), "{both:?}");

    let backwards = found(&tank(r#"[{"raw": [65535, 0], "reads": [0, 160]}]"#, ""));
    assert_eq!(backwards.len(), 1, "{backwards:?}");
    assert!(
        backwards[0].contains("ends before it starts"),
        "{backwards:?}"
    );
}

#[test]
fn a_stretchs_colour_on_glass_that_draws_none_is_refused() {
    let p = tank(
        r#"[{"raw": [0, 65535], "reads": [0, 160], "colour": "red"}]"#,
        "",
    );
    let found = found(&p);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("colour or size, which only a text grid draws"),
        "{found:?}"
    );
}

#[test]
fn a_bands_colour_on_glass_that_draws_none_is_refused() {
    // The same answer a piece's own colour gets there. The UFC is segments: it
    // has no colours to draw, so a band asking for one is a setting nothing
    // would ever honour.
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33", "source": "NEEDLE",
             "reads": [-1.5, 1.5], "decimals": 1,
             "value_aliases": {"0.1..1.5": {"text": "NU", "colour": "green"}}}
        ]"#,
    );
    let found = found(&p);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("colour or size, which only a text grid draws"),
        "{found:?}"
    );
}

#[test]
fn what_dcs_bios_says_a_signal_is_cautions_rather_than_refuses() {
    // DCS-BIOS marks CHAN as text, so aliases for its values should mean
    // nothing. Its metadata is not right for every module, so the user is
    // told and the profile still loads.
    // A clean field comes first, so the caution has to find its way to the
    // second one rather than landing on whatever is at the top.
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "20-23", "source": "KNOB"},
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-31", "source": "CHAN",
             "value_aliases": {"0": "OFF"}}
        ]"#,
    );
    assert!(found(&p).is_empty(), "{:?}", found(&p));
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    let cautions = p.field_cautions(&module(), &devices, &displays);
    assert_eq!(cautions.len(), 1, "{cautions:?}");
    assert_eq!(cautions[0].0, 1, "on the field it is about");
    assert!(
        cautions[0].1.contains("aliases for its values"),
        "{:?}",
        cautions[0]
    );
}

#[test]
fn a_field_too_narrow_for_its_text_is_cautioned_on_that_field() {
    let p = profile(
        r#""readouts": [
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "20-23", "source": "KNOB"},
            {"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-31", "text": "ABCDE"}
        ]"#,
    );
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    let cautions = p.field_cautions(&module(), &devices, &displays);
    assert_eq!(cautions.len(), 1, "{cautions:?}");
    assert_eq!(cautions[0].0, 1, "on the field it is about");
    assert!(
        cautions[0].1.starts_with("This field needs up to 5 cells"),
        "{:?}",
        cautions[0]
    );
    // And not in the profile's own list, which is for the profile as a whole.
    assert!(p.cautions(&devices).is_empty());
}
