//! Pages as the editor works with them: one module's pages at a time.
//!
//! A page belongs to the library, not to the profile open in the window, so
//! it is saved and deleted on its own and the profile's Save writes only which
//! page sits in which slot. Every profile on the module shows the same page,
//! so editing one here edits it everywhere: the window is told where each page
//! is used so it can say so, and deleting one empties the slots showing it in
//! every profile.
//!
//! The module's stored signals live in the same file and are kept the same
//! way: each saved on its own, and shared by every page that draws it.

use std::collections::BTreeSet;

use dsc_config::paths::Paths;
use dsc_config::{Page, PageFile, PageLibrary, Profile, StoredSignal};

use crate::{fail, Reply};

/// One slot showing a page, somewhere in the active profiles.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PageUse {
    pub page: String,
    pub file: String,
    pub profile: String,
    pub device: String,
    /// Counting from 1.
    pub slot: usize,
}

/// A module's pages, and everything the window needs to show them.
#[derive(Debug, serde::Serialize)]
pub struct PagesView {
    pub pages: Vec<Page>,
    /// Why the module's page file would not load, when it would not. Its
    /// pages cannot be edited until it is fixed by hand, since saving would
    /// write over what is there.
    pub broken: Option<String>,
    pub used: Vec<PageUse>,
    /// The module's pages as they shipped, so one field of a shipped page can
    /// be put back without touching the rest. Empty for a module that ships
    /// none; a page the user made is simply not in it.
    pub shipped: Vec<Page>,
    /// The module's stored signals, in file order.
    pub signals: Vec<StoredSignal>,
    /// The stored signals as they shipped, for putting one back.
    pub shipped_signals: Vec<StoredSignal>,
}

/// Pages as the window sends them, put the way a page file loads: every
/// field on its page's display and on no device. A field the window added is
/// made on the device whose screen it was added to, which the page does not
/// keep.
pub fn tidy(pages: &[Page]) -> Vec<Page> {
    let mut pages = pages.to_vec();
    for page in &mut pages {
        for f in &mut page.fields {
            f.device.clear();
            f.display = page.display.clone();
            f.page = None;
        }
    }
    pages
}

/// The library in use, with each page being edited in place of the saved
/// one with its id, or added when it is new.
pub fn library_with(paths: &Paths, module: &str, working: &[Page]) -> PageLibrary {
    let mut lib = paths.pages.library();
    if working.is_empty() {
        return lib;
    }
    let file = lib
        .files
        .entry(module.to_string())
        .or_insert_with(|| PageFile {
            module: module.to_string(),
            pages: Vec::new(),
            signals: Vec::new(),
        });
    for page in tidy(working) {
        match file.pages.iter_mut().find(|p| p.id == page.id) {
            Some(there) => *there = page,
            None => file.pages.push(page),
        }
    }
    file.attach_signals();
    lib
}

/// Every active profile on `module`, by file name.
fn profiles_on(paths: &Paths, module: &str) -> Vec<(String, Profile)> {
    let Ok(entries) = std::fs::read_dir(&paths.profiles.active) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Profile)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .filter_map(|p| {
            let file = p.file_name()?.to_string_lossy().into_owned();
            let profile = Profile::load(&p).ok()?;
            (profile.module == module).then_some((file, profile))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Every slot on `module` showing a page, across the active profiles.
pub fn usage(paths: &Paths, module: &str) -> Vec<PageUse> {
    let mut out = Vec::new();
    for (file, p) in profiles_on(paths, module) {
        for (device, slots) in &p.screens {
            // A follower's slots are kept and not shown, so they use nothing.
            if p.follows.contains_key(device) {
                continue;
            }
            for (i, id) in slots.pages() {
                out.push(PageUse {
                    page: id.to_string(),
                    file: file.clone(),
                    profile: p.name.clone(),
                    device: device.clone(),
                    slot: i + 1,
                });
            }
        }
    }
    out
}

/// One module's pages, for a profile on it being opened.
#[tauri::command]
pub fn open_pages(module: String) -> Reply<PagesView> {
    let paths = Paths::resolve();
    let lib = paths.pages.library();
    let shipped = PageLibrary::load_dir(&paths.pages.defaults);
    Ok(PagesView {
        pages: lib.on_module(&module).to_vec(),
        broken: lib.broken(&module).map(str::to_string),
        used: usage(&paths, &module),
        shipped: shipped.on_module(&module).to_vec(),
        signals: lib.signals_on(&module).to_vec(),
        shipped_signals: shipped.signals_on(&module).to_vec(),
    })
}

/// An id for a new page: none in the library has it, and neither do `avoid`,
/// the new pages the window holds that are not saved yet.
#[tauri::command]
pub fn new_page_id(avoid: Vec<String>) -> Reply<String> {
    let paths = Paths::resolve();
    let avoid: BTreeSet<String> = avoid.into_iter().collect();
    Ok(paths.pages.library().fresh_id_avoiding(&avoid))
}

/// Save one page into its module's file, adding it or replacing the page with
/// its id, and hand back the module's pages as they now are.
///
/// Refused when the page could not be shown: a name another page on the
/// module has, glass that is not a text grid, fields that overlap or run off
/// the screen, or characters the font of the profile being edited cannot
/// draw. A page is saved on its own, apart from the profile, because it is
/// the library's rather than the profile's: every profile on the module shows
/// the same page.
#[tauri::command]
pub fn save_page(
    profile: Profile,
    page: Page,
    device: String,
    cache: tauri::State<crate::check::Cache>,
) -> Reply<PagesView> {
    let paths = Paths::resolve();
    let module = profile.module.clone();
    let mut lib = paths.pages.library();
    if let Some(why) = lib.broken(&module) {
        return Err(format!(
            "{} was not saved, because the page file for {module} would not load and saving would replace it: {why}",
            page.name.trim()
        ));
    }
    let mut page = tidy(std::slice::from_ref(&page)).remove(0);
    page.name = page.name.trim().to_string();
    if let Some(file) = lib.files.get(&module) {
        file.attach_to(&mut page);
    }
    let problems = cache.page_problems(&paths, &lib, &profile, &page, &device);
    if !problems.is_empty() {
        return Err(format!(
            "{} was not saved:\n{}",
            page.name,
            problems.join("\n")
        ));
    }

    let file = lib.files.entry(module.clone()).or_insert_with(|| PageFile {
        module: module.clone(),
        pages: Vec::new(),
        signals: Vec::new(),
    });
    match file.pages.iter_mut().find(|p| p.id == page.id) {
        Some(there) => *there = page,
        None => file.pages.push(page),
    }
    lib.save_module(&paths.pages.active, &module)
        .map_err(|e| fail(&format!("writing the pages for {module}"), e))?;
    open_pages(module)
}

/// Delete a page from its module's file, emptying every slot showing it in
/// the saved profiles on the module but `current`, whose slots the window
/// empties itself so that its own Save stays the one that writes it.
///
/// Returns the module's pages as they now are, and names the profiles changed.
#[tauri::command]
pub fn delete_page(module: String, id: String, current: String) -> Reply<(PagesView, Vec<String>)> {
    let paths = Paths::resolve();
    let mut lib = paths.pages.library();
    if let Some(why) = lib.broken(&module) {
        return Err(format!(
            "the page file for {module} would not load, so nothing was deleted: {why}"
        ));
    }
    if let Some(file) = lib.files.get_mut(&module) {
        file.pages.retain(|p| p.id != id);
    }
    lib.save_module(&paths.pages.active, &module)
        .map_err(|e| fail(&format!("writing the pages for {module}"), e))?;

    let mut touched = Vec::new();
    for (name, mut p) in profiles_on(&paths, &module) {
        if name == current {
            continue;
        }
        let changed = p
            .screens
            .values_mut()
            .fold(false, |any, s| !s.clear_page(&id).is_empty() || any);
        if changed {
            p.save(&paths.profiles.active.join(&name))
                .map_err(|e| fail(&format!("writing {name}"), e))?;
            touched.push(p.name);
        }
    }
    Ok((open_pages(module)?, touched))
}

/// Save one stored signal into its module's file, adding it or replacing the
/// one with its id, and hand back the module's pages as they now are.
///
/// Refused when it is not finished or another signal on the module has its
/// name. Every page drawing it draws the change, in every profile on the
/// module, since the signal is the library's like the pages.
#[tauri::command]
pub fn save_signal(
    module: String,
    signal: StoredSignal,
    cache: tauri::State<crate::check::Cache>,
) -> Reply<PagesView> {
    let paths = Paths::resolve();
    let mut lib = paths.pages.library();
    if let Some(why) = lib.broken(&module) {
        return Err(format!(
            "{} was not saved, because the page file for {module} would not load and saving would replace it: {why}",
            signal.name.trim()
        ));
    }
    let mut signal = signal;
    signal.name = signal.name.trim().to_string();
    let problems = cache
        .with_module(&paths, &module, |m| lib.signal_problems(&signal, m))
        .map_err(|e| format!("{} was not saved: {e}", signal.name))?;
    if !problems.is_empty() {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        return Err(format!(
            "{} was not saved:\n{}",
            signal.name,
            lines.join("\n")
        ));
    }
    let file = lib.files.entry(module.clone()).or_insert_with(|| PageFile {
        module: module.clone(),
        pages: Vec::new(),
        signals: Vec::new(),
    });
    match file.signals.iter_mut().find(|s| s.id == signal.id) {
        Some(there) => *there = signal,
        None => file.signals.push(signal),
    }
    lib.save_module(&paths.pages.active, &module)
        .map_err(|e| fail(&format!("writing the pages for {module}"), e))?;
    open_pages(module)
}

/// Delete a stored signal from its module's file.
///
/// Refused while a saved page draws it, naming the pages: the piece would be
/// left drawing a signal that is not there, which every profile showing the
/// page would be refused for.
#[tauri::command]
pub fn delete_signal(module: String, id: String) -> Reply<PagesView> {
    let paths = Paths::resolve();
    let mut lib = paths.pages.library();
    if let Some(why) = lib.broken(&module) {
        return Err(format!(
            "the page file for {module} would not load, so nothing was deleted: {why}"
        ));
    }
    let drawing: Vec<&str> = lib
        .on_module(&module)
        .iter()
        .filter(|p| dsc_config::bundle::signals_named(std::slice::from_ref(*p)).contains(&id))
        .map(|p| p.name.as_str())
        .collect();
    if !drawing.is_empty() {
        return Err(format!(
            "It was not deleted, because these pages draw it: {}. Take it off them first.",
            drawing.join(", ")
        ));
    }
    // Lamps light by one from their profiles, which this window may not have
    // open: every saved profile on the module is asked.
    let lighting: Vec<String> = profiles_on(&paths, &module)
        .iter()
        .flat_map(|(_, p)| {
            p.bindings
                .iter()
                .filter(|b| b.signal.as_deref() == Some(id.as_str()))
                .map(move |b| format!("{} in {}", b.led, p.name))
        })
        .collect();
    if !lighting.is_empty() {
        return Err(format!(
            "It was not deleted, because these lamps light by it: {}. Take it off them first.",
            lighting.join(", ")
        ));
    }
    if let Some(file) = lib.files.get_mut(&module) {
        file.signals.retain(|s| s.id != id);
    }
    lib.save_module(&paths.pages.active, &module)
        .map_err(|e| fail(&format!("writing the pages for {module}"), e))?;
    open_pages(module)
}
