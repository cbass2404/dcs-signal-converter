//! Game controller buttons, read through DirectInput as DCS reads them.
//!
//! A button here has the number DCS shows for it, so JOY_BTN12 in DCS's
//! controls is button 12 here, because DCS asks DirectInput too. A controller
//! is known by the instance GUID Windows gives it, the same one DCS names its
//! binding files after, so two identical sticks are told apart as DCS tells
//! them apart.
//!
//! Nothing is read in between: a button's state is asked for at the moment it
//! matters, as the keyboard's Ctrl is, and anything that waits for a button
//! sleeps until a controller sends something ([`wait_any`]). Opening a controller reads it shared,
//! in the background, so DCS and anything else reading it see every press as
//! before, whichever window has focus.

use std::time::{Duration, Instant};

/// DCS reads at most this many buttons on a controller, and so does this.
pub const BUTTONS: usize = 128;

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

/// Something another thread sets to end a [`wait_any`] early, such as the
/// window's Cancel while it waits for a button.
pub struct Cancel {
    signal: imp::Signal,
}

impl Cancel {
    pub fn new() -> Result<Cancel> {
        Ok(Cancel {
            signal: imp::Signal::new(true)?,
        })
    }

    /// End the wait, and any wait after it until [`reset`](Cancel::reset).
    pub fn set(&self) {
        self.signal.set();
    }

    pub fn reset(&self) {
        self.signal.reset();
    }
}

/// Why [`wait_any`] returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Woke {
    /// The controller at this index in the list changed: a button, an axis,
    /// anything it reports. Its buttons are worth reading again.
    Changed(usize),
    Cancelled,
    TimedOut,
}

/// Sleep until one of `devices` changes, `cancel` is set, or `timeout`
/// passes, whichever is first. With no timeout it waits as long as it takes.
///
/// Nothing is read while it waits: Windows wakes it when a controller sends
/// something. A rare old controller that only answers when asked is asked
/// every [`POLLED_EVERY`] instead, and only while one of those is in the list.
pub fn wait_any(
    devices: &[Device],
    cancel: Option<&Cancel>,
    timeout: Option<Duration>,
) -> Result<Woke> {
    let inner: Vec<&imp::Device> = devices.iter().map(|d| &d.inner).collect();
    imp::wait_any(&inner, cancel.map(|c| &c.signal), timeout)
}

/// How often a controller that cannot say it changed is asked, while
/// [`wait_any`] waits on one.
pub const POLLED_EVERY: Duration = Duration::from_millis(15);

/// Wait for a button to go down on any controller, and say which.
///
/// A button already held when this starts counts only once it is let go and
/// pressed again, so a switch resting on a position cannot answer for the
/// user. Gives up after `within`, or as soon as `cancel` is set, with `None`.
pub fn capture(within: Duration, cancel: &Cancel) -> Result<Option<Pressed>> {
    let (mut devices, _) = open_all()?;
    let until = Instant::now() + within;
    // A controller just opened reads as nothing held for a moment, so what is
    // held to start with is read once it has settled.
    if wait_any(&[], Some(cancel), Some(SETTLE))? == Woke::Cancelled {
        return Ok(None);
    }
    let mut held: Vec<Vec<u16>> = devices
        .iter()
        .map(|d| d.pressed().unwrap_or_default())
        .collect();
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(None);
        }
        let i = match wait_any(&devices, Some(cancel), Some(left))? {
            Woke::Changed(i) => i,
            Woke::Cancelled | Woke::TimedOut => return Ok(None),
        };
        let Ok(now) = devices[i].pressed() else {
            // Unplugged while waiting; the others can still answer.
            devices.remove(i);
            held.remove(i);
            continue;
        };
        if let Some(&button) = now.iter().find(|b| !held[i].contains(b)) {
            return Ok(Some(Pressed {
                controller: devices[i].controller.clone(),
                button,
            }));
        }
        held[i] = now;
    }
}

/// Call `changed` whenever a HID device arrives or leaves, for as long as the
/// returned value is kept. Every controller is one, so a controller plugged in
/// or pulled out is heard about as it happens rather than looked for.
pub fn on_device_change(changed: Box<dyn Fn() + Send + Sync>) -> Result<DeviceChanges> {
    Ok(DeviceChanges {
        _inner: imp::DeviceChanges::register(changed)?,
    })
}

/// A registration from [`on_device_change`]; dropping it stops the calls.
pub struct DeviceChanges {
    _inner: imp::DeviceChanges,
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    use std::time::{Duration, Instant};
    use windows::core::{IUnknown, Interface, BOOL, GUID};

    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_Register_Notification, CM_Unregister_Notification, CM_NOTIFY_ACTION,
        CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL, CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL,
        CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE, CR_SUCCESS,
        HCMNOTIFICATION,
    };
    use windows::Win32::Devices::HumanInterfaceDevice::{
        DirectInput8Create, IDirectInput8W, IDirectInputDevice8W, DI8DEVCLASS_GAMECTRL,
        DIDATAFORMAT, DIDC_POLLEDDATAFORMAT, DIDC_POLLEDDEVICE, DIDEVCAPS, DIDEVICEINSTANCEW,
        DIDFT_ANYINSTANCE, DIDFT_BUTTON, DIDF_ABSAXIS, DIEDFL_ATTACHEDONLY, DIENUM_CONTINUE,
        DIERR_INPUTLOST, DIERR_NOTACQUIRED, DIOBJECTDATAFORMAT, DIRECTINPUT_VERSION,
        DISCL_BACKGROUND, DISCL_NONEXCLUSIVE, GUID_DEVINTERFACE_HID,
    };
    use windows::Win32::Foundation::{
        CloseHandle, HANDLE, HINSTANCE, HWND, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::{
        CreateEventW, ResetEvent, SetEvent, WaitForMultipleObjects, INFINITE,
    };

    use super::{Controller, Error, Result, Woke, BUTTONS};

    /// How many handles one wait can take, a Windows limit.
    const MAXIMUM_WAIT_OBJECTS: u32 = 64;

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
        /// Set by DirectInput whenever the controller sends something.
        changed: Signal,
        /// Whether it only reports when asked, so its event is set only then.
        polled: bool,
    }

    impl Drop for Device {
        fn drop(&mut self) {
            // Told to stop setting the event before the event is closed.
            unsafe {
                let _ = self.device.Unacquire();
                let _ = self.device.SetEventNotification(HANDLE::default());
            }
        }
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
            // Its event is set whenever it sends anything, so a wait can sleep
            // until then. Set before acquiring, as DirectInput requires.
            let changed = Signal::new(false)?;
            unsafe { device.SetEventNotification(changed.0) }
                .map_err(|e| windows("asking the controller to signal", e))?;
            let mut caps: DIDEVCAPS = unsafe { std::mem::zeroed() };
            caps.dwSize = std::mem::size_of::<DIDEVCAPS>() as u32;
            let polled = unsafe { device.GetCapabilities(&mut caps) }.is_ok()
                && caps.dwFlags & (DIDC_POLLEDDEVICE | DIDC_POLLEDDATAFORMAT) != 0;
            // Acquired again on the first read if this fails.
            let _ = unsafe { device.Acquire() };
            Ok(Device {
                device,
                _input: input,
                changed,
                polled,
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

    /// A Windows event: something a thread sleeps on until another sets it.
    pub struct Signal(HANDLE);

    // An event handle may be set and waited on from any thread.
    unsafe impl Send for Signal {}
    unsafe impl Sync for Signal {}

    impl Signal {
        /// `manual`: stays set until reset, rather than clearing as the one
        /// waiting on it wakes.
        pub fn new(manual: bool) -> Result<Signal> {
            unsafe { CreateEventW(None, manual, false, PCWSTR::null()) }
                .map(Signal)
                .map_err(|e| windows("making an event", e))
        }

        pub fn set(&self) {
            let _ = unsafe { SetEvent(self.0) };
        }

        pub fn reset(&self) {
            let _ = unsafe { ResetEvent(self.0) };
        }
    }

    impl Drop for Signal {
        fn drop(&mut self) {
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    /// The most handles one wait can take, a Windows limit, less one for the
    /// cancel.
    const MOST: usize = MAXIMUM_WAIT_OBJECTS as usize - 1;

    pub fn wait_any(
        devices: &[&Device],
        cancel: Option<&Signal>,
        timeout: Option<Duration>,
    ) -> Result<Woke> {
        let devices = &devices[..devices.len().min(MOST)];
        let mut handles: Vec<HANDLE> = devices.iter().map(|d| d.changed.0).collect();
        if let Some(c) = cancel {
            handles.push(c.0);
        }
        let polled: Vec<&&Device> = devices.iter().filter(|d| d.polled).collect();
        let until = timeout.map(|t| Instant::now() + t);
        loop {
            // A polled controller only sets its event when asked, so it is
            // asked first, and the wait is cut short to ask it again.
            for d in &polled {
                let _ = unsafe { d.device.Poll() };
            }
            let left = until.map(|u| u.saturating_duration_since(Instant::now()));
            let slice = match (left, polled.is_empty()) {
                (Some(left), true) => left,
                (Some(left), false) => left.min(super::POLLED_EVERY),
                (None, true) => Duration::MAX,
                (None, false) => super::POLLED_EVERY,
            };
            let ms = if slice == Duration::MAX {
                INFINITE
            } else {
                u32::try_from(slice.as_millis()).unwrap_or(INFINITE - 1)
            };
            if handles.is_empty() {
                std::thread::sleep(slice);
                return Ok(Woke::TimedOut);
            }
            let woke = unsafe { WaitForMultipleObjects(&handles, false, ms) };
            if woke == WAIT_TIMEOUT {
                if left.is_some_and(|l| l <= slice) {
                    return Ok(Woke::TimedOut);
                }
                continue;
            }
            if woke == WAIT_FAILED {
                return Err(Error::Windows(format!(
                    "waiting on the controllers: {}",
                    std::io::Error::last_os_error()
                )));
            }
            let at = woke.0.wrapping_sub(WAIT_OBJECT_0.0) as usize;
            return Ok(if at < devices.len() {
                Woke::Changed(at)
            } else {
                Woke::Cancelled
            });
        }
    }

    /// A registration with Windows for HID devices coming and going.
    pub struct DeviceChanges {
        handle: HCMNOTIFICATION,
        /// The callback, held where Windows was told to find it.
        changed: *mut Box<dyn Fn() + Send + Sync>,
    }

    // Only dropped, which unregisters before the callback goes.
    unsafe impl Send for DeviceChanges {}
    unsafe impl Sync for DeviceChanges {}

    unsafe extern "system" fn heard(
        _handle: HCMNOTIFICATION,
        context: *const c_void,
        action: CM_NOTIFY_ACTION,
        _data: *const CM_NOTIFY_EVENT_DATA,
        _size: u32,
    ) -> u32 {
        if action == CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL
            || action == CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL
        {
            let changed = &*(context as *const Box<dyn Fn() + Send + Sync>);
            changed();
        }
        0
    }

    impl DeviceChanges {
        pub fn register(changed: Box<dyn Fn() + Send + Sync>) -> Result<DeviceChanges> {
            let changed = Box::into_raw(Box::new(changed));
            let mut filter = CM_NOTIFY_FILTER {
                cbSize: std::mem::size_of::<CM_NOTIFY_FILTER>() as u32,
                FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
                ..Default::default()
            };
            filter.u.DeviceInterface.ClassGuid = GUID_DEVINTERFACE_HID;
            let mut handle = HCMNOTIFICATION::default();
            let result = unsafe {
                CM_Register_Notification(
                    &filter,
                    Some(changed as *const c_void),
                    Some(heard),
                    &mut handle,
                )
            };
            if result != CR_SUCCESS {
                drop(unsafe { Box::from_raw(changed) });
                return Err(Error::Windows(format!(
                    "asking to hear of controllers coming and going: error {}",
                    result.0
                )));
            }
            Ok(DeviceChanges { handle, changed })
        }
    }

    impl Drop for DeviceChanges {
        fn drop(&mut self) {
            // Waits for a callback under way, so the box is free to go after.
            unsafe { CM_Unregister_Notification(self.handle) };
            drop(unsafe { Box::from_raw(self.changed) });
        }
    }
}

/// DirectInput is Windows' alone, so elsewhere there is nothing to read.
#[cfg(not(windows))]
mod imp {
    use std::time::Duration;

    use super::{Controller, Error, Result, Woke, BUTTONS};

    pub fn list() -> Result<Vec<Controller>> {
        Ok(Vec::new())
    }

    pub struct Signal;

    impl Signal {
        pub fn new(_manual: bool) -> Result<Signal> {
            Ok(Signal)
        }
        pub fn set(&self) {}
        pub fn reset(&self) {}
    }

    pub fn wait_any(
        _devices: &[&Device],
        _cancel: Option<&Signal>,
        timeout: Option<Duration>,
    ) -> Result<Woke> {
        std::thread::sleep(timeout.unwrap_or(Duration::from_secs(3600)));
        Ok(Woke::TimedOut)
    }

    pub struct DeviceChanges;

    impl DeviceChanges {
        pub fn register(_changed: Box<dyn Fn() + Send + Sync>) -> Result<DeviceChanges> {
            Ok(DeviceChanges)
        }
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
