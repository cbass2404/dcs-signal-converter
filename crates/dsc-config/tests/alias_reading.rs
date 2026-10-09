//! A band that shows the reading: the number as it would draw anyway, styled.
//!
//! A band is how a reading is singled out, and sometimes the point is not a
//! word in its place but the same number in red: a needle past 120 is still
//! the number, and it is the colour that says something. `"reading": true`
//! says so outright, because a blank `text` already means draw nothing.

use std::path::Path;

use dsc_config::{Colour, DeviceInventory, DisplayCatalogue, Module, Profile, Reading, Readout};

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
              "id": "NEEDLE",
              "control_type": "analog_gauge",
              "outputs": [{"address": 400, "mask": 65535, "shift": 0, "max_value": 65535, "max_length": null}]
            }
          ]
        }"#,
    )
    .expect("the fixture module parses")
}

/// A needle read 0 to 200 on the MCDU's top row, with these bands.
fn field(aliases: &str) -> Readout {
    serde_json::from_str(&format!(
        r#"{{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
             "source": "NEEDLE", "reads": [0, 200], "value_aliases": {{{aliases}}}}}"#
    ))
    .expect("the fixture field parses")
}

fn profile(aliases: &str) -> Profile {
    let mut p: Profile = serde_json::from_str(&format!(
        r#"{{"schema_version": 2, "name": "T", "aircraft": ["TEST"], "module": "TEST",
             "font": "../mcdu/f14bu-font-21x31.json",
             "readouts": [{{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9",
               "source": "NEEDLE", "reads": [0, 200], "value_aliases": {{{aliases}}}}}]}}"#
    ))
    .expect("the fixture profile parses");
    for r in &mut p.readouts {
        r.page = Some("fixture".into());
    }
    p
}

fn refusals(p: &Profile) -> Vec<String> {
    let devices = DeviceInventory::load_dir(&r("data/devices")).expect("devices");
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

/// The first glyphs drawn with the needle at `raw`: the text and its colour.
fn drawn(r: &Readout, raw: u16) -> (String, Option<Colour>) {
    let glyphs = r
        .compose(|_| {
            Some(Reading::Number {
                value: raw,
                max: 65535,
            })
        })
        .expect("draws");
    let text: String = glyphs.iter().map(|g| g.text.as_str()).collect();
    (text.trim().to_string(), glyphs[0].colour)
}

#[test]
fn the_number_draws_in_the_bands_colour() {
    let r = field(r#""120..200": {"reading": true, "colour": "red"}"#);
    assert_eq!(drawn(&r, 65535), ("200".to_string(), Some(Colour::Red)));
    assert_eq!(drawn(&r, 0), ("0".to_string(), None), "outside the band");
}

#[test]
fn a_blank_band_still_draws_nothing() {
    // The spelling every shipped page relies on, untouched.
    let r = field(r#""0": """#);
    assert_eq!(drawn(&r, 0).0, "");
}

#[test]
fn the_sign_still_goes_for_abs() {
    let r: Readout = serde_json::from_str(
        r#"{"device": "MCDU_Captain", "display": "MCDU", "cells": "0-9", "source": "NEEDLE",
            "reads": [-100, 100], "abs": true,
            "value_aliases": {"-100..-50": {"reading": true, "colour": "amber"}}}"#,
    )
    .expect("parses");
    assert_eq!(drawn(&r, 0), ("100".to_string(), Some(Colour::Amber)));
}

#[test]
fn it_is_written_back_without_text() {
    let written = r#"{"device":"MCDU_Captain","display":"MCDU","cells":"0-9","source":"NEEDLE","reads":[0.0,200.0],"value_aliases":{"0":"","120..200":{"colour":"red","reading":true}}}"#;
    let r: Readout = serde_json::from_str(written).expect("parses");
    assert_eq!(serde_json::to_string(&r).expect("writes"), written);
}

#[test]
fn bands_showing_the_reading_hide_no_number() {
    // Every reading banded, but one band shows the number, so the width is
    // the number's and not the longest word's.
    let p = profile(r#""0..119": "LOW", "120..200": {"reading": true, "colour": "red"}"#);
    assert_eq!(p.readouts[0].width(&module()).widest, 3);
}

#[test]
fn showing_the_reading_and_text_is_refused() {
    let p = profile(r#""120..200": {"text": "HI", "reading": true, "colour": "red"}"#);
    let found = refusals(&p);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("it can do one"), "{found:?}");
}

#[test]
fn showing_the_reading_with_no_style_is_refused() {
    let p = profile(r#""120..200": {"reading": true}"#);
    let found = refusals(&p);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("no colour, inverse or small font"),
        "{found:?}"
    );
}

#[test]
fn a_styled_band_showing_the_reading_loads_clean() {
    let p = profile(r#""120..200": {"reading": true, "colour": "red"}"#);
    assert!(refusals(&p).is_empty(), "{:?}", refusals(&p));
}

#[test]
fn a_band_can_draw_small() {
    // Either asking is enough, the way a conversion row's small is.
    let r = field(
        r#""0..119": {"text": "LOW", "small": true}, "120..200": {"reading": true, "small": true}"#,
    );
    let at = |raw: u16| {
        r.compose(|_| {
            Some(Reading::Number {
                value: raw,
                max: 65535,
            })
        })
        .expect("draws")[0]
            .small
    };
    assert!(at(0), "a word drawn small");
    assert!(at(65535), "the reading drawn small");
    let written = serde_json::to_string(&r).expect("writes");
    assert!(written.contains(r#""small":true"#), "{written}");
}
