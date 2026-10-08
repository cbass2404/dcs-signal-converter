//! The page modifier, asked whether it is held at the moment a page key goes
//! down.
//!
//! A keyboard key is asked of Windows and a controller button of DirectInput,
//! both at the press, so nothing is read in between. Windows says when a
//! controller is plugged in or pulled out, and only then does the watch's own
//! thread look: for the chosen controller if it is missing, and at the open
//! one, to see whether it was the one that went. Looking is on that thread
//! because listing the controllers can take longer than a pass of the main
//! loop should. What the watch finds goes into lines the main loop writes to
//! the log.
//!
//! A controller is held open only while some panel with page keys is driven,
//! as the key readers are: with nothing to swap, nothing asks for the
//! modifier, so nothing looks for it either.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use dsc_config::settings::{Modifier, PageModifier};
use dsc_input::{Device, DeviceChanges};

/// How long between looks for a missing controller, only where Windows would
/// not say when controllers come and go.
const LOOK_EVERY: Duration = Duration::from_secs(2);

/// After Windows says a controller arrived, DirectInput can take a moment
/// longer to list it. So a controller still missing is looked for again this
/// often, this many times, before the watch waits for the next arrival.
const CATCH_UP: Duration = Duration::from_millis(250);
const CATCH_UP_LOOKS: u8 = 8;

pub enum Line {
    Say(String),
    Warn(String),
}

pub struct Watch {
    state: Mutex<State>,
    wake: Condvar,
    /// Windows calling the watch as controllers come and go, kept for as long
    /// as the watch is. `None` where it would not, and the watch looks every
    /// [`LOOK_EVERY`] instead.
    changes: Mutex<Option<DeviceChanges>>,
}

struct State {
    setting: PageModifier,
    /// The chosen controller, while it is open.
    device: Option<Device>,
    /// Whether the log has been told it is missing since it was last there.
    said_missing: bool,
    lines: Vec<Line>,
    /// Whether a panel with page keys is driven, so the modifier is wanted.
    wanted: bool,
    /// Counts controllers coming and going, so the thread can tell that
    /// waking was one of them.
    changed: u64,
    /// Looks left for a controller that has just arrived; see [`CATCH_UP`].
    catching_up: u8,
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
                wanted: false,
                changed: 0,
                catching_up: 0,
            }),
            wake: Condvar::new(),
            changes: Mutex::new(None),
        });
        // Weak, so the registration the watch keeps does not keep it alive.
        let told = Arc::downgrade(&watch);
        match dsc_input::on_device_change(Box::new(move || {
            if let Some(watch) = told.upgrade() {
                watch.controllers_changed();
            }
        })) {
            Ok(changes) => *watch.changes.lock().unwrap_or_else(|e| e.into_inner()) = Some(changes),
            Err(e) => watch.lock().lines.push(Line::Warn(format!(
                "keys     Windows will not say when controllers come and go ({e}), so a missing page modifier is looked for every {} s",
                LOOK_EVERY.as_secs()
            ))),
        }
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

    /// A controller was plugged in or pulled out. Called by Windows, on a
    /// thread of its own, so it only wakes the watch's.
    fn controllers_changed(&self) {
        let mut state = self.lock();
        state.changed += 1;
        state.catching_up = CATCH_UP_LOOKS;
        drop(state);
        self.wake.notify_all();
    }

    /// Whether Windows says when controllers come and go.
    fn hears_changes(&self) -> bool {
        self.changes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Sleep until something changes, or at most `limit`.
    fn sleep<'a>(
        &'a self,
        state: MutexGuard<'a, State>,
        limit: Option<Duration>,
    ) -> MutexGuard<'a, State> {
        match limit {
            Some(limit) => {
                self.wake
                    .wait_timeout(state, limit)
                    .unwrap_or_else(|e| e.into_inner())
                    .0
            }
            None => self.wake.wait(state).unwrap_or_else(|e| e.into_inner()),
        }
    }

    /// Say whether any panel with page keys is driven. Without one, the
    /// controller is closed and not looked for.
    pub fn want(&self, wanted: bool) {
        let mut state = self.lock();
        if state.wanted == wanted {
            return;
        }
        state.wanted = wanted;
        if !wanted {
            state.device = None;
        }
        self.wake.notify_all();
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
        let fallback = (!self.hears_changes()).then_some(LOOK_EVERY);
        let mut state = self.lock();
        loop {
            let PageModifier::Button(button) = state.setting.clone() else {
                state = self.wake.wait(state).unwrap_or_else(|e| e.into_inner());
                continue;
            };
            if !state.wanted {
                state = self.wake.wait(state).unwrap_or_else(|e| e.into_inner());
                continue;
            }
            if state.device.is_some() {
                // Asked whether it is still there when a controller goes, so
                // an unplugged one is in the log before a page key is pressed
                // and does nothing. One read, a few microseconds.
                let seen = state.changed;
                state = self.sleep(state, fallback);
                if (state.changed != seen || fallback.is_some())
                    && state.device.as_ref().is_some_and(|d| d.pressed().is_err())
                {
                    state.device = None;
                }
                continue;
            }
            // Opened without the lock, so a press is never kept waiting on it.
            drop(state);
            let opened = Device::open(&button.device, &button.product);
            state = self.lock();
            if state.setting != PageModifier::Button(button.clone()) || !state.wanted {
                // Changed, or no longer wanted, while looking; start over.
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
                    // Looked for again when a controller arrives, and a few
                    // times just after, while DirectInput catches up.
                    let limit = if state.catching_up > 0 {
                        state.catching_up -= 1;
                        Some(CATCH_UP)
                    } else {
                        fallback
                    };
                    state = self.sleep(state, limit);
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
