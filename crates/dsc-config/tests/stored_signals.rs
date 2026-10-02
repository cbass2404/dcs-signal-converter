//! Stored signals: a number worked out once, named, and drawn by any page.
//!
//! The F-16's fuel totalizer is three drums, each its own needle, and the
//! reading is the three laid side by side. These hold the promises that make
//! that work: each term settles on its own before it is laid down, a field
//! draws the joined characters and only styles them, nothing draws until
//! every term has arrived, a paint works each signal out once however many
//! fields draw it, and sharing and updates carry the signals with the pages.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use dsc_config::bundle;
use dsc_config::{
    Colour, DeviceInventory, DisplayCatalogue, Module, Page, PageFile, PageLibrary, Pages, Profile,
    Reading, Readout, StoredCache, StoredSignal,
};

fn r(p: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

/// Three drums and a line of characters.
fn module() -> Module {
    serde_json::from_str(
        r#"{
          "module": "TEST",
          "aircraft": ["TEST"],
          "signals": [
            {"id": "D10K", "control_type": "analog_gauge",
             "outputs": [{"address": 400, "mask": 65535, "shift": 0, "max_value": 65535, "max_length": null}]},
            {"id": "D1K", "control_type": "analog_gauge",
             "outputs": [{"address": 402, "mask": 65535, "shift": 0, "max_value": 65535, "max_length": null}]},
            {"id": "D100", "control_type": "analog_gauge",
             "outputs": [{"address": 404, "mask": 65535, "shift": 0, "max_value": 65535, "max_length": null}]},
            {"id": "CHAN", "control_type": "display",
             "outputs": [{"address": 200, "mask": null, "max_value": null, "max_length": 2, "type": "string"}]}
          ]
        }"#,
    )
    .expect("the fixture module parses")
}

/// One drum digit: 0 to 10 across the needle, rounded down, wrapped at 10.
fn drum(source: &str) -> String {
    format!(
        r#"{{"source": "{source}", "reads": [0, 10], "round": "down", "wrap": 10, "digits": 1, "width": 1}}"#
    )
}

/// The totalizer as the F-16 wants it: three drums side by side.
fn fuel() -> StoredSignal {
    serde_json::from_str(&format!(
        r#"{{"id": "fuel01", "name": "Fuel", "note": "Totalizer, hundreds of pounds",
             "terms": [{}, {}, {}]}}"#,
        drum("D10K"),
        drum("D1K"),
        drum("D100")
    ))
    .expect("the fixture signal parses")
}

/// A page file on TEST holding `signals` and one page of `fields`, loaded
/// the way the library loads one, so every piece has its signal attached.
fn page_file(fields: &str, signals: &[StoredSignal]) -> PageFile {
    let text = format!(
        r#"{{"module": "TEST",
             "pages": [{{"id": "page01", "name": "Fuel", "display": "MCDU", "fields": [{fields}]}}],
             "signals": {}}}"#,
        serde_json::to_string(signals).unwrap()
    );
    let dir = scratch("page_file");
    let path = dir.join("test.json");
    std::fs::write(&path, text).unwrap();
    PageFile::load(&path).expect("the fixture page file loads")
}

/// The page's fields as a screen gets them, on the Captain's MCDU.
fn fields(file: &PageFile) -> Vec<Readout> {
    file.pages[0]
        .fields
        .iter()
        .cloned()
        .map(|mut f| {
            f.device = "MCDU_Captain".into();
            f.page = Some("page01".into());
            f
        })
        .collect()
}

/// A folder of its own under the system's temporary folder, empty. Every
/// call gets another, since the tests run side by side.
fn scratch(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "dsc-stored-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The count that puts a 0 to 10 drum at `reading`.
fn at(reading: f64) -> u16 {
    (reading / 10.0 * 65535.0).round() as u16
}

/// What the field draws with each drum at the reading given, trimmed, or
/// None while it waits. A drum given None has not arrived.
fn drawn(field: &Readout, drums: [Option<f64>; 3]) -> Option<String> {
    let glyphs = field.compose(|id| {
        let i = ["D10K", "D1K", "D100"].iter().position(|d| *d == id)?;
        drums[i].map(|d| Reading::Number {
            value: at(d),
            max: 65535,
        })
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

fn profile(readouts: Vec<Readout>) -> Profile {
    let mut p: Profile = serde_json::from_str(
        r#"{"schema_version": 2, "name": "T", "aircraft": ["TEST"], "module": "TEST",
            "font": "../mcdu/f14bu-font-21x31.json"}"#,
    )
    .unwrap();
    p.readouts = readouts;
    p
}

fn refusals(p: &Profile) -> Vec<String> {
    let devices = DeviceInventory::load(&r("data/devices.json")).expect("devices");
    let displays = DisplayCatalogue::load_dir(&r("data/displays")).expect("displays");
    p.problems(&module(), &devices, &displays, &PageLibrary::default())
        .iter()
        .map(|e| e.to_string())
        .collect()
}

const FUEL_FIELD: &str = r#"{"cells": "0-5", "signal": "fuel01"}"#;

#[test]
fn drums_are_laid_side_by_side_each_settled_on_its_own() {
    let file = page_file(FUEL_FIELD, &[fuel()]);
    let field = &fields(&file)[0];
    // The 10K drum is a third of the way to its 2: rolling, it still reads 1.
    assert_eq!(
        drawn(field, [Some(1.35), Some(2.9), Some(3.0)]).as_deref(),
        Some("123")
    );
    // Wrapped: a drum that has gone all the way round reads 0.
    assert_eq!(
        drawn(field, [Some(0.0), Some(10.0), Some(0.0)]).as_deref(),
        Some("000")
    );
}

#[test]
fn nothing_draws_until_every_term_has_arrived() {
    let file = page_file(FUEL_FIELD, &[fuel()]);
    let field = &fields(&file)[0];
    assert_eq!(drawn(field, [Some(1.0), None, Some(3.0)]), None);
}

#[test]
fn a_label_beside_it_draws_while_it_waits() {
    let file = page_file(
        r#"{"cells": "0-9", "content": [{"text": "FUEL "}, {"signal": "fuel01"}]}"#,
        &[fuel()],
    );
    let field = &fields(&file)[0];
    assert_eq!(drawn(field, [None, None, None]).as_deref(), Some("FUEL"));
    assert_eq!(
        drawn(field, [Some(0.0), Some(4.0), Some(5.0)]).as_deref(),
        Some("FUEL 045")
    );
}

#[test]
fn the_field_styles_the_number_by_its_bands() {
    let file = page_file(
        r#"{"cells": "0-5", "signal": "fuel01",
            "value_aliases": {"0..9": {"reading": true, "colour": "red"}, "999": "FULL"}}"#,
        &[fuel()],
    );
    let field = &fields(&file)[0];
    let read = |drums: [f64; 3]| {
        field
            .compose(|id| {
                let i = ["D10K", "D1K", "D100"].iter().position(|d| *d == id)?;
                Some(Reading::Number {
                    value: at(drums[i]),
                    max: 65535,
                })
            })
            .unwrap()
    };
    let low = read([0.0, 0.0, 5.0]);
    assert_eq!(low[0].text, "0");
    assert_eq!(low[2].colour, Some(Colour::Red));
    let full = read([9.0, 9.0, 9.0]);
    let text: String = full.iter().map(|g| g.text.as_str()).collect();
    assert_eq!(text.trim(), "FULL");
    let middle = read([1.0, 2.0, 3.0]);
    assert_eq!(middle[0].colour, None);
}

#[test]
fn rounding_up_shows_the_next_step_once_it_has_started() {
    let up: StoredSignal = serde_json::from_str(
        r#"{"id": "up0001", "name": "Up",
            "terms": [{"source": "D100", "reads": [0, 10], "round": "up"}]}"#,
    )
    .unwrap();
    let file = page_file(r#"{"cells": "0-5", "signal": "up0001"}"#, &[up]);
    let field = &fields(&file)[0];
    assert_eq!(drawn(field, [None, None, Some(3.2)]).as_deref(), Some("4"));
    assert_eq!(drawn(field, [None, None, Some(3.0)]).as_deref(), Some("3"));
}

#[test]
fn a_paint_works_each_signal_out_once() {
    let file = page_file(
        &format!(
            "{FUEL_FIELD}, {}",
            r#"{"cells": "10-15", "signal": "fuel01"}"#
        ),
        &[fuel()],
    );
    let both = fields(&file);
    let read = |id: &str| {
        ["D10K", "D1K", "D100"]
            .contains(&id)
            .then_some(Reading::Number {
                value: 0,
                max: 65535,
            })
    };
    // Each field looks the drums up to see whether they moved; the drums are
    // shaped, and told to `seen`, once.
    let shaped = Cell::new(0);
    let cache = StoredCache::default();
    for field in &both {
        field
            .compose_cached(read, &cache, |_, _, _| shaped.set(shaped.get() + 1))
            .unwrap();
    }
    assert_eq!(shaped.get(), 3, "three drums, shaped once for both fields");
}

#[test]
fn a_field_reads_every_signal_its_stored_signal_does() {
    let file = page_file(FUEL_FIELD, &[fuel()]);
    assert_eq!(fields(&file)[0].sources(), vec!["D10K", "D1K", "D100"]);
}

#[test]
fn the_page_file_comes_back_from_a_save_as_it_was_written() {
    let file = page_file(FUEL_FIELD, &[fuel()]);
    let dir = scratch("round_trip");
    let path = dir.join("test.json");
    file.save(&path).unwrap();
    let once = std::fs::read_to_string(&path).unwrap();
    PageFile::load(&path).unwrap().save(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), once);
    // One piece is written flat, its signal beside the cells.
    assert!(once.contains(r#""signal": "fuel01""#));
    assert!(once.contains(r#""note": "Totalizer, hundreds of pounds""#));
}

#[test]
fn a_signal_the_page_file_lacks_is_refused() {
    let file = page_file(r#"{"cells": "0-5", "signal": "gone01"}"#, &[fuel()]);
    let found = refusals(&profile(fields(&file)));
    assert!(
        found.iter().any(|e| e.contains("gone01")),
        "refused for the missing signal: {found:?}"
    );
}

#[test]
fn the_piece_drawing_it_cannot_shape_the_number_again() {
    let file = page_file(
        r#"{"cells": "0-5", "signal": "fuel01", "decimals": 1}"#,
        &[fuel()],
    );
    let found = refusals(&profile(fields(&file)));
    assert!(
        found.iter().any(|e| e.contains("shapes its own")),
        "refused for shaping twice: {found:?}"
    );
}

#[test]
fn a_signal_is_held_to_what_a_reading_is() {
    let mut bad = fuel();
    bad.terms[1].0.source.clear();
    bad.terms[0].0.conversions = vec![dsc_config::Conversion {
        raw: [0, 65535],
        reads: [0.0, 10.0],
        colour: Some(Colour::Red),
        small: false,
    }];
    bad.terms[0].0.reads = None;
    let found: Vec<String> = bad
        .problems(&module())
        .iter()
        .map(|e| e.to_string())
        .collect();
    assert!(
        found.iter().any(|e| e.contains("no signal chosen")),
        "{found:?}"
    );
    assert!(
        found.iter().any(|e| e.contains("colour or size")),
        "{found:?}"
    );
}

/// A radio frequency: whole megahertz, a typed point, then hundredths kept
/// at two digits so 5 reads 05.
fn radio() -> StoredSignal {
    serde_json::from_str(&format!(
        r#"{{"id": "radio1", "name": "Radio",
             "terms": [{}, {}, {{"text": "."}},
                       {{"source": "D100", "reads": [0, 100], "round": "down", "digits": 2}}]}}"#,
        drum("D10K"),
        drum("D1K"),
    ))
    .expect("the fixture signal parses")
}

#[test]
fn a_symbol_part_is_laid_down_between_the_readings() {
    let file = page_file(r#"{"cells": "0-5", "signal": "radio1"}"#, &[radio()]);
    let field = &fields(&file)[0];
    assert_eq!(
        drawn(field, [Some(3.0), Some(0.0), Some(5.0)]).as_deref(),
        Some("30.50")
    );
    assert_eq!(
        drawn(field, [Some(3.0), Some(0.0), Some(0.5)]).as_deref(),
        Some("30.05")
    );
    // It reads nothing, so nothing waits on it.
    assert_eq!(fields(&file)[0].sources(), vec!["D10K", "D1K", "D100"]);
}

#[test]
fn a_symbol_part_keeps_the_result_a_number() {
    let file = page_file(
        r#"{"cells": "0-5", "signal": "radio1", "value_aliases": {"30.5": "GUARD"}}"#,
        &[radio()],
    );
    let field = &fields(&file)[0];
    assert_eq!(
        drawn(field, [Some(3.0), Some(0.0), Some(5.0)]).as_deref(),
        Some("GUARD")
    );
    assert_eq!(radio().tolerance(), 0.005);
    // Hundredths read 0 to 100, so the widest is three digits after the point.
    assert_eq!(radio().widest(&module()), Some(6));
    assert!(radio().problems(&module()).is_empty());
}

#[test]
fn a_symbol_part_takes_only_what_a_number_is_written_with() {
    let mut bad = radio();
    bad.terms[2].0.text = "MHZ".into();
    let found: Vec<String> = bad
        .problems(&module())
        .iter()
        .map(|e| e.to_string())
        .collect();
    assert!(found.iter().any(|e| e.contains("symbol part")), "{found:?}");
}

#[test]
fn a_name_is_unique_on_its_module() {
    let mut other = fuel();
    other.id = "fuel02".into();
    other.name = " fuel ".into();
    let lib = PageLibrary::with_signals("TEST", Vec::new(), vec![fuel()]);
    let found: Vec<String> = lib
        .signal_problems(&other, &module())
        .iter()
        .map(|e| e.to_string())
        .collect();
    assert!(
        found.iter().any(|e| e.contains("already go by")),
        "{found:?}"
    );
}

#[test]
fn a_shared_page_brings_the_signal_it_draws() {
    let file = page_file(FUEL_FIELD, &[fuel()]);
    let lib = PageLibrary::with_signals("TEST", file.pages.clone(), file.signals.clone());
    let p = profile(Vec::new());
    let shared = bundle::Bundle::of(&p, &lib, &["page01".to_string()]);
    assert_eq!(shared.signals.len(), 1);

    // Here a different signal already has its id, so it comes in under a new
    // one, renamed, and the page follows it.
    let mut taken = fuel();
    taken.terms.truncate(1);
    let here = PageLibrary::with_signals("TEST", Vec::new(), vec![taken]);
    let mut pages = shared.pages.clone();
    let added = bundle::bring_in_signals(&here, "TEST", &shared.signals, &mut pages);
    assert_eq!(added.len(), 1);
    assert_ne!(added[0].id, "fuel01");
    assert_eq!(added[0].name, "Fuel 2");
    assert_eq!(
        bundle::signals_named(&pages)
            .into_iter()
            .collect::<Vec<_>>(),
        vec![added[0].id.clone()]
    );

    // The same signal already here is used as it is.
    let same = PageLibrary::with_signals("TEST", Vec::new(), vec![fuel()]);
    let mut pages = shared.pages.clone();
    assert!(bundle::bring_in_signals(&same, "TEST", &shared.signals, &mut pages).is_empty());
}

/// A shipped signal, the snapshot of it, and the library's copy, through one
/// update.
fn update(
    shipped: &[StoredSignal],
    was: &[StoredSignal],
    mine: &[StoredSignal],
) -> Vec<StoredSignal> {
    let dir = scratch("update");
    let file = |signals: &[StoredSignal]| PageFile {
        module: "TEST".into(),
        pages: Vec::<Page>::new(),
        signals: signals.to_vec(),
    };
    for (sub, signals) in [("shipped", shipped), ("previous", was), ("active", mine)] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
        file(signals)
            .save(&dir.join(sub).join("test.json"))
            .unwrap();
    }
    let pages =
        Pages::new(dir.join("shipped"), dir.join("active")).with_previous(dir.join("previous"));
    pages.merge_new("9.9.9").unwrap();
    pages.library().signals_on("TEST").to_vec()
}

#[test]
fn an_update_follows_an_untouched_signal_and_leaves_a_changed_one() {
    let mut newer = fuel();
    newer.note = "Newer".into();
    // Untouched: it follows the release.
    let after = update(&[newer.clone()], &[fuel()], &[fuel()]);
    assert_eq!(after[0].note, "Newer");
    // Changed here: it stays as the user left it.
    let mut mine = fuel();
    mine.name = "Gas".into();
    let after = update(&[newer.clone()], &[fuel()], &[mine]);
    assert_eq!(after[0].name, "Gas");
    assert_eq!(after[0].note, "Totalizer, hundreds of pounds");
    // Deleted here: it stays deleted.
    assert!(update(&[newer.clone()], &[fuel()], &[]).is_empty());
    // New in the release: it comes in.
    let mut gauge = fuel();
    gauge.id = "new001".into();
    gauge.name = "Other".into();
    let after = update(&[newer, gauge], &[fuel()], &[fuel()]);
    assert_eq!(after.len(), 2);
}
