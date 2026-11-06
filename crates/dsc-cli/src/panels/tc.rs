//! The Total Controls panels, over HID feature reports.
//!
//! The Apache MPD frames have one lamp the host can drive, the backlight, at
//! four levels. The maker gave the command (2026-11-06): the feature report
//! `'B'` then the level 0 to 3 as a raw byte. The report has no id, so it
//! goes out as id 0 and then the eight bytes the descriptor declares.
//!
//! Every write is stored in the panel's EEPROM and wears it (the maker,
//! 2026-11-06), so a level goes out only once it has held for
//! [`SETTLE`], and only if the panel does not already have it. A dimmer swept
//! from full to off crosses four levels and costs one write.
//!
//! Only `'B'` is ever written into that report. Other first bytes are
//! undocumented and may switch the panel's identity or start its bootloader.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use dsc_config::{DeviceSpec, DisplayCatalogue, TC_MANUFACTURER, TC_VENDOR_ID};
use dsc_engine::{LcdWrite, LedWrite};
use hidapi::{HidApi, HidDevice};

use super::{Found, Panel, Protocol, Sent};

/// How long a level has to hold before it is written.
const SETTLE: Duration = Duration::from_millis(500);

/// How often the writer looks for a level that has settled.
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// Report id 0, then the declared eight bytes: `'B'`, the level, padding.
fn backlight_report(level: u8) -> [u8; 9] {
    [0x00, b'B', level, 0, 0, 0, 0, 0, 0]
}

/// The panel's level for a brightness: 0 is off, and the rest of the range
/// in thirds is dim, medium and full.
fn level(value: u8) -> u8 {
    if value == 0 {
        0
    } else {
        1 + (value - 1) / 85
    }
}

pub struct Tc {
    /// Held rather than rebuilt per call: it caches the device list that
    /// enumerating and opening both read.
    api: HidApi,
}

impl Tc {
    pub fn new() -> Result<Self> {
        Ok(Tc {
            api: HidApi::new().context("opening HID API")?,
        })
    }

    fn theirs(&self) -> impl Iterator<Item = &hidapi::DeviceInfo> {
        self.api.device_list().filter(|d| {
            d.vendor_id() == TC_VENDOR_ID && d.manufacturer_string() == Some(TC_MANUFACTURER)
        })
    }
}

impl Protocol for Tc {
    fn name(&self) -> &'static str {
        dsc_config::TC_PROTOCOL
    }

    fn present(&self) -> Result<Vec<Found>> {
        Ok(self
            .theirs()
            .map(|d| Found {
                ident: format!("pid 0x{:04x}", d.product_id()),
                product: d.product_string().unwrap_or_default().to_string(),
                serial: d.serial_number().unwrap_or_default().to_string(),
            })
            .collect())
    }

    fn is_connected(&self, spec: &DeviceSpec) -> Result<bool> {
        Ok(self.theirs().any(|d| d.product_id() == spec.usb_pid))
    }

    fn refresh(&mut self) -> Result<()> {
        self.api
            .refresh_devices()
            .context("listing the HID devices again")
    }

    fn open(&self, spec: &DeviceSpec, _displays: &DisplayCatalogue) -> Result<Box<dyn Panel>> {
        let info = self
            .theirs()
            .find(|d| d.product_id() == spec.usb_pid)
            .with_context(|| format!("{} is not plugged in", spec.display_name))?;
        let dev = info
            .open_device(&self.api)
            .with_context(|| format!("opening {}", spec.display_name))?;
        Ok(Box::new(TcPanel {
            dev,
            key: spec.key.clone(),
            settle: Settle::default(),
            sent: Sent::default(),
        }))
    }
}

/// Which level the panel has and which it is waiting to be given.
#[derive(Default)]
struct Settle {
    /// What was last written, `None` until something has been this run. The
    /// panel cannot be asked.
    written: Option<u8>,
    /// The latest level asked for, and since when.
    wanted: Option<(u8, Instant)>,
}

impl Settle {
    fn ask(&mut self, level: u8, now: Instant) {
        if self.wanted.map(|(l, _)| l) != Some(level) {
            self.wanted = Some((level, now));
        }
    }

    /// The level to write now, if one has held long enough and is new.
    fn due(&self, now: Instant) -> Option<u8> {
        self.wanted
            .filter(|(_, since)| now.saturating_duration_since(*since) >= SETTLE)
            .and_then(|(l, _)| self.pending(l))
    }

    /// The level to write at the end, held or not.
    fn last(&self) -> Option<u8> {
        self.wanted.and_then(|(l, _)| self.pending(l))
    }

    fn pending(&self, level: u8) -> Option<u8> {
        (self.written != Some(level)).then_some(level)
    }
}

struct TcPanel {
    dev: HidDevice,
    key: String,
    settle: Settle,
    sent: Sent,
}

impl TcPanel {
    fn write(&mut self, level: u8) -> Result<()> {
        let report = backlight_report(level);
        let start = Instant::now();
        let result = self.dev.send_feature_report(&report);
        self.sent.writing += start.elapsed();
        result.with_context(|| format!("writing {} backlight", self.key))?;
        self.sent.reports += 1;
        self.sent.bytes += report.len() as u64;
        self.settle.written = Some(level);
        Ok(())
    }
}

impl Panel for TcPanel {
    /// Noted, not written: [`check`](Panel::check) writes it once it settles.
    fn set_lamp(&mut self, w: &LedWrite) -> Result<()> {
        self.settle.ask(level(w.value), Instant::now());
        Ok(())
    }

    fn write_display(&mut self, w: &LcdWrite) -> Result<()> {
        anyhow::bail!("{} has no display to write {}", self.key, w.display)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn ready_at(&self, _w: &LcdWrite) -> Option<Instant> {
        None
    }

    fn sent(&self) -> Sent {
        self.sent
    }

    fn check_every(&self) -> Option<Duration> {
        Some(LOOK_EVERY)
    }

    fn check(&mut self) -> Result<()> {
        match self.settle.due(Instant::now()) {
            Some(level) => self.write(level),
            None => Ok(()),
        }
    }

    /// The exit's clearing write goes out now rather than waiting to settle.
    fn finish(&mut self) -> Result<()> {
        match self.settle.last() {
            Some(level) => self.write(level),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_off_and_the_rest_is_thirds() {
        let levels: Vec<u8> = [0, 1, 85, 86, 170, 171, 255].map(level).to_vec();
        assert_eq!(levels, [0, 1, 1, 2, 2, 3, 3]);
    }

    #[test]
    fn the_report_is_only_ever_b_and_a_level() {
        assert_eq!(backlight_report(2), [0, 0x42, 2, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn a_level_is_written_only_once_it_has_held() {
        let t = Instant::now();
        let mut s = Settle::default();
        s.ask(3, t);
        assert_eq!(s.due(t + SETTLE / 2), None);
        assert_eq!(s.due(t + SETTLE), Some(3));
    }

    /// Full to off crosses every level and writes only the last.
    #[test]
    fn a_sweep_is_one_write() {
        let t = Instant::now();
        let mut s = Settle {
            written: Some(3),
            wanted: Some((3, t)),
        };
        let ms = Duration::from_millis;
        s.ask(2, t + ms(100));
        assert_eq!(s.due(t + ms(200)), None);
        s.ask(1, t + ms(200));
        s.ask(0, t + ms(300));
        assert_eq!(s.due(t + ms(700)), None);
        assert_eq!(s.due(t + ms(800)), Some(0));
    }

    #[test]
    fn a_level_the_panel_already_has_is_not_written_again() {
        let t = Instant::now();
        let mut s = Settle {
            written: Some(2),
            wanted: None,
        };
        s.ask(3, t);
        s.ask(2, t + Duration::from_millis(10));
        assert_eq!(s.due(t + SETTLE * 2), None);
        assert_eq!(s.last(), None);
    }

    #[test]
    fn the_last_level_goes_out_at_the_end_without_waiting() {
        let t = Instant::now();
        let mut s = Settle {
            written: Some(3),
            wanted: None,
        };
        s.ask(0, t);
        assert_eq!(s.due(t), None);
        assert_eq!(s.last(), Some(0));
    }
}
