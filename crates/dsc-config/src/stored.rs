//! Stored signals: a module's numbers worked out once and named, for any page
//! on it to draw. See docs/CONFIG.md "Stored signals".
//!
//! A reading shapes its own number, and two fields drawing the same fuel
//! drums each say how, in full, and each work it out every paint. A stored
//! signal says it once. It is a chain of terms, each one DCS-BIOS signal
//! shaped the way a reading is, laid side by side as characters: the F-16's
//! three fuel drums, each rounded down and wrapped at 10, become one `123`.
//! Laid side by side rather than added, because a drum that is rolling is
//! already partway to the next digit, and each one settled on its own is what
//! keeps the one above it from counting that twice.
//!
//! Only the number lives here. How it is drawn, its words, colour and size,
//! is the field's, so two pages can draw one signal two ways.
//!
//! Kept in the module's page file beside the pages, since pages are what draw
//! them. A field names one by `signal`, its id, and the page file attaches the
//! definition when it loads, so everything downstream reads a field exactly
//! as it always has.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::display::{is_left, is_nearest, is_zero_u8, is_zero_usize};
use crate::{Align, Branch, Condition, Conversion, Pick, Reading, Round, Span};

/// A named number on one module, made from one or more DCS-BIOS signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSignal {
    /// Fixed when the signal is made and unique across the library, like a
    /// page's. What a field points at, so renaming one rewrites nothing.
    pub id: String,
    /// What the editor shows. Unique within the module, ignoring case and
    /// surrounding space.
    pub name: String,
    /// What it is for, in the user's words.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// What it reads, in the order the characters are laid down. A number
    /// for a screen. Empty on a signal of lamp conditions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terms: Vec<Term>,
    /// Tests that must all hold, written as a lamp's are, for a signal a
    /// lamp draws: one set of conditions, many lamps. Each test lights the
    /// lamp at its own `on` or leaves it at its `off`, so the answer is on or
    /// off, except a scale, which is a dimmer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    /// Alternatives, each a set of tests, as a lamp's `any_of` is: the CH-47's
    /// master caution is the pilot's lamp in the pilot's seat and the
    /// copilot's in the copilot's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub any_of: Vec<Branch>,
    /// How the alternatives are picked between, as on a lamp.
    #[serde(default, skip_serializing_if = "Pick::is_brightest")]
    pub pick: Pick,
}

/// One DCS-BIOS signal in a stored signal, and how it is shaped.
///
/// Held as a [`Span`] so the arithmetic is the one a reading does, to the
/// digit. Written with only the keys that shape a number and the box it
/// fills, because the rest of a span is how a field draws, and that belongs
/// to the field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "TermRepr", into = "TermRepr")]
pub struct Term(pub Span);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TermRepr {
    #[serde(default)]
    source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reads: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    conversions: Vec<Conversion>,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    decimals: u8,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    digits: u8,
    #[serde(default, skip_serializing_if = "is_nearest")]
    round: Round,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wrap: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    abs: bool,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    width: usize,
    #[serde(default, skip_serializing_if = "is_left")]
    align: Align,
}

impl From<TermRepr> for Term {
    fn from(r: TermRepr) -> Self {
        Term(Span {
            source: r.source,
            reads: r.reads,
            conversions: r.conversions,
            decimals: r.decimals,
            digits: r.digits,
            round: r.round,
            wrap: r.wrap,
            abs: r.abs,
            width: r.width,
            align: r.align,
            ..Span::default()
        })
    }
}

impl From<Term> for TermRepr {
    fn from(t: Term) -> Self {
        let s = t.0;
        TermRepr {
            source: s.source,
            reads: s.reads,
            conversions: s.conversions,
            decimals: s.decimals,
            digits: s.digits,
            round: s.round,
            wrap: s.wrap,
            abs: s.abs,
            width: s.width,
            align: s.align,
        }
    }
}

/// What a stored signal reads right now.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredValue {
    /// The characters, every term laid down in order.
    pub text: String,
    /// The same characters read as a number, where they are one. What a
    /// field's bands are matched against.
    pub number: Option<f64>,
}

impl StoredValue {
    fn new(text: String) -> Self {
        let number = text.trim().parse::<f64>().ok().filter(|n| n.is_finite());
        StoredValue { text, number }
    }

    /// Half of the last decimal place the characters show, which is how close
    /// a reading has to be to a band's edge to count as inside it: the same
    /// allowance a reading gives its own bands.
    pub fn tolerance(&self) -> f64 {
        let places = self
            .text
            .trim()
            .split_once('.')
            .map_or(0, |(_, after)| after.chars().count());
        10f64.powi(-(places as i32)) / 2.0
    }
}

impl StoredSignal {
    /// Whether this is lamp conditions rather than a number made of parts.
    pub fn is_lamp(&self) -> bool {
        !self.conditions.is_empty() || !self.any_of.is_empty()
    }

    /// Every DCS-BIOS signal this reads: its parts, or its conditions.
    pub fn sources(&self) -> impl Iterator<Item = &str> {
        self.terms.iter().map(|t| t.0.source.as_str()).chain(
            self.conditions
                .iter()
                .chain(self.any_of.iter().flat_map(|b| b.conditions.iter()))
                .map(|c| c.source.as_str()),
        )
    }

    /// What each term's signal reads right now, in term order, or None until
    /// every one has arrived.
    ///
    /// All of them rather than what there is, because a number missing a
    /// digit is a different number: `23` where the drums say `123`. A field
    /// beside it can still draw its label.
    fn inputs<F>(&self, read: &F) -> Option<Vec<Reading>>
    where
        F: Fn(&str) -> Option<Reading>,
    {
        self.terms.iter().map(|t| read(&t.0.source)).collect()
    }

    /// What this reads given how signals read right now, or None until every
    /// term has arrived.
    ///
    /// `seen` is told each number a term converts, as a reading tells it.
    pub fn evaluate<'a, F, S>(&'a self, read: &F, seen: &mut S) -> Option<StoredValue>
    where
        F: Fn(&str) -> Option<Reading>,
        S: FnMut(&'a str, u16, &str),
    {
        Some(self.evaluate_from(&self.inputs(read)?, seen))
    }

    /// What this reads from `inputs`, one reading per term.
    fn evaluate_from<'a, S>(&'a self, inputs: &[Reading], seen: &mut S) -> StoredValue
    where
        S: FnMut(&'a str, u16, &str),
    {
        let mut text = String::new();
        for (term, input) in self.terms.iter().zip(inputs) {
            let span = &term.0;
            let drawn = match input {
                Reading::Text(t) => t.clone(),
                Reading::Number { value, max } => {
                    let drawn = span.format_number(*value, *max);
                    seen(&span.source, *value, &drawn);
                    drawn
                }
            };
            text.push_str(&span.fit_text(&drawn));
        }
        StoredValue::new(text)
    }
}

/// Stored signals worked out only when what they read has moved.
///
/// Each signal's last inputs are kept with what they made, so a signal whose
/// terms all read what they read last time is handed its last answer: the
/// second field drawing one in a paint, and every paint after while its
/// drums stand still. Asking costs a lookup of each term's signal, which the
/// paint does anyway; the shaping is what is saved.
///
/// The engine keeps one across paints and starts a fresh one whenever it
/// takes a profile, since a signal edited in the editor keeps its id and an
/// answer worked out from its old terms would be wrong.
#[derive(Debug, Default)]
pub struct StoredCache(RefCell<HashMap<String, (Vec<Reading>, StoredValue)>>);

impl StoredCache {
    /// The value of `signal`, worked out again only if a term has moved.
    pub(crate) fn get<'a, F, S>(
        &self,
        signal: &'a StoredSignal,
        read: &F,
        seen: &mut S,
    ) -> Option<StoredValue>
    where
        F: Fn(&str) -> Option<Reading>,
        S: FnMut(&'a str, u16, &str),
    {
        let inputs = signal.inputs(read)?;
        if let Some((was, value)) = self.0.borrow().get(&signal.id) {
            if *was == inputs {
                return Some(value.clone());
            }
        }
        let value = signal.evaluate_from(&inputs, seen);
        self.0
            .borrow_mut()
            .insert(signal.id.clone(), (inputs, value.clone()));
        Some(value)
    }
}

/// Every field piece on a page that names a stored signal, given its
/// definition from `signals`, or none where there is no such signal.
///
/// Inside switch cases too, whose pieces were resolved from what the case
/// wrote and are what a frame draws.
pub(crate) fn attach(fields: &mut [crate::Readout], signals: &BTreeMap<String, Arc<StoredSignal>>) {
    let find = |span: &mut Span| {
        span.stored = if span.signal.is_empty() {
            None
        } else {
            signals.get(&span.signal).cloned()
        };
    };
    for field in fields {
        for span in &mut field.content {
            find(span);
            span.switch_stored = if span.switch_signal.is_empty() {
                None
            } else {
                signals.get(&span.switch_signal).cloned()
            };
            for case in &mut span.cases.0 {
                case.pieces.iter_mut().for_each(&find);
            }
        }
    }
}
