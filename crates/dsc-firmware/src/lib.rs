//! The DscDevice firmware built for the PC, so tests drive the code a board
//! runs: its serial framing, its HID report handling over a TinyUSB stand-in,
//! and the message handling behind both. The sketch has three lamps: `FIRE`,
//! an indicator on pin 2; `BACKLIGHT`, a dimmer to 255 on pin 3; and `GAUGE`,
//! a dimmer to 100 on pin 4.
//!
//! Build with `DSC_ASAN=1` set to compile the firmware with AddressSanitizer,
//! so a read or write past any buffer crashes the test that caused it.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard};

extern "C" {
    fn dsc_serial_new() -> *mut c_void;
    fn dsc_serial_free(board: *mut c_void);
    fn dsc_serial_feed(board: *mut c_void, bytes: *const u8, len: usize);
    fn dsc_serial_take(board: *mut c_void, out: *mut u8, cap: usize) -> usize;
    fn dsc_pin(pin: u8) -> i32;
    fn dsc_hid_begin();
    fn dsc_hid_report(id: u8, data: *const u8, len: u16);
    fn dsc_hid_queue(id: u8, data: *const u8, len: u16);
    fn dsc_hid_poll();
    fn dsc_hid_link(up: bool);
    fn dsc_hid_take(out: *mut u8, cap: usize) -> usize;
}

pub const FIRE: u8 = 2;
pub const BACKLIGHT: u8 = 3;
pub const GAUGE: u8 = 4;

/// A pin's last value on this thread, `None` until the firmware wrote it.
pub fn pin(pin: u8) -> Option<u8> {
    u8::try_from(unsafe { dsc_pin(pin) }).ok()
}

/// A board on a serial port, freshly reset.
pub struct SerialBoard(*mut c_void);

// Only ever used by the thread holding it.
unsafe impl Send for SerialBoard {}

impl SerialBoard {
    #[allow(clippy::new_without_default)]
    pub fn new() -> SerialBoard {
        SerialBoard(unsafe { dsc_serial_new() })
    }

    /// Bytes down the wire, framed or not; the board handles what arrived.
    pub fn feed(&mut self, bytes: &[u8]) {
        unsafe { dsc_serial_feed(self.0, bytes.as_ptr(), bytes.len()) }
    }

    /// Everything the board has written since last asked.
    pub fn take(&mut self) -> Vec<u8> {
        let mut out = vec![0u8; 4096];
        let n = unsafe { dsc_serial_take(self.0, out.as_mut_ptr(), out.len()) };
        out.truncate(n);
        out
    }
}

impl Drop for SerialBoard {
    fn drop(&mut self) {
        unsafe { dsc_serial_free(self.0) }
    }
}

/// The HID board. There is one, as TinyUSB has one interface, so a test
/// holds it alone until done.
pub struct HidBoard {
    _alone: MutexGuard<'static, ()>,
}

static HID: Mutex<()> = Mutex::new(());

impl HidBoard {
    pub fn open() -> HidBoard {
        let alone = HID.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { dsc_hid_begin() };
        HidBoard { _alone: alone }
    }

    /// An output report as the USB stack hands it over: `id` 0 with the
    /// report ID still first, as from the OUT endpoint, or the ID apart, as
    /// from a control transfer. The board handles it at once.
    pub fn report(&mut self, id: u8, data: &[u8]) {
        let len = u16::try_from(data.len()).expect("a report fits in u16");
        unsafe { dsc_hid_report(id, data.as_ptr(), len) }
    }

    /// An output report taken by the USB stack while the sketch's `loop()`
    /// is busy elsewhere: queued, not yet handled.
    pub fn queue(&mut self, id: u8, data: &[u8]) {
        let len = u16::try_from(data.len()).expect("a report fits in u16");
        unsafe { dsc_hid_queue(id, data.as_ptr(), len) }
    }

    /// The sketch's `loop()` reaching `poll()`.
    pub fn poll(&mut self) {
        unsafe { dsc_hid_poll() }
    }

    /// The cable in or out, then one `poll()`.
    pub fn link(&mut self, up: bool) {
        unsafe { dsc_hid_link(up) }
    }

    /// The next input report the board sent, its ID first.
    pub fn take(&mut self) -> Option<Vec<u8>> {
        let mut out = vec![0u8; 256];
        let n = unsafe { dsc_hid_take(out.as_mut_ptr(), out.len()) };
        out.truncate(n);
        (n > 0).then_some(out)
    }
}
