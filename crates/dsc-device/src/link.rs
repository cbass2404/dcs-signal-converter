//! An open board: messages out, messages back, over serial or HID.

use std::collections::VecDeque;
use std::ffi::CString;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

use crate::wire::{self, Deframer};

/// One board, opened. What it is, and whether it is ours, is the scan's
/// business; a link only carries messages.
pub trait Link: Send {
    fn send(&mut self, message: &[u8]) -> io::Result<()>;

    /// The next whole message, waiting up to `timeout` for one. `None` when
    /// none came. A zero timeout only looks.
    fn receive(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>>;
}

// ------------------------------------------------------------------ serial

/// A board on a COM port.
pub struct SerialLink {
    port: Box<dyn serialport::SerialPort>,
    deframer: Deframer,
    ready: VecDeque<Vec<u8>>,
}

impl SerialLink {
    /// Open `name`, such as `COM5`. Most boards reset when their port opens
    /// and take a moment to answer; see [`crate::session::SERIAL_WAIT`].
    pub fn open(name: &str) -> io::Result<SerialLink> {
        let port = serialport::new(name, wire::BAUD)
            .timeout(Duration::from_millis(10))
            .open()
            .map_err(io::Error::from)?;
        Ok(SerialLink {
            port,
            deframer: Deframer::default(),
            ready: VecDeque::new(),
        })
    }
}

impl Link for SerialLink {
    fn send(&mut self, message: &[u8]) -> io::Result<()> {
        self.port.write_all(&wire::frame(message))?;
        self.port.flush()
    }

    fn receive(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let until = Instant::now() + timeout;
        let mut buf = [0u8; 256];
        loop {
            if let Some(m) = self.ready.pop_front() {
                return Ok(Some(m));
            }
            let waiting = self.port.bytes_to_read().map_err(io::Error::from)?;
            if waiting > 0 || Instant::now() < until {
                match self.port.read(&mut buf) {
                    Ok(n) => self.ready.extend(self.deframer.push(&buf[..n])),
                    Err(e) if e.kind() == io::ErrorKind::TimedOut => {}
                    Err(e) => return Err(e),
                }
            }
            if self.ready.is_empty() && Instant::now() >= until {
                return Ok(None);
            }
        }
    }
}

// --------------------------------------------------------------------- HID

/// A board on our HID collection.
pub struct HidLink {
    dev: hidapi::HidDevice,
}

impl HidLink {
    pub fn open(api: &hidapi::HidApi, path: &str) -> io::Result<HidLink> {
        let path =
            CString::new(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let dev = api.open_path(&path).map_err(hid_error)?;
        Ok(HidLink { dev })
    }
}

fn hid_error(e: hidapi::HidError) -> io::Error {
    io::Error::other(e.to_string())
}

impl Link for HidLink {
    fn send(&mut self, message: &[u8]) -> io::Result<()> {
        self.dev.write(&wire::report(message)).map_err(hid_error)?;
        Ok(())
    }

    fn receive(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        let mut buf = [0u8; wire::REPORT_LEN];
        let n = self
            .dev
            .read_timeout(&mut buf, timeout.as_millis().min(i32::MAX as u128) as i32)
            .map_err(hid_error)?;
        Ok(if n == 0 {
            None
        } else {
            wire::unreport(&buf[..n])
        })
    }
}

/// The paths of every HID collection that is one of our boards, by its usage
/// page and usage, so a board is found whatever its vendor and product ids.
pub fn hid_boards(api: &hidapi::HidApi) -> Vec<String> {
    api.device_list()
        .filter(|d| d.usage_page() == wire::USAGE_PAGE && d.usage() == wire::USAGE)
        .map(|d| d.path().to_string_lossy().into_owned())
        .collect()
}

/// The serial ports that exist now, by name, such as `COM5`. Listing them
/// opens none.
pub fn serial_ports() -> Vec<String> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.port_name)
        .collect()
}
