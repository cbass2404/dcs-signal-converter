//! The base an update judges each user file against: the shipped default as
//! it was last seeded or merged into that file, kept in the active folder's
//! `.base`.
//!
//! It replaced judging against the previous release only, which left anyone
//! who skipped a release with every default changed in it read as their own
//! edit. Seen on beta.012: a tester's Hornet still disabled the ICP the way
//! beta.002 shipped it, because the beta.011 snapshot did not, so theirs read
//! as a decision. See issue #137.

use std::path::{Path, PathBuf};

use dsc_config::{DeviceInventory, Page, PageFile, PageLibrary, Pages, Profile, Profiles, Readout};

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = std::env::temp_dir().join(format!(
        "dsc-base-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    for sub in [
        "defaults",
        "defaults-previous",
        "active/.base",
        "default-pages",
        "default-pages-previous",
        "pages/.base",
    ] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    Scratch(dir)
}

fn inventory() -> DeviceInventory {
    DeviceInventory::load_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/devices"))
        .expect("the inventory loads")
}

/// A profile from its settings and one lamp row, as JSON text.
fn profile(settings: &str, lamp: &str) -> String {
    format!(
        r#"{{
      "schema_version": 2,
      "name": "Hog",
      "aircraft": ["A-10C"],
      "module": "A-10C",
      {settings}
      "bindings": [{lamp}]
    }}"#
    )
}

/// The MFD L backlight at a brightness, so each release can differ.
fn lamp(on: u8) -> String {
    format!(r#"{{ "device": "CarrierAce_MFD_L", "led": "Backlight", "always": true, "on": {on} }}"#)
}

const ICP_OFF: &str = r#""disabled_devices": ["ViperAce_ICP"],"#;

fn put(dir: &Path, sub: &str, text: &str) {
    std::fs::write(dir.join(sub).join("a-10c.json"), text).unwrap();
}

fn profiles(dir: &Path) -> Profiles {
    Profiles::new(dir.join("defaults"), dir.join("active"))
}

fn merge(dir: &Path, version: &str) -> Vec<String> {
    profiles(dir)
        .merge_new(&inventory(), version)
        .expect("merge runs")
}

fn mine(dir: &Path) -> Profile {
    Profile::load(&dir.join("active/a-10c.json")).expect("still loads")
}

fn on(p: &Profile) -> Option<u8> {
    p.bindings
        .iter()
        .find(|b| b.device == "CarrierAce_MFD_L" && b.led == "Backlight")
        .and_then(|b| b.on)
}

fn base(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("active/.base/a-10c.json")).ok()
}

// ------------------------------------------------------------------ profiles

/// The beta.012 report. The user last merged beta.002, which disabled the
/// ICP; beta.003 stopped, and the last release before this one never did.
/// Judged against that release their ICP read as their own choice; judged
/// against what they were actually given, it is ours to turn back on.
#[test]
fn a_user_several_releases_behind_takes_a_change_they_skipped() {
    let dir = scratch("behind");
    put(&dir, "active/.base", &profile(ICP_OFF, &lamp(200)));
    put(&dir, "defaults-previous", &profile("", &lamp(200)));
    put(&dir, "defaults", &profile("", &lamp(200)));
    put(&dir, "active", &profile(ICP_OFF, &lamp(200)));

    merge(&dir, "beta.012");

    assert!(
        mine(&dir).disabled_devices.is_empty(),
        "the ICP is driven again"
    );
}

/// An edit is the user's for good, not just for the release that found it,
/// while a row they left alone follows every release.
#[test]
fn a_row_the_user_edited_survives_two_updates_in_a_row() {
    let dir = scratch("twice");
    put(&dir, "active/.base", &profile("", &lamp(200)));
    put(&dir, "defaults", &profile(ICP_OFF, &lamp(150)));
    put(&dir, "active", &profile("", &lamp(90)));

    merge(&dir, "beta.012");
    assert_eq!(on(&mine(&dir)), Some(90), "theirs after the first");
    assert_eq!(
        mine(&dir).disabled_devices,
        ["ViperAce_ICP"],
        "the setting they left alone moved on"
    );

    put(&dir, "defaults", &profile("", &lamp(120)));
    merge(&dir, "beta.013");
    assert_eq!(on(&mine(&dir)), Some(90), "and after the second");
    assert!(
        mine(&dir).disabled_devices.is_empty(),
        "the setting followed the second release too"
    );
}

/// The changeover: a file from before base copies is judged against the last
/// release once, as it always was, and has a base from then on.
#[test]
fn with_no_base_the_previous_release_is_the_judge_once() {
    let dir = scratch("changeover");
    std::fs::remove_dir_all(dir.join("active/.base")).unwrap();
    put(&dir, "defaults-previous", &profile("", &lamp(200)));
    put(&dir, "defaults", &profile("", &lamp(150)));
    put(&dir, "active", &profile("", &lamp(200)));

    merge(&dir, "beta.013");

    assert_eq!(on(&mine(&dir)), Some(150), "untouched, so it followed");
    assert_eq!(
        base(&dir),
        std::fs::read_to_string(dir.join("defaults/a-10c.json")).ok(),
        "and the shipped file is its base now"
    );
}

/// The base is what shipped, not what the merge made of it, so a row the
/// user kept still differs from it next time.
#[test]
fn the_base_is_the_shipped_file_not_the_merged_one() {
    let dir = scratch("shipped");
    put(&dir, "active/.base", &profile("", &lamp(200)));
    put(&dir, "defaults", &profile("", &lamp(150)));
    put(&dir, "active", &profile("", &lamp(90)));

    merge(&dir, "beta.013");

    assert_eq!(
        base(&dir),
        std::fs::read_to_string(dir.join("defaults/a-10c.json")).ok()
    );
}

/// Once per version, like the merge: a second start of the same version
/// leaves the base alone.
#[test]
fn the_base_moves_only_when_a_version_merges() {
    let dir = scratch("once");
    put(&dir, "active/.base", &profile("", &lamp(200)));
    put(&dir, "defaults", &profile("", &lamp(150)));
    put(&dir, "active", &profile("", &lamp(200)));
    merge(&dir, "beta.013");

    put(&dir, "defaults", &profile("", &lamp(120)));
    merge(&dir, "beta.013");

    assert_eq!(base(&dir), Some(profile("", &lamp(150))));
}

#[test]
fn seeding_writes_a_base_and_a_profile_of_the_users_gets_none() {
    let dir = scratch("seed");
    put(&dir, "defaults", &profile("", &lamp(200)));
    let theirs = profile("", &lamp(90)).replace(r#"["A-10C"]"#, r#"["A-10C_2"]"#);
    std::fs::write(dir.join("active/hog.json"), theirs).unwrap();

    let profiles = profiles(&dir);
    profiles.seed().unwrap();
    profiles.merge_new(&inventory(), "beta.013").unwrap();

    assert_eq!(base(&dir), Some(profile("", &lamp(200))));
    assert!(!dir.join("active/.base/hog.json").exists());
}

/// Reset puts the shipped default back; the base already is the shipped
/// default this release merged, so it has nothing to do.
#[test]
fn reset_leaves_the_base_alone() {
    let dir = scratch("reset");
    put(&dir, "active/.base", &profile("", &lamp(200)));
    put(&dir, "defaults", &profile("", &lamp(150)));
    put(&dir, "active", &profile("", &lamp(90)));

    profiles(&dir).reset_to_default("a-10c.json").unwrap();

    assert_eq!(on(&mine(&dir)), Some(150));
    assert_eq!(base(&dir), Some(profile("", &lamp(200))));
}

/// A profile the user later makes under a deleted default's name is theirs.
#[test]
fn deleting_a_profile_deletes_its_base() {
    let dir = scratch("delete");
    put(&dir, "active/.base", &profile("", &lamp(200)));
    put(&dir, "active", &profile("", &lamp(200)));

    profiles(&dir).delete("a-10c.json").unwrap();

    assert_eq!(base(&dir), None);
}

// --------------------------------------------------------------------- pages

fn page(id: &str, name: &str, source: &str) -> Page {
    let mut f = Readout::reading("", "MCDU", "0-1".parse().unwrap(), source);
    f.device.clear();
    Page {
        id: id.into(),
        name: name.into(),
        display: "MCDU".into(),
        fields: vec![f],
    }
}

fn pages_in(dir: &Path, sub: &str, pages: Vec<Page>) {
    PageFile {
        module: "A-10C".into(),
        pages,
        signals: Vec::new(),
    }
    .save(&dir.join(sub).join("a-10c.json"))
    .unwrap();
}

fn page_store(dir: &Path) -> Pages {
    Pages::new(dir.join("default-pages"), dir.join("pages"))
}

fn page_ids(dir: &Path) -> Vec<String> {
    PageLibrary::load_dir(&dir.join("pages"))
        .on_module("A-10C")
        .iter()
        .map(|p| p.id.clone())
        .collect()
}

/// A page shipped two releases ago and retired by the last one goes from a
/// library that never touched it, which judging against the last release
/// alone could not tell.
#[test]
fn a_page_retired_in_a_skipped_release_goes() {
    let dir = scratch("pages-behind");
    let radios = page("aaaaaa", "Radios", "A");
    let old = page("bbbbbb", "Old", "B");
    pages_in(&dir, "pages/.base", vec![radios.clone(), old.clone()]);
    pages_in(&dir, "default-pages-previous", vec![radios.clone()]);
    pages_in(&dir, "default-pages", vec![radios.clone()]);
    pages_in(&dir, "pages", vec![radios, old]);

    page_store(&dir).merge_new("beta.013").unwrap();

    assert_eq!(page_ids(&dir), ["aaaaaa"]);
}

#[test]
fn pages_fall_back_to_the_previous_release_and_then_have_a_base() {
    let dir = scratch("pages-changeover");
    std::fs::remove_dir_all(dir.join("pages/.base")).unwrap();
    let radios = page("aaaaaa", "Radios", "A");
    let fuel = page("bbbbbb", "Fuel", "B");
    pages_in(
        &dir,
        "default-pages-previous",
        vec![radios.clone(), fuel.clone()],
    );
    pages_in(&dir, "default-pages", vec![radios.clone(), fuel]);
    // The user deleted Fuel.
    pages_in(&dir, "pages", vec![radios]);

    page_store(&dir).merge_new("beta.013").unwrap();

    assert_eq!(page_ids(&dir), ["aaaaaa"], "Fuel stays deleted");
    assert_eq!(
        std::fs::read_to_string(dir.join("pages/.base/a-10c.json")).ok(),
        std::fs::read_to_string(dir.join("default-pages/a-10c.json")).ok(),
    );
}

#[test]
fn seeding_pages_writes_a_base() {
    let dir = scratch("pages-seed");
    pages_in(&dir, "default-pages", vec![page("aaaaaa", "Radios", "A")]);

    page_store(&dir).seed().unwrap();

    assert_eq!(
        std::fs::read_to_string(dir.join("pages/.base/a-10c.json")).ok(),
        std::fs::read_to_string(dir.join("default-pages/a-10c.json")).ok(),
    );
}

/// Reset brings every shipped page back as it shipped, the changed and the
/// deleted alike, and keeps the user's own, renamed off a shipped name.
#[test]
fn resetting_pages_restores_the_shipped_and_keeps_the_users() {
    let dir = scratch("pages-reset");
    let radios = page("aaaaaa", "Radios", "A");
    let fuel = page("bbbbbb", "Fuel", "B");
    pages_in(&dir, "pages/.base", vec![radios.clone(), fuel.clone()]);
    pages_in(&dir, "default-pages", vec![radios.clone(), fuel]);
    // Radios edited, Fuel deleted, and a page of their own named Fuel.
    pages_in(
        &dir,
        "pages",
        vec![page("aaaaaa", "Radios", "Z"), page("cccccc", "Fuel", "C")],
    );
    let base = std::fs::read_to_string(dir.join("pages/.base/a-10c.json")).ok();

    assert!(page_store(&dir).reset_module("A-10C").unwrap());

    assert_eq!(page_ids(&dir), ["aaaaaa", "bbbbbb", "cccccc"]);
    let lib = PageLibrary::load_dir(&dir.join("pages"));
    assert_eq!(
        serde_json::to_value(lib.page_on("A-10C", "aaaaaa")).unwrap(),
        serde_json::to_value(Some(&radios)).unwrap(),
    );
    assert_eq!(lib.page_on("A-10C", "cccccc").unwrap().name, "Fuel 2");
    assert_eq!(
        std::fs::read_to_string(dir.join("pages/.base/a-10c.json")).ok(),
        base,
        "the base is left alone"
    );
}

#[test]
fn resetting_pages_replaces_a_file_that_will_not_load() {
    let dir = scratch("pages-reset-broken");
    pages_in(&dir, "default-pages", vec![page("aaaaaa", "Radios", "A")]);
    std::fs::write(dir.join("pages/a-10c.json"), "{ not json").unwrap();

    assert!(page_store(&dir).reset_module("A-10C").unwrap());

    assert_eq!(page_ids(&dir), ["aaaaaa"]);
}

#[test]
fn resetting_a_module_that_ships_no_pages_does_nothing() {
    let dir = scratch("pages-reset-none");
    pages_in(&dir, "pages", vec![page("cccccc", "Mine", "C")]);

    assert!(!page_store(&dir).reset_module("A-10C").unwrap());

    assert_eq!(page_ids(&dir), ["cccccc"]);
}
