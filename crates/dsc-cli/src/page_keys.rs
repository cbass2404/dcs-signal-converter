//! Page keys, read while the converter runs.
//!
//! Every connected panel that lists page keys in `devices.json` gets a thread
//! reading its buttons, and each key going down arrives on one channel with
//! whether the page modifier was held at that moment. What a key means is
//! decided by the caller, from the device's own `page_keys`, so nothing here
//! knows a slot or an MCDU.
//!
//! Only reads. Windows gives every open handle its own copy of each input
//! report, so DCS and SimAppPro see every press as before.
//!
//! A web device has no keys of its own, so its page keys are the keyboard's
//! digits: key n is the digit n, in the row above the letters. They come by
//! Raw Input, a copy of each keystroke whichever window has focus, so a page
//! swaps without leaving DCS and DCS still sees the key.
//!
//! **A reader runs only while its device is driven.** The main loop says
//! which are, through [`Driven`], as aircraft and profiles change. A panel's
//! reader closes its handle until then, and the keyboard is not listened to
//! at all, so a panel a profile leaves alone, or every panel while no
//! aircraft is loaded, costs nothing.

use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use dsc_config::DeviceInventory;

#[cfg(windows)]
use crate::buttons;
use crate::modifier::Watch;

pub enum KeyEvent {
    /// A button went down on a device, and whether the page modifier, and
    /// only it, was held at the time.
    Down {
        device: String,
        number: u16,
        modifier: bool,
    },
    /// A reader stopped, most likely because the panel was unplugged. Its
    /// keys do nothing until the converter starts again.
    Lost { device: String, why: String },
}

/// The devices the active profile drives, for the readers to wait on.
#[derive(Default)]
pub struct Driven {
    state: Mutex<DrivenState>,
    changed: Condvar,
}

#[derive(Default)]
struct DrivenState {
    keys: BTreeSet<String>,
    /// Told of every change, for a reader that waits on something other
    /// than the condvar, as the keyboard's waits on its messages.
    wakers: Vec<Box<dyn Fn() + Send>>,
}

impl Driven {
    fn lock(&self) -> MutexGuard<'_, DrivenState> {
        // A reader that panicked holding it left nothing half done.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Say which devices are driven now.
    pub fn set(&self, keys: BTreeSet<String>) {
        let mut state = self.lock();
        if state.keys == keys {
            return;
        }
        state.keys = keys;
        for wake in &state.wakers {
            wake();
        }
        drop(state);
        self.changed.notify_all();
    }

    pub fn is(&self, key: &str) -> bool {
        self.lock().keys.contains(key)
    }

    /// Wait until `key` is driven.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn wait_for(&self, key: &str) {
        let mut state = self.lock();
        while !state.keys.contains(key) {
            state = self.changed.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn on_change(&self, wake: Box<dyn Fn() + Send>) {
        self.lock().wakers.push(wake);
    }
}

/// Keys are read through Windows' own HID parser, so elsewhere there are none.
#[cfg(not(windows))]
pub fn start(
    _inventory: &DeviceInventory,
    _connected: &[String],
    _modifier: &Arc<Watch>,
    _driven: &Arc<Driven>,
) -> (Receiver<KeyEvent>, Vec<String>) {
    (mpsc::channel().1, Vec::new())
}

/// Start a reader for each connected device with page keys. Returns the
/// channel keys arrive on and a line per device for the log.
#[cfg(windows)]
pub fn start(
    inventory: &DeviceInventory,
    connected: &[String],
    modifier: &Arc<Watch>,
    driven: &Arc<Driven>,
) -> (Receiver<KeyEvent>, Vec<String>) {
    let (tx, rx) = mpsc::channel();
    let mut lines = Vec::new();
    start_digits(inventory, connected, modifier, driven, &tx, &mut lines);
    let with_keys: Vec<_> = inventory
        .devices
        .iter()
        // Only panels whose keys arrive over HID; another protocol's keys,
        // such as the web view's, come some other way.
        .filter(|d| d.protocol == dsc_config::DEFAULT_PROTOCOL)
        .filter(|d| !d.page_keys.is_empty() && connected.contains(&d.key))
        .collect();
    if with_keys.is_empty() {
        return (rx, lines);
    }
    let api = match hidapi::HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            lines.push(format!(
                "keys     could not list the panels to read page keys: {e}"
            ));
            return (rx, lines);
        }
    };
    let found = wctrl_hid::enumerate(&api);
    for spec in with_keys {
        // The collection that declares buttons: on the MCDU the game
        // controller, one of several interfaces under the one PID. Its path
        // is kept, to open it again whenever the panel is driven again.
        let opened = found
            .iter()
            .filter(|d| d.product_id == spec.usb_pid)
            .filter_map(|d| {
                buttons::Collection::open(&d.path)
                    .ok()
                    .map(|c| (d.path.clone(), c))
            })
            .find(|(_, c)| !c.buttons.is_empty());
        let Some((path, collection)) = opened else {
            lines.push(format!(
                "keys     {}: no collection declares buttons, so its page keys do nothing",
                spec.key
            ));
            continue;
        };
        lines.push(format!(
            "keys     {}: reading {} page key(s) on usage page 0x{:04x} usage 0x{:04x} while driven",
            spec.key,
            spec.page_keys.len(),
            collection.usage_page,
            collection.usage
        ));
        let tx = tx.clone();
        let device = spec.key.clone();
        let modifier = Arc::clone(modifier);
        let driven = Arc::clone(driven);
        // Named so a profiler, or tools/bench_daemon.py, can tell the
        // readers' cost from the main loop's.
        let spawned = std::thread::Builder::new()
            .name(format!("keys {device}"))
            .spawn(move || {
                let mut collection = Some(collection);
                let mut buf = Vec::new();
                let mut report: Vec<u8> = Vec::new();
                let mut last: Vec<u16> = Vec::new();
                // The first report from a newly opened handle sets which keys
                // are already down, rather than reading them as pressed.
                let mut fresh = true;
                loop {
                    if !driven.is(&device) {
                        // Closed, not just left unread: nothing reads it, and
                        // nothing buffered is waiting when it comes back.
                        collection = None;
                        driven.wait_for(&device);
                    }
                    if collection.is_none() {
                        match buttons::Collection::open(&path) {
                            Ok(c) => collection = Some(c),
                            Err(e) => {
                                let _ = tx.send(KeyEvent::Lost {
                                    device,
                                    why: e.to_string(),
                                });
                                return;
                            }
                        }
                        report.clear();
                        fresh = true;
                    }
                    let c = collection.as_ref().expect("opened above");
                    if let Err(e) = c.read(&mut buf) {
                        let _ = tx.send(KeyEvent::Lost {
                            device,
                            why: e.to_string(),
                        });
                        return;
                    }
                    // The panels send 100 reports a second whether or not
                    // anything moved. One the same as the last holds the same
                    // keys, so it can put none down and is not parsed.
                    if buf == report {
                        continue;
                    }
                    report.clear();
                    report.extend_from_slice(&buf);
                    // A report with no buttons in it leaves them as they were.
                    // Acting only on keys going down, never on the report
                    // changing, is what keeps the MCDU's restless bytes 17 to 24
                    // from reading as presses.
                    let Some(down) = c.pressed(&buf) else {
                        continue;
                    };
                    if fresh {
                        fresh = false;
                        last = down;
                        continue;
                    }
                    let new: Vec<u16> =
                        down.iter().copied().filter(|b| !last.contains(b)).collect();
                    if !new.is_empty() {
                        let held = modifier.held();
                        for number in new {
                            let event = KeyEvent::Down {
                                device: device.clone(),
                                number,
                                modifier: held,
                            };
                            if tx.send(event).is_err() {
                                return;
                            }
                        }
                    }
                    last = down;
                }
            });
        if let Err(e) = spawned {
            lines.push(format!(
                "keys     {}: could not start its reader: {e}",
                spec.key
            ));
        }
    }
    (rx, lines)
}

/// One reader for every connected web device's page keys, which are digits.
#[cfg(windows)]
fn start_digits(
    inventory: &DeviceInventory,
    connected: &[String],
    modifier: &Arc<Watch>,
    driven: &Arc<Driven>,
    tx: &mpsc::Sender<KeyEvent>,
    lines: &mut Vec<String>,
) {
    use crate::keyboard::{Keyboard, Typed};

    // Each device with the button numbers of its page keys.
    let devices: Vec<(String, Vec<u16>)> = inventory
        .devices
        .iter()
        .filter(|d| d.protocol == dsc_config::WEB_PROTOCOL)
        .filter(|d| !d.page_keys.is_empty() && connected.contains(&d.key))
        .map(|d| {
            let numbers = d
                .page_keys
                .iter()
                .filter_map(|k| d.button(k))
                .map(|b| b.number)
                .filter(|n| *n <= 9)
                .collect();
            (d.key.clone(), numbers)
        })
        .collect();
    if devices.is_empty() {
        return;
    }
    for (device, numbers) in &devices {
        lines.push(format!(
            "keys     {device}: reading the keyboard's digits {numbers:?} as its page keys while driven"
        ));
    }
    let tx = tx.clone();
    let modifier = Arc::clone(modifier);
    let driven = Arc::clone(driven);
    let spawned = std::thread::Builder::new()
        .name("keys digits".into())
        .spawn(move || {
            let lost = |why: String| {
                for (device, _) in &devices {
                    let _ = tx.send(KeyEvent::Lost {
                        device: device.clone(),
                        why: why.clone(),
                    });
                }
            };
            let keyboard = match Keyboard::new() {
                Ok(k) => k,
                Err(e) => return lost(e.to_string()),
            };
            driven.on_change(Box::new(keyboard.waker()));
            let mut listening = false;
            // Digits down now, so a key held and repeating counts once.
            let mut held: Vec<u16> = Vec::new();
            loop {
                let wanted = devices.iter().any(|(d, _)| driven.is(d));
                if wanted != listening {
                    if let Err(e) = keyboard.listen(wanted) {
                        return lost(e.to_string());
                    }
                    listening = wanted;
                    held.clear();
                }
                let (digit, down) = match keyboard.next() {
                    Typed::Key { vkey, down }
                        if (u16::from(b'0')..=u16::from(b'9')).contains(&vkey) =>
                    {
                        (vkey - u16::from(b'0'), down)
                    }
                    Typed::Key { .. } | Typed::Other => continue,
                    Typed::Closed => return,
                };
                if !down {
                    held.retain(|n| *n != digit);
                    continue;
                }
                if held.contains(&digit) {
                    continue;
                }
                held.push(digit);
                let modifier = modifier.held();
                for (device, numbers) in &devices {
                    if !numbers.contains(&digit) || !driven.is(device) {
                        continue;
                    }
                    let event = KeyEvent::Down {
                        device: device.clone(),
                        number: digit,
                        modifier,
                    };
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
        });
    if let Err(e) = spawned {
        lines.push(format!("keys     could not start reading the digits: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::Driven;
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    fn keys(k: &[&str]) -> BTreeSet<String> {
        k.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_reader_waits_until_its_device_is_driven() {
        let driven = Arc::new(Driven::default());
        let theirs = Arc::clone(&driven);
        let reader = std::thread::spawn(move || theirs.wait_for("CDU_Kneeboard"));
        std::thread::sleep(Duration::from_millis(50));
        assert!(!reader.is_finished(), "returned before it was driven");
        driven.set(keys(&["MCDU_Captain"]));
        std::thread::sleep(Duration::from_millis(50));
        assert!(!reader.is_finished(), "another device woke it");
        driven.set(keys(&["MCDU_Captain", "CDU_Kneeboard"]));
        reader.join().unwrap();
    }

    #[test]
    fn wakers_hear_each_change_once() {
        let driven = Driven::default();
        let woken = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&woken);
        driven.on_change(Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }));
        driven.set(keys(&["CDU_Kneeboard"]));
        driven.set(keys(&["CDU_Kneeboard"]));
        driven.set(keys(&[]));
        assert_eq!(woken.load(Ordering::SeqCst), 2);
        assert!(!driven.is("CDU_Kneeboard"));
    }
}
