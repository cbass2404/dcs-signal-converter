//! Messages, and how each transport carries them.
//!
//! A message is a type byte and its fields, at most [`MAX_MESSAGE`] bytes.
//! Over serial it travels as COBS(message + CRC-8/SMBUS) and a `0x00`; over
//! HID as report 1 holding a length byte, the message and padding.

use thiserror::Error;

pub const VERSION: u8 = 1;
/// The oldest version a board may answer with and still be driven.
pub const OLDEST: u8 = 1;
pub const MAX_MESSAGE: usize = 62;

/// Where a host finds a board over HID: its collection's usage page and usage.
pub const USAGE_PAGE: u16 = 0xFFD5;
pub const USAGE: u16 = 0x01;
pub const REPORT_ID: u8 = 0x01;
/// A HID report, its ID included.
pub const REPORT_LEN: usize = 64;

pub const BAUD: u32 = 115_200;

/// Pairs that fit in one `SET_LAMPS`.
pub const PAIRS_PER_MESSAGE: usize = 30;

const HELLO: u8 = 0x01;
const DESCRIBE: u8 = 0x02;
pub const STATE: u8 = 0x03;
const SET_LAMPS: u8 = 0x10;
const ALL_OFF: u8 = 0x11;
pub const HELLO_REPLY: u8 = 0x81;
const LAMP: u8 = 0x82;
const STATE_REPLY: u8 = 0x83;
const ERROR: u8 = 0xFF;

/// What the host sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Hello,
    Describe(u8),
    /// What the board holds, as a CRC: see [`crc16`].
    State,
    /// At most [`PAIRS_PER_MESSAGE`] (index, value) pairs; see [`set_lamps`].
    SetLamps(Vec<(u8, u8)>),
    AllOff,
}

impl Request {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Request::Hello => vec![HELLO, VERSION],
            Request::Describe(i) => vec![DESCRIBE, *i],
            Request::State => vec![STATE],
            Request::SetLamps(pairs) => {
                let mut m = vec![SET_LAMPS, pairs.len() as u8];
                for (i, v) in pairs {
                    m.extend([*i, *v]);
                }
                m
            }
            Request::AllOff => vec![ALL_OFF],
        }
    }
}

/// Any number of lamp changes as the messages that carry them.
pub fn set_lamps(pairs: &[(u8, u8)]) -> Vec<Request> {
    pairs
        .chunks(PAIRS_PER_MESSAGE)
        .map(|c| Request::SetLamps(c.to_vec()))
        .collect()
}

/// Who a board is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelloReply {
    pub version: u8,
    pub lamps: u8,
    pub flags: u8,
    pub vendor: String,
    pub model: String,
    pub unit: String,
    pub firmware: String,
}

/// One lamp, as the board describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LampInfo {
    pub index: u8,
    /// 0 a dimmer, 1 an indicator. Anything else is from a newer version and
    /// read as a dimmer, which takes every value.
    pub kind: u8,
    pub max: u8,
    pub flags: u8,
    pub name: String,
    pub label: String,
}

impl LampInfo {
    pub const BACKLIGHT: u8 = 0x01;

    pub fn is_indicator(&self) -> bool {
        self.kind == 1
    }

    pub fn is_backlight(&self) -> bool {
        self.flags & Self::BACKLIGHT != 0
    }
}

/// What a board answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Hello(HelloReply),
    Lamp(LampInfo),
    /// [`crc16`] over every lamp's value as the board holds it.
    State(u16),
    /// The message type that failed, and why.
    Error {
        of: u8,
        code: u8,
    },
}

impl Reply {
    pub fn decode(m: &[u8]) -> Result<Reply, WireError> {
        let mut r = Reader { m, at: 1 };
        match m.first() {
            Some(&HELLO_REPLY) => Ok(Reply::Hello(HelloReply {
                version: r.byte()?,
                lamps: r.byte()?,
                flags: r.byte()?,
                vendor: r.string("vendor", 12)?,
                model: r.string("model", 16)?,
                unit: r.string("unit", 8)?,
                firmware: r.string("firmware", 12)?,
            })),
            Some(&LAMP) => Ok(Reply::Lamp(LampInfo {
                index: r.byte()?,
                kind: r.byte()?,
                max: r.byte()?.max(1),
                flags: r.byte()?,
                name: r.string("name", 20)?,
                label: r.string("label", 30)?,
            })),
            Some(&STATE_REPLY) => Ok(Reply::State(u16::from_le_bytes([r.byte()?, r.byte()?]))),
            Some(&ERROR) => Ok(Reply::Error {
                of: r.byte()?,
                code: r.byte()?,
            }),
            Some(&other) => Err(WireError::UnknownType(other)),
            None => Err(WireError::Short),
        }
    }

    /// For the log.
    pub fn error_text(code: u8) -> &'static str {
        match code {
            0x01 => "unknown message type",
            0x02 => "lamp index past the end",
            0x03 => "message too short",
            _ => "an error this version does not know",
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireError {
    #[error("message ends before its fields do")]
    Short,
    #[error("message type 0x{0:02x} is not one a board sends")]
    UnknownType(u8),
    #[error("its {field} is {len} bytes, and the protocol allows {most}")]
    TooLong {
        field: &'static str,
        len: usize,
        most: usize,
    },
}

struct Reader<'a> {
    m: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, WireError> {
        let b = *self.m.get(self.at).ok_or(WireError::Short)?;
        self.at += 1;
        Ok(b)
    }

    /// A length byte and that many bytes of ASCII, at most `most` of them.
    /// Anything outside printable ASCII becomes `?`, so a confused board
    /// cannot put control characters in the log or a file.
    fn string(&mut self, field: &'static str, most: usize) -> Result<String, WireError> {
        let n = self.byte()? as usize;
        if n > most {
            return Err(WireError::TooLong {
                field,
                len: n,
                most,
            });
        }
        let bytes = self.m.get(self.at..self.at + n).ok_or(WireError::Short)?;
        self.at += n;
        Ok(bytes
            .iter()
            .map(|&b| {
                if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    '?'
                }
            })
            .collect())
    }
}

// ------------------------------------------------------------------ serial

/// CRC-8/SMBUS: polynomial 0x07, initial 0, no reflection, no final XOR.
pub fn crc8(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |mut crc, &b| {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x07
            } else {
                crc << 1
            };
        }
        crc
    })
}

/// CRC-16/CCITT-FALSE: polynomial 0x1021, initial 0xFFFF, no reflection, no
/// final XOR. Over every lamp's value in index order, it is a `STATE`.
pub fn crc16(data: &[u8]) -> u16 {
    data.iter().fold(0xFFFFu16, |mut crc, &b| {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
        crc
    })
}

fn cobs_encode(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8];
    let (mut code_at, mut code) = (0usize, 1u8);
    for &b in data {
        if b == 0 {
            out[code_at] = code;
            code_at = out.len();
            out.push(0);
            code = 1;
        } else {
            out.push(b);
            code += 1;
            if code == 0xFF {
                out[code_at] = code;
                code_at = out.len();
                out.push(0);
                code = 1;
            }
        }
    }
    out[code_at] = code;
    out
}

fn cobs_decode(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        let code = data[i] as usize;
        i += 1;
        if code == 0 || i + code - 1 > data.len() {
            return None;
        }
        out.extend_from_slice(&data[i..i + code - 1]);
        i += code - 1;
        if code < 0xFF && i < data.len() {
            out.push(0);
        }
    }
    Some(out)
}

/// One message as the bytes that go down a serial port.
pub fn frame(message: &[u8]) -> Vec<u8> {
    let mut raw = message.to_vec();
    raw.push(crc8(message));
    let mut out = cobs_encode(&raw);
    out.push(0);
    out
}

/// Bytes from a serial port in, whole messages out.
///
/// A frame that fails its CRC, will not decode or is too long to be a message
/// is dropped without a word: the protocol's answer to a bad frame is silence,
/// and the host asks again.
#[derive(Debug, Default)]
pub struct Deframer {
    pending: Vec<u8>,
    overflow: bool,
}

impl Deframer {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for &b in bytes {
            if b != 0 {
                // The longest frame: a message, its CRC and one COBS code.
                if self.pending.len() < MAX_MESSAGE + 2 {
                    self.pending.push(b);
                } else {
                    self.overflow = true;
                }
                continue;
            }
            let frame = std::mem::take(&mut self.pending);
            if std::mem::take(&mut self.overflow) || frame.is_empty() {
                continue;
            }
            let Some(raw) = cobs_decode(&frame) else {
                continue;
            };
            if raw.len() < 2 || raw.len() > MAX_MESSAGE + 1 {
                continue;
            }
            let (message, crc) = raw.split_at(raw.len() - 1);
            if crc8(message) == crc[0] {
                out.push(message.to_vec());
            }
        }
        out
    }
}

// --------------------------------------------------------------------- HID

/// One message as an output report, report ID first.
pub fn report(message: &[u8]) -> [u8; REPORT_LEN] {
    let mut r = [0u8; REPORT_LEN];
    r[0] = REPORT_ID;
    r[1] = message.len() as u8;
    r[2..2 + message.len()].copy_from_slice(message);
    r
}

/// The message in an input report, which may or may not still start with
/// its report ID depending on how it was read. A whole report with another
/// ID, or a read of any other size, is not ours.
pub fn unreport(r: &[u8]) -> Option<Vec<u8>> {
    let payload = match r.len() {
        REPORT_LEN if r[0] == REPORT_ID => &r[1..],
        n if n == REPORT_LEN - 1 => r,
        _ => return None,
    };
    let n = *payload.first()? as usize;
    if n == 0 || n > MAX_MESSAGE {
        return None;
    }
    payload.get(1..1 + n).map(<[u8]>::to_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crc_matches_the_specs_check_value() {
        assert_eq!(crc8(b"123456789"), 0xF4);
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }

    #[test]
    fn frames_round_trip_and_never_hold_a_zero_inside() {
        for sample in [
            vec![0x01],
            vec![0x00],
            vec![0x10, 0x02, 0x00, 0x01, 0x03, 0xFF],
            (0..62).collect::<Vec<u8>>(),
        ] {
            let f = frame(&sample);
            assert_eq!(f.last(), Some(&0));
            assert!(!f[..f.len() - 1].contains(&0), "{sample:?}");
            assert_eq!(Deframer::default().push(&f), vec![sample]);
        }
    }

    #[test]
    fn a_frame_split_across_reads_still_arrives_whole() {
        let f = frame(&Request::Describe(3).encode());
        let mut d = Deframer::default();
        assert!(d.push(&f[..2]).is_empty());
        assert_eq!(d.push(&f[2..]), vec![vec![0x02, 3]]);
    }

    #[test]
    fn a_bad_frame_is_dropped_and_the_next_one_still_arrives() {
        let mut bad = frame(&[0x81, 1, 2]);
        bad[1] ^= 0x40;
        let good = frame(&[0xFF, 0x10, 0x02]);
        let mut d = Deframer::default();
        assert_eq!(d.push(&[bad, good].concat()), vec![vec![0xFF, 0x10, 0x02]]);
    }

    #[test]
    fn noise_longer_than_any_frame_is_dropped_up_to_the_next_zero() {
        let mut d = Deframer::default();
        let mut bytes = vec![0x55; 200];
        bytes.push(0);
        bytes.extend(frame(&[0x82, 0]));
        assert_eq!(d.push(&bytes), vec![vec![0x82, 0]]);
    }

    #[test]
    fn requests_encode_as_the_spec_lays_them_out() {
        assert_eq!(Request::Hello.encode(), [0x01, 1]);
        assert_eq!(Request::Describe(7).encode(), [0x02, 7]);
        assert_eq!(
            Request::SetLamps(vec![(0, 1), (3, 200)]).encode(),
            [0x10, 2, 0, 1, 3, 200]
        );
        assert_eq!(Request::AllOff.encode(), [0x11]);
        assert_eq!(Request::State.encode(), [0x03]);
        assert_eq!(
            Reply::decode(&[0x83, 0xB1, 0x29]).unwrap(),
            Reply::State(0x29B1)
        );
    }

    #[test]
    fn many_lamp_changes_split_into_messages_that_fit() {
        let pairs: Vec<(u8, u8)> = (0..65).map(|i| (i, 1)).collect();
        let msgs = set_lamps(&pairs);
        assert_eq!(msgs.len(), 3);
        for m in &msgs {
            assert!(m.encode().len() <= MAX_MESSAGE);
        }
    }

    #[test]
    fn replies_decode_and_strings_are_kept_printable() {
        let mut m = vec![0x81, 1, 4, 0];
        for s in [&b"Arduino"[..], b"Caution Panel", b"", b"1.0\x07"] {
            m.push(s.len() as u8);
            m.extend_from_slice(s);
        }
        let Reply::Hello(h) = Reply::decode(&m).unwrap() else {
            panic!("a hello reply");
        };
        assert_eq!(
            (
                h.lamps,
                h.vendor.as_str(),
                h.model.as_str(),
                h.unit.as_str()
            ),
            (4, "Arduino", "Caution Panel", "")
        );
        assert_eq!(
            h.firmware, "1.0?",
            "a control character never reaches the log"
        );

        let lamp = [0x82, 2, 1, 0, 0x01, 4, b'F', b'I', b'R', b'E', 0];
        let Reply::Lamp(l) = Reply::decode(&lamp).unwrap() else {
            panic!("a lamp");
        };
        assert!(l.is_indicator() && l.is_backlight());
        assert_eq!(
            (l.max, l.name.as_str(), l.label.as_str()),
            (1, "FIRE", ""),
            "max 0 reads as 1"
        );

        assert_eq!(
            Reply::decode(&[0xFF, 0x10, 0x02]).unwrap(),
            Reply::Error { of: 0x10, code: 2 }
        );
        assert_eq!(Reply::decode(&[0x81, 1]), Err(WireError::Short));
        assert_eq!(Reply::decode(&[0x05]), Err(WireError::UnknownType(0x05)));
    }

    /// Every message cut short at every length is refused, never read past.
    #[test]
    fn a_reply_cut_short_anywhere_is_refused() {
        let mut hello = vec![0x81, 1, 4, 0];
        for s in [&b"Arduino"[..], b"Panel", b"L", b"1.0"] {
            hello.push(s.len() as u8);
            hello.extend_from_slice(s);
        }
        let lamp = vec![0x82, 2, 1, 1, 0, 4, b'F', b'I', b'R', b'E', 1, b'F'];
        for m in [hello, lamp, vec![0xFF, 0x10, 0x02]] {
            assert!(Reply::decode(&m).is_ok(), "{m:?} whole");
            for cut in 0..m.len() {
                assert!(Reply::decode(&m[..cut]).is_err(), "{m:?} cut at {cut}");
            }
        }
    }

    #[test]
    fn a_string_past_its_limit_is_refused() {
        let mut m = vec![0x81, 1, 0, 0, 13];
        m.extend_from_slice(b"Thirteen Byte");
        m.extend([0, 0, 0]);
        assert_eq!(
            Reply::decode(&m),
            Err(WireError::TooLong {
                field: "vendor",
                len: 13,
                most: 12
            })
        );
    }

    #[test]
    fn a_frame_longer_than_any_message_is_dropped() {
        let longest = frame(&[0x55; MAX_MESSAGE]);
        assert_eq!(longest.len(), MAX_MESSAGE + 3, "message, CRC, code, 0x00");
        assert_eq!(Deframer::default().push(&longest).len(), 1);
        // One byte more than that, valid COBS and a good CRC.
        let mut raw = vec![0x55; MAX_MESSAGE + 1];
        raw.push(crc8(&raw));
        let mut over = cobs_encode(&raw);
        over.push(0);
        assert!(Deframer::default().push(&over).is_empty());
    }

    #[test]
    fn a_report_with_another_id_or_size_is_not_ours() {
        let mut r = report(&[0xFF, 0x10, 0x02]);
        r[0] = 0x02;
        assert_eq!(unreport(&r), None);
        assert_eq!(unreport(&report(&[0x11])[..10]), None);
        let mut lying = report(&[0x11]);
        lying[1] = 63;
        assert_eq!(unreport(&lying), None, "a length above 62");
    }

    #[test]
    fn reports_round_trip_with_or_without_their_id() {
        let msg = Request::SetLamps(vec![(1, 255)]).encode();
        let r = report(&msg);
        assert_eq!((r[0], r[1]), (REPORT_ID, msg.len() as u8));
        assert_eq!(unreport(&r).unwrap(), msg);
        assert_eq!(
            unreport(&r[1..]).unwrap(),
            msg,
            "read with the ID taken off"
        );
        assert_eq!(unreport(&[0u8; 63]), None, "an empty report is nothing");
    }
}
