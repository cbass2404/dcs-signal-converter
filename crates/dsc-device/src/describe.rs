//! What a board says about itself, as an inventory entry.
//!
//! A board needs no device file written by hand: its `HELLO_REPLY` and one
//! `LAMP` per lamp are everything an entry holds. The key comes from vendor,
//! model and unit, so the same board makes the same key on every plug and
//! every PC, which is what lets a profile's bindings find it again.

use std::collections::BTreeSet;

use dsc_config::{DeviceSpec, Led, LedKind, Part, DSC_PROTOCOL};
use thiserror::Error;

use crate::wire::{HelloReply, LampInfo};

/// Everything a board said about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description {
    pub hello: HelloReply,
    /// In index order, one per lamp the hello counted.
    pub lamps: Vec<LampInfo>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DescribeError {
    #[error("the board says neither a vendor nor a model, so it has no name to bind it by")]
    NoIdentity,
    #[error("lamp {0} has no name, so a profile cannot bind it")]
    Unnamed(u8),
    #[error("lamp {0} is called {1:?}; a name is letters, digits and _ only")]
    BadName(u8, String),
    #[error("two lamps are called {0:?}; a profile could not say which it means")]
    NameTwice(String),
    #[error("lamp {0} answered as lamp {1}")]
    WrongIndex(u8, u8),
}

/// A device key from a board's identity: each part reduced to letters,
/// digits and underscores, joined by underscores, empty parts left out.
///
/// `"Arduino", "Caution Panel", "L"` is `Arduino_Caution_Panel_L`.
pub fn key(vendor: &str, model: &str, unit: &str) -> String {
    [vendor, model, unit]
        .iter()
        .map(|part| {
            part.chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect::<String>()
                .split('_')
                .filter(|w| !w.is_empty())
                .collect::<Vec<_>>()
                .join("_")
        })
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

impl Description {
    pub fn key(&self) -> String {
        key(&self.hello.vendor, &self.hello.model, &self.hello.unit)
    }

    /// What people are shown: the model, and the unit when there is one.
    pub fn display_name(&self) -> String {
        let model = if self.hello.model.is_empty() {
            &self.hello.vendor
        } else {
            &self.hello.model
        };
        if self.hello.unit.is_empty() {
            model.clone()
        } else {
            format!("{model} {}", self.hello.unit)
        }
    }

    /// The board as an inventory entry, or why it cannot be one.
    ///
    /// Refused rather than patched up: a board with two lamps of one name
    /// would bind one of them silently, and a fix in the firmware is the
    /// only one that lasts.
    pub fn spec(&self) -> Result<DeviceSpec, DescribeError> {
        let key = self.key();
        if key.is_empty() {
            return Err(DescribeError::NoIdentity);
        }
        let mut names = BTreeSet::new();
        let mut leds = Vec::with_capacity(self.lamps.len());
        for (i, lamp) in self.lamps.iter().enumerate() {
            if lamp.index as usize != i {
                return Err(DescribeError::WrongIndex(i as u8, lamp.index));
            }
            if lamp.name.is_empty() {
                return Err(DescribeError::Unnamed(lamp.index));
            }
            if !lamp
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(DescribeError::BadName(lamp.index, lamp.name.clone()));
            }
            if !names.insert(lamp.name.as_str()) {
                return Err(DescribeError::NameTwice(lamp.name.clone()));
            }
            leds.push(Led {
                index: lamp.index,
                name: lamp.name.clone(),
                label: if lamp.label.is_empty() {
                    lamp.name.clone()
                } else {
                    lamp.label.clone()
                },
                kind: if lamp.is_indicator() {
                    LedKind::Indicator
                } else {
                    LedKind::Dimmer
                },
                max: Some(lamp.max),
                on_value: Some(lamp.max),
                // The board said so itself, which is as verified as it gets.
                verified: true,
                note: String::new(),
                governs: Vec::new(),
                lights_display: false,
                backlight: lamp.is_backlight(),
            });
        }
        let display_name = self.display_name();
        Ok(DeviceSpec {
            key,
            display_name: display_name.clone(),
            product_name: [self.hello.vendor.as_str(), self.hello.model.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(" "),
            protocol: DSC_PROTOCOL.to_string(),
            usb_pid: 0,
            parts: vec![Part {
                part_id: 0,
                name: display_name,
                leds,
                display: None,
            }],
            buttons: Vec::new(),
            page_keys: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(vendor: &str, model: &str, unit: &str, lamps: u8) -> HelloReply {
        HelloReply {
            version: 1,
            lamps,
            flags: 0,
            vendor: vendor.into(),
            model: model.into(),
            unit: unit.into(),
            firmware: "1.0".into(),
        }
    }

    fn lamp(index: u8, kind: u8, max: u8, flags: u8, name: &str, label: &str) -> LampInfo {
        LampInfo {
            index,
            kind,
            max,
            flags,
            name: name.into(),
            label: label.into(),
        }
    }

    #[test]
    fn the_key_is_the_identity_with_only_safe_characters() {
        assert_eq!(key("Arduino", "Caution Panel", ""), "Arduino_Caution_Panel");
        assert_eq!(
            key("Arduino", "Caution Panel", "L"),
            "Arduino_Caution_Panel_L"
        );
        assert_eq!(
            key("Total Controls", "MPD - Left", ""),
            "Total_Controls_MPD_Left"
        );
        assert_eq!(key("", "", ""), "");
    }

    #[test]
    fn a_board_becomes_an_entry_with_its_own_lamps() {
        let d = Description {
            hello: hello("Arduino", "Caution Panel", "L", 2),
            lamps: vec![
                lamp(0, 1, 1, 0, "MASTER_CAUTION", "Master caution"),
                lamp(1, 0, 255, 0x01, "BACKLIGHT", ""),
            ],
        };
        let spec = d.spec().unwrap();
        assert_eq!(spec.key, "Arduino_Caution_Panel_L");
        assert_eq!(spec.display_name, "Caution Panel L");
        assert_eq!(spec.product_name, "Arduino Caution Panel");
        assert_eq!(spec.protocol, DSC_PROTOCOL);
        let leds = &spec.parts[0].leds;
        assert!(matches!(leds[0].kind, LedKind::Indicator));
        assert_eq!((leds[0].max, leds[0].on_value), (Some(1), Some(1)));
        assert!(leds[1].backlight && matches!(leds[1].kind, LedKind::Dimmer));
        assert_eq!(leds[1].label, "BACKLIGHT", "no label shows the name");
    }

    #[test]
    fn a_board_that_cannot_be_bound_is_refused_saying_why() {
        let two = |a: &str, b: &str| Description {
            hello: hello("Arduino", "Panel", "", 2),
            lamps: vec![lamp(0, 1, 1, 0, a, ""), lamp(1, 1, 1, 0, b, "")],
        };
        assert_eq!(
            two("FIRE", "FIRE").spec().unwrap_err(),
            DescribeError::NameTwice("FIRE".into())
        );
        assert_eq!(
            two("FIRE", "").spec().unwrap_err(),
            DescribeError::Unnamed(1)
        );
        assert_eq!(
            two("FIRE", "GEAR DOWN").spec().unwrap_err(),
            DescribeError::BadName(1, "GEAR DOWN".into())
        );
        let nobody = Description {
            hello: hello("", "", "", 0),
            lamps: vec![],
        };
        assert_eq!(nobody.spec().unwrap_err(), DescribeError::NoIdentity);
    }
}
