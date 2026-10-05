//! Game controller buttons, read through DirectInput as DCS reads them.
//!
//! A button here has the number DCS shows for it, so JOY_BTN12 in DCS's
//! controls is button 12 here, because DCS asks DirectInput too. A controller
//! is known by the instance GUID Windows gives it, the same one DCS names its
//! binding files after, so two identical sticks are told apart as DCS tells
//! them apart.
//!
//! Nothing is read in between: a button's state is asked for at the moment it
//! matters, as the keyboard's Ctrl is. Opening a controller reads it shared,
//! in the background, so DCS and anything else reading it see every press as
//! before, whichever window has focus.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// DCS reads at most this many buttons on a controller, and so does this.
pub const BUTTONS: usize = 128;

/// How often [`capture`] looks at every controller.
const CAPTURE_EVERY: Duration = Duration::from_millis(15);

/// How long a controller just opened can read as nothing held. Its first
/// reads come back empty, and a switch resting on a position would otherwise
/// read as pressed a moment later.
pub const SETTLE: Duration = Duration::from_millis(100);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not connected")]
    NotFound,
    /// It was open, and has gone: most likely unplugged.
    #[error("stopped answering ({0})")]
    Lost(String),
    #[error("{0}")]
    Windows(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// One controller as Windows lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Controller {
    /// This one controller on this PC.
    pub instance: String,
    /// Its make and model, shared by every controller of the same kind.
    pub product: String,
    /// What it calls itself.
    pub name: String,
}

/// A button that went down during [`capture`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pressed {
    pub controller: Controller,
    /// Numbered from 1, as DCS numbers it.
    pub button: u16,
}

/// Every game controller attached now. Panels too: DirectInput sees a
/// WinCtrl panel's keys as a game controller's buttons, as DCS does.
pub fn list() -> Result<Vec<Controller>> {
    imp::list()
}

/// An open controller, asked for its buttons when wanted.
pub struct Device {
    controller: Controller,
    inner: imp::Device,
}

impl Device {
    /// Open the controller with this instance. If Windows no longer knows it,
    /// which it can do after the controller moves to another USB port, the
    /// one attached controller of the same product stands in. With two of
    /// that product attached, neither is guessed at.
    pub fn open(instance: &str, product: &str) -> Result<Device> {
        let all = list()?;
        let found = all.iter().find(|c| c.instance == instance).or_else(|| {
            let mut same = all.iter().filter(|c| c.product == product);
            match (same.next(), same.next()) {
                (Some(only), None) => Some(only),
                _ => None,
            }
        });
        let Some(controller) = found else {
            return Err(Error::NotFound);
        };
        Device::open_listed(controller.clone())
    }

    fn open_listed(controller: Controller) -> Result<Device> {
        let inner = imp::Device::open(&controller.instance)?;
        Ok(Device { controller, inner })
    }

    pub fn controller(&self) -> &Controller {
        &self.controller
    }

    /// Every button down now, numbered from 1.
    pub fn pressed(&self) -> Result<Vec<u16>> {
        let state = self.inner.state()?;
        Ok(state
            .iter()
            .enumerate()
            .filter(|(_, b)| **b & 0x80 != 0)
            .map(|(i, _)| i as u16 + 1)
            .collect())
    }

    /// Whether one button is down now. A number the controller does not have
    /// is never down.
    pub fn is_down(&self, button: u16) -> Result<bool> {
        let state = self.inner.state()?;
        Ok(button >= 1
            && state
                .get(button as usize - 1)
                .is_some_and(|b| b & 0x80 != 0))
    }
}

/// The controllers that opened, and those that would not with the reason.
pub type Opened = (Vec<Device>, Vec<(Controller, Error)>);

/// Every controller attached now, opened. Those that would not open are
/// returned with the reason, for a caller that wants to say so.
pub fn open_all() -> Result<Opened> {
    let mut open = Vec::new();
    let mut failed = Vec::new();
    for controller in list()? {
        match Device::open_listed(controller.clone()) {
            Ok(d) => open.push(d),
            Err(e) => failed.push((controller, e)),
        }
    }
    Ok((open, failed))
}

/// Wait for a button to go down on any controller, and say which.
///
/// A button already held when this starts counts only once it is let go and
/// pressed again, so a switch resting on a position cannot answer for the
/// user. Gives up after `within`, or as soon as `cancel` is set, with `None`.
pub fn capture(within: Duration, cancel: &AtomicBool) -> Result<Option<Pressed>> {
    let (mut devices, _) = open_all()?;
    let mut held: Vec<Vec<u16>> = vec![Vec::new(); devices.len()];
    let start = Instant::now();
    let until = start + within;
    while Instant::now() < until && !cancel.load(Ordering::Relaxed) {
        std::thread::sleep(CAPTURE_EVERY);
        let settling = start.elapsed() < SETTLE;
        let mut i = 0;
        while i < devices.len() {
            let Ok(now) = devices[i].pressed() else {
                // Unplugged while waiting; the others can still answer.
                devices.remove(i);
                held.remove(i);
                continue;
            };
            if settling {
                // Whatever is down now was down before anyone was asked.
                for b in now {
                    if !held[i].contains(&b) {
                        held[i].push(b);
                    }
                }
                i += 1;
                continue;
            }
            if let Some(&button) = now.iter().find(|b| !held[i].contains(b)) {
                return Ok(Some(Pressed {
                    controller: devices[i].controller.clone(),
                    button,
                }));
            }
            held[i] = now;
            i += 1;
        }
    }
    Ok(None)
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    use windows::core::{IUnknown, Interface, BOOL, GUID};
    use windows::Win32::Devices::HumanInterfaceDevice::{
        DirectInput8Create, IDirectInput8W, IDirectInputDevice8W, DI8DEVCLASS_GAMECTRL,
        DIDATAFORMAT, DIDEVICEINSTANCEW, DIDFT_ANYINSTANCE, DIDFT_BUTTON, DIDF_ABSAXIS,
        DIEDFL_ATTACHEDONLY, DIENUM_CONTINUE, DIERR_INPUTLOST, DIERR_NOTACQUIRED,
        DIOBJECTDATAFORMAT, DIRECTINPUT_VERSION, DISCL_BACKGROUND, DISCL_NONEXCLUSIVE,
    };
    use windows::Win32::Foundation::{HINSTANCE, HWND};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;

    use super::{Controller, Error, Result, BUTTONS};

    /// Missing from the bindings: a format object the device may not have.
    const DIDFT_OPTIONAL: u32 = 0x8000_0000;

    fn windows(context: &str, e: windows::core::Error) -> Error {
        Error::Windows(format!("{context}: {e}"))
    }

    fn input() -> Result<IDirectInput8W> {
        let module = unsafe { GetModuleHandleW(None) }.map_err(|e| windows("DirectInput", e))?;
        let mut out: *mut c_void = std::ptr::null_mut();
        unsafe {
            DirectInput8Create(
                HINSTANCE(module.0),
                DIRECTINPUT_VERSION,
                &IDirectInput8W::IID,
                &mut out,
                None::<&IUnknown>,
            )
        }
        .map_err(|e| windows("DirectInput", e))?;
        Ok(unsafe { IDirectInput8W::from_raw(out) })
    }

    /// A GUID as text, the way Windows writes one without the braces.
    fn guid_text(g: &GUID) -> String {
        format!("{g:?}")
    }

    fn guid_from(text: &str) -> Option<GUID> {
        let hex: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if hex.len() != 32 {
            return None;
        }
        u128::from_str_radix(&hex, 16).ok().map(GUID::from_u128)
    }

    fn wide(text: &[u16]) -> String {
        let end = text.iter().position(|&c| c == 0).unwrap_or(text.len());
        String::from_utf16_lossy(&text[..end]).trim().to_string()
    }

    unsafe extern "system" fn each(found: *mut DIDEVICEINSTANCEW, list: *mut c_void) -> BOOL {
        let list = &mut *(list as *mut Vec<Controller>);
        let found = &*found;
        list.push(Controller {
            instance: guid_text(&found.guidInstance),
            product: guid_text(&found.guidProduct),
            name: wide(&found.tszProductName),
        });
        BOOL(DIENUM_CONTINUE as i32)
    }

    pub fn list() -> Result<Vec<Controller>> {
        let input = input()?;
        let mut list: Vec<Controller> = Vec::new();
        unsafe {
            input.EnumDevices(
                DI8DEVCLASS_GAMECTRL,
                Some(each),
                &mut list as *mut _ as *mut c_void,
                DIEDFL_ATTACHEDONLY,
            )
        }
        .map_err(|e| windows("listing the controllers", e))?;
        Ok(list)
    }

    pub struct Device {
        device: IDirectInputDevice8W,
        // Kept for as long as the device it made.
        _input: IDirectInput8W,
    }

    // DirectInput is not tied to the thread that opened it. Every caller holds
    // a device behind a lock, so it is never asked from two threads at once.
    unsafe impl Send for Device {}

    impl Device {
        pub fn open(instance: &str) -> Result<Device> {
            let guid = guid_from(instance).ok_or(Error::NotFound)?;
            let input = input()?;
            let mut device: Option<IDirectInputDevice8W> = None;
            unsafe { input.CreateDevice(&guid, &mut device, None::<&IUnknown>) }
                .map_err(|_| Error::NotFound)?;
            let device = device.ok_or(Error::NotFound)?;

            // Buttons only, in order, as DCS's own format reads them. Each
            // entry takes the next button the device has, and one it lacks is
            // left at zero. DirectInput copies the format, so it can go after.
            let mut objects: Vec<DIOBJECTDATAFORMAT> = (0..BUTTONS)
                .map(|i| DIOBJECTDATAFORMAT {
                    pguid: std::ptr::null(),
                    dwOfs: i as u32,
                    dwType: DIDFT_BUTTON | DIDFT_ANYINSTANCE | DIDFT_OPTIONAL,
                    dwFlags: 0,
                })
                .collect();
            let mut format = DIDATAFORMAT {
                dwSize: std::mem::size_of::<DIDATAFORMAT>() as u32,
                dwObjSize: std::mem::size_of::<DIOBJECTDATAFORMAT>() as u32,
                dwFlags: DIDF_ABSAXIS,
                dwDataSize: BUTTONS as u32,
                dwNumObjs: BUTTONS as u32,
                rgodf: objects.as_mut_ptr(),
            };
            unsafe { device.SetDataFormat(&mut format) }
                .map_err(|e| windows("setting the controller's format", e))?;
            // Shared, and read whichever window has focus. No window of ours
            // to name, and DirectInput asks for none in the background.
            unsafe {
                device.SetCooperativeLevel(HWND::default(), DISCL_BACKGROUND | DISCL_NONEXCLUSIVE)
            }
            .map_err(|e| windows("sharing the controller", e))?;
            // Acquired again on the first read if this fails.
            let _ = unsafe { device.Acquire() };
            Ok(Device {
                device,
                _input: input,
            })
        }

        pub fn state(&self) -> Result<[u8; BUTTONS]> {
            let mut state = [0u8; BUTTONS];
            let mut read = || unsafe {
                // Most controllers answer without it; those that do not need it.
                let _ = self.device.Poll();
                self.device
                    .GetDeviceState(BUTTONS as u32, state.as_mut_ptr().cast())
            };
            match read() {
                Ok(()) => {}
                Err(e) if e.code() == DIERR_INPUTLOST || e.code() == DIERR_NOTACQUIRED => {
                    unsafe { self.device.Acquire() }.map_err(|e| Error::Lost(e.to_string()))?;
                    read().map_err(|e| Error::Lost(e.to_string()))?;
                }
                Err(e) => return Err(Error::Lost(e.to_string())),
            }
            Ok(state)
        }
    }
}

/// DirectInput is Windows' alone, so elsewhere there is nothing to read.
#[cfg(not(windows))]
mod imp {
    use super::{Controller, Error, Result, BUTTONS};

    pub fn list() -> Result<Vec<Controller>> {
        Ok(Vec::new())
    }

    pub struct Device;

    impl Device {
        pub fn open(_instance: &str) -> Result<Device> {
            Err(Error::NotFound)
        }

        pub fn state(&self) -> Result<[u8; BUTTONS]> {
            Err(Error::NotFound)
        }
    }
}
