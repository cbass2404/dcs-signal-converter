//! A profile shared with the pages it shows.
//!
//! Pages live in a library apart from profiles, so a profile file alone is a
//! screen full of slots pointing at pages the other machine does not have. An
//! export is the profile and its pages as one file; an import brings in the
//! pages the user ticks and settles each one against the library already
//! there. The stored signals those pages draw travel with them and are
//! settled the same way. See docs/CONFIG.md "Sharing pages".

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::pages::same_name;
use crate::{Error, Page, PageLibrary, Profile, Result, StoredSignal, SCHEMA_VERSION};

/// What an export writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub schema_version: u32,
    pub profile: Profile,
    #[serde(default)]
    pub pages: Vec<Page>,
    /// The stored signals the pages draw, and no others.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<StoredSignal>,
}

/// The two shapes a shared file can have: a bundle, or a profile on its own.
#[derive(Deserialize)]
#[serde(untagged)]
enum Shared {
    Bundle(Bundle),
    Profile(Profile),
}

impl Bundle {
    /// `profile` with every page its slots show, and the pages in `also`,
    /// from the library. Only pages on the profile's module go in, in the
    /// order the library holds them.
    pub fn of(profile: &Profile, lib: &PageLibrary, also: &[String]) -> Bundle {
        let used = profile.pages_used();
        let pages: Vec<Page> = lib
            .on_module(&profile.module)
            .iter()
            .filter(|p| used.contains(&p.id) || also.contains(&p.id))
            .cloned()
            .collect();
        let named = signals_named(&pages);
        let signals = lib
            .signals_on(&profile.module)
            .iter()
            .filter(|s| named.contains(&s.id))
            .cloned()
            .collect();
        Bundle {
            schema_version: SCHEMA_VERSION,
            profile: profile.clone(),
            pages,
            signals,
        }
    }

    /// Read a shared file: a bundle, or a profile without pages. Each page's
    /// fields take the page's display, as a page file's do.
    pub fn load(path: &Path) -> Result<Bundle> {
        let text = std::fs::read_to_string(path)?;
        let shared: Shared =
            serde_json::from_str(&text).map_err(|e| Error::Json(e, path.display().to_string()))?;
        let mut bundle = match shared {
            Shared::Bundle(b) => b,
            Shared::Profile(p) => Bundle {
                schema_version: p.schema_version,
                profile: p,
                pages: Vec::new(),
                signals: Vec::new(),
            },
        };
        for page in &mut bundle.pages {
            for f in &mut page.fields {
                f.display = page.display.clone();
                f.device.clear();
            }
        }
        let mut own = crate::PageFile {
            module: bundle.profile.module.clone(),
            pages: std::mem::take(&mut bundle.pages),
            signals: bundle.signals.clone(),
        };
        own.attach_signals();
        bundle.pages = own.pages;
        Ok(bundle)
    }

    /// Write the bundle, CRLF like every file here, each page field without
    /// its device and display.
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut bare = self.clone();
        for page in &mut bare.pages {
            for f in &mut page.fields {
                f.display.clear();
                f.device.clear();
            }
        }
        let text = serde_json::to_string_pretty(&bare)
            .map_err(|e| Error::Json(e, path.display().to_string()))?
            .replace('\n', "\r\n");
        std::fs::write(path, text)?;
        Ok(())
    }
}

/// What becomes of a page brought in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fate {
    /// Not here: it comes in as it is.
    New,
    /// Already here, the same page: left alone, and the slots keep showing it.
    Same,
    /// A different page already has its id: it comes in under a new one.
    NewId,
}

/// One page offered by an import, as the dialog shows it.
#[derive(Debug, Clone, Serialize)]
pub struct PagePlan {
    pub id: String,
    pub name: String,
    pub fields: usize,
    /// Whether a slot in the profile shows it.
    pub used: bool,
    pub fate: Fate,
    /// The name it would come in under: its own, or with a number added when
    /// another page on the module has it. The dialog can change it.
    pub name_after: String,
}

/// A page ticked for import, under the name given it.
#[derive(Debug, Clone, Deserialize)]
pub struct PageTake {
    pub id: String,
    pub name: String,
}

/// Whether two pages draw the same thing. The name is left out: a page
/// renamed here is still the page that was shared.
fn same_page(a: &Page, b: &Page) -> bool {
    a.display == b.display
        && serde_json::to_value(&a.fields).ok() == serde_json::to_value(&b.fields).ok()
}

fn fate(lib: &PageLibrary, module: &str, page: &Page) -> Fate {
    match lib.find(&page.id) {
        None => Fate::New,
        Some((on, here)) if on == module && same_page(here, page) => Fate::Same,
        Some(_) => Fate::NewId,
    }
}

/// What an import of `incoming` with `profile` would do to each page. Every
/// page in a bundle is taken as the profile's module's, since an export writes
/// no other.
pub fn plan(lib: &PageLibrary, profile: &Profile, incoming: &[Page]) -> Vec<PagePlan> {
    let used = profile.pages_used();
    let mut names = lib.clone();
    let module = &profile.module;
    let mut out = Vec::new();
    for page in incoming {
        let fate = fate(lib, module, page);
        let name_after = match fate {
            Fate::Same => lib
                .page_on(module, &page.id)
                .map_or(page.name.clone(), |p| p.name.clone()),
            _ => names.free_name(module, &page.name, None),
        };
        if fate != Fate::Same {
            // Held while the rest are named, so two pages brought in under
            // one name are told apart too.
            let named = Page {
                name: name_after.clone(),
                ..page.clone()
            };
            names = with_added(&names, module, std::slice::from_ref(&named));
        }
        out.push(PagePlan {
            id: page.id.clone(),
            name: page.name.clone(),
            fields: page.fields.len(),
            used: used.contains(&page.id),
            fate,
            name_after,
        });
    }
    out
}

/// Bring the ticked pages into the library, and point `profile`'s slots at
/// them as they arrive.
///
/// Returns the pages to add to the module's file, with their final ids and
/// names. A page left unticked empties the slots showing it. A page already
/// here unchanged adds nothing and its slots keep its id. One whose id a
/// different page has comes in under a new id, and its slots follow it.
///
/// Refused when a name given is empty or taken on the module, since the user
/// would then have two pages they cannot tell apart.
pub fn bring_in(
    lib: &PageLibrary,
    profile: &mut Profile,
    incoming: &[Page],
    take: &[PageTake],
) -> std::result::Result<Vec<Page>, String> {
    let module = profile.module.clone();
    let mut names = lib.clone();
    let mut avoid: BTreeSet<String> = incoming.iter().map(|p| p.id.clone()).collect();
    let mut added = Vec::new();
    for page in incoming {
        let Some(t) = take.iter().find(|t| t.id == page.id) else {
            for slots in profile.screens.values_mut() {
                slots.clear_page(&page.id);
            }
            continue;
        };
        let fate = fate(lib, &module, page);
        if fate == Fate::Same {
            continue;
        }
        let name = t.name.trim().to_string();
        if name.is_empty() {
            return Err(format!(
                "the page {:?} needs a name to come in under",
                page.name
            ));
        }
        if names
            .on_module(&module)
            .iter()
            .any(|p| same_name(&p.name, &name))
        {
            return Err(format!(
                "a page on {module} is already called {name}; give {:?} another name",
                page.name
            ));
        }
        let id = match fate {
            Fate::NewId => {
                let id = lib.fresh_id_avoiding(&avoid);
                avoid.insert(id.clone());
                for slots in profile.screens.values_mut() {
                    for slot in slots.slots.iter_mut().flatten() {
                        if slot.page.as_deref() == Some(page.id.as_str()) {
                            slot.page = Some(id.clone());
                        }
                    }
                }
                id
            }
            _ => page.id.clone(),
        };
        let arrived = Page {
            id,
            name,
            display: page.display.clone(),
            fields: page.fields.clone(),
        };
        names
            .files
            .entry(module.clone())
            .or_insert_with(|| crate::PageFile {
                module: module.clone(),
                pages: Vec::new(),
                signals: Vec::new(),
            })
            .pages
            .push(arrived.clone());
        added.push(arrived);
    }
    Ok(added)
}

/// Every stored signal id the fields of `pages` draw, switch cases included.
pub fn signals_named(pages: &[Page]) -> BTreeSet<String> {
    pages
        .iter()
        .flat_map(|p| &p.fields)
        .flat_map(|f| f.pieces())
        .filter(|s| s.is_stored())
        .map(|s| s.signal.clone())
        .collect()
}

/// Whether two stored signals read the same thing. The name and note are
/// left out, as a page's name is: one renamed here is still the one shared.
fn same_signal(a: &StoredSignal, b: &StoredSignal) -> bool {
    serde_json::to_value(&a.terms).ok() == serde_json::to_value(&b.terms).ok() && a.sum == b.sum
}

/// Settle the stored signals `pages` draw against the library, the way
/// [`bring_in`] settles pages, and return the ones to add to the module's
/// file.
///
/// One already here and reading the same is used as it is. One not here
/// comes in, renamed with a number where its name is taken. One whose id a
/// different signal has comes in under a new id, and every piece in `pages`
/// drawing it follows. Only signals the pages draw come in.
pub fn bring_in_signals(
    lib: &PageLibrary,
    module: &str,
    offered: &[StoredSignal],
    pages: &mut [Page],
) -> Vec<StoredSignal> {
    let needed = signals_named(pages);
    let mut names = lib.clone();
    let mut avoid: BTreeSet<String> = offered.iter().map(|s| s.id.clone()).collect();
    let mut added = Vec::new();
    for signal in offered.iter().filter(|s| needed.contains(&s.id)) {
        let id = match lib.find_signal(&signal.id) {
            Some((on, here)) if on == module && same_signal(here, signal) => continue,
            None => signal.id.clone(),
            Some(_) => {
                let id = lib.fresh_id_avoiding(&avoid);
                avoid.insert(id.clone());
                for page in pages.iter_mut() {
                    renamed_signal(page, &signal.id, &id);
                }
                id
            }
        };
        let arrived = StoredSignal {
            id,
            name: names.free_signal_name(module, &signal.name, None),
            ..signal.clone()
        };
        names = with_signals(&names, module, std::slice::from_ref(&arrived));
        added.push(arrived);
    }
    added
}

/// Point every piece on `page` drawing the stored signal `from` at `to`.
fn renamed_signal(page: &mut Page, from: &str, to: &str) {
    for field in &mut page.fields {
        for span in &mut field.content {
            if span.signal == from {
                span.signal = to.to_string();
            }
            if span.cases.is_empty() {
                continue;
            }
            for case in &mut span.cases.0 {
                for patch in case.written.patches_mut() {
                    if patch.signal == from {
                        patch.signal = to.to_string();
                    }
                }
            }
            span.resolve_cases();
        }
    }
}

/// `lib` with `pages` added to `module`'s file.
pub fn with_added(lib: &PageLibrary, module: &str, pages: &[Page]) -> PageLibrary {
    let mut lib = lib.clone();
    let file = lib
        .files
        .entry(module.to_string())
        .or_insert_with(|| crate::PageFile {
            module: module.to_string(),
            pages: Vec::new(),
            signals: Vec::new(),
        });
    file.pages.extend(pages.iter().cloned());
    file.attach_signals();
    lib
}

/// `lib` with `signals` added to `module`'s file.
pub fn with_signals(lib: &PageLibrary, module: &str, signals: &[StoredSignal]) -> PageLibrary {
    let mut lib = lib.clone();
    let file = lib
        .files
        .entry(module.to_string())
        .or_insert_with(|| crate::PageFile {
            module: module.to_string(),
            pages: Vec::new(),
            signals: Vec::new(),
        });
    file.signals.extend(signals.iter().cloned());
    file.attach_signals();
    lib
}
