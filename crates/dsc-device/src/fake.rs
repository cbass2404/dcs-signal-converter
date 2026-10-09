//! A board in memory that answers as the DscDevice library does, for tests
//! that have no hardware. Its state is shared, so a test can look at what it
//! was told after the link has gone into a panel.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::link::Link;
use crate::wire::{HelloReply, LampInfo, VERSION};

#[derive(Debug, Default)]
pub struct Board {
    pub hello: Option<HelloReply>,
    pub lamps: Vec<LampInfo>,
    /// Each lamp's value as last set, clamped as the firmware clamps.
    pub values: Vec<u8>,
    /// `HELLO`s to ignore first, as a board does while it resets.
    pub deaf_for: usize,
    /// Every message it was sent, in order.
    pub heard: Vec<Vec<u8>>,
    outbox: VecDeque<Vec<u8>>,
}

pub type Shared = Arc<Mutex<Board>>;

/// A link to a new fake board, and the board itself to look at.
pub fn board(vendor: &str, model: &str, unit: &str, lamps: Vec<LampInfo>) -> (FakeLink, Shared) {
    let shared = Arc::new(Mutex::new(Board {
        hello: Some(HelloReply {
            version: VERSION,
            lamps: lamps.len() as u8,
            flags: 0,
            vendor: vendor.into(),
            model: model.into(),
            unit: unit.into(),
            firmware: "fake".into(),
        }),
        values: vec![0; lamps.len()],
        lamps,
        ..Board::default()
    }));
    (FakeLink(Arc::clone(&shared)), shared)
}

/// An indicator, or with `max` above 1 a dimmer.
pub fn lamp(index: u8, name: &str, max: u8) -> LampInfo {
    LampInfo {
        index,
        kind: if max == 1 { 1 } else { 0 },
        max,
        flags: 0,
        name: name.into(),
        label: String::new(),
    }
}

pub struct FakeLink(pub Shared);

fn string(out: &mut Vec<u8>, s: &str) {
    out.push(s.len() as u8);
    out.extend_from_slice(s.as_bytes());
}

impl Link for FakeLink {
    fn send(&mut self, m: &[u8]) -> io::Result<()> {
        let mut b = self.0.lock().unwrap();
        b.heard.push(m.to_vec());
        let reply = match m.first() {
            Some(0x01) => {
                if b.deaf_for > 0 {
                    b.deaf_for -= 1;
                    return Ok(());
                }
                let Some(h) = b.hello.clone() else {
                    return Ok(());
                };
                let mut out = vec![0x81, h.version, h.lamps, h.flags];
                for s in [&h.vendor, &h.model, &h.unit, &h.firmware] {
                    string(&mut out, s);
                }
                Some(out)
            }
            Some(0x02) => match b.lamps.get(m[1] as usize).cloned() {
                Some(l) => {
                    let mut out = vec![0x82, l.index, l.kind, l.max, l.flags];
                    string(&mut out, &l.name);
                    string(&mut out, &l.label);
                    Some(out)
                }
                None => Some(vec![0xFF, 0x02, 0x02]),
            },
            Some(0x10) => {
                let count = m[1] as usize;
                let mut err = None;
                for i in 0..count {
                    let (index, value) = (m[2 + 2 * i] as usize, m[3 + 2 * i]);
                    if index >= b.lamps.len() {
                        err = Some(vec![0xFF, 0x10, 0x02]);
                        break;
                    }
                    let max = b.lamps[index].max;
                    b.values[index] = value.min(max);
                }
                err
            }
            Some(0x11) => {
                b.values.iter_mut().for_each(|v| *v = 0);
                None
            }
            Some(&t) => Some(vec![0xFF, t, 0x01]),
            None => None,
        };
        if let Some(r) = reply {
            b.outbox.push_back(r);
        }
        Ok(())
    }

    fn receive(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().outbox.pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{scan, ScanError};

    #[test]
    fn a_scan_reads_who_the_board_is_and_every_lamp() {
        let (mut link, _) = board(
            "Arduino",
            "Caution Panel",
            "",
            vec![lamp(0, "FIRE", 1), lamp(1, "BACKLIGHT", 255)],
        );
        let d = scan(&mut link, Duration::from_millis(100)).unwrap();
        assert_eq!(d.key(), "Arduino_Caution_Panel");
        assert_eq!(d.lamps.len(), 2);
        assert_eq!(d.lamps[1].name, "BACKLIGHT");
    }

    #[test]
    fn a_board_that_is_resetting_is_asked_again() {
        let (mut link, board) = board("Arduino", "Panel", "", vec![lamp(0, "FIRE", 1)]);
        board.lock().unwrap().deaf_for = 3;
        assert!(scan(&mut link, Duration::from_secs(2)).is_ok());
        let hellos = board
            .lock()
            .unwrap()
            .heard
            .iter()
            .filter(|m| m[0] == 0x01)
            .count();
        assert_eq!(hellos, 4);
    }

    #[test]
    fn something_that_never_answers_is_given_up_on() {
        let (mut link, board) = board("A", "B", "", vec![]);
        board.lock().unwrap().hello = None;
        assert!(matches!(
            scan(&mut link, Duration::from_millis(300)),
            Err(ScanError::NoAnswer)
        ));
    }
}
