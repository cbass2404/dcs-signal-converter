//! The WinCtrl / WinWing panels, over [`wctrl_hid`].
//!
//! Everything peculiar to these panels is here: that a lamp is addressed by
//! part and index, that a pixel screen shows nothing until it is committed,
//! that a text grid has to be declared and given a font before it draws, and
//! that two screens sent back to back garble unless there is a gap between
//! them.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use dsc_config::mcdu_font::{font_upload, McduFont, PacketMap, UploadStep};
use dsc_config::{DeviceSpec, DisplayCatalogue, Transport};
use dsc_engine::{LcdWrite, LedWrite};
use hidapi::HidApi;
use wctrl_hid::{Device, GridCell};

use super::{Found, Panel, Protocol, Sent};

/// Text screens sent back to back can garble; WwDevicesDotnet leaves this
/// long after each one for the same reason. A text screen is not ready again
/// until this long after its last paint finished ([`Panel::ready_at`]), and
/// the panel's writer holds a sooner paint, as the latest screen, until it
/// is. Nothing else waits: the writer is this panel's own thread.
///
/// The pixel and segment screens are always ready: neither is known to need
/// a gap. Whether the text grid acknowledges a screen, which would say when
/// it is ready instead of this, has not been captured.
const TEXT_GRID_GAP: Duration = Duration::from_millis(40);

pub struct Wctrl {
    /// Held rather than rebuilt per call: it caches the device list that
    /// enumerating and opening both read.
    api: HidApi,
}

impl Wctrl {
    pub fn new() -> Result<Self> {
        Ok(Wctrl {
            api: HidApi::new().context("opening HID API")?,
        })
    }
}

impl Protocol for Wctrl {
    fn name(&self) -> &'static str {
        dsc_config::DEFAULT_PROTOCOL
    }

    fn present(&self) -> Result<Vec<Found>> {
        Ok(wctrl_hid::enumerate(&self.api)
            .into_iter()
            .map(|d| Found {
                ident: format!("pid 0x{:04x}", d.product_id),
                product: d.product,
                serial: d.serial,
            })
            .collect())
    }

    fn is_connected(&self, spec: &DeviceSpec) -> Result<bool> {
        Ok(wctrl_hid::enumerate(&self.api)
            .iter()
            .any(|d| d.product_id == spec.usb_pid))
    }

    fn open(&self, spec: &DeviceSpec, displays: &DisplayCatalogue) -> Result<Box<dyn Panel>> {
        let dev = Device::open(&self.api, spec.usb_pid)
            .with_context(|| format!("opening {}", spec.display_name))?;
        Ok(Box::new(WctrlPanel {
            dev,
            key: spec.key.clone(),
            displays: displays.clone(),
            font: None,
            pending: Vec::new(),
            ready_at: HashMap::new(),
        }))
    }
}

struct WctrlPanel {
    dev: Device,
    /// The device's key, for error messages, since a write names the device it
    /// was meant for and a commit does not.
    key: String,
    displays: DisplayCatalogue,
    /// The font this panel's text grid holds: `None` until the grid has been
    /// declared this run, `Some(None)` once declared with no font sent. The
    /// panel keeps no font across a power cycle and cannot be asked which it
    /// has, so a run starts by assuming nothing.
    font: Option<Option<String>>,
    /// Parts written to since the last flush, each to be committed once.
    pending: Vec<u32>,
    /// When each text grid, by part, is ready for its next paint.
    ready_at: HashMap<u32, Instant>,
}

impl WctrlPanel {
    /// Get a text grid ready for `w`: declared, and holding the font it needs.
    ///
    /// The font upload is the panel's whole glyph set, about 600 reports, so it
    /// goes out only when the font changes, not per paint. It resets the grid
    /// to the size SimAppPro uses, so the grid is declared again after it,
    /// exactly as WwDevicesDotnet does.
    fn prepare_text_grid(&mut self, w: &LcdWrite) -> Result<()> {
        let grid = self
            .displays
            .get(&w.display)
            .and_then(|d| d.text.as_ref())
            .with_context(|| format!("display {} has no text grid", w.display))?;
        let origin = (grid.origin[0], grid.origin[1]);
        let (rows, columns) = (grid.rows as u16, grid.columns as u16);
        let upload = match (&w.font, &self.font) {
            (Some(want), Some(Some(held))) if want == held => None,
            (Some(want), _) => Some(want.clone()),
            (None, Some(_)) => return Ok(()),
            (None, None) => None,
        };
        self.dev.declare_grid(w.part_id, origin, rows, columns)?;
        if let Some(file) = &upload {
            self.dev
                .paint_grid(&vec![GridCell::BLANK; grid.rows * grid.columns])?;
            let map = PacketMap::load(&grid.path(&grid.upload))?;
            let font = McduFont::load(&grid.path(file))?;
            let at = (grid.font_origin[0], grid.font_origin[1]);
            for step in font_upload(&map, &font, w.part_id, at, 255)? {
                match step {
                    UploadStep::Report(r) => {
                        self.dev.send_screen_reports(std::slice::from_ref(&r))?;
                    }
                    UploadStep::SetLed { index, value } => {
                        self.dev.set_led(w.part_id, index, value)?;
                    }
                }
            }
            self.dev.declare_grid(w.part_id, origin, rows, columns)?;
        }
        self.font = Some(upload.or_else(|| self.font.clone().flatten()));
        Ok(())
    }

    /// Paint a whole text screen now, and note when the grid is ready again.
    fn paint_text(&mut self, w: &LcdWrite) -> Result<()> {
        let mut result = self.prepare_text_grid(w);
        if result.is_ok() {
            let cells: Vec<GridCell> = dsc_config::text_cells(&w.bytes)
                .into_iter()
                .map(|c| GridCell {
                    ch: c.ch,
                    fg: c.fg,
                    bg: c.bg,
                    small: c.small,
                })
                .collect();
            result = self.dev.paint_grid(&cells).map_err(Into::into);
        }
        // The gap follows even a failed paint: the next one is the
        // correction, and sending it at once is what garbles.
        self.ready_at
            .insert(w.part_id, Instant::now() + TEXT_GRID_GAP);
        result.with_context(|| format!("writing {} text screen", w.device))
    }
}

impl Panel for WctrlPanel {
    fn set_lamp(&mut self, w: &LedWrite) -> Result<()> {
        self.dev
            .set_led(w.id.part_id, w.id.index, w.value)
            .with_context(|| format!("writing {} index {}", w.id.device, w.id.index))
    }

    fn write_display(&mut self, w: &LcdWrite) -> Result<()> {
        match w.transport {
            Transport::Segment => self
                .dev
                .set_lcd(w.part_id, w.group, &w.bytes)
                .with_context(|| format!("writing {} display group {}", w.device, w.group)),
            Transport::Pixel => {
                self.dev
                    .write_pixels(w.part_id, w.offset, &w.bytes)
                    .with_context(|| format!("writing {} screen from row {}", w.device, w.group))?;
                // Nothing is on the glass until the batch is flushed.
                if !self.pending.contains(&w.part_id) {
                    self.pending.push(w.part_id);
                }
                Ok(())
            }
            // Every text write is the whole screen. The writer calls this only
            // once the grid is ready, having held any sooner paint.
            Transport::Text => self.paint_text(w),
        }
    }

    fn ready_at(&self, w: &LcdWrite) -> Option<Instant> {
        match w.transport {
            Transport::Text => self
                .ready_at
                .get(&w.part_id)
                .copied()
                .filter(|t| *t > Instant::now()),
            Transport::Segment | Transport::Pixel => None,
        }
    }

    fn flush(&mut self) -> Result<()> {
        for part in self.pending.drain(..) {
            self.dev
                .commit_pixels(part)
                .with_context(|| format!("committing {} screen", self.key))?;
        }
        Ok(())
    }

    fn sent(&self) -> Sent {
        let t = self.dev.traffic();
        Sent {
            reports: t.reports,
            bytes: t.bytes,
            writing: t.writing,
            // Counted by the writer, which does the replacing.
            superseded: 0,
        }
    }
}
