//! The page modifier, asked whether it is held at the moment a page key goes
//! down.
//!
//! A keyboard key is asked of Windows and a controller button of DirectInput,
//! both at the press, so nothing is read in between. A controller that is not
//! there is looked for every two seconds on a thread of its own, because
//! listing the controllers can take longer than a pass of the main loop
//! should. One that is open is asked as often whether it is still there.
//! What the watch finds goes into lines the main loop writes to the log.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use dsc_config::settings::{Modifier, PageModifier};
use dsc_input::Device;

/// How long between looks for a controller that is not there.
const LOOK_EVERY: Duration = Duration::from_secs(2);

pub enum Line {
    Say(String),
    Warn(String),
}

pub struct Watch {
    state: Mutex<State>,
    wake: Condvar,
}

struct State {
    setting: PageModifier,
    /// The chosen controller, while it is open.
    device: Option<Device>,
    /// Whether the log has been told it is missing since it was last there.
    said_missing: bool,
    lines: Vec<Line>,
}

impl Watch {
    /// Start watching. The first look for a controller happens at once, on
    /// the watch's own thread.
    pub fn start(setting: PageModifier) -> Arc<Watch> {
        let watch = Arc::new(Watch {
            state: Mutex::new(State {
                setting,
                device: None,
                said_missing: false,
                lines: Vec::new(),
            }),
            wake: Condvar::new(),
        });
        let looking = Arc::clone(&watch);
        let spawned = std::thread::Builder::new()
            .name("modifier".into())
            .spawn(move || looking.look());
        if let Err(e) = spawned {
            watch.lock().lines.push(Line::Warn(format!(
                "keys     could not start looking for the page modifier: {e}"
            )));
        }
        watch
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A reader that panicked holding it left nothing half done.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Use another setting, from a reloaded settings file. The same one
    /// changes nothing, so an unrelated change keeps the controller open.
    pub fn set(&self, setting: PageModifier) {
        let mut state = self.lock();
        if state.setting == setting {
            return;
        }
        state.setting = setting;
        state.device = None;
        state.said_missing = false;
        self.wake.notify_all();
    }

    /// Whether the page modifier, and only it, is held now. Asked by a page
    /// key's reader as the key goes down.
    pub fn held(&self) -> bool {
        let keys = keys_held();
        let mut state = self.lock();
        let down = match &state.setting {
            PageModifier::Key(_) => false,
            PageModifier::Button(b) => match state.device.as_ref().map(|d| d.is_down(b.button)) {
                Some(Ok(down)) => down,
                Some(Err(_)) => {
                    // Gone. The watch says so and looks for it again.
                    state.device = None;
                    self.wake.notify_all();
                    false
                }
                None => false,
            },
        };
        state.setting.counts(&keys, down)
    }

    /// What the watch has to say since it was last asked.
    pub fn lines(&self) -> Vec<Line> {
        std::mem::take(&mut self.lock().lines)
    }

    /// The watch's thread: open the chosen controller whenever it is not
    /// open, and wait while there is nothing to do.
    fn look(&self) {
        let mut state = self.lock();
        loop {
            let PageModifier::Button(button) = state.setting.clone() else {
                state = self.wake.wait(state).unwrap_or_else(|e| e.into_inner());
                continue;
            };
            if state.device.is_some() {
                // Asked now and then whether it is still there, so an
                // unplugged controller is in the log before a page key is
                // pressed and does nothing. One read, a few microseconds.
                state = self
                    .wake
                    .wait_timeout(state, LOOK_EVERY)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
                if state.device.as_ref().is_some_and(|d| d.pressed().is_err()) {
                    state.device = None;
                }
                continue;
            }
            // Opened without the lock, so a press is never kept waiting on it.
            drop(state);
            let opened = Device::open(&button.device, &button.product);
            state = self.lock();
            if state.setting != PageModifier::Button(button.clone()) {
                // Changed while looking; look for the new one.
                continue;
            }
            match opened {
                Ok(device) => {
                    state.device = Some(device);
                    if state.said_missing {
                        state.said_missing = false;
                        state.lines.push(Line::Say(format!(
                            "keys     {} is back. Page swapping works again.",
                            button.name
                        )));
                    }
                }
                Err(e) => {
                    if !state.said_missing {
                        state.said_missing = true;
                        let why = match e {
                            dsc_input::Error::NotFound => String::new(),
                            e => format!(" ({e})"),
                        };
                        state.lines.push(Line::Warn(format!(
                            "keys     {} not found{why}. Page swapping is disabled until it's back or another modifier is selected.",
                            button.name
                        )));
                    }
                    state = self
                        .wake
                        .wait_timeout(state, LOOK_EVERY)
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
            }
        }
    }
}

#[cfg(windows)]
fn keys_held() -> Vec<Modifier> {
    crate::keyboard::held_now()
}

#[cfg(not(windows))]
fn keys_held() -> Vec<Modifier> {
    Vec::new()
}
