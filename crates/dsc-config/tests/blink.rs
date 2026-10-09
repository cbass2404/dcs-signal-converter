//! Flashing lamps: the beat every lamp shares, and where a flash is set.
//!
//! A flash is set per block of conditions: a lamp's own, each alternative,
//! and each block of a stored signal of lamp conditions, which is the one
//! source of truth for every lamp lit by it. Absent is steady, and steady is
//! never written, so no profile changes until someone sets a flash.

use std::path::{Path, PathBuf};
use std::time::Duration;

use dsc_config::{Beat, Binding, Blink, DeviceInventory, PageLibrary, Profile, StoredSignal};

fn r(p: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn devices() -> DeviceInventory {
    DeviceInventory::load_dir(&r("data/devices")).expect("devices")
}

/// How long each stretch of lit or dark lasts across `ms`, at one rate.
fn runs(ms: u64, lit: impl Fn(Beat) -> bool) -> Vec<u64> {
    let mut out = Vec::new();
    let mut run = 0;
    let mut was = true;
    for t in 0..ms {
        let now = lit(Beat::at(Duration::from_millis(t)));
        if now != was {
            out.push(run);
            run = 0;
            was = now;
        }
        run += 1;
    }
    out
}

#[test]
fn fast_is_three_flashes_a_second_lit_and_dark_evenly() {
    // A third of a second does not divide into milliseconds, so each half is
    // 166 or 167, never drifting further than that.
    let halves = runs(1000, |b| b.fast);
    assert_eq!(halves.len(), 5, "{halves:?}");
    assert!(halves.iter().all(|h| *h == 166 || *h == 167), "{halves:?}");
}

#[test]
fn slow_is_two_flashes_a_second_lit_and_dark_evenly() {
    assert_eq!(runs(1000, |b| b.slow), vec![250, 250, 250]);
}

#[test]
fn the_next_turn_is_whichever_rate_turns_first() {
    let fast = Beat::next_turn(Duration::ZERO);
    assert!(
        fast > Duration::from_millis(166) && fast <= Duration::from_millis(167),
        "{fast:?}"
    );
    // From 200 ms, slow turns at 250 before fast at 333.
    assert_eq!(
        Beat::next_turn(Duration::from_millis(200)),
        Duration::from_millis(50)
    );
}

#[test]
fn steady_is_never_written() {
    let row = r#"{"device": "D", "led": "L",
                  "any_of": [{"conditions": [{"source": "A", "on_when": {"equals": 1}}]},
                             {"conditions": [{"source": "B", "on_when": {"equals": 1}}], "blink": "fast"}]}"#;
    let b: Binding = serde_json::from_str(row).unwrap();
    assert_eq!(b.blink, Blink::Steady);
    assert_eq!(b.any_of[0].blink, Blink::Steady);
    assert_eq!(b.any_of[1].blink, Blink::Fast);
    let written = serde_json::to_string(&b).unwrap();
    assert_eq!(written.matches("\"blink\"").count(), 1, "{written}");
    assert!(written.contains(r#""blink":"fast""#), "{written}");
}

#[test]
fn a_lamp_lit_by_a_stored_signal_flashes_as_the_signal_says() {
    let signal: StoredSignal = serde_json::from_str(
        r#"{"id": "mc0001", "name": "Master caution", "blink": "slow",
            "conditions": [{"source": "MC", "on_when": {"equals": 1}}]}"#,
    )
    .unwrap();
    // The row's own blink is the one a lamp of its own conditions would use,
    // and lighting by the signal sets it aside.
    let p: Profile = serde_json::from_str(
        r#"{"schema_version": 2, "name": "T", "aircraft": ["TEST"], "module": "TEST",
            "bindings": [{"device": "TAKEOFF_PLANEL_2", "led": "HOOK", "signal": "mc0001", "blink": "fast"}]}"#,
    )
    .unwrap();
    let p = p.with_pages(&PageLibrary::with_signals("TEST", Vec::new(), vec![signal]));
    let devices = devices();
    let b = &p.bindings[0];
    let (_, led) = devices.device(&b.device).unwrap().led(&b.led).unwrap();
    let at = |slow, fast| p.resolve_binding_at(b, led, |_| Some(1), |_| None, Beat { slow, fast });

    assert!(p.blinks(b));
    assert_eq!(at(true, false), Some(1), "slow lit, fast dark: lit");
    assert_eq!(at(false, true), Some(0), "slow dark: dark");
}

#[test]
fn a_check_reads_a_flashing_lamp_as_lit() {
    let b: Binding = serde_json::from_str(
        r#"{"device": "TAKEOFF_PLANEL_2", "led": "HOOK", "blink": "fast",
            "conditions": [{"source": "MC", "on_when": {"equals": 1}}]}"#,
    )
    .unwrap();
    let devices = devices();
    let (_, led) = devices.device(&b.device).unwrap().led(&b.led).unwrap();
    assert_eq!(b.resolve(led, |_| Some(1)), Some(1));
}
