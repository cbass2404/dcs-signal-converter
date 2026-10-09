//! Boards that describe their own lamps, over HID or serial. See
//! docs/PROTOCOL-DSC.md and [`dsc_device`].
//!
//! A board is found by asking it who it is, so finding one means opening it.
//! The link is then kept for as long as the board is there, and the panel the
//! converter drives shares it: a serial board resets every time its port is
//! opened, so it is never opened twice. A place that did not answer is not
//! asked again until it goes away and comes back, so a wrong port in the
//! settings costs one wait, not one per USB event.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use dsc_config::settings::Settings;
use dsc_config::{DeviceSpec, DisplayCatalogue, DSC_PROTOCOL};
use dsc_device::wire::{set_lamps, Reply, Request};
use dsc_device::{scan, Description, Link};
use dsc_engine::{LcdWrite, LedWrite};

use super::{Found, Panel, Protocol, Sent};

type SharedLink = Arc<Mutex<Box<dyn Link>>>;

/// Where boards may be, and how to open one. The real one is HID and the
/// serial ports the settings allow; a test's is fake boards.
pub trait Places {
    /// Every place a board could be now.
    fn places(&mut self) -> Vec<String>;
    /// Open one, and how long a board there gets to answer.
    fn open(&mut self, place: &str) -> io::Result<(Box<dyn Link>, Duration)>;
}

/// HID collections on our usage page, and the allowed COM ports that exist.
struct Hardware {
    api: hidapi::HidApi,
    settings: PathBuf,
}

impl Places for Hardware {
    fn places(&mut self) -> Vec<String> {
        let _ = self.api.refresh_devices();
        let mut out = dsc_device::hid_boards(&self.api);
        // Read each time, so a port chosen in the editor counts without a
        // restart. A file that will not parse allows no ports.
        let allowed = Settings::load(&self.settings)
            .map(|s| s.dsc_ports)
            .unwrap_or_default();
        if !allowed.is_empty() {
            let existing: HashSet<String> = dsc_device::serial_ports()
                .into_iter()
                .map(|p| p.to_ascii_uppercase())
                .collect();
            out.extend(
                allowed
                    .into_iter()
                    .filter(|p| existing.contains(&p.to_ascii_uppercase())),
            );
        }
        out
    }

    fn open(&mut self, place: &str) -> io::Result<(Box<dyn Link>, Duration)> {
        if is_port(place) {
            Ok((
                Box::new(dsc_device::SerialLink::open(place)?),
                dsc_device::SERIAL_WAIT,
            ))
        } else {
            Ok((
                Box::new(dsc_device::HidLink::open(&self.api, place)?),
                dsc_device::HID_WAIT,
            ))
        }
    }
}

fn is_port(place: &str) -> bool {
    place.to_ascii_uppercase().starts_with("COM")
}

struct Board {
    place: String,
    description: Description,
    link: SharedLink,
}

pub struct Dsc {
    places: Box<dyn Places>,
    /// By device key.
    boards: HashMap<String, Board>,
    /// Places that did not answer, or answered as a board already found.
    passed_over: HashSet<String>,
    lines: Vec<String>,
}

impl Dsc {
    pub fn new(settings: PathBuf) -> Result<Dsc> {
        let api = hidapi::HidApi::new().context("opening HID API")?;
        Ok(Dsc::with(Box::new(Hardware { api, settings })))
    }

    pub fn with(places: Box<dyn Places>) -> Dsc {
        let mut d = Dsc {
            places,
            boards: HashMap::new(),
            passed_over: HashSet::new(),
            lines: Vec::new(),
        };
        d.look();
        d
    }

    /// Forget what has gone, and ask anything new who it is.
    fn look(&mut self) {
        let here: HashSet<String> = self.places.places().into_iter().collect();
        self.boards.retain(|_, b| here.contains(&b.place));
        self.passed_over.retain(|p| here.contains(p));
        let held: HashSet<String> = self.boards.values().map(|b| b.place.clone()).collect();
        let mut new: Vec<&String> = here
            .iter()
            .filter(|p| !held.contains(*p) && !self.passed_over.contains(*p))
            .collect();
        new.sort();
        for place in new {
            match self.ask(place) {
                Ok((description, link)) => {
                    let key = description.key();
                    if let Some(other) = self.boards.get(&key) {
                        self.lines.push(format!(
                            "dsc      {place} and {} both say they are {key}; give one a unit in its sketch. {place} is left alone",
                            other.place
                        ));
                        self.passed_over.insert(place.clone());
                        continue;
                    }
                    self.lines.push(format!(
                        "dsc      {place}: {} {} unit {:?}, firmware {}, {} lamps",
                        description.hello.vendor,
                        description.hello.model,
                        description.hello.unit,
                        description.hello.firmware,
                        description.lamps.len()
                    ));
                    self.boards.insert(
                        key,
                        Board {
                            place: place.clone(),
                            description,
                            link: Arc::new(Mutex::new(link)),
                        },
                    );
                }
                Err(e) => {
                    self.lines.push(format!("dsc      {place}: {e:#}"));
                    self.passed_over.insert(place.clone());
                }
            }
        }
    }

    fn ask(&mut self, place: &str) -> Result<(Description, Box<dyn Link>)> {
        let (mut link, wait) = self.places.open(place)?;
        let description = scan(link.as_mut(), wait)?;
        Ok((description, link))
    }
}

impl Protocol for Dsc {
    fn name(&self) -> &'static str {
        DSC_PROTOCOL
    }

    fn present(&self) -> Result<Vec<Found>> {
        let mut out: Vec<Found> = self
            .boards
            .iter()
            .map(|(key, b)| Found {
                ident: b.place.clone(),
                product: format!("{} ({key})", b.description.display_name()),
                serial: String::new(),
            })
            .collect();
        out.sort_by(|a, b| a.ident.cmp(&b.ident));
        Ok(out)
    }

    fn is_connected(&self, spec: &DeviceSpec) -> Result<bool> {
        Ok(self.boards.contains_key(&spec.key))
    }

    fn refresh(&mut self) -> Result<()> {
        self.look();
        Ok(())
    }

    fn lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }

    fn open(&self, spec: &DeviceSpec, _displays: &DisplayCatalogue) -> Result<Box<dyn Panel>> {
        let board = self
            .boards
            .get(&spec.key)
            .with_context(|| format!("{} is not plugged in", spec.display_name))?;
        // Bindings reach a lamp by the index the inventory gives it. If the
        // board now numbers its lamps differently, those would light the
        // wrong ones, so it is refused until it is described again.
        let now = board.description.spec().with_context(|| {
            format!(
                "{} describes itself in a way that cannot be bound",
                spec.display_name
            )
        })?;
        if !same_lamps(&now, spec) {
            bail!(
                "{} has changed its lamps since it was added; add it again in the editor",
                spec.display_name
            );
        }
        let mut link = board.link.lock().unwrap();
        link.send(&Request::AllOff.encode())
            .with_context(|| format!("clearing {}", spec.display_name))?;
        drop(link);
        Ok(Box::new(DscPanel {
            name: spec.display_name.clone(),
            link: Arc::clone(&board.link),
            pending: Vec::new(),
            sent: Sent::default(),
        }))
    }
}

/// Every lamp at the same index with the same name, kind and range.
fn same_lamps(a: &DeviceSpec, b: &DeviceSpec) -> bool {
    let lamps = |s: &DeviceSpec| {
        let mut v: Vec<(u8, String, bool, Option<u8>)> = s
            .leds()
            .map(|(_, l)| {
                (
                    l.index,
                    l.name.clone(),
                    matches!(l.kind, dsc_config::LedKind::Indicator),
                    l.max,
                )
            })
            .collect();
        v.sort();
        v
    };
    lamps(a) == lamps(b)
}

struct DscPanel {
    name: String,
    link: SharedLink,
    /// Lamps set since the last flush, last value winning.
    pending: Vec<(u8, u8)>,
    sent: Sent,
}

impl Panel for DscPanel {
    fn set_lamp(&mut self, w: &LedWrite) -> Result<()> {
        match self.pending.iter_mut().find(|(i, _)| *i == w.id.index) {
            Some(p) => p.1 = w.value,
            None => self.pending.push((w.id.index, w.value)),
        }
        Ok(())
    }

    fn write_display(&mut self, w: &LcdWrite) -> Result<()> {
        bail!("{} has no display, but was sent {}", self.name, w.display)
    }

    fn flush(&mut self) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let pairs = std::mem::take(&mut self.pending);
        let mut link = self.link.lock().unwrap();
        let started = Instant::now();
        for request in set_lamps(&pairs) {
            let bytes = request.encode();
            link.send(&bytes)
                .with_context(|| format!("writing {}", self.name))?;
            self.sent.reports += 1;
            self.sent.bytes += bytes.len() as u64;
        }
        self.sent.writing += started.elapsed();
        // A board answers lamps only when something is wrong. Take what is
        // waiting so it never piles up; an index past the end cannot happen
        // to a board that matched its inventory entry on opening.
        while let Some(m) = link.receive(Duration::ZERO)? {
            if let Ok(Reply::Error { of, code }) = Reply::decode(&m) {
                bail!(
                    "{} refused {of:#04x}: {}",
                    self.name,
                    Reply::error_text(code)
                );
            }
        }
        Ok(())
    }

    fn ready_at(&self, _w: &LcdWrite) -> Option<Instant> {
        None
    }

    fn sent(&self) -> Sent {
        self.sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsc_device::fake::{self, lamp, FakeLink};
    use dsc_engine::LedId;

    /// Fake boards by place, each handed out once per open.
    struct FakePlaces {
        boards: HashMap<String, fake::Shared>,
        opened: Arc<Mutex<Vec<String>>>,
    }

    impl Places for FakePlaces {
        fn places(&mut self) -> Vec<String> {
            self.boards.keys().cloned().collect()
        }
        fn open(&mut self, place: &str) -> io::Result<(Box<dyn Link>, Duration)> {
            self.opened.lock().unwrap().push(place.to_string());
            let b = Arc::clone(&self.boards[place]);
            Ok((Box::new(FakeLink(b)), Duration::from_millis(50)))
        }
    }

    fn caution() -> (fake::Shared, DeviceSpec) {
        let (link, board) = fake::board(
            "Arduino",
            "Caution Panel",
            "",
            vec![lamp(0, "FIRE", 1), lamp(1, "BACKLIGHT", 255)],
        );
        let mut l = link;
        let spec = scan(&mut l, Duration::from_millis(50))
            .unwrap()
            .spec()
            .unwrap();
        board.lock().unwrap().heard.clear();
        (board, spec)
    }

    fn write(spec: &DeviceSpec, index: u8, value: u8) -> LedWrite {
        LedWrite {
            id: LedId {
                device: spec.key.clone(),
                part_id: 0,
                index,
            },
            value,
        }
    }

    #[test]
    fn a_board_is_found_opened_and_driven_in_batches() {
        let (board, spec) = caution();
        let opened = Arc::new(Mutex::new(Vec::new()));
        let mut dsc = Dsc::with(Box::new(FakePlaces {
            boards: HashMap::from([("COM5".to_string(), Arc::clone(&board))]),
            opened: Arc::clone(&opened),
        }));
        assert!(dsc.is_connected(&spec).unwrap());
        assert!(dsc.lines()[0].contains("Caution Panel"));

        let mut panel = dsc.open(&spec, &DisplayCatalogue::default()).unwrap();
        panel.set_lamp(&write(&spec, 0, 1)).unwrap();
        panel.set_lamp(&write(&spec, 1, 40)).unwrap();
        panel.set_lamp(&write(&spec, 1, 90)).unwrap();
        assert_eq!(
            board.lock().unwrap().values,
            [0, 0],
            "nothing before the flush"
        );
        panel.flush().unwrap();
        assert_eq!(board.lock().unwrap().values, [1, 90]);
        assert_eq!(panel.sent().reports, 1, "one batch, one message");

        // Another USB event: the board is still there and is not asked again,
        // which on serial would have reset it.
        dsc.refresh().unwrap();
        assert!(dsc.is_connected(&spec).unwrap());
        assert_eq!(*opened.lock().unwrap(), ["COM5"]);
    }

    #[test]
    fn opening_clears_the_board_first() {
        let (board, spec) = caution();
        board.lock().unwrap().values = vec![1, 255];
        let dsc = Dsc::with(Box::new(FakePlaces {
            boards: HashMap::from([("COM5".to_string(), Arc::clone(&board))]),
            opened: Arc::default(),
        }));
        dsc.open(&spec, &DisplayCatalogue::default()).unwrap();
        assert_eq!(board.lock().unwrap().values, [0, 0]);
    }

    #[test]
    fn a_board_whose_lamps_changed_is_not_driven() {
        let (_, spec) = caution();
        let (_, renumbered) = fake::board(
            "Arduino",
            "Caution Panel",
            "",
            vec![lamp(0, "BACKLIGHT", 255), lamp(1, "FIRE", 1)],
        );
        let dsc = Dsc::with(Box::new(FakePlaces {
            boards: HashMap::from([("COM5".to_string(), renumbered)]),
            opened: Arc::default(),
        }));
        let err = dsc.open(&spec, &DisplayCatalogue::default()).err().unwrap();
        assert!(err.to_string().contains("changed its lamps"), "{err}");
    }

    #[test]
    fn two_boards_with_one_identity_leave_the_second_alone() {
        let (_, a) = fake::board("Arduino", "Panel", "", vec![lamp(0, "FIRE", 1)]);
        let (_, b) = fake::board("Arduino", "Panel", "", vec![lamp(0, "FIRE", 1)]);
        let mut dsc = Dsc::with(Box::new(FakePlaces {
            boards: HashMap::from([("COM3".to_string(), a), ("COM4".to_string(), b)]),
            opened: Arc::default(),
        }));
        assert_eq!(dsc.present().unwrap().len(), 1);
        let lines = dsc.lines();
        assert!(
            lines.iter().any(|l| l.contains("COM4 and COM3 both say")),
            "{lines:?}"
        );
    }

    #[test]
    fn a_place_that_never_answers_is_asked_once() {
        let (_, silent) = fake::board("A", "B", "", vec![]);
        silent.lock().unwrap().hello = None;
        let opened = Arc::new(Mutex::new(Vec::new()));
        let mut dsc = Dsc::with(Box::new(FakePlaces {
            boards: HashMap::from([("COM9".to_string(), silent)]),
            opened: Arc::clone(&opened),
        }));
        dsc.refresh().unwrap();
        dsc.refresh().unwrap();
        assert_eq!(opened.lock().unwrap().len(), 1);
        assert!(dsc.lines()[0].contains("no answer to HELLO"));
    }
}
