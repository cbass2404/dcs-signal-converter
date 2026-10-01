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
use crate::{Align, Conversion, Reading, Round, Span};

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
    /// What it reads, in the order the characters are laid down.
    #[serde(default)]
    pub terms: Vec<Term>,
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
    /// Every DCS-BIOS signal this reads, in term order.
    pub fn sources(&self) -> impl Iterator<Item = &str> {
        self.terms.iter().map(|t| t.0.source.as_str())
    }

    /// What this reads given how signals read right now, or None until every
    /// term has arrived.
    ///
    /// All of them rather than what there is, because a number missing a
    /// digit is a different number: `23` where the drums say `123`. A field
    /// beside it can still draw its label.
    ///
    /// `seen` is told each number a term converts, as a reading tells it.
    pub fn evaluate<'a, F, S>(&'a self, read: &F, seen: &mut S) -> Option<StoredValue>
    where
        F: Fn(&str) -> Option<Reading>,
        S: FnMut(&'a str, u16, &str),
    {
        let mut text = String::new();
        for term in &self.terms {
            let span = &term.0;
            let drawn = match read(&span.source)? {
                Reading::Text(t) => t,
                Reading::Number { value, max } => {
                    let drawn = span.format_number(value, max);
                    seen(&span.source, value, &drawn);
                    drawn
                }
            };
            text.push_str(&span.fit_text(&drawn));
        }
        Some(StoredValue::new(text))
    }
}

/// Stored signals worked out once a paint, however many fields draw them.
///
/// Every signal reads the state as it is for the whole paint, so the second
/// field drawing one is handed the first one's answer. A fresh cache is a
/// fresh paint.
#[derive(Debug, Default)]
pub struct StoredCache(RefCell<HashMap<String, Option<StoredValue>>>);

impl StoredCache {
    /// The value of `signal`, worked out on the first ask this paint.
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
        if let Some(known) = self.0.borrow().get(&signal.id) {
            return known.clone();
        }
        let value = signal.evaluate(read, seen);
        self.0.borrow_mut().insert(signal.id.clone(), value.clone());
        value
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
            for case in &mut span.cases.0 {
                case.pieces.iter_mut().for_each(&find);
            }
        }
    }
}
