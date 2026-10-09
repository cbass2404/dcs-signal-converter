//! The host's protocol code against the firmware's, and the firmware against
//! messages that are cut short, too long or random.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use dsc_device::wire::{frame, report, unreport, Deframer, Reply, Request};
use dsc_device::{scan, Link};
use dsc_firmware::{pin, HidBoard, SerialBoard, BACKLIGHT, FIRE, GAUGE};

const UNKNOWN: u8 = 0x01;
const NO_SUCH_LAMP: u8 = 0x02;
const SHORT: u8 = 0x03;

fn error(of: u8, code: u8) -> Vec<u8> {
    vec![0xFF, of, code]
}

/// The firmware at the far end of a serial port, as the host sees it.
struct Serial {
    board: SerialBoard,
    deframer: Deframer,
    ready: VecDeque<Vec<u8>>,
}

impl Serial {
    fn new() -> Serial {
        Serial {
            board: SerialBoard::new(),
            deframer: Deframer::default(),
            ready: VecDeque::new(),
        }
    }

    /// Bytes down the wire, whatever they are.
    fn raw(&mut self, bytes: &[u8]) {
        self.board.feed(bytes);
    }

    /// Every message the board sent since last asked.
    fn replies(&mut self) -> Vec<Vec<u8>> {
        let mut out: Vec<Vec<u8>> = self.ready.drain(..).collect();
        out.extend(self.deframer.push(&self.board.take()));
        out
    }

    /// One message, framed, and what came back.
    fn ask(&mut self, message: &[u8]) -> Vec<Vec<u8>> {
        self.raw(&frame(message));
        self.replies()
    }
}

impl Link for Serial {
    fn send(&mut self, message: &[u8]) -> io::Result<()> {
        self.raw(&frame(message));
        Ok(())
    }

    fn receive(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        if self.ready.is_empty() {
            let got = self.deframer.push(&self.board.take());
            self.ready.extend(got);
        }
        Ok(self.ready.pop_front())
    }
}

/// xorshift64, so a failure replays from its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn byte(&mut self) -> u8 {
        self.next() as u8
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.byte()).collect()
    }
}

#[test]
fn the_host_scans_the_real_firmware() {
    let mut s = Serial::new();
    let d = scan(&mut s, Duration::from_millis(100)).unwrap();
    assert_eq!(d.key(), "Arduino_Test_Panel");
    let names: Vec<&str> = d.lamps.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["FIRE", "BACKLIGHT", "GAUGE"]);
    assert!(d.lamps[1].is_backlight());
    d.spec().unwrap();
}

#[test]
fn lamps_are_set_clamped_to_their_max_and_cleared() {
    let mut s = Serial::new();
    let set = Request::SetLamps(vec![(0, 1), (1, 128), (2, 200)]).encode();
    assert!(s.ask(&set).is_empty(), "no answer on success");
    assert_eq!(
        (pin(FIRE), pin(BACKLIGHT), pin(GAUGE)),
        (Some(1), Some(128), Some(255)),
        "200 on a lamp whose max is 100 is full"
    );
    s.ask(&Request::AllOff.encode());
    assert_eq!(
        (pin(FIRE), pin(BACKLIGHT), pin(GAUGE)),
        (Some(0), Some(0), Some(0))
    );
}

#[test]
fn a_request_cut_short_is_refused_and_changes_nothing() {
    let mut s = Serial::new();
    let cut: [&[u8]; 5] = [
        &[0x01],
        &[0x02],
        &[0x10],
        &[0x10, 1, 0],
        // Promises three pairs and holds two.
        &[0x10, 3, 0, 1, 1, 9],
    ];
    for m in cut {
        assert_eq!(s.ask(m), [error(m[0], SHORT)], "{m:?}");
    }
    assert_eq!((pin(FIRE), pin(BACKLIGHT)), (Some(0), Some(0)));
}

#[test]
fn a_lamp_past_the_end_stops_the_batch_there() {
    let mut s = Serial::new();
    assert_eq!(
        s.ask(&[0x10, 3, 0, 1, 7, 1, 1, 50]),
        [error(0x10, NO_SUCH_LAMP)]
    );
    assert_eq!((pin(FIRE), pin(BACKLIGHT)), (Some(1), Some(0)));
    assert_eq!(s.ask(&[0x02, 3]), [error(0x02, NO_SUCH_LAMP)]);
}

#[test]
fn an_unknown_type_is_refused_and_extra_bytes_are_ignored() {
    let mut s = Serial::new();
    assert_eq!(s.ask(&[0x42, 1, 2]), [error(0x42, UNKNOWN)]);
    assert_eq!(s.ask(&[0x81]), [error(0x81, UNKNOWN)], "a reply type");
    let r = s.ask(&[0x01, 1, 0xAA, 0xBB]);
    assert!(
        matches!(Reply::decode(&r[0]), Ok(Reply::Hello(_))),
        "bytes after the fields are from a newer version"
    );
}

#[test]
fn a_frame_broken_in_transport_gets_silence_and_the_next_is_answered() {
    let mut s = Serial::new();
    let mut bad_crc = frame(&[0x02, 0]);
    bad_crc[1] ^= 0x01;
    s.raw(&bad_crc);
    // Longer than any frame.
    s.raw(&[0x55; 200]);
    s.raw(&[0]);
    // A COBS code promising more bytes than the frame holds.
    s.raw(&[0x05, 0x01, 0x00]);
    // Decodes to nothing, so not even room for a CRC.
    s.raw(&[0x01, 0x00]);
    s.raw(&[0x00, 0x00]);
    assert!(s.replies().is_empty());
    assert_eq!(s.ask(&[0x02, 0]).len(), 1);
}

#[test]
fn the_longest_message_is_taken_and_one_byte_more_is_not() {
    let mut s = Serial::new();
    let mut longest = vec![0x01, 1];
    longest.resize(62, 0xAA);
    assert_eq!(s.ask(&longest).len(), 1);
    let mut over = longest;
    over.push(0xAA);
    assert!(s.ask(&over).is_empty());
}

/// Random messages with good CRCs, so each reaches the message handling, and
/// raw noise between them. Under `DSC_ASAN` a read past any buffer crashes
/// here; either way, everything the board answers is a message the host reads.
#[test]
fn random_serial_input_never_breaks_the_firmware() {
    let mut s = Serial::new();
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    for _ in 0..20_000 {
        let len = rng.below(70);
        let mut m = rng.bytes(len);
        // Mostly types it knows, so each handler sees every length.
        let t = [0x01, 0x02, 0x10, 0x11, rng.byte()][rng.below(5)];
        if let Some(first) = m.first_mut() {
            *first = t;
        }
        if rng.below(4) == 0 {
            let noise = rng.below(80);
            s.raw(&rng.bytes(noise));
        } else {
            s.raw(&frame(&m));
        }
        for r in s.replies() {
            assert!(Reply::decode(&r).is_ok(), "{r:?} after {m:?}");
        }
    }
    // Close any frame the noise left open; the board still answers.
    s.raw(&[0]);
    let r = s.ask(&Request::Hello.encode());
    assert!(matches!(Reply::decode(&r[0]), Ok(Reply::Hello(_))));
}

#[test]
fn hid_reports_are_taken_either_way_and_bad_ones_dropped() {
    let mut h = HidBoard::open();
    let hello = report(&Request::Hello.encode());

    // From the OUT endpoint, the report ID still first.
    h.report(0, &hello);
    let r = h.take().unwrap();
    assert_eq!(r.len(), 64);
    assert!(matches!(
        Reply::decode(&unreport(&r).unwrap()),
        Ok(Reply::Hello(_))
    ));
    // From a control transfer, the ID apart.
    h.report(1, &hello[1..]);
    assert!(h.take().is_some());

    let mut wrong_id = hello;
    wrong_id[0] = 2;
    h.report(0, &wrong_id);
    h.report(2, &hello[1..]);
    for n in [0u8, 63, 200] {
        let mut r = hello;
        r[1] = n;
        h.report(0, &r);
    }
    // A length of 5 in a payload of 3.
    h.report(0, &[1, 5, 0x01, 1]);
    h.report(0, &[]);
    h.report(0, &[1]);
    h.report(1, &[]);
    assert_eq!(h.take(), None);

    h.report(0, &report(&[0x02]));
    assert_eq!(unreport(&h.take().unwrap()), Some(error(0x02, SHORT)));
}

#[test]
fn random_hid_reports_never_break_the_firmware() {
    let mut h = HidBoard::open();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..20_000 {
        let len = rng.below(80);
        let mut r = rng.bytes(len);
        // Mostly our report ID and a length that fits, so most reach the
        // message handling.
        let id = rng.below(2) as u8;
        if id == 0 && !r.is_empty() && rng.below(4) != 0 {
            r[0] = 1;
        }
        let at = if id == 0 { 1 } else { 0 };
        if r.len() > at && rng.below(4) != 0 {
            r[at] = (rng.below(62) + 1) as u8;
        }
        h.report(id, &r);
        while let Some(sent) = h.take() {
            let m = unreport(&sent).expect("every report the board sends is whole");
            assert!(Reply::decode(&m).is_ok(), "{m:?} after {r:?}");
        }
    }
    h.report(0, &report(&Request::Hello.encode()));
    let m = unreport(&h.take().unwrap()).unwrap();
    assert!(matches!(Reply::decode(&m), Ok(Reply::Hello(_))));
}
