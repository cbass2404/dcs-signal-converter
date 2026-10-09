//! The host side of the DSC device protocol, docs/PROTOCOL-DSC.md.
//!
//! A board running the DscDevice library, or any firmware that speaks the
//! protocol, says who it is and what lamps it has, and sets what it is told.
//! This crate is what the daemon and the editor share about that: the
//! messages and their framing on each transport ([`wire`]), and turning what
//! a board says about itself into an inventory entry ([`describe`]).
//!
//! Nothing here opens a port. Which ports may be opened, and when, is the
//! caller's business, because a serial port can belong to anything.

pub mod describe;
pub mod fake;
pub mod link;
pub mod session;
pub mod wire;

pub use describe::{key, DescribeError, Description};
pub use link::{hid_boards, serial_ports, HidLink, Link, SerialLink};
pub use session::{scan, ScanError, HID_WAIT, SERIAL_WAIT};
pub use wire::{HelloReply, LampInfo, Reply, Request};
