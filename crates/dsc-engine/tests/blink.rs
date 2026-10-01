//! Lamps that flash while their conditions hold.
//!
//! Nothing here names an aircraft: the module is a fixture with two
//! switches, and the clock is the one `ingest` and `tick` are handed.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dsc_bios::Write;
use dsc_config::{Catalogue, DeviceInventory, Module, Profile};
use dsc_engine::{Batch, Engine, ACFT_NAME_LEN};

const PTO2: &str = "TAKEOFF_PLANEL_2";
const MASTER_CAUTION: u8 = 4;
const HOOK: u8 = 17;
const WARN: u16 = 200;
const TEST: u16 = 202;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn catalogue() -> Catalogue {
    let json = format!(
        r#"{{
            "module": "TEST_BLINK",
            "aircraft": ["TEST_BLINK"],
            "signals": [
                {{ "id": "WARN", "control_type": "led",
                   "outputs": [{{ "address": {WARN}, "mask": 1, "shift": 0, "max_value": 1 }}] }},
                {{ "id": "LAMP_TEST", "control_type": "selector",
                   "outputs": [{{ "address": {TEST}, "mask": 1, "shift": 0, "max_value": 1 }}] }}
            ]
        }}"#
    );
    Catalogue::from_modules(vec![
        serde_json::from_str::<Module>(&json).expect("fixture parses")
    ])
}

/// The master caution flashes fast on the warning, and the hook lamp is
/// steady on the lamp test and flashes slow on the warning.
fn profile() -> Profile {
    serde_json::from_str(
        r#"{
            "name": "Blink",
            "aircraft": ["TEST_BLINK"],
            "module": "TEST_BLINK",
            "bindings": [
                {
                    "device": "TAKEOFF_PLANEL_2",
                    "led": "Master_Caution",
                    "conditions": [ { "source": "WARN", "on_when": { "equals": 1 } } ],
                    "blink": "fast"
                },
                {
                    "device": "TAKEOFF_PLANEL_2",
                    "led": "HOOK",
                    "any_of": [
                        { "conditions": [ { "source": "LAMP_TEST", "on_when": { "equals": 1 } } ] },
                        { "conditions": [ { "source": "WARN", "on_when": { "equals": 1 } } ],
                          "blink": "slow" }
                    ]
                }
            ]
        }"#,
    )
    .expect("fixture profile parses")
}

fn acft_name(name: &str) -> Vec<Write> {
    let mut bytes = name.as_bytes().to_vec();
    bytes.resize(ACFT_NAME_LEN as usize, 0);
    (0..ACFT_NAME_LEN / 2)
        .map(|i| {
            let b = i as usize * 2;
            Write {
                address: i * 2,
                value: u16::from_le_bytes([bytes[b], bytes[b + 1]]),
            }
        })
        .collect()
}

fn w(address: u16, value: u16) -> Write {
    Write { address, value }
}

struct Pit {
    engine: Engine,
    /// When the flash clock started: the first datagram.
    t0: Instant,
    caution: Option<u8>,
    hook: Option<u8>,
}

impl Pit {
    /// Loaded and settled at 1 s on the flash clock, warning off.
    fn loaded() -> Self {
        let devices = DeviceInventory::load(&root().join("data/devices.json")).expect("devices");
        let mut engine = Engine::new(devices, catalogue(), vec![profile()]);
        engine.set_connected(vec![PTO2.to_string()]);
        let t0 = Instant::now();
        let mut pit = Pit {
            engine,
            t0,
            caution: None,
            hook: None,
        };
        let batch = pit.engine.ingest(&acft_name("TEST_BLINK"), t0);
        pit.take(&batch);
        let batch = pit.engine.ingest(&[w(WARN, 0), w(TEST, 0)], t0);
        pit.take(&batch);
        pit.tick(1000);
        pit
    }

    fn at(&self, ms: u64) -> Instant {
        self.t0 + Duration::from_millis(ms)
    }

    fn feed(&mut self, ms: u64, writes: &[Write]) {
        let batch = self.engine.ingest(writes, self.at(ms));
        self.take(&batch);
    }

    fn tick(&mut self, ms: u64) {
        let batch = self.engine.tick(self.at(ms));
        self.take(&batch);
    }

    fn take(&mut self, batch: &Batch) {
        for write in &batch.writes {
            if write.id.device != PTO2 {
                continue;
            }
            match write.id.index {
                MASTER_CAUTION => self.caution = Some(write.value),
                HOOK => self.hook = Some(write.value),
                _ => {}
            }
        }
    }
}

#[test]
fn a_fast_flash_is_lit_and_dark_for_a_sixth_of_a_second_each() {
    let mut pit = Pit::loaded();
    assert_eq!(pit.caution, Some(0));
    // Into the lit half of a flash that starts on the second, past the
    // frame the sweep sent.
    pit.feed(1045, &[w(WARN, 1)]);
    assert_eq!(pit.caution, Some(1));
    pit.tick(1160);
    assert_eq!(pit.caution, Some(1), "still lit before a sixth of a second");
    pit.tick(1170);
    assert_eq!(pit.caution, Some(0), "dark for the second sixth");
    pit.tick(1330);
    assert_eq!(pit.caution, Some(0));
    pit.tick(1340);
    assert_eq!(pit.caution, Some(1), "lit again for the next flash");
}

#[test]
fn a_flash_turns_on_time_even_inside_the_frame_cap() {
    let mut pit = Pit::loaded();
    pit.feed(1045, &[w(WARN, 1)]);
    // A datagram 10 ms before the turn sends a frame, and the turn comes
    // inside the next 40. The flash goes anyway.
    pit.feed(1157, &[w(TEST, 1)]);
    pit.tick(1167);
    assert_eq!(pit.caution, Some(0));
}

#[test]
fn the_engine_says_when_the_next_flash_turns() {
    let mut pit = Pit::loaded();
    pit.feed(1045, &[w(WARN, 1)]);
    // Fast turns at 1166.67 ms, before slow's 1250.
    let wait = pit
        .engine
        .next_beat(pit.at(1100))
        .expect("something flashes");
    assert!(
        wait > Duration::from_millis(66) && wait <= Duration::from_millis(67),
        "{wait:?}"
    );
}

#[test]
fn a_steady_alternative_wins_over_a_flashing_one() {
    let mut pit = Pit::loaded();
    pit.feed(1045, &[w(WARN, 1)]);
    assert_eq!(pit.hook, Some(1));
    pit.tick(1250);
    assert_eq!(pit.hook, Some(0), "slow flash dark from a quarter second");
    // The lamp test holds the lamp steady over the flash.
    pit.feed(1300, &[w(TEST, 1)]);
    assert_eq!(pit.hook, Some(1));
    pit.tick(1750);
    assert_eq!(pit.hook, Some(1));
}

#[test]
fn a_lamp_that_stops_holding_stops_flashing() {
    let mut pit = Pit::loaded();
    pit.feed(1045, &[w(WARN, 1)]);
    pit.feed(1100, &[w(WARN, 0)]);
    assert_eq!(pit.caution, Some(0));
    pit.tick(1340);
    assert_eq!(pit.caution, Some(0));
}

#[test]
fn nothing_flashing_means_no_beat_to_wait_for() {
    let mut profile = profile();
    for b in &mut profile.bindings {
        b.blink = dsc_config::Blink::Steady;
        for branch in &mut b.any_of {
            branch.blink = dsc_config::Blink::Steady;
        }
    }
    let devices = DeviceInventory::load(&root().join("data/devices.json")).expect("devices");
    let mut engine = Engine::new(devices, catalogue(), vec![profile]);
    engine.set_connected(vec![PTO2.to_string()]);
    let t0 = Instant::now();
    engine.ingest(&acft_name("TEST_BLINK"), t0);
    engine.tick(t0 + Duration::from_secs(1));
    assert_eq!(engine.next_beat(t0 + Duration::from_secs(1)), None);
}
