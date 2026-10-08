//! The web view: a text grid drawn in a web page rather than on a panel.
//!
//! For anyone without a CDU of their own. The page is served on this machine
//! only, at [`WEB_ADDRESS`], for OpenKneeboard's Web Dashboard tab, a browser, or a
//! spare monitor. It draws the grid in the same font the panel would be sent,
//! so a page looks the way it does on the glass.
//!
//! A web device is always there, so it is always connected, and a profile
//! turns it on or off like any panel. Nothing listens until a profile first
//! writes to one: a device a profile leaves disabled is never written, so a
//! user who has not turned it on has no port open.
//!
//! The page asks for the screen with a long poll: `/screen?after=n` answers
//! as soon as the screen is newer than `n`, or after [`POLL_FOR`] with the
//! same one, so a paint reaches it at once and an idle page costs nothing.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use dsc_config::mcdu_font::McduFont;
use dsc_config::{DeviceSpec, DisplayCatalogue, Transport, WEB_ADDRESS, WEB_PROTOCOL};
use dsc_engine::{LcdWrite, LedWrite};
use serde_json::{json, Value};
use tiny_http::{Header, Request, Response, Server};

use super::{Found, Panel, Protocol, Sent};

/// How long a poll waits for a newer screen before answering with the same
/// one, which keeps a page that lost its server from waiting forever.
const POLL_FOR: Duration = Duration::from_secs(10);

/// The page itself, built into the binary so there is nothing to install.
const PAGE: &str = include_str!("web.html");

pub struct Web {
    shared: Arc<Shared>,
}

impl Web {
    pub fn new() -> Web {
        Web {
            shared: Arc::new(Shared::default()),
        }
    }
}

impl Protocol for Web {
    fn name(&self) -> &'static str {
        WEB_PROTOCOL
    }

    fn present(&self) -> Result<Vec<Found>> {
        Ok(vec![Found {
            ident: format!("http://{WEB_ADDRESS}"),
            product: "web view, served once a profile turns it on".into(),
            serial: String::new(),
        }])
    }

    fn is_connected(&self, _spec: &DeviceSpec) -> Result<bool> {
        Ok(true)
    }

    fn open(&self, spec: &DeviceSpec, displays: &DisplayCatalogue) -> Result<Box<dyn Panel>> {
        Ok(Box::new(WebPanel {
            key: spec.key.clone(),
            name: spec.display_name.clone(),
            backlights: spec.display_lamps().map(|(_, l)| l.index).collect(),
            displays: displays.clone(),
            shared: Arc::clone(&self.shared),
            sent: Sent::default(),
        }))
    }
}

/// One web device's screen as the page last needs to draw it.
#[derive(Default)]
struct Screen {
    name: String,
    /// Counts paints, so a poll can say which it already has.
    version: u64,
    columns: usize,
    rows: usize,
    /// The font file, as the profile names it. Kept across paints that name
    /// none, as the panel keeps the font it holds.
    font: Option<String>,
    /// Per cell: character, foreground, background, small.
    cells: Vec<(char, u8, u8, bool)>,
    /// The screen backlight, 0 to 255; full until it is first written.
    backlight: Option<u8>,
}

#[derive(Default)]
struct State {
    /// By device key, in the order the devices were first painted.
    screens: Vec<(String, Screen)>,
    /// Each font the screens have used, as the page reads it.
    fonts: HashMap<String, String>,
    /// Whether the server is up, or why it could not start.
    serving: Option<Result<(), String>>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    painted: Condvar,
}

impl State {
    /// A device's screen, made the first time it is asked for.
    fn screen(&mut self, key: &str, name: &str) -> &mut Screen {
        let at = match self.screens.iter().position(|(k, _)| k == key) {
            Some(at) => at,
            None => {
                self.screens.push((key.to_string(), Screen::default()));
                self.screens.len() - 1
            }
        };
        let screen = &mut self.screens[at].1;
        screen.name = name.to_string();
        screen
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A thread that panicked holding it left nothing half done.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Start the server the first time a web device is written to.
    fn serve(self: &Arc<Self>) -> Result<()> {
        let mut state = self.lock();
        if let Some(serving) = &state.serving {
            return serving.clone().map_err(|e| anyhow!(e));
        }
        let started = Server::http(WEB_ADDRESS)
            .map_err(|e| format!("serving the web view on {WEB_ADDRESS}: {e}"))
            .and_then(|server| {
                let shared = Arc::clone(self);
                std::thread::Builder::new()
                    .name("web view".into())
                    .spawn(move || listen(&server, &shared))
                    .map(|_| ())
                    .map_err(|e| format!("starting the web view: {e}"))
            });
        state.serving = Some(started.clone());
        started.map_err(|e| anyhow!(e))
    }
}

struct WebPanel {
    key: String,
    name: String,
    /// The indices of the lamps that light the screen.
    backlights: Vec<u8>,
    displays: DisplayCatalogue,
    shared: Arc<Shared>,
    sent: Sent,
}

impl WebPanel {
    /// The font as the page reads it: each character's rows, in both sizes.
    fn font_json(&self, display: &str, file: &str) -> Result<String> {
        let grid = self
            .displays
            .get(display)
            .and_then(|d| d.text.as_ref())
            .with_context(|| format!("display {display} has no text grid"))?;
        let font =
            McduFont::load(&grid.path(file)).with_context(|| format!("reading the font {file}"))?;
        let table = |glyphs: &[dsc_config::mcdu_font::Glyph]| -> Value {
            glyphs
                .iter()
                .map(|g| (g.character.to_string(), json!(g.bit_array)))
                .collect::<serde_json::Map<_, _>>()
                .into()
        };
        Ok(json!({
            "width": font.glyph_width,
            "height": font.glyph_height,
            "cell": font.glyph_full_width,
            "large": table(&font.large_glyphs),
            "small": table(&font.small_glyphs),
        })
        .to_string())
    }
}

impl Panel for WebPanel {
    /// The screen backlight, which the page draws. A web view has no other
    /// lamp for a write to reach.
    fn set_lamp(&mut self, w: &LedWrite) -> Result<()> {
        if !self.backlights.contains(&w.id.index) {
            return Ok(());
        }
        self.shared.serve()?;
        let mut state = self.shared.lock();
        let screen = state.screen(&self.key, &self.name);
        if screen.backlight == Some(w.value) {
            return Ok(());
        }
        screen.backlight = Some(w.value);
        screen.version += 1;
        drop(state);
        self.shared.painted.notify_all();
        self.sent.reports += 1;
        self.sent.bytes += 1;
        Ok(())
    }

    fn write_display(&mut self, w: &LcdWrite) -> Result<()> {
        // Only a text grid has a page that can draw it.
        if w.transport != Transport::Text {
            return Ok(());
        }
        self.shared.serve()?;
        let grid = self
            .displays
            .get(&w.display)
            .and_then(|d| d.text.as_ref())
            .with_context(|| format!("display {} has no text grid", w.display))?;
        let font = match &w.font {
            Some(file) if !self.shared.lock().fonts.contains_key(file) => {
                Some((file.clone(), self.font_json(&w.display, file)?))
            }
            _ => None,
        };
        let cells: Vec<_> = dsc_config::text_cells(&w.bytes)
            .into_iter()
            .map(|c| (c.ch, c.fg, c.bg, c.small))
            .collect();

        let mut state = self.shared.lock();
        if let Some((file, json)) = font {
            state.fonts.insert(file, json);
        }
        let screen = state.screen(&self.key, &self.name);
        screen.version += 1;
        screen.columns = grid.columns;
        screen.rows = grid.rows;
        if w.font.is_some() {
            screen.font = w.font.clone();
        }
        screen.cells = cells;
        drop(state);
        self.shared.painted.notify_all();

        self.sent.reports += 1;
        self.sent.bytes += w.bytes.len() as u64;
        Ok(())
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
}

/// Answer requests until the process ends. A poll waits on its own thread,
/// so a page waiting for a paint holds up nothing else.
fn listen(server: &Server, shared: &Arc<Shared>) {
    for request in server.incoming_requests() {
        let shared = Arc::clone(shared);
        let _ = std::thread::Builder::new()
            .name("web request".into())
            .spawn(move || answer(request, &shared));
    }
}

fn answer(request: Request, shared: &Shared) {
    let url = request.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((&url, ""));
    let arg = |name: &str| {
        query
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == name)
            .map(|(_, v)| decode(v))
    };
    let reply = match path {
        "/" => Some(("text/html; charset=utf-8", PAGE.to_string())),
        "/screen" => {
            let after = arg("after").and_then(|v| v.parse().ok()).unwrap_or(0);
            Some(("application/json", screen(shared, arg("device"), after)))
        }
        // Only a font a screen has used, so the path cannot reach anything
        // else on disk.
        "/font" => arg("file")
            .and_then(|f| shared.lock().fonts.get(&f).cloned())
            .map(|json| ("application/json", json)),
        _ => None,
    };
    let _ = match reply {
        Some((kind, body)) => request.respond(
            Response::from_string(body)
                .with_header(header("Content-Type", kind))
                .with_header(header("Cache-Control", "no-store")),
        ),
        None => request.respond(Response::from_string("not found").with_status_code(404)),
    };
}

/// The screen for `device`, or the first one painted, once it is newer than
/// `after` or the poll runs out.
fn screen(shared: &Shared, device: Option<String>, after: u64) -> String {
    let deadline = Instant::now() + POLL_FOR;
    let mut state = shared.lock();
    loop {
        let found = state
            .screens
            .iter()
            .find(|(k, _)| device.as_ref().is_none_or(|d| d == k));
        let newer = found.is_some_and(|(_, s)| s.version != after);
        let now = Instant::now();
        if newer || now >= deadline {
            let Some((key, s)) = found else {
                return json!({ "device": null }).to_string();
            };
            let cells: Vec<Value> = s
                .cells
                .iter()
                .map(|(ch, fg, bg, small)| json!([ch.to_string(), fg, bg, u8::from(*small)]))
                .collect();
            return json!({
                "device": key,
                "name": s.name,
                "version": s.version,
                "columns": s.columns,
                "rows": s.rows,
                "font": s.font,
                "backlight": s.backlight.unwrap_or(255),
                "cells": cells,
            })
            .to_string();
        }
        state = shared
            .painted
            .wait_timeout(state, deadline - now)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("a valid header")
}

/// A query value with its `%xx` escapes undone, which a font's path has.
fn decode(v: &str) -> String {
    let bytes = v.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn a_query_value_is_unescaped() {
        assert_eq!(
            decode("..%2Fmcdu%2Fah64d-font-21x31.json"),
            "../mcdu/ah64d-font-21x31.json"
        );
        assert_eq!(decode("50%"), "50%");
        assert_eq!(decode("a+b"), "a b");
    }
}
