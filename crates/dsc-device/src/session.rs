//! Asking a board who it is and what it has.

use std::time::{Duration, Instant};

use thiserror::Error;

use crate::describe::Description;
use crate::link::Link;
use crate::wire::{HelloReply, Reply, Request, WireError};

/// How long a serial board gets to answer: opening its port resets most of
/// them, and the bootloader runs first.
pub const SERIAL_WAIT: Duration = Duration::from_secs(3);
/// How long a HID board gets. It was running before the host looked.
pub const HID_WAIT: Duration = Duration::from_secs(1);
/// Between one unanswered `HELLO` and the next.
const HELLO_EVERY: Duration = Duration::from_millis(250);
/// For each `DESCRIBE`, which a running board answers at once.
const DESCRIBE_WAIT: Duration = Duration::from_millis(500);

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("no answer to HELLO: not a board running the DSC protocol, or not running yet")]
    NoAnswer,
    #[error("no answer describing lamp {0}")]
    NoLamp(u8),
    #[error("the board refused {of:#04x}: {}", Reply::error_text(*.code))]
    Refused { of: u8, code: u8 },
    #[error("{0}")]
    Wire(#[from] WireError),
}

/// Ask until the board answers `HELLO` or `wait` runs out.
pub fn hello(link: &mut dyn Link, wait: Duration) -> Result<HelloReply, ScanError> {
    let until = Instant::now() + wait;
    while Instant::now() < until {
        link.send(&Request::Hello.encode())?;
        let asked = Instant::now();
        while asked.elapsed() < HELLO_EVERY {
            match link.receive(HELLO_EVERY.saturating_sub(asked.elapsed()))? {
                Some(m) => {
                    // Anything else is left over from before; skip it.
                    if let Ok(Reply::Hello(h)) = Reply::decode(&m) {
                        return Ok(h);
                    }
                }
                None => break,
            }
        }
    }
    Err(ScanError::NoAnswer)
}

/// Everything the board says about itself: `HELLO`, then each lamp.
pub fn scan(link: &mut dyn Link, wait: Duration) -> Result<Description, ScanError> {
    let hello = hello(link, wait)?;
    let mut lamps = Vec::with_capacity(hello.lamps as usize);
    for index in 0..hello.lamps {
        link.send(&Request::Describe(index).encode())?;
        let asked = Instant::now();
        let lamp = loop {
            let left = DESCRIBE_WAIT.saturating_sub(asked.elapsed());
            let Some(m) = link.receive(left)? else {
                return Err(ScanError::NoLamp(index));
            };
            match Reply::decode(&m)? {
                Reply::Lamp(l) if l.index == index => break l,
                Reply::Error { of, code } => return Err(ScanError::Refused { of, code }),
                // A late answer to an earlier question.
                _ => continue,
            }
        };
        lamps.push(lamp);
    }
    Ok(Description { hello, lamps })
}
