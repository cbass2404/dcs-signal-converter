//! The PC's own settings, read and written for the Settings dialog.
//!
//! The daemon reads the same file and picks up a change the way it picks up a
//! saved profile, so nothing here needs to reach it.
//!
//! Also the dialog's "press the button" for a controller page modifier, which
//! reads every controller shared and only while it waits, as DCS goes on
//! reading them too.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dsc_config::paths::Paths;
use dsc_config::settings::{ControllerButton, PageModifier, Settings};

use crate::{fail, Reply};

/// The settings as the window shows them.
#[derive(serde::Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    /// Why the file could not be read, when it could not. The defaults are
    /// shown instead, and saving writes over the broken file.
    pub problem: Option<String>,
    /// The page modifier is a controller button and that controller is not
    /// connected, so the converter swaps no pages until it is.
    pub modifier_missing: bool,
}

/// How long the dialog waits for a button before giving up.
const CAPTURE_FOR: Duration = Duration::from_secs(10);

/// Set to stop a capture early, when the dialog is closed or the choice
/// changed while it waited.
static CAPTURE_CANCEL: AtomicBool = AtomicBool::new(false);

/// Whether the controller a button modifier names is attached now, by the
/// same rule the converter finds it.
fn missing(modifier: &PageModifier) -> bool {
    match modifier {
        PageModifier::Key(_) => false,
        PageModifier::Button(b) => dsc_input::Device::open(&b.device, &b.product).is_err(),
    }
}

#[tauri::command]
pub fn settings_read() -> Reply<SettingsView> {
    let paths = Paths::resolve();
    Ok(match Settings::load(&paths.settings) {
        Ok(settings) => SettingsView {
            modifier_missing: missing(&settings.page_modifier),
            settings,
            problem: None,
        },
        Err(e) => SettingsView {
            settings: Settings::default(),
            problem: Some(e.to_string()),
            modifier_missing: false,
        },
    })
}

#[tauri::command]
pub fn settings_save(settings: Settings) -> Reply<()> {
    let paths = Paths::resolve();
    settings
        .save(&paths.settings)
        .map_err(|e| fail("saving the settings", e))
}

/// Wait for a button on any controller and return it as a page modifier, or
/// nothing if none was pressed in time or the wait was cancelled. Off the
/// main thread, so the window keeps drawing while it waits.
#[tauri::command]
pub async fn controller_capture() -> Reply<Option<ControllerButton>> {
    CAPTURE_CANCEL.store(false, Ordering::Relaxed);
    let pressed =
        tauri::async_runtime::spawn_blocking(|| dsc_input::capture(CAPTURE_FOR, &CAPTURE_CANCEL))
            .await
            .map_err(|e| fail("waiting for a button", e))?
            .map_err(|e| fail("reading the controllers", e))?;
    Ok(pressed.map(|p| ControllerButton {
        device: p.controller.instance,
        product: p.controller.product,
        name: p.controller.name,
        button: p.button,
    }))
}

#[tauri::command]
pub fn controller_capture_cancel() {
    CAPTURE_CANCEL.store(true, Ordering::Relaxed);
}
