//! A switch: one piece whose reading is decided by another signal.
//!
//! The Huey's ADF needle runs 0 to 65535 whichever band is selected, and the
//! band switch says what that means: 190 to 400 kHz, 400 to 850 or 850 to
//! 1750. These hold the promises that make that work: a case draws only while
//! the selector is in it, a case says only what differs from the switch, a
//! file comes back from a save as it was written, and every check a plain
//! field gets is asked of each case.

use std::path::Path;

use dsc_config::{
    Colour, DeviceInventory, DisplayCatalogue, Module, Profile, Reading, Readout, Span,
};

fn r(p: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

/// BAND is a three way selector, NEEDLE a gauge, CHAN characters.
fn module() -> Module {
    serde_json::from_str(
        r#"{
          "module": "TEST",
          "aircraft": ["TEST"],
          "signals": [
            {
              "id": "BAND",
              "control_type": "selector",
              "outputs": [{"address": 300, "mask": 192, "shift": 6, "max_value": 2, "max_length": null}]
            },
            {
              "id": "NEEDLE",
              "control_type": "analog_gauge",
              "outputs": [{"address": 400, "mask": 65535, "shift": 0, "max_value": 65535, "max_length": null}]
            },
            {
              "id": "CHAN",
              "control_type": "display",
              "outputs": [{"address": 200, "mask": null, "max_value": null, "max_length": 2, "type": "string"}]
            }
          ]
        }"#,
    )
    .expect("the fixture module parses")
}

/// A profile of version 2 on TEST, its fields marked as a page's. TEST has
/// no CDU of its own, so the profile picks the MCDU's font.
fn profile(readouts: &str) -> Profile {
    let mut p: Profile = serde_json::from_str(&format!(
        r#"{{"schema_version": 2, "name": "T", "aircraft": ["TEST"], "module": "TEST",
             "font": "../mcdu/f14bu-font-21x31.json", "readouts": [{readouts}]}}"#
    ))
    .expect("the fixture profile parses");
    for r in &mut p.readouts {
        r.page = Some("fixture".into());
    }
    p
}

/// A field on the MCDU's top row, which takes colours and text.
fn field(content: &str) -> Readout {
    serde_json::from_str(&format!(
        r#"{{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9", "content": [{content}]}}"#
    ))
    .expect("the fixture field parses")
}

/// The ADF as the Huey wants it: one needle, three bands.
const ADF: &str = r#"{"switch": "BAND", "source": "NEEDLE", "colour": "green",
    "cases": {
      "0": {"reads": [190, 400]},
      "1": {"reads": [400, 850]},
      "2": {"reads": [850, 1750]}
    }}"#;

fn refusals(p: &Profile) -> Vec<String> {
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    p.problems(
        &module(),
        &devices,
        &displays,
        &dsc_config::PageLibrary::default(),
    )
    .iter()
    .filter(|e| !e.is_advisory())
    .map(|e| e.to_string())
    .collect()
}

fn cautions(p: &Profile) -> Vec<String> {
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    p.field_cautions(&module(), &devices, &displays)
        .into_iter()
        .map(|(_, text)| text)
        .collect()
}

/// What the field draws with the band at `band` and the needle at `needle`,
/// trimmed, or None while it waits. `band` None is a selector not yet sent.
fn drawn(r: &Readout, band: Option<u16>, needle: u16) -> Option<String> {
    let glyphs = r.compose(|id| match id {
        "BAND" => band.map(|value| Reading::Number { value, max: 2 }),
        "NEEDLE" => Some(Reading::Number {
            value: needle,
            max: 65535,
        }),
        _ => None,
    })?;
    Some(
        glyphs
            .iter()
            .map(|g| g.text.as_str())
            .collect::<String>()
            .trim()
            .to_string(),
    )
}

#[test]
fn the_selector_decides_what_the_needle_reads() {
    let r = field(ADF);
    assert_eq!(drawn(&r, Some(0), 0).as_deref(), Some("190"));
    assert_eq!(drawn(&r, Some(0), 65535).as_deref(), Some("400"));
    assert_eq!(drawn(&r, Some(1), 65535).as_deref(), Some("850"));
    assert_eq!(drawn(&r, Some(2), 65535).as_deref(), Some("1750"));
}

#[test]
fn a_case_takes_what_it_leaves_unset_from_the_switch() {
    let r = field(ADF);
    let glyphs = r
        .compose(|id| match id {
            "BAND" => Some(Reading::Number { value: 1, max: 2 }),
            _ => Some(Reading::Number {
                value: 0,
                max: 65535,
            }),
        })
        .expect("draws");
    assert_eq!(glyphs[0].colour, Some(Colour::Green), "the shared colour");
}

#[test]
fn a_selector_still_to_arrive_leaves_the_cells_alone() {
    // The same as any other signal: writing blanks first would announce an
    // empty field and then fill it in.
    assert_eq!(drawn(&field(ADF), None, 0), None);
}

#[test]
fn a_position_no_case_claims_draws_nothing_but_the_rest_still_draws() {
    let r = field(&format!(r#"{{"text": "ADF "}}, {ADF}"#));
    let only_two = field(
        r#"{"text": "ADF "}, {"switch": "BAND", "source": "NEEDLE",
            "cases": {"0": {"reads": [190, 400]}, "1": {"reads": [400, 850]}}}"#,
    );
    assert_eq!(drawn(&r, Some(2), 0).as_deref(), Some("ADF 850"));
    assert_eq!(drawn(&only_two, Some(2), 0).as_deref(), Some("ADF"));
}

#[test]
fn else_claims_every_position_no_band_does() {
    let r = field(
        r#"{"switch": "BAND", "source": "NEEDLE",
            "cases": {"0": {"reads": [190, 400]}, "else": {"text": "OFF"}}}"#,
    );
    assert_eq!(drawn(&r, Some(0), 0).as_deref(), Some("190"));
    assert_eq!(drawn(&r, Some(2), 0).as_deref(), Some("OFF"));
}

#[test]
fn a_case_can_be_a_chain_and_its_text_takes_only_the_styling() {
    // The unit takes the switch's colour and never its decimals: a typed
    // piece has no number to shape.
    let r = field(
        r#"{"switch": "BAND", "source": "NEEDLE", "decimals": 1, "colour": "amber",
            "cases": {"0": [{"reads": [0, 10]}, {"text": "K"}]}}"#,
    );
    let case = &r.content[0].cases.0[0];
    let unit = &case.pieces[1];
    assert_eq!(unit.colour, Some(Colour::Amber));
    assert_eq!(unit.decimals, 0);
    assert!(unit.source.is_empty(), "a typed piece reads nothing");
    assert_eq!(drawn(&r, Some(0), 65535).as_deref(), Some("10.0K"));
}

#[test]
fn null_clears_a_shared_option_for_one_case() {
    let r = field(
        r#"{"switch": "BAND", "source": "NEEDLE", "reads": [0, 720], "wrap": 360,
            "cases": {"0": {}, "1": {"wrap": null}}}"#,
    );
    assert_eq!(drawn(&r, Some(0), 65535).as_deref(), Some("0"));
    assert_eq!(drawn(&r, Some(1), 65535).as_deref(), Some("720"));
}

#[test]
fn a_case_converting_by_stretches_drops_the_shared_range() {
    // One choice made two ways: inheriting the range beside the stretches
    // would be refused for converting twice.
    let r = field(
        r#"{"switch": "BAND", "source": "NEEDLE", "reads": [0, 100],
            "cases": {"0": {}, "1": {"conversions": [{"raw": [0, 65535], "reads": [5, 6]}]}}}"#,
    );
    let pieces: Vec<&Span> = r.pieces().collect();
    assert_eq!(pieces[0].reads, Some([0.0, 100.0]));
    assert_eq!(pieces[1].reads, None);
    assert_eq!(pieces[1].conversions.len(), 1);
}

#[test]
fn a_saved_switch_comes_back_as_it_was_written() {
    // Cases in the order they match, a one piece case as an object, a chain
    // as an array, and a cleared option still null.
    let written = r#"{"device":"MCDU_Captain","display":"MCDU","cells":"0-9","content":[{"source":"NEEDLE","switch":"BAND","wrap":360.0,"cases":{"0":{"reads":[190.0,400.0]},"1":[{"wrap":null},{"text":"K"}],"else":{"text":"OFF"}}}]}"#;
    let r: Readout = serde_json::from_str(written).expect("parses");
    assert_eq!(serde_json::to_string(&r).expect("writes"), written);
}

#[test]
fn cases_are_held_in_the_order_they_match() {
    let r = field(
        r#"{"switch": "BAND", "source": "NEEDLE",
            "cases": {"else": {"text": "X"}, "2": {}, "0..1": {}}}"#,
    );
    let keys: Vec<String> = r.content[0]
        .cases
        .0
        .iter()
        .map(|c| c.when.to_string())
        .collect();
    assert_eq!(keys, ["0..1", "2", "else"]);
}

#[test]
fn turning_the_selector_repaints_the_field() {
    // The engine repaints a field when something it reads changes, so the
    // selector has to be one of them as well as the needle.
    let r = field(ADF);
    let sources = r.sources();
    assert!(sources.contains(&"BAND"), "{sources:?}");
    assert!(sources.contains(&"NEEDLE"), "{sources:?}");
}

#[test]
fn the_field_is_as_wide_as_its_widest_case() {
    let p = profile(
        r#"{"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-32",
            "content": [{"switch": "BAND", "source": "NEEDLE",
              "cases": {"0": {"reads": [190, 400]}, "2": {"reads": [850, 1750]}}}]}"#,
    );
    let width = p.readouts[0].width(&module());
    assert_eq!(width.widest, 4, "1750, not 400 and 1750 added up");
    assert!(cautions(&p)
        .iter()
        .any(|c| c.contains("needs up to 4 cells")));
}

#[test]
fn the_adf_loads_clean() {
    let p = profile(&format!(
        r#"{{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9", "content": [{ADF}]}}"#
    ));
    assert!(refusals(&p).is_empty(), "{:?}", refusals(&p));
    assert!(cautions(&p).is_empty(), "{:?}", cautions(&p));
}

#[test]
fn a_switch_with_no_cases_is_refused() {
    let p = profile(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
            "content": [{"switch": "BAND", "source": "NEEDLE"}]}"#,
    );
    let found = refusals(&p);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("has no cases"), "{found:?}");
}

#[test]
fn cases_with_no_switch_are_refused() {
    let p = profile(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
            "content": [{"source": "NEEDLE", "cases": {"0": {}}}]}"#,
    );
    assert!(refusals(&p)
        .iter()
        .any(|e| e.contains("no switch to choose")));
}

#[test]
fn a_switch_inside_a_case_is_refused() {
    let p = profile(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
            "content": [{"switch": "BAND", "source": "NEEDLE",
              "cases": {"0": {"switch": "BAND"}}}]}"#,
    );
    assert!(refusals(&p)
        .iter()
        .any(|e| e.contains("cannot sit inside another")));
}

#[test]
fn a_switch_drawing_its_own_text_is_refused() {
    let p = profile(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
            "content": [{"switch": "BAND", "text": "ADF", "cases": {"0": {"text": "A"}}}]}"#,
    );
    assert!(refusals(&p)
        .iter()
        .any(|e| e.contains("has characters to draw")));
}

#[test]
fn an_unfinished_case_is_refused_like_an_unfinished_piece() {
    // No source on the switch and none in the case: a piece with nothing in it.
    let p = profile(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
            "content": [{"switch": "BAND", "cases": {"0": {"reads": [0, 1]}}}]}"#,
    );
    assert!(refusals(&p).iter().any(|e| e.contains("nothing in it")));
}

#[test]
fn a_fault_every_case_shares_is_said_once() {
    // A colour on the UFC's segments, which draw none, inherited by both.
    let p = profile(
        r#"{"device": "CarrierAce_UFC", "display": "UFC1", "cells": "30-33",
            "content": [{"switch": "BAND", "source": "NEEDLE", "colour": "red",
              "cases": {"0": {"reads": [190, 400]}, "1": {"reads": [400, 850]}}}]}"#,
    );
    let found = refusals(&p);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("only a text grid draws"), "{found:?}");
}

#[test]
fn positions_are_cautioned_not_refused() {
    let p = profile(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
            "content": [{"switch": "BAND", "source": "NEEDLE",
              "cases": {"0..1": {}, "1": {}, "5": {}}}]}"#,
    );
    assert!(refusals(&p).is_empty(), "{:?}", refusals(&p));
    let said = cautions(&p);
    assert!(
        said.iter()
            .any(|c| c.contains("both claim the same position")),
        "{said:?}"
    );
    assert!(
        said.iter()
            .any(|c| c.contains("outside the positions it sends, 0 to 2")),
        "{said:?}"
    );
    assert!(
        said.iter()
            .any(|c| c.contains("positions 2 of \"BAND\" have no case")),
        "{said:?}"
    );
}

#[test]
fn else_leaves_no_position_uncovered() {
    let p = profile(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
            "content": [{"switch": "BAND", "source": "NEEDLE",
              "cases": {"0": {}, "else": {"text": "OFF"}}}]}"#,
    );
    assert!(cautions(&p).is_empty(), "{:?}", cautions(&p));
}

#[test]
fn a_selector_that_reports_characters_is_cautioned() {
    let p = profile(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
            "content": [{"switch": "CHAN", "source": "NEEDLE", "cases": {"0": {}}}]}"#,
    );
    assert!(cautions(&p)
        .iter()
        .any(|c| c.contains("no position for a switch")));
}
