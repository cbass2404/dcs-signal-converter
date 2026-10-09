//! The PC's own settings, read and written for the Settings dialog.
//!
//! The daemon reads the same file and picks up a change the way it picks up a
//! saved profile, so nothing here needs to reach it.
//!
//! Also the dialog's "press the button" for a controller page modifier, which
//! reads every controller shared and only while it waits, as DCS goes on
//! reading them too, and sleeps until one of them sends something.

use std::sync::OnceLock;
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
    /// A development checkout, where features not yet open to everyone show.
    pub dev: bool,
}

/// How long the dialog waits for a button before giving up.
const CAPTURE_FOR: Duration = Duration::from_secs(10);

/// Set to stop a capture early, when the dialog is closed or the choice
/// changed while it waited. It wakes the wait rather than being looked at.
static CAPTURE_CANCEL: OnceLock<Result<dsc_input::Cancel, String>> = OnceLock::new();

fn capture_cancel() -> Reply<&'static dsc_input::Cancel> {
    CAPTURE_CANCEL
        .get_or_init(|| dsc_input::Cancel::new().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| fail("waiting for a button", e))
}

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
    let dev = paths.is_dev();
    Ok(match Settings::load(&paths.settings) {
        Ok(settings) => SettingsView {
            modifier_missing: missing(&settings.page_modifier),
            settings,
            problem: None,
            dev,
        },
        Err(e) => SettingsView {
            settings: Settings::default(),
            problem: Some(e.to_string()),
            modifier_missing: false,
            dev,
        },
    })
}

#[tauri::command]
pub fn settings_save(settings: Settings) -> Reply<()> {
    let paths = Paths::resolve();
    // The dialog owns the theme and the modifier, not the boards' ports, which
    // are added with a board; keep whatever the file already allows.
    let mut settings = settings;
    if let Ok(on_file) = Settings::load(&paths.settings) {
        settings.dsc_ports = on_file.dsc_ports;
    }
    settings
        .save(&paths.settings)
        .map_err(|e| fail("saving the settings", e))
}

/// Wait for a button on any controller and return it as a page modifier, or
/// nothing if none was pressed in time or the wait was cancelled. Off the
/// main thread, so the window keeps drawing while it waits.
#[tauri::command]
pub async fn controller_capture() -> Reply<Option<ControllerButton>> {
    let cancel = capture_cancel()?;
    cancel.reset();
    let pressed =
        tauri::async_runtime::spawn_blocking(move || dsc_input::capture(CAPTURE_FOR, cancel))
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
    if let Ok(cancel) = capture_cancel() {
        cancel.set();
    }
}
