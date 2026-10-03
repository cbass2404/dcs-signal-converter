//! Displays: the cell and glyph map, and a host-side screen buffer.
//!
//! A WinCtrl panel with glass does not take text. It takes a bitmap of
//! segments, written a few bytes at a time, and a character position is a set
//! of bit indices scattered through that bitmap. `data/displays/*.json` holds
//! the map, transcribed from SimAppPro's tables and confirmed against captured
//! hardware traffic. See `docs/PROTOCOL-WINCTRL.md`.
//!
//! A pixel screen is the same model with a regular layout. Its bit index is a
//! pixel, `y * width + x`, so a character cell is the pixels of its box and a
//! glyph is the pixels it lights. The ICP's DED is 120 such cells, generated
//! from a [`Grid`] rather than listed, with the font drawn as rows of `#`.
//!
//! Two things here are correctness requirements rather than optimisations:
//!
//! * A write group holds bits belonging to more than one cell, so changing one
//!   character means read-modify-write of its group. That is what [`Screen`]
//!   exists for; it is not a cache.
//! * A cell can straddle two groups, so a character change can take two frames
//!   and the cell reads as a different, wrong letter in between. Diffing whole
//!   groups from a settled buffer is what keeps that off the glass.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::stored::{StoredCache, StoredSignal, StoredValue};
use crate::{read_json, Error, Result};

/// One character position on a display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub index: usize,
    /// Which glyph table this cell draws from, such as `alnum16` or `digit7`.
    /// A 7-segment cell cannot show a letter, and asking it to is an error
    /// rather than a blank, because silently blanking a cell looks like a
    /// wiring fault and sends you hunting in the wrong place.
    pub shape: String,
    /// How many characters this cell holds.
    ///
    /// Nearly always one. The UFC comm windows are two: a units digit and a
    /// partial tens, on one cell, addressed as one glyph. A wide cell fits its
    /// value to this width before the glyph is looked up, because modules do
    /// not agree on the padding: the Hornet sends `" 2"`, the Hind sends `"1"`
    /// from a one-character field and `"1 "` from a two-character one, and all
    /// three mean the same channel.
    #[serde(default = "one")]
    pub width: usize,
    /// Absolute bit index in the device buffer for each of this cell's segment
    /// slots, in the order the glyph tables index them.
    pub segments: Vec<u16>,
}

fn one() -> usize {
    1
}

/// How one shape's slots are drawn on a screen that is not the panel.
///
/// The editor previews a field before anyone flies with it, and a preview of
/// glass like this cannot be made of characters: a cell draws whatever its
/// glyph table says it draws, and that is a set of slots, not a letter. The
/// two kinds of glass answer "what does slot 3 look like" differently, so this
/// is what the answer is asked for through.
///
/// Knowing what a slot looks like is knowing the panel, not knowing the
/// profile, which is why it lives here beside the glyph tables rather than in
/// the window.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ShapeArt {
    /// A rectangle of pixels: slot `n` is the pixel `n % width`, `n / width`.
    /// Generated from the [`Grid`] that made the cells, so a pixel screen
    /// never writes its art down.
    Pixels { width: usize, height: usize },
    /// Strokes per slot, for glass whose slots are segments rather than
    /// pixels. There is nothing to generate these from: a segment's shape is
    /// nowhere in a map of bit indices, so they are written down in the
    /// display file.
    Strokes(StrokeArt),
}

/// The segments of one shape, drawn.
///
/// A slot is a list of strokes, and a stroke is a flat run of `x, y` pairs, so
/// `[1, 2, 5, 2]` is a line and `[3, 4, 3, 4]` a dot. A list rather than one
/// stroke because a slot is not always one mark: the UFC's option cue is a
/// single slot that draws two dots.
///
/// The coordinates are in a box of `width` by `height` of this shape's own,
/// which is how a narrow cue cell sits beside a wide letter cell at the size
/// each really is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokeArt {
    pub width: f32,
    pub height: f32,
    /// How thick a lit segment is drawn, in the same units.
    pub stroke: f32,
    /// Per slot, in the order the glyph tables number them.
    pub slots: Vec<Vec<Vec<f32>>>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// How a display's buffer reaches the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    /// `SET_LCDS` on the command channel, one group of `group_bytes` at a time.
    /// Never acknowledged.
    #[default]
    Segment,
    /// Report `0xf0`: writes of any length into a framebuffer, then a commit
    /// to show them. A bit index is a pixel.
    Pixel,
    /// Report `0xf2`: a grid of characters the panel draws from a font it was
    /// sent. The MCDU. Only ever written whole, so the buffer is one group.
    Text,
}

/// A character grid the panel renders itself, from a font uploaded to it.
///
/// Unlike a segment or pixel display, a cell holds a character and a colour,
/// not a bitmap. The panel keeps no font across a power cycle, so which font
/// to upload is part of the display's description. It is not the user's
/// choice: an aircraft with a CDU of its own has glyphs drawn to match what
/// DCS-BIOS sends for it, and that font is the aircraft's.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextGrid {
    pub columns: usize,
    pub rows: usize,
    /// Top-left of the grid on the panel's 640x480 surface, in pixels.
    pub origin: [u16; 2],
    /// Where the font upload puts the text, which is not quite `origin`.
    pub font_origin: [u16; 2],
    /// The font upload to fill with glyphs, relative to this file.
    pub upload: String,
    /// Runtime aircraft name to its font, relative to this file. Keyed by
    /// aircraft rather than module, because one module can carry two
    /// cockpits: the F-14B and F-14B(U) share `F-14` and only one has a CDNU.
    #[serde(default)]
    pub native_fonts: HashMap<String, String>,
    /// The directory this display was loaded from, which the paths above are
    /// relative to.
    #[serde(skip)]
    pub dir: std::path::PathBuf,
    /// Per font file, the characters it can draw in each size.
    #[serde(skip)]
    pub charsets: HashMap<String, FontChars>,
}

/// The characters one font can draw.
#[derive(Debug, Clone, Default)]
pub struct FontChars {
    pub large: std::collections::HashSet<char>,
    pub small: std::collections::HashSet<char>,
}

impl TextGrid {
    /// The font file for an aircraft, if it has a native one.
    pub fn font_for(&self, aircraft: &str) -> Option<&str> {
        self.native_fonts.get(aircraft).map(String::as_str)
    }

    /// The font to actually upload for an aircraft, given what the profile
    /// chose.
    ///
    /// The aircraft's own font wins wherever there is one. A module that draws
    /// its CDU has glyphs drawn to match what it sends, and overriding that
    /// with a font picked for its looks would put the wrong symbol on the
    /// glass rather than a differently shaped right one.
    pub fn font_with(&self, aircraft: &str, chosen: Option<&str>) -> Option<&str> {
        if let Some(native) = self.font_for(aircraft) {
            return Some(native);
        }
        let chosen = chosen?;
        self.charsets
            .get_key_value(chosen)
            .map(|(name, _)| name.as_str())
    }

    /// Every font this display can be given, in a stable order.
    ///
    /// The aircraft fonts are the list: they are the only ones drawn for this
    /// panel, and each is a complete set of glyphs at both sizes. A profile
    /// for an aircraft without a CDU of its own picks one of them.
    pub fn fonts(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.charsets.keys().map(String::as_str).collect();
        out.sort_unstable();
        out
    }

    /// What one font can draw at the size asked for.
    pub fn charset(&self, font: &str, small: bool) -> Option<&std::collections::HashSet<char>> {
        let chars = self.charsets.get(font)?;
        Some(if small { &chars.small } else { &chars.large })
    }

    pub fn path(&self, relative: &str) -> std::path::PathBuf {
        self.dir.join(relative)
    }
}

/// The colours a text grid can draw, in the order the panel indexes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Colour {
    Black,
    Amber,
    #[default]
    White,
    Cyan,
    Green,
    Magenta,
    Red,
    Yellow,
    Brown,
    Grey,
    Khaki,
}

impl Colour {
    pub fn ordinal(self) -> u8 {
        self as u8
    }

    /// Every colour, in the order the panel indexes them.
    ///
    /// Here rather than in the editor so a colour cannot be offered that the
    /// hardware has no index for, and so the two lists cannot drift apart.
    pub const ALL: [Colour; 11] = [
        Colour::Black,
        Colour::Amber,
        Colour::White,
        Colour::Cyan,
        Colour::Green,
        Colour::Magenta,
        Colour::Red,
        Colour::Yellow,
        Colour::Brown,
        Colour::Grey,
        Colour::Khaki,
    ];

    /// The name this colour is written under in a profile.
    pub fn name(self) -> &'static str {
        match self {
            Colour::Black => "black",
            Colour::Amber => "amber",
            Colour::White => "white",
            Colour::Cyan => "cyan",
            Colour::Green => "green",
            Colour::Magenta => "magenta",
            Colour::Red => "red",
            Colour::Yellow => "yellow",
            Colour::Brown => "brown",
            Colour::Grey => "grey",
            Colour::Khaki => "khaki",
        }
    }
}

/// One cell of a text grid as the buffer holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextCell {
    pub ch: char,
    pub fg: u8,
    pub bg: u8,
    pub small: bool,
}

/// Bytes per cell in a text grid's buffer: the character in the low 21 bits,
/// then foreground, background and size. All zero is a blank cell.
pub const TEXT_CELL_BYTES: usize = 4;

impl TextCell {
    fn encode(self) -> [u8; TEXT_CELL_BYTES] {
        let code = u32::from(self.ch)
            | u32::from(self.fg & 0x0f) << 21
            | u32::from(self.bg & 0x0f) << 25
            | u32::from(self.small) << 29;
        code.to_le_bytes()
    }

    fn decode(bytes: &[u8]) -> Self {
        let code = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if code == 0 {
            return TextCell {
                ch: ' ',
                fg: Colour::White.ordinal(),
                bg: 0,
                small: false,
            };
        }
        TextCell {
            ch: char::from_u32(code & 0x1f_ffff).unwrap_or(' '),
            fg: ((code >> 21) & 0x0f) as u8,
            bg: ((code >> 25) & 0x0f) as u8,
            small: code >> 29 & 1 == 1,
        }
    }
}

/// A text grid's buffer as cells, row by row, for the transport to send.
pub fn text_cells(bytes: &[u8]) -> Vec<TextCell> {
    bytes
        .chunks(TEXT_CELL_BYTES)
        .map(TextCell::decode)
        .collect()
}

/// A regular grid of character cells over a pixel framebuffer.
///
/// Spares a pixel display from listing a bit index for every pixel of every
/// cell, which for the DED would be 12,480 numbers. The cells are generated
/// from this at load, row by row, left to right.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grid {
    pub columns: usize,
    pub rows: usize,
    pub cell_width: usize,
    pub cell_height: usize,
    /// Pixels in one framebuffer row.
    pub width: usize,
    /// The glyph table every generated cell draws from.
    pub shape: String,
    /// How far below the top of its cell a font bitmap starts.
    #[serde(default)]
    pub ink_top: usize,
    /// First and last row of a cell that an inverse character fills, counted
    /// from the top of the cell.
    pub inverse_rows: [usize; 2],
}

/// How DCS-BIOS reports which crew station the player is in.
///
/// Spelled the same way in every module that has one, so naming it here is a
/// DCS-BIOS convention rather than knowledge of any particular aircraft. Only 5
/// of the 50 catalogued modules publish it, which is why a seat is optional and
/// rejected where it would never resolve.
pub const SEAT_SIGNAL: &str = "SEAT_POSITION";

/// One named area of a display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    /// The cells it covers, written the way a profile writes them: `"34"` or
    /// `"30-33"`.
    pub cells: String,
    /// What the aircraft this panel was built for puts here. Often the only
    /// thing that makes the name meaningful.
    #[serde(default)]
    pub note: String,
}

/// Fit a value to the number of characters a wide cell holds.
///
/// Trimmed and then right aligned, which is what a number wants: a short value
/// keeps its leading blank and a long one loses its leading digits rather than
/// its trailing ones. Trimming both ends rather than one, because the padding
/// side is the module's choice: the Hornet sends `" 2"`, the Hind sends `"1 "`.
///
/// Trimming is safe here and is not safe on a run of cells. A run gives each
/// cell one character, so its padding is the layout and removing it would shift
/// every character sideways. On one cell the whole value is a single glyph and
/// the padding is only alignment inside it.
fn fit(value: &str, width: usize) -> String {
    let trimmed = value.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() >= width {
        chars[chars.len() - width..].iter().collect()
    } else {
        let mut out = " ".repeat(width - chars.len());
        out.push_str(trimmed);
        out
    }
}

/// What the unlit ground and a lit slot look like on a glass, as CSS colours.
///
/// Only the preview reads it; nothing sent to the panel depends on it. The DED
/// is why it exists: a lit pixel there is dark on a green ground, so an
/// inverse cell, which lights its box and knocks the glyph out, reads green on
/// black.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Glass {
    pub ground: String,
    pub ink: String,
}

/// A kind of screen: its cells, glyphs and how it is written.
///
/// Not tied to a part. Which parts carry it is said once, by `display` on the
/// part in devices.json, so one map serves every panel with the same glass:
/// the MCDU and the three PFPs share one, and so share their pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Display {
    pub key: String,
    #[serde(default)]
    pub transport: Transport,
    /// Worked out from `text` on a text grid, so it can be left out there.
    #[serde(default)]
    pub buffer_bytes: usize,
    /// The unit a change is found and written in. For a segment display that
    /// is the device's write group; for a pixel display it is one row; for a
    /// text grid it is the whole screen.
    #[serde(default)]
    pub group_bytes: usize,
    /// Generates `cells` when they are not listed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<Grid>,
    /// The character grid, on a `text` display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<TextGrid>,
    #[serde(default)]
    pub cells: Vec<Cell>,
    /// Shape name, then glyph, then which of a cell's slots that glyph lights.
    ///
    /// Keyed by the whole field value, not by character. A two-character field
    /// can occupy one cell, and those glyphs are not the union of their parts:
    /// `'0'` and `' 0'` share almost no segments.
    #[serde(default)]
    pub glyphs: HashMap<String, HashMap<String, Vec<u8>>>,
    /// Glyphs drawn as rows of `#` and `.`, per shape, added to `glyphs` at
    /// load. Only on a display with a `grid`, which says how wide a row is and
    /// where in the cell the first one goes.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub fonts: HashMap<String, HashMap<String, Vec<String>>>,
    /// Per shape, what its slots look like, for a preview away from the
    /// panel. Only on glass whose slots are segments: a pixel screen's art
    /// falls out of its `grid`, and a text grid draws from a font the editor
    /// reads for itself.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub art: HashMap<String, StrokeArt>,
    /// The colours of the glass, for the preview. Absent draws white on black.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glass: Option<Glass>,
    /// Look glyphs up only as sent, never uppercased first.
    ///
    /// The DED needs it: DCS-BIOS spells its arrow `a` and its degree sign
    /// `o`, so trying `A` and `O` first would draw the wrong character with
    /// no error anywhere.
    #[serde(default, skip_serializing_if = "is_false")]
    pub exact_case: bool,
    /// Per shape, the slots an inverse character flips. Generated from the
    /// grid; a shape with none cannot be drawn inverse.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub inverse: HashMap<String, Vec<u8>>,
    /// Named areas of the glass, for choosing where a field goes.
    ///
    /// A cell run says nothing to someone deciding what to put on a panel, and
    /// `30-33` in particular is the kind of thing that turns into a support
    /// question. These name the positions instead. They describe where a region
    /// is rather than what one aircraft uses it for, because the same glass
    /// serves other modules, and the note says what the aircraft it was built
    /// for does with it.
    ///
    /// Advisory, not a constraint: a profile may still name any run of cells.
    #[serde(default)]
    pub regions: Vec<Region>,
    /// Values this hardware spells differently from the way a source reports
    /// them, applied before the glyph is looked up.
    ///
    /// A glyph table is the vendor's, and the vendor did not have to agree with
    /// DCS-BIOS about how to write a number down. The UFC comm preset cells are
    /// the case: two digits on one cell, where the tens is a partial digit that
    /// can only draw 1 or 2, and the vendor spells those `` `X `` and `~X`.
    /// DCS-BIOS sends `"12"`.
    ///
    /// Held on the display rather than on a `Readout`, because it is a fact
    /// about the panel and not a choice the user made. Every profile that ever
    /// drives this cell needs the same rewrite, and none of them should have to
    /// know about it.
    #[serde(default)]
    pub spellings: HashMap<String, String>,
}

impl Display {
    pub fn cell(&self, index: usize) -> Option<&Cell> {
        self.cells.get(index)
    }

    /// Whether any cell here can be drawn inverse. A text grid always can: it
    /// swaps a cell's colours.
    /// Whether this glass is a text grid.
    ///
    /// A grid draws characters from a font it was given, so it can draw
    /// anything in that font; the others draw from a fixed glyph table. That is
    /// what decides colour, the small font and a divider.
    pub fn is_text_grid(&self) -> bool {
        self.transport == Transport::Text
    }

    pub fn draws_inverse(&self) -> bool {
        self.transport == Transport::Text || self.inverse.values().any(|slots| !slots.is_empty())
    }

    /// Build the cells, glyphs and inverse slots a `grid` implies. Called at
    /// load; a display without a grid is left as it is.
    pub fn expand(&mut self) -> Result<()> {
        let key = self.key.clone();
        let bad = |why: String| Error::BadDisplay(key.clone(), why);
        if self.transport == Transport::Text {
            let Some(t) = &self.text else {
                return Err(bad(
                    "a text display needs a `text` block saying its size".into()
                ));
            };
            if self.grid.is_some()
                || !self.fonts.is_empty()
                || !self.glyphs.is_empty()
                || !self.art.is_empty()
            {
                return Err(bad(
                    "a text display draws from the panel's font, not from glyphs or a pixel grid"
                        .into(),
                ));
            }
            let n = t.columns * t.rows;
            self.cells = (0..n)
                .map(|index| Cell {
                    index,
                    shape: "text".into(),
                    width: 1,
                    segments: Vec::new(),
                })
                .collect();
            self.buffer_bytes = n * TEXT_CELL_BYTES;
            self.group_bytes = self.buffer_bytes;
            return Ok(());
        }
        if self.text.is_some() {
            return Err(bad("only a text display takes a `text` block".into()));
        }
        if self.buffer_bytes == 0 || self.group_bytes == 0 {
            return Err(bad("buffer_bytes and group_bytes are required".into()));
        }
        let Some(g) = self.grid.clone() else {
            if !self.fonts.is_empty() {
                return Err(bad("a font needs a grid to say where its rows go".into()));
            }
            return self.check_art();
        };
        let slots = g.cell_width * g.cell_height;
        if slots > 256 {
            return Err(bad(format!(
                "a {}x{} cell has more pixels than a glyph can name",
                g.cell_width, g.cell_height
            )));
        }
        if g.columns * g.cell_width > g.width {
            return Err(bad(format!(
                "{} columns of {} pixels do not fit in {}",
                g.columns, g.cell_width, g.width
            )));
        }
        // The last line may hang off the bottom of the buffer. The DED does:
        // five 13 row lines on a 64 row screen, so line 5 has no bottom row.
        // That row is margin, so it is dropped rather than refused, as long as
        // everything a character can light is still on the screen.
        let bits = (self.buffer_bytes * 8).min(usize::from(u16::MAX) + 1);
        let lowest = (g.rows - 1) * g.cell_height + g.inverse_rows[1].max(g.ink_top);
        if (lowest + 1) * g.width > bits {
            return Err(bad(format!(
                "row {lowest} of the grid is past the end of a {bits} bit buffer"
            )));
        }
        if self.cells.is_empty() {
            for row in 0..g.rows {
                for col in 0..g.columns {
                    // Row by row, so a clipped cell loses only its last slots
                    // and every slot a glyph names still means the same pixel.
                    let mut segments = Vec::with_capacity(slots);
                    'rows: for y in 0..g.cell_height {
                        for x in 0..g.cell_width {
                            let pixel =
                                (row * g.cell_height + y) * g.width + col * g.cell_width + x;
                            if pixel >= bits {
                                break 'rows;
                            }
                            segments.push(pixel as u16);
                        }
                    }
                    self.cells.push(Cell {
                        index: self.cells.len(),
                        shape: g.shape.clone(),
                        width: 1,
                        segments,
                    });
                }
            }
        }
        for (shape, font) in &self.fonts {
            let table = self.glyphs.entry(shape.clone()).or_default();
            for (value, rows) in font {
                if g.ink_top + rows.len() > g.cell_height {
                    return Err(bad(format!(
                        "glyph {value:?} is {} rows and the cell has room for {}",
                        rows.len(),
                        g.cell_height - g.ink_top
                    )));
                }
                let mut lit = Vec::new();
                for (y, row) in rows.iter().enumerate() {
                    if row.chars().count() > g.cell_width {
                        return Err(bad(format!(
                            "glyph {value:?} has a row wider than {} pixels",
                            g.cell_width
                        )));
                    }
                    for (x, c) in row.chars().enumerate() {
                        match c {
                            '#' => lit.push(((g.ink_top + y) * g.cell_width + x) as u8),
                            '.' => {}
                            other => {
                                return Err(bad(format!(
                                    "glyph {value:?} has {other:?} in it; a row is # and . only"
                                )))
                            }
                        }
                    }
                }
                table.insert(value.clone(), lit);
            }
        }
        let [top, bottom] = g.inverse_rows;
        if top > bottom || bottom >= g.cell_height {
            return Err(bad(format!(
                "inverse rows {top} to {bottom} are not inside a {} row cell",
                g.cell_height
            )));
        }
        self.inverse.entry(g.shape.clone()).or_insert_with(|| {
            (top * g.cell_width..(bottom + 1) * g.cell_width)
                .map(|s| s as u8)
                .collect()
        });
        self.check_art()
    }

    /// Every shape's art names a shape this glass draws in, and gives it one
    /// stroke list per slot.
    ///
    /// The length is the fault worth catching. A glyph names slots by number,
    /// so art one entry short would draw every glyph of that shape with its
    /// last segment missing, in the one place nobody can compare it against
    /// the panel.
    fn check_art(&self) -> Result<()> {
        let bad = |why: String| Error::BadDisplay(self.key.clone(), why);
        for (shape, art) in &self.art {
            let Some(cell) = self.cells.iter().find(|c| &c.shape == shape) else {
                return Err(bad(format!("the art draws {shape}, which no cell is")));
            };
            if art.slots.len() != cell.segments.len() {
                return Err(bad(format!(
                    "the {shape} art draws {} slots and a {shape} cell has {}",
                    art.slots.len(),
                    cell.segments.len()
                )));
            }
            for (slot, strokes) in art.slots.iter().enumerate() {
                for points in strokes {
                    if points.len() < 4 || points.len() % 2 != 0 {
                        return Err(bad(format!(
                            "{shape} slot {slot} has a stroke of {} numbers; a stroke is x and y pairs, two pairs at least",
                            points.len()
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// What each of this display's shapes looks like, for a preview.
    ///
    /// A shape with nothing to draw it from is left out rather than guessed
    /// at, so the window can say it has no picture of this glass instead of
    /// showing a picture of nothing in particular.
    pub fn shape_art(&self) -> BTreeMap<String, ShapeArt> {
        let mut out = BTreeMap::new();
        for cell in &self.cells {
            if out.contains_key(&cell.shape) {
                continue;
            }
            if let Some(art) = self.art.get(&cell.shape) {
                out.insert(cell.shape.clone(), ShapeArt::Strokes(art.clone()));
            } else if let Some(g) = &self.grid {
                if g.shape == cell.shape {
                    out.insert(
                        cell.shape.clone(),
                        ShapeArt::Pixels {
                            width: g.cell_width,
                            height: g.cell_height,
                        },
                    );
                }
            }
        }
        out
    }

    /// The glyph for `value` on `cell`, if that cell's shape can draw it.
    ///
    /// Two passes, uppercase then as sent, and within each pass the form the
    /// cell prefers and then the bare value. Every quirk this hardware has
    /// falls out of those four tries.
    ///
    /// **Uppercase leads** because this panel was built for the Hornet, and
    /// DCS-BIOS reports the Hornet's UFC in capitals throughout. The lowercase
    /// glyphs are real and distinct, `'g'` lights four slots where `'G'` lights
    /// eight, but they were never exercised: every letter in both captured
    /// pages is a capital. The set is also incomplete, with no small r, u, w, y
    /// or z, which is not what a font meant to be used looks like. Another
    /// module naming a guard channel `"g"` should reach the glass looking like
    /// the rest of the panel rather than in a small form nothing else uses.
    ///
    /// Falling back to the value as sent is what keeps that from being a
    /// gamble. `digit7` has a `'p'` and a `'w'` and no capitals at all, so
    /// uppercasing alone would have taken those off the glass.
    ///
    /// The **preferred** form differs by cell. A wide cell wants its full
    /// width, trimmed and right aligned, because the wrong width can still hit
    /// a real entry: a glyph table is shared by every cell of a shape, and the
    /// same slot numbers are different bits on different cells, so `'1'` draws
    /// two strokes of the units digit on a comm window. An ordinary cell wants
    /// the spaced form of a single character, because a digit has two forms
    /// there and the only one ever captured is the spaced one: the COMM page
    /// has cell 0 reading `' 3'`.
    ///
    /// The **bare** value then rescues everything with no spaced form, which is
    /// every letter and every mark. DCS-BIOS pads a string out to its
    /// `max_length` while DCS's own indication does not, so a scratchpad letter
    /// arrives as `" G"` and a guard channel as `" g"`, and both would
    /// otherwise leave the cell dark. A digit never reaches this fallback,
    /// because its preferred form is always in the table.
    pub fn glyph(&self, cell: &Cell, value: &str) -> Option<&Vec<u8>> {
        let table = self.glyphs.get(&cell.shape)?;
        // A spelling only rescues a value the table does not already have, so a
        // display that spells "20" for itself is never overridden by ours.
        let look = |value: &str| -> Option<&Vec<u8>> {
            table
                .get(value)
                .or_else(|| table.get(self.spellings.get(value)?))
        };
        let bare = value.trim();
        let upper = bare.to_uppercase();
        let passes: &[&str] = if self.exact_case {
            &[bare]
        } else {
            &[upper.as_str(), bare]
        };
        for &candidate in passes {
            let preferred = if cell.width > 1 {
                fit(candidate, cell.width)
            } else if candidate.chars().count() <= 1 {
                format!(" {candidate}")
            } else {
                candidate.to_string()
            };
            if let Some(lit) = look(&preferred).or_else(|| look(candidate)) {
                return Some(lit);
            }
        }
        None
    }

    /// Which of a cell's slots are lit, drawing `value` in it.
    ///
    /// The glyph table's answer with the inverse flip applied, which is the
    /// whole of what reaches the glass, and so the whole of what a preview
    /// away from the panel has to draw. `None` where the table has nothing
    /// for the value: on the panel that cell stays dark.
    ///
    /// Slots past the end of the cell are dropped, because they are past the
    /// end of the screen too. The DED's last row of cells hangs off the
    /// bottom of the buffer by one pixel row.
    pub fn lit(&self, cell: &Cell, value: &str, inverse: bool) -> Option<Vec<u8>> {
        let glyph = self.glyph(cell, value)?;
        // A slot is a u8, so 256 covers every slot a glyph can name.
        let mut on = [false; 256];
        for &slot in glyph {
            on[slot as usize] = true;
        }
        if inverse {
            for &slot in self.inverse.get(&cell.shape).into_iter().flatten() {
                on[slot as usize] ^= true;
            }
        }
        Some(
            (0..cell.segments.len())
                .filter(|&slot| on.get(slot).copied().unwrap_or(false))
                .map(|slot| slot as u8)
                .collect(),
        )
    }

    /// How many write groups the buffer is divided into.
    pub fn groups(&self) -> usize {
        self.buffer_bytes.div_ceil(self.group_bytes)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisplayCatalogue {
    #[serde(default)]
    pub displays: Vec<Display>,
}

impl DisplayCatalogue {
    /// Load every `*.json` in a directory. A missing directory is not an error:
    /// a user with no glass panels has no `data/displays`.
    pub fn load_dir(dir: &Path) -> Result<Self> {
        let mut out = Self::default();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e.into()),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        paths.sort();
        for path in paths {
            let mut one: DisplayCatalogue = read_json(&path)?;
            for display in &mut one.displays {
                display.expand()?;
                if let Some(text) = &mut display.text {
                    text.dir = dir.to_path_buf();
                    load_charsets(&display.key, text)?;
                }
            }
            out.displays.extend(one.displays);
        }
        Ok(out)
    }

    pub fn get(&self, key: &str) -> Option<&Display> {
        self.displays.iter().find(|d| d.key == key)
    }
}

/// Read what each native font can draw, so a profile can be checked against
/// it. A font that will not load is an error now rather than a blank screen
/// in the middle of a flight.
fn load_charsets(key: &str, text: &mut TextGrid) -> Result<()> {
    let files: std::collections::BTreeSet<String> = text.native_fonts.values().cloned().collect();
    for file in files {
        let font = crate::mcdu_font::McduFont::load(&text.path(&file))
            .map_err(|e| Error::BadDisplay(key.to_string(), format!("font {file}: {e}")))?;
        text.charsets.insert(
            file,
            FontChars {
                large: font.large_glyphs.iter().map(|g| g.character).collect(),
                small: font.small_glyphs.iter().map(|g| g.character).collect(),
            },
        );
    }
    Ok(())
}

/// A host-side copy of a display's segment buffer.
///
/// Starts blank, which matches a display that has just been cleared. It does
/// not match one whose state is unknown, so a first paint should clear the
/// device rather than assume it agrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    bytes: Vec<u8>,
    group_bytes: usize,
}

impl Screen {
    pub fn new(display: &Display) -> Self {
        Screen {
            bytes: vec![0; display.buffer_bytes],
            group_bytes: display.group_bytes,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn clear(&mut self) {
        self.bytes.iter_mut().for_each(|b| *b = 0);
    }

    /// Draw `value` into one cell, replacing whatever was there.
    ///
    /// Every one of the cell's segments is written, lit or not, so a cell never
    /// keeps a stroke from the character before it.
    pub fn draw(&mut self, display: &Display, index: usize, value: &str) -> Result<()> {
        self.draw_styled(display, index, value, false)
    }

    /// Draw `value`, optionally inverse: the glyph knocked out of a filled box.
    ///
    /// Inverse is drawn here, not by the device. SimAppPro does the same; the
    /// screen has no such mode. On a shape with no inverse slots the flag does
    /// nothing, which validation reports before it gets this far.
    pub fn draw_styled(
        &mut self,
        display: &Display,
        index: usize,
        value: &str,
        inverse: bool,
    ) -> Result<()> {
        let cell = display
            .cell(index)
            .ok_or_else(|| Error::NoSuchCell(display.key.clone(), index))?;
        let lit = display.lit(cell, value, inverse).ok_or_else(|| {
            Error::NoSuchGlyph(value.to_string(), cell.shape.clone(), display.key.clone())
        })?;
        // A lookup table rather than `contains`, because a DED cell is 104
        // slots and the whole screen is repainted on every batch.
        let mut on = [false; 256];
        for slot in lit {
            on[slot as usize] = true;
        }
        for (slot, &bit) in cell.segments.iter().enumerate() {
            let (byte, mask) = (bit as usize / 8, 1u8 << (bit % 8));
            if on.get(slot).copied().unwrap_or(false) {
                self.bytes[byte] |= mask;
            } else {
                self.bytes[byte] &= !mask;
            }
        }
        Ok(())
    }

    /// Put one character in a text grid's cell, replacing what was there.
    ///
    /// Inverse swaps the colours, which is how a text grid highlights: the
    /// cell fills with the field's colour and the character is cut out of it
    /// in black. The character is not checked against a font here; which
    /// characters a font draws is a profile check, and a stray one on the live
    /// stream draws as a gap rather than stopping the rest of the screen.
    pub fn draw_text(
        &mut self,
        display: &Display,
        index: usize,
        ch: char,
        colour: Colour,
        small: bool,
        inverse: bool,
    ) -> Result<()> {
        if display.transport != Transport::Text || index >= display.cells.len() {
            return Err(Error::NoSuchCell(display.key.clone(), index));
        }
        let (fg, bg) = if inverse {
            (Colour::Black, colour)
        } else {
            (colour, Colour::Black)
        };
        let cell = TextCell {
            ch,
            fg: fg.ordinal(),
            bg: bg.ordinal(),
            small,
        };
        let at = index * TEXT_CELL_BYTES;
        self.bytes[at..at + TEXT_CELL_BYTES].copy_from_slice(&cell.encode());
        Ok(())
    }

    /// Write groups that differ from `previous`, as `(group index, bytes)`.
    ///
    /// Whole groups, because that is the write granularity: a group cannot be
    /// partially written, and the bytes it holds may belong to cells that did
    /// not change.
    pub fn changes_from(&self, previous: &Screen) -> Vec<(u8, Vec<u8>)> {
        let n = self.group_bytes;
        self.bytes
            .chunks(n)
            .zip(previous.bytes.chunks(n))
            .enumerate()
            .filter(|(_, (now, was))| now != was)
            .map(|(g, (now, _))| (g as u8, now.to_vec()))
            .collect()
    }

    /// Every group, for a first paint or a resync where the device's state is
    /// not known to match ours.
    pub fn all_groups(&self) -> Vec<(u8, Vec<u8>)> {
        self.bytes
            .chunks(self.group_bytes)
            .enumerate()
            .map(|(g, bytes)| (g as u8, bytes.to_vec()))
            .collect()
    }
}

// ------------------------------------------------------------- profile side

/// A run of cells, written `"2-8"` or `"34"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRange {
    pub first: usize,
    pub last: usize,
}

impl CellRange {
    pub fn len(&self) -> usize {
        self.last + 1 - self.first
    }

    pub fn is_empty(&self) -> bool {
        false // `last` is inclusive and never below `first`, so a range is >= 1
    }

    pub fn contains(&self, cell: usize) -> bool {
        cell >= self.first && cell <= self.last
    }

    pub fn overlaps(&self, other: &CellRange) -> bool {
        self.first <= other.last && other.first <= self.last
    }

    pub fn cells(&self) -> impl Iterator<Item = usize> {
        self.first..=self.last
    }
}

impl std::fmt::Display for CellRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.first == self.last {
            write!(f, "{}", self.first)
        } else {
            write!(f, "{}-{}", self.first, self.last)
        }
    }
}

impl std::str::FromStr for CellRange {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, String> {
        let bad = || format!("expected a cell like \"34\" or a run like \"2-8\", got {s:?}");
        let (a, b) = match s.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (s.trim(), s.trim()),
        };
        let first: usize = a.parse().map_err(|_| bad())?;
        let last: usize = b.parse().map_err(|_| bad())?;
        if last < first {
            return Err(format!("cell run {s:?} ends before it starts"));
        }
        Ok(CellRange { first, last })
    }
}

impl Serialize for CellRange {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for CellRange {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Which readings one alias claims: a value, a list of them, or a closed band.
///
/// Written `"3"`, `"0,1,2"` or `"-1.5..-0.1"`, and matched against what the
/// face reads rather than the raw count DCS-BIOS sends. The numbers here are
/// the ones on the dial, so a trim indicator converted to -1.5 to 1.5 is
/// banded in those units and stays banded if the range is retuned. A needle
/// sitting between two bands lands in one of them, because the reading is
/// rounded to `decimals` before it is matched.
///
/// `-` is not the separator, unlike a cell run: a cell is never negative and
/// `"-1.5--1.0"` has no unambiguous reading. `..` is what gets written, and
/// `to` is accepted in its place so a band can be typed the way it is said.
///
/// The spelling it arrived as is kept rather than normalised into one form.
/// Expanding `"0,1,2"` into three entries would rewrite the user's file.
#[derive(Debug, Clone)]
pub enum ValueBand {
    One(f64),
    List(Vec<f64>),
    Range { lo: f64, hi: f64 },
}

impl ValueBand {
    /// Whether this band claims `reading`, within `tol` of its edges.
    ///
    /// The tolerance is half of the last decimal place the reading is shown
    /// to. Both sides are decimal numbers, one parsed from the profile and one
    /// arrived at by converting and rounding a raw count, and they can differ
    /// in the last bit without differing in anything the user can see.
    pub fn matches(&self, reading: f64, tol: f64) -> bool {
        match self {
            ValueBand::One(v) => (v - reading).abs() <= tol,
            ValueBand::List(vs) => vs.iter().any(|v| (v - reading).abs() <= tol),
            ValueBand::Range { lo, hi } => reading >= lo - tol && reading <= hi + tol,
        }
    }

    /// The lowest reading this band claims.
    pub fn lowest(&self) -> f64 {
        match self {
            ValueBand::One(v) => *v,
            ValueBand::List(vs) => vs.iter().copied().fold(f64::INFINITY, f64::min),
            ValueBand::Range { lo, .. } => *lo,
        }
    }

    /// The highest reading this band claims.
    pub fn highest(&self) -> f64 {
        match self {
            ValueBand::One(v) => *v,
            ValueBand::List(vs) => vs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            ValueBand::Range { hi, .. } => *hi,
        }
    }

    /// Whether this band and `other` both claim some reading.
    pub fn overlaps(&self, other: &ValueBand, tol: f64) -> bool {
        // Two ranges meet when neither ends before the other starts, which is
        // the same sum `CellRange::overlaps` does. A value or a list has to be
        // asked value by value: 0 and 2 do not overlap 1 even though they
        // straddle it.
        match (self, other) {
            (ValueBand::Range { .. }, ValueBand::Range { .. }) => {
                self.lowest() <= other.highest() + tol && other.lowest() <= self.highest() + tol
            }
            (ValueBand::Range { .. }, one) | (one, ValueBand::Range { .. }) => one
                .values()
                .iter()
                .any(|v| self.matches(*v, tol) && other.matches(*v, tol)),
            (a, b) => a.values().iter().any(|v| b.matches(*v, tol)),
        }
    }

    /// The values a band names one by one, empty for a range.
    fn values(&self) -> Vec<f64> {
        match self {
            ValueBand::One(v) => vec![*v],
            ValueBand::List(vs) => vs.clone(),
            ValueBand::Range { .. } => Vec::new(),
        }
    }

    /// Everything that tells one band from another, in the order they sort by:
    /// where it starts, where it ends, which spelling it is, and then the
    /// values themselves, so two lists that start and end together are still
    /// two keys rather than one that quietly replaced the other.
    fn ordering(&self) -> (f64, f64, u8, Vec<f64>) {
        let rank = match self {
            ValueBand::One(_) => 0,
            ValueBand::List(_) => 1,
            ValueBand::Range { .. } => 2,
        };
        let parts = match self {
            ValueBand::Range { lo, hi } => vec![*lo, *hi],
            other => other.values(),
        };
        (self.lowest(), self.highest(), rank, parts)
    }
}

impl Ord for ValueBand {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let (al, ah, ar, av) = self.ordering();
        let (bl, bh, br, bv) = other.ordering();
        al.total_cmp(&bl)
            .then(ah.total_cmp(&bh))
            .then(ar.cmp(&br))
            .then(av.len().cmp(&bv.len()))
            .then_with(|| {
                av.iter()
                    .zip(&bv)
                    .map(|(x, y)| x.total_cmp(y))
                    .find(|o| o.is_ne())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

impl PartialOrd for ValueBand {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ValueBand {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for ValueBand {}

impl std::fmt::Display for ValueBand {
    /// The canonical spelling, which is what gets written back.
    ///
    /// Rust draws a whole f64 without a fractional part, so `One(3.0)` writes
    /// `"3"` and not `"3.0"`. That is load-bearing rather than tidy: the
    /// update merge decides whether a row is still as it shipped by comparing
    /// rows as JSON, so a key that changed spelling here would make every
    /// aliased row in every profile read as one the user had edited.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueBand::One(v) => write!(f, "{v}"),
            ValueBand::List(vs) => {
                let written: Vec<String> = vs.iter().map(|v| v.to_string()).collect();
                write!(f, "{}", written.join(","))
            }
            ValueBand::Range { lo, hi } => {
                // An open end is written as nothing, the way it is read.
                let side = |v: &f64| {
                    if v.is_finite() {
                        v.to_string()
                    } else {
                        String::new()
                    }
                };
                write!(f, "{}..{}", side(lo), side(hi))
            }
        }
    }
}

impl std::str::FromStr for ValueBand {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, String> {
        let bad = || {
            format!(
                "expected a reading like \"3\", a list like \"0,1,2\", a band like \"-1.5..-0.1\", or an open band like \"1000..\" or \"..999\", got {s:?}"
            )
        };
        let one = |part: &str| -> std::result::Result<f64, String> {
            match part.trim().parse::<f64>() {
                Ok(v) if v.is_finite() => Ok(v),
                _ => Err(bad()),
            }
        };
        let s = s.trim();
        if let Some((a, b)) = s.split_once("..").or_else(|| s.split_once(" to ")) {
            // An open end is at least or at most: `"1000.."` claims every
            // reading from 1000 up, `"..999"` every one up to 999.
            let end = |part: &str, open: f64| -> std::result::Result<f64, String> {
                if part.trim().is_empty() {
                    Ok(open)
                } else {
                    one(part)
                }
            };
            if a.trim().is_empty() && b.trim().is_empty() {
                return Err(bad());
            }
            let (lo, hi) = (end(a, f64::NEG_INFINITY)?, end(b, f64::INFINITY)?);
            if hi < lo {
                return Err(format!("band {s:?} ends before it starts"));
            }
            return Ok(ValueBand::Range { lo, hi });
        }
        if s.contains(',') {
            let values = s
                .split(',')
                .map(one)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            return Ok(ValueBand::List(values));
        }
        Ok(ValueBand::One(one(s)?))
    }
}

impl Serialize for ValueBand {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ValueBand {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// What an alias draws: the characters, and a colour of its own where it wants
/// one, or inverse.
///
/// A band is often a warning about where the needle is, and a warning that
/// reads in the same colour as the row around it is one nobody catches. The
/// colour belongs to the band rather than the piece because that is the whole
/// point: the same reading draws amber in one band and red in the next. Inverse
/// is the same thing on glass with no colours to pick from, such as the DED,
/// and a blank drawn inverse is a solid block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "AliasRepr", into = "AliasRepr")]
pub struct AliasDraw {
    pub text: String,
    pub colour: Option<Colour>,
    pub inverse: bool,
    /// Draw this band in the small font. Text grids only, like a piece's own
    /// `small`, and either asking is enough, the way a conversion row's is.
    pub small: bool,
    /// Draw the reading itself, in this band's colour or inverse, rather than
    /// characters in its place.
    ///
    /// A band is also how a reading is styled: a needle past 120 is the same
    /// number, and the point is that it draws red. An empty `text` cannot say
    /// that, because a blank is already a thing a band draws on purpose, the
    /// centre of a trim indicator worth a row of its own that draws nothing.
    /// So it is said outright, and `problems` refuses it beside `text`.
    pub reading: bool,
}

/// An alias as it is written on disk: bare characters, or an object once it
/// has a colour or inverse to carry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum AliasRepr {
    Text(String),
    Styled {
        // Absent only on a band that draws the reading, which has no
        // characters of its own. Every other band writes it, blank or not.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        colour: Option<Colour>,
        #[serde(default, skip_serializing_if = "is_false")]
        inverse: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        small: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        reading: bool,
    },
}

impl From<AliasRepr> for AliasDraw {
    fn from(r: AliasRepr) -> Self {
        match r {
            AliasRepr::Text(text) => AliasDraw {
                text,
                colour: None,
                inverse: false,
                small: false,
                reading: false,
            },
            AliasRepr::Styled {
                text,
                colour,
                inverse,
                small,
                reading,
            } => AliasDraw {
                text: text.unwrap_or_default(),
                colour,
                inverse,
                small,
                reading,
            },
        }
    }
}

impl From<AliasDraw> for AliasRepr {
    /// A colour or inverse is the only reason for the object shape, so an
    /// alias with neither goes back as the bare string it arrived as.
    ///
    /// Not a tidiness: the update merge compares rows as JSON to decide
    /// whether one is still as the last release shipped it. An alias that came
    /// back in a different shape would make every aliased row in every profile
    /// read as one the user had edited, and those rows are never updated
    /// again.
    ///
    /// A band drawing the reading has no characters, and writes none: `text`
    /// beside `reading` would be two answers to one question.
    fn from(a: AliasDraw) -> Self {
        if a.colour.is_none() && !a.inverse && !a.small && !a.reading {
            return AliasRepr::Text(a.text);
        }
        AliasRepr::Styled {
            text: (!a.reading || !a.text.is_empty()).then_some(a.text),
            colour: a.colour,
            inverse: a.inverse,
            small: a.small,
            reading: a.reading,
        }
    }
}

impl From<&str> for AliasDraw {
    fn from(text: &str) -> Self {
        AliasDraw::from(text.to_string())
    }
}

impl From<String> for AliasDraw {
    fn from(text: String) -> Self {
        AliasDraw {
            text,
            colour: None,
            inverse: false,
            small: false,
            reading: false,
        }
    }
}

/// One stretch of a needle's travel, and what the dial is marked with along it.
///
/// `reads` is a straight line from one end of the signal to the other, which is
/// right for an evenly marked dial and wrong for one that is not. A fuel gauge
/// shaped by its tank crowds its top marks together: the Mosquito's inner tanks
/// put 0 to 20 gallons across a fifth of the travel and 120 to 146 across less
/// than a tenth. A chain of these is the dial as it is marked, each stretch a
/// straight line of its own between two marks.
///
/// `raw` is in the counts DCS-BIOS sends, because that is what Learn shows and
/// what a module's cockpit scripts can be turned into. Both ends are claimed,
/// so the next stretch starts one count on: 0 to 12910, then 12911 to 31785.
///
/// A stretch can carry its own colour and size, which the piece's own give way
/// to, so a gauge can draw its last few gallons red and small.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversion {
    /// The counts at each end of this stretch, lowest first.
    pub raw: [u16; 2],
    /// What the dial reads at each of those ends, in the same order.
    pub reads: [f64; 2],
    /// The colour a reading in this stretch draws in. Text grids only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Colour>,
    /// Draw a reading in this stretch in the small font. Text grids only.
    #[serde(default, skip_serializing_if = "is_false")]
    pub small: bool,
}

impl Conversion {
    /// Whether `value` is one of the counts this stretch claims.
    pub fn claims(&self, value: u16) -> bool {
        self.raw[0] <= value && value <= self.raw[1]
    }

    /// How far `value` is from this stretch, in counts: 0 inside it.
    fn distance(&self, value: u16) -> u16 {
        if value < self.raw[0] {
            self.raw[0] - value
        } else {
            value.saturating_sub(self.raw[1])
        }
    }

    /// What the dial reads at `value`, and half of one count in those units.
    ///
    /// A count outside the stretch reads as the nearer end, the way a needle
    /// parked past its last mark still points at that mark.
    fn convert(&self, value: u16) -> (f64, f64) {
        let [a, b] = self.raw;
        let [lo, hi] = self.reads;
        let counts = f64::from(b.saturating_sub(a));
        if counts == 0.0 {
            return (lo, 0.0);
        }
        let travel = ((f64::from(value) - f64::from(a)) / counts).clamp(0.0, 1.0);
        (lo + travel * (hi - lo), (hi - lo).abs() / counts / 2.0)
    }
}

/// How a converted number lands on its last decimal place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Round {
    /// To the nearest, which is what a needle wants and what every reading
    /// did before this was a choice.
    #[default]
    Nearest,
    /// Down, which is what a drum or anything else that clicks over wants.
    Down,
    /// Up, for a count that shows the next step as soon as it has started
    /// towards it.
    Up,
}

/// Which end of a cell run the text is anchored to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    /// What a scratchpad wants: digits enter at the rightmost cell and shift
    /// left, and DCS-BIOS can hand over more characters than there are cells.
    Right,
    /// Equal blanks each side, the odd one going left, the way `divider_rule`
    /// sets a label into a line of dashes.
    ///
    /// Worth saying what this does to a value that changes width, because it
    /// is the reason it is offered per piece rather than only per field: every
    /// character lost moves both edges in, so a number counting down drifts
    /// half a cell at a time. That is what centring is, and it is right for a
    /// label and wrong for a reading. `Right` inside a box is what keeps
    /// digits pinned while the blanks grow on the left.
    Centre,
}

impl Align {
    /// How many blanks go before the content when it is padded to `width`.
    ///
    /// One place rather than three, because a box, a field and a rule's label
    /// all have to put the odd cell on the same side or the screen stops
    /// lining up with itself.
    pub(crate) fn pad_before(&self, len: usize, width: usize) -> usize {
        let spare = width.saturating_sub(len);
        match self {
            Align::Left => 0,
            Align::Right => spare,
            Align::Centre => spare - spare / 2,
        }
    }
}

/// One cell of a rule: the glyph, and whether it is part of the label.
///
/// The label is marked rather than measured out again by each caller, because
/// it is drawn in its own colour and the editor has to show the same thing.
/// The blanks each side of the label are not part of it: they draw nothing
/// whichever colour they are given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCell {
    pub text: String,
    pub label: bool,
}

/// The narrowest run a rule carrying `label` can be drawn in.
///
/// A dash, a blank, the label, a blank, a dash. The blank each side of the
/// label is what keeps it from reading as part of the line, and a side with no
/// dash left on it is not a rule any more, so both are required rather than
/// dropped to make a long label fit.
///
/// One for an unlabelled rule, which is to say no constraint at all: a single
/// cell draws a single dash, and a `CellRange` is never shorter than that.
pub fn min_divider_cells(label: &str) -> usize {
    if label.is_empty() {
        return 1;
    }
    label.chars().count() + 4
}

/// The rule a divider draws across `width` cells, one glyph per cell.
///
/// An unbroken run of dashes, corner to corner of its run: `--------`. Spaced
/// dashes were tried first, on the glass, and read as a dotted line rather
/// than a rule. It was inset by a blank at each end for a while, which was
/// reasoned about rather than looked at: a CDU line and a piece of typed text
/// both start in the first cell of their run, so the rule was the one thing on
/// the screen not lining up with what sat above and below it.
///
/// A label is set into the middle of that, with a blank each side of it:
/// `--- FUEL ---`. It is centred, and where the dashes cannot be split evenly
/// the odd one goes to the left, the way a gap gives its remainder to the
/// earlier side.
///
/// A label with no room for its blanks and a dash each side is left off,
/// leaving a plain rule, and `problems` refuses one before it gets here. This
/// is a function of the width and the label alone so the editor can show the
/// same rule it will draw, by asking rather than working it out again.
pub fn divider_rule(width: usize, label: &str) -> Vec<RuleCell> {
    let plain = |text: &str| RuleCell {
        text: text.to_string(),
        label: false,
    };
    let mut out: Vec<RuleCell> = std::iter::repeat_with(|| plain("-")).take(width).collect();
    let chars: Vec<char> = label.chars().collect();
    if chars.is_empty() || width < min_divider_cells(label) {
        return out;
    }
    // The dashes on both sides, which the guard above has put at two or more.
    let spare = width - chars.len() - 2;
    let at = (spare - spare / 2) + 1;
    out[at - 1] = plain(" ");
    out[at + chars.len()] = plain(" ");
    for (i, c) in chars.iter().enumerate() {
        out[at + i] = RuleCell {
            text: c.to_string(),
            label: true,
        };
    }
    out
}

/// The whole rule as one string, for a caller that only wants to read it.
pub fn divider_text(width: usize, label: &str) -> String {
    divider_rule(width, label)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

/// One piece of a field's content: characters the user typed, or a signal.
///
/// A field is a chain of these, drawn end to end, because a reading on its own
/// is rarely a readout. `250` says nothing that `RALT 250M` does not say
/// better, and the label, the number and the unit each want their own colour
/// and size. Writing them as three fields would mean counting cells by hand,
/// and would come apart the moment the number changed width.
///
/// A part carries `text` or `source`, never both. Everything else on it shapes
/// the one value it draws, which is why `reads`, `replace` and the rest belong
/// here rather than on the field: one chain can hold two signals that need
/// different treatment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Span {
    /// Characters drawn exactly as given, reading nothing.
    ///
    /// Every one of them has to be a character the font draws, which is not
    /// the same as a character you can type: only the F-14BU font has
    /// lowercase, and in the A-10C font `%` draws a question mark. The editor
    /// draws the font's own glyphs rather than the typed string for that
    /// reason.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Catalogue signal id. Empty on a literal part, and on a part nobody has
    /// finished yet, which `problems` says so about.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// A stored signal's id, on a piece that draws one. Empty on every other
    /// piece.
    ///
    /// A third answer to what a piece draws, beside `text` and `source`. The
    /// signal says what the number is; the piece says only how it looks, so
    /// nothing here that shapes a number applies, and its bands, colour and
    /// size do.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signal: String,
    /// The stored signal `signal` names, attached when the page file loads.
    /// Never written. None where the page file has no such signal, which the
    /// checks refuse.
    #[serde(skip)]
    pub stored: Option<Arc<StoredSignal>>,
    /// A selector whose position picks which of `cases` this piece draws.
    /// Empty on every piece that is not a switch.
    ///
    /// DCS-BIOS reports some readings in units another knob decides. The
    /// Huey's ADF needle runs 0 to 65535 whichever band is selected, and the
    /// band switch is what says whether that is 190 to 400 kHz or 850 to
    /// 1750. One piece cannot say both, and three fields on one run of cells
    /// would be three owners for it.
    ///
    /// Everything else written on a switch is shared by its cases: a case
    /// piece that leaves an option unset takes the switch's. So `source`,
    /// `decimals` and a colour are written once, and each case says only
    /// what is its own.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub switch: String,
    /// A stored signal's id, deciding the cases in place of `switch`, so a
    /// label can colour by the fuel drums' total. Its number is already
    /// shaped, so cases are written in its units and nothing converts it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub switch_signal: String,
    /// The stored signal `switch_signal` names, attached when the page file
    /// loads. Never written.
    #[serde(skip)]
    pub switch_stored: Option<Arc<StoredSignal>>,
    /// What the selector reads at each end of its travel, so cases can be
    /// written in the units a gauge is marked with rather than in counts.
    ///
    /// A switch is usually a knob, whose positions are the numbers its cases
    /// name. Read off a gauge instead, it turns a label red below 1000 lb of
    /// fuel, and `"0..999"` says that where `"0..5957"` would not. Converted
    /// in a straight line like `reads`, then rounded to a whole number, so
    /// `"0..999"` and `"1000..2999"` leave nothing between them. None matches
    /// the position as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_reads: Option<[f64; 2]>,
    /// Draw nothing, and take whatever cells the rest of the chain leaves.
    ///
    /// How content reaches both ends of a line. A CDU page puts a label at the
    /// left and its value hard against the right, and counting the blanks by
    /// hand only works until the value changes width, which is the moment it
    /// matters. A gap is measured after everything else is laid out, so the
    /// two ends stay put whatever happens between them.
    ///
    /// Two or more gaps split what is left evenly, the remainder going to the
    /// earlier ones, which spaces three pieces across a line. A gap with
    /// nothing spare draws nothing at all rather than pushing anything off the
    /// end.
    ///
    /// Carries no text and no source, and `align` stops meaning anything
    /// beside one: the content already fills the run exactly.
    #[serde(default, skip_serializing_if = "is_false")]
    pub gap: bool,
    /// Fill the gap with a rule rather than with blanks.
    ///
    /// A rule between two pieces of a chain, where `divider` is a rule instead
    /// of a whole field. `NAV ----- 250` on one row, with the dashes taking
    /// whatever the two ends leave, which is the arrangement that was three
    /// fields with hand counted cells before this: the rule could not move, so
    /// a reading one character wider than planned overran into its cells and
    /// was cropped.
    ///
    /// Only on a gap. Elsewhere the piece has its own content to draw and a
    /// rule would have nowhere to go.
    #[serde(default, skip_serializing_if = "is_false")]
    pub rule: bool,
    /// Characters set into the middle of this piece's rule, naming what it
    /// divides. Empty on anything that is not a rule.
    ///
    /// Needs `width`, and `problems` refuses it without one. An elastic rule
    /// is as wide as the rest of the chain leaves, so a label that fits at one
    /// reading may not fit at the next, and `divider_rule` leaves a label it
    /// cannot fit off the line. That is safe but silent: the label would drop
    /// away and come back as the number beside it grew and shrank.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// The label's colour, its own rather than the rule's, because a label
    /// drawn in the line's colour reads as part of the line. Falls back to the
    /// piece's `colour`, which is what a label added to a rule that already
    /// had one should look like until it is told otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_colour: Option<Colour>,
    /// Draw this piece in exactly this many cells, whatever it reads. 0 for a
    /// piece that takes the room its value needs, which is every piece written
    /// before this existed.
    ///
    /// Without it a chain only holds still at its ends. A value that goes from
    /// four characters to three pulls everything after it one cell left, so a
    /// layout built around a reading at one width comes apart at another. A
    /// box is measured before the gaps are, so what surrounds it never moves.
    ///
    /// It also bounds a piece nothing else bounds: a gauge with no `reads`
    /// range can draw any width at all, and in a box it draws `width`. That
    /// turns the editor's "this may run past its cells" into an exact answer.
    ///
    /// Content too wide for its box is cropped the way a field is, from the
    /// end `align` anchors away from.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub width: usize,
    /// Where the value sits inside `width`. Means nothing without one: with no
    /// box the piece is exactly as wide as its value and there is nothing to
    /// sit inside.
    #[serde(default, skip_serializing_if = "is_left")]
    pub align: Align,
    /// What the gauge reads in the cockpit, for a numeric source: the real
    /// values at the bottom and top of its travel.
    ///
    /// DCS-BIOS reports a needle as a position, not a quantity, and nothing in
    /// the catalogue says what the face is marked with. So the user reads the
    /// dial and says "this one goes 0 to 300". The conversion is linear in
    /// needle travel, which is exact for an evenly marked dial and approximate
    /// for one that is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reads: Option<[f64; 2]>,
    /// What the gauge reads along each stretch of its travel, for a dial whose
    /// marks are not evenly spaced. In place of `reads`, never beside it.
    ///
    /// A count no stretch claims reads as the end of the nearest one, and
    /// `problems` says which counts those are, so the editor can offer a
    /// stretch for them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversions: Vec<Conversion>,
    /// Decimal places for a numeric source.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub decimals: u8,
    /// The fewest digits before the decimal point, made up with leading zeros.
    ///
    /// A counter shows every drum whatever it reads: a 000 to 999 barrel at 1
    /// reads 001, and drawing 1 would leave two drums blank that are not. The
    /// sign stays in front of the zeros. 0 pads nothing, which is every number
    /// written before this existed.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub digits: u8,
    /// How a number lands on its last decimal place: to the nearest, or down.
    ///
    /// Nearest is right for a needle, where 4.6 is closer to 5 than to 4.
    /// Down is right for anything that clicks over: an odometer drum shows 4
    /// until the 5 has fully arrived, and rounding to the nearest would put
    /// the 5 up while the drum beside it still reads 9.
    #[serde(default, skip_serializing_if = "is_nearest")]
    pub round: Round,
    /// Start again from zero every this many, after converting and rounding.
    ///
    /// A drum or a needle that goes round more than once reports where it is
    /// in its turn, and a scale that goes all the way round reads 0 at the top
    /// rather than its full value. So the reading is the remainder: `reads` 0
    /// to 10 wrapping at 10 is one drum digit, and 0 to 360 wrapping at 360 is
    /// a compass that draws 0 where it would have drawn 360. None for a
    /// reading that never starts over; anything not above zero counts as none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<f64>,
    /// What to draw for each reading of a numeric source, in place of the
    /// number.
    ///
    /// A knob reports its position, and `3` on a screen says less than `SEMI`
    /// does. The editor fills this from the position names the catalogue has,
    /// and the user shortens them to fit. Looked up on the whole value, before
    /// it is split into cells, so an alias is drawn as the characters it is. A
    /// reading no band claims draws as the number.
    ///
    /// The key is matched against **what the face reads**: converted by
    /// `reads`, rounded to `decimals` and wrapped by `wrap` first. That is the
    /// number the user is looking at, and it is what lets a band be written in
    /// the units the dial is marked with instead of in raw counts. A signal
    /// with no `reads` converts through its own range, which is the identity,
    /// so a key naming a position still matches the position.
    ///
    /// Not `aliases`, which rewrites characters a module sends as text. This
    /// one names numbers.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub value_aliases: BTreeMap<ValueBand, AliasDraw>,
    /// Draw a converted reading without its sign.
    ///
    /// A face that runs each way from zero is read as a magnitude and a
    /// direction, not as a negative number: the F-16's trim indicators are
    /// marked in units nose up and units nose down, and a needle below zero
    /// reading `-1.0 ND` says the same thing twice. With the sign dropped the
    /// number is the magnitude and a band beside it names the direction.
    ///
    /// Applied last, after a band has had its turn, so a band written for
    /// negative readings still matches on a piece that draws magnitudes.
    #[serde(default, skip_serializing_if = "is_false")]
    pub abs: bool,
    /// Values this module words differently from the glyph table.
    ///
    /// DCS-BIOS does not always report what DCS's own indication does: the
    /// Hornet scratchpad's second string arrives as `"--"` where the cockpit
    /// says `"_"`, and `"--"` is not a glyph. That is a property of the module,
    /// so it is recorded with the module's mapping rather than in the engine.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<String, String>,
    /// A second string signal, laid out like `source`, whose `i` marks the
    /// characters to draw inverse.
    ///
    /// The F-16 DED is the case: DCS-BIOS sends each line as `DED_Ln` and its
    /// highlighting as `DED_Ln_FORMAT`, one character for one. Any other mark
    /// draws normally; `b`, for big, is sent too and this screen has no large
    /// font.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// The colour a text grid draws this part in. Text grids only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<Colour>,
    /// Draw in the text grid's small font. Text grids only.
    ///
    /// Every font here draws fewer characters small than large, so marking a
    /// part small can take away a character that was fine at full size.
    #[serde(default, skip_serializing_if = "is_false")]
    pub small: bool,
    /// Draw this whole part inverse, on glass that draws inverse at all.
    ///
    /// The `format` signal does this per character for a source the module
    /// highlights itself. This is the same thing for text the user typed or a
    /// reading they placed, where there is no signal to ask.
    #[serde(default, skip_serializing_if = "is_false")]
    pub inverse: bool,
    /// A second string signal, laid out like `source`, whose characters pick
    /// each cell's colour through `codes`. Text grids only.
    ///
    /// The CH-47F is the case: DCS-BIOS sends each CDU line with a
    /// `_COLOR` twin, one letter per character. The letters are the module's
    /// own, so the profile says what they mean. A letter with no entry, and
    /// every cell until the signal arrives, draws in `colour`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colours: Option<ColourSource>,
    /// Characters this module sends in place of the ones it means, rewritten
    /// one for one before the part is laid out.
    ///
    /// DCS-BIOS cannot export every symbol a CDU draws, so it sends a
    /// stand-in: the A-10C's arrows arrive as `»` and `«`. Unlike `aliases`,
    /// which swap a whole value, this works inside a line of text. Each key
    /// and value is one character.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub replace: BTreeMap<String, String>,
    /// What a switch draws at each position of its selector. Empty on every
    /// piece that is not a switch.
    #[serde(default, skip_serializing_if = "Cases::is_empty")]
    pub cases: Cases,
}

/// One option on a piece inside a switch case: left to the switch, cleared, or
/// set.
///
/// Three states rather than an `Option`, because absent and cleared mean
/// different things here. Absent takes the switch's value, which is the whole
/// point of writing shared options once. `null` takes none, for the case that
/// wants no wrap where every other case wraps at 360.
#[derive(Debug, Clone)]
pub enum Tri<T> {
    Inherit,
    Clear,
    Set(T),
}

// By hand, because the derive would ask every `T` for a default it never
// uses, and `ColourSource` has none.
#[allow(clippy::derivable_impls)]
impl<T> Default for Tri<T> {
    fn default() -> Self {
        Tri::Inherit
    }
}

impl<T> Tri<T> {
    pub fn is_inherit(&self) -> bool {
        matches!(self, Tri::Inherit)
    }
}

impl<T: Clone> Tri<T> {
    /// The same as [`pick`](Self::pick) for an option that is absent rather
    /// than empty when unset.
    fn pick_opt(&self, inherits: bool, shared: &Option<T>) -> Option<T> {
        match self {
            Tri::Set(v) => Some(v.clone()),
            Tri::Clear => None,
            Tri::Inherit if inherits => shared.clone(),
            Tri::Inherit => None,
        }
    }
}

impl<T: Clone + Default> Tri<T> {
    /// The value a piece ends up with, given what the switch shares and
    /// whether this kind of piece takes it at all.
    fn pick(&self, inherits: bool, shared: &T) -> T {
        match self {
            Tri::Set(v) => v.clone(),
            Tri::Clear => T::default(),
            Tri::Inherit if inherits => shared.clone(),
            Tri::Inherit => T::default(),
        }
    }
}

impl<T: Serialize> Serialize for Tri<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            // Never written: every field of this type skips it.
            Tri::Inherit | Tri::Clear => s.serialize_none(),
            Tri::Set(v) => v.serialize(s),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Tri<T> {
    /// Only ever called for a key that is present, absent ones taking the
    /// default, so `null` here is the user clearing the option.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Ok(Option::<T>::deserialize(d)?.map_or(Tri::Clear, Tri::Set))
    }
}

fn is_inherit<T>(t: &Tri<T>) -> bool {
    t.is_inherit()
}

/// A piece as a switch case writes it: only what differs from the switch.
///
/// The same keys as [`Span`], in the same order so a case reads like any
/// other piece in the file. What a piece draws and where it sits (`text`,
/// `gap`, a rule and its label, a box) is never shared, since a switch has
/// none of its own to share; everything that shapes or styles a value is.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpanPatch {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub source: Tri<String>,
    /// A stored signal, never shared: like characters, it is what one case
    /// draws rather than how every case reads.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signal: String,
    /// Kept only so that a switch inside a case can be refused by name
    /// rather than silently dropped.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub switch: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub gap: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub rule: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_colour: Option<Colour>,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub width: usize,
    #[serde(default, skip_serializing_if = "is_left")]
    pub align: Align,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub reads: Tri<[f64; 2]>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub conversions: Tri<Vec<Conversion>>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub decimals: Tri<u8>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub digits: Tri<u8>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub round: Tri<Round>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub wrap: Tri<f64>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub value_aliases: Tri<BTreeMap<ValueBand, AliasDraw>>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub abs: Tri<bool>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub aliases: Tri<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub format: Tri<String>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub colour: Tri<Colour>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub small: Tri<bool>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub inverse: Tri<bool>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub colours: Tri<ColourSource>,
    #[serde(default, skip_serializing_if = "is_inherit")]
    pub replace: Tri<BTreeMap<String, String>>,
}

impl SpanPatch {
    /// The piece this draws, with what it leaves unset taken from `shared`.
    ///
    /// Each kind of piece takes only what applies to it. Characters typed into
    /// a case take the switch's colour, size, inverse and replacements, and
    /// never its `decimals`; a gap takes only the styling a rule draws in.
    ///
    /// `reads` and `conversions` are one choice made two ways, so a case that
    /// makes it either way, or clears it, takes neither from the switch. A
    /// case converting by stretches would otherwise inherit a range beside
    /// them and be refused for converting twice.
    ///
    /// A switch sharing a stored signal rather than a source hands it to a
    /// case that draws nothing else, with its words: that is a switch showing
    /// words for the positions of the stored number deciding it.
    fn resolve(&self, shared: &Span) -> Span {
        let shares_stored = !self.gap
            && self.text.is_empty()
            && self.signal.is_empty()
            && self.source.is_inherit()
            && shared.source.is_empty()
            && !shared.signal.is_empty();
        let signal = if shares_stored {
            shared.signal.clone()
        } else {
            self.signal.clone()
        };
        let reading = !self.gap && self.text.is_empty() && signal.is_empty();
        let typed = !self.gap;
        let converts = reading && self.reads.is_inherit() && self.conversions.is_inherit();
        Span {
            text: self.text.clone(),
            source: self.source.pick(reading, &shared.source),
            signal,
            stored: None,
            switch: self.switch.clone(),
            switch_signal: String::new(),
            switch_stored: None,
            switch_reads: None,
            gap: self.gap,
            rule: self.rule,
            label: self.label.clone(),
            label_colour: self.label_colour,
            width: self.width,
            align: self.align,
            reads: self.reads.pick_opt(converts, &shared.reads),
            conversions: self.conversions.pick(converts, &shared.conversions),
            decimals: self.decimals.pick(reading, &shared.decimals),
            digits: self.digits.pick(reading, &shared.digits),
            round: self.round.pick(reading, &shared.round),
            wrap: self.wrap.pick_opt(reading, &shared.wrap),
            value_aliases: self
                .value_aliases
                .pick(reading || shares_stored, &shared.value_aliases),
            abs: self.abs.pick(reading || shares_stored, &shared.abs),
            aliases: self.aliases.pick(reading, &shared.aliases),
            format: self.format.pick_opt(reading, &shared.format),
            colour: self.colour.pick_opt(true, &shared.colour),
            small: self.small.pick(true, &shared.small),
            inverse: self.inverse.pick(true, &shared.inverse),
            colours: self.colours.pick_opt(reading, &shared.colours),
            replace: self.replace.pick(typed, &shared.replace),
            cases: Cases::default(),
        }
    }
}

/// Which selector positions a case claims: a band in the `value_aliases`
/// spelling, or every position no band claims.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CaseKey {
    Band(ValueBand),
    /// Written `"else"`. Sorts after every band, which is where it is read.
    Else,
}

impl std::fmt::Display for CaseKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaseKey::Band(band) => band.fmt(f),
            CaseKey::Else => f.write_str("else"),
        }
    }
}

impl std::str::FromStr for CaseKey {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, String> {
        if s.trim() == "else" {
            return Ok(CaseKey::Else);
        }
        s.parse().map(CaseKey::Band)
    }
}

/// A case as written: one piece, or a chain of them.
///
/// The shape is kept so a file round trips as it was written: a case that
/// draws one reading is an object, and only a case that needs a label or a
/// unit beside it is an array.
#[derive(Debug, Clone)]
pub enum CaseWritten {
    One(Box<SpanPatch>),
    Chain(Vec<SpanPatch>),
}

impl CaseWritten {
    pub fn patches(&self) -> &[SpanPatch] {
        match self {
            CaseWritten::One(p) => std::slice::from_ref(&**p),
            CaseWritten::Chain(ps) => ps,
        }
    }

    pub fn patches_mut(&mut self) -> &mut [SpanPatch] {
        match self {
            CaseWritten::One(p) => std::slice::from_mut(&mut **p),
            CaseWritten::Chain(ps) => ps,
        }
    }
}

/// One position or band of a switch's selector, and what it draws.
#[derive(Debug, Clone)]
pub struct Case {
    pub when: CaseKey,
    pub written: CaseWritten,
    /// The pieces this case draws, with the switch's shared options filled
    /// in. Worked out once, when the field is loaded, rather than every
    /// frame: see [`Span::resolve_cases`].
    pub pieces: Vec<Span>,
}

/// A switch's cases, in the order they are matched: bands by where they start,
/// then `else`.
///
/// Held in that order whatever order they were written in, the same bargain
/// `value_aliases` makes, so two cases claiming one position settle the same
/// way on every machine: the lower one draws.
#[derive(Debug, Clone, Default)]
pub struct Cases(pub Vec<Case>);

impl Cases {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for Cases {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for case in &self.0 {
            let key = case.when.to_string();
            match &case.written {
                CaseWritten::One(p) => map.serialize_entry(&key, p)?,
                CaseWritten::Chain(ps) => map.serialize_entry(&key, ps)?,
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Cases {
    /// Read through JSON values rather than an untagged enum, so a mistake
    /// inside a case is reported as itself, with the case it is in, and not
    /// as "data did not match any variant".
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        use serde::de::Error as _;
        let raw = BTreeMap::<String, serde_json::Value>::deserialize(d)?;
        let mut cases = Vec::with_capacity(raw.len());
        for (key, value) in raw {
            let when: CaseKey = key.parse().map_err(D::Error::custom)?;
            let bad = |e: serde_json::Error| D::Error::custom(format!("case {key:?}: {e}"));
            let written = if value.is_array() {
                CaseWritten::Chain(serde_json::from_value(value).map_err(bad)?)
            } else {
                CaseWritten::One(serde_json::from_value(value).map_err(bad)?)
            };
            cases.push(Case {
                when,
                written,
                pieces: Vec::new(),
            });
        }
        cases.sort_by(|a, b| a.when.cmp(&b.when));
        Ok(Cases(cases))
    }
}

/// How close a selector's position has to be to a case's edge to count as
/// inside it. A selector sends whole positions and a case names them, so
/// this only absorbs a key written as `1.0` rather than `1`.
const SELECTOR_TOL: f64 = 1e-9;

/// What a switch draws in the current frame.
pub enum Picked<'a> {
    /// The selector has not arrived, so nothing is known to draw yet.
    Waiting,
    /// The case its position falls in, or None where no case claims it and
    /// there is no `else`: the switch then draws nothing.
    Case(Option<&'a Case>),
}

impl Span {
    /// Whether this part reads a signal rather than drawing what it was given.
    pub fn is_signal(&self) -> bool {
        !self.source.is_empty()
    }

    /// Whether this part draws a stored signal.
    pub fn is_stored(&self) -> bool {
        !self.signal.is_empty()
    }

    /// Whether this part draws something that arrives, rather than what it
    /// was given: a DCS-BIOS signal or a stored one.
    pub fn reads(&self) -> bool {
        self.is_signal() || self.is_stored()
    }

    /// Whether this piece picks what it draws by a selector's position.
    pub fn is_switch(&self) -> bool {
        !self.switch.is_empty() || !self.switch_signal.is_empty()
    }

    /// What decides a switch, as the checks name it: the stored signal's
    /// name, or the DCS-BIOS signal.
    pub fn switch_name(&self) -> String {
        match &self.switch_stored {
            Some(s) => s.name.clone(),
            None if !self.switch_signal.is_empty() => self.switch_signal.clone(),
            None => self.switch.clone(),
        }
    }

    /// Fill in each case's pieces from what it wrote and what the switch
    /// shares.
    ///
    /// Done once when a field is loaded, because a frame is the wrong place
    /// to be merging options, and because `compose` hands out references to
    /// the pieces it draws, which have to live as long as the field does.
    /// Anything that changes a switch's shared options after loading calls
    /// this again.
    pub fn resolve_cases(&mut self) {
        let mut cases = std::mem::take(&mut self.cases);
        for case in &mut cases.0 {
            case.pieces = case
                .written
                .patches()
                .iter()
                .map(|p| p.resolve(self))
                .collect();
        }
        self.cases = cases;
    }

    /// The case a selector reading falls in: the lowest band claiming it,
    /// and failing that `else`.
    pub fn case_for(&self, position: f64) -> Option<&Case> {
        self.cases
            .0
            .iter()
            .find(|c| matches!(&c.when, CaseKey::Band(b) if b.matches(position, SELECTOR_TOL)))
            .or_else(|| self.cases.0.iter().find(|c| c.when == CaseKey::Else))
    }

    /// Where a selector sending `value` of `max` is, in the units its cases
    /// are written in: as sent, or converted by `switch_reads` and rounded
    /// to a whole number.
    pub fn switch_position(&self, value: u16, max: u16) -> f64 {
        let Some([low, high]) = self.switch_reads else {
            return f64::from(value);
        };
        if max == 0 {
            return low.round();
        }
        (low + f64::from(value) / f64::from(max) * (high - low)).round()
    }

    /// The whole positions a selector can be at, lowest first: 0 to its
    /// maximum as sent, or every whole number `switch_reads` runs between.
    pub fn switch_span(&self, max: u16) -> (f64, f64) {
        match self.switch_reads {
            None => (0.0, f64::from(max)),
            Some([a, b]) => (a.min(b).round(), a.max(b).round()),
        }
    }

    /// What this switch draws given how signals read right now.
    pub fn pick<F>(&self, read: &F) -> Picked<'_>
    where
        F: Fn(&str) -> Option<Reading>,
    {
        self.pick_cached(read, &StoredCache::default(), &mut |_, _, _| {})
    }

    /// [`pick`](Self::pick), taking a stored signal that decides it from
    /// `cache`. Its number is matched as it is, already shaped; characters
    /// that are not a number leave only `else` to claim them.
    pub fn pick_cached<'a, F, S>(
        &'a self,
        read: &F,
        cache: &StoredCache,
        seen: &mut S,
    ) -> Picked<'a>
    where
        F: Fn(&str) -> Option<Reading>,
        S: FnMut(&'a str, u16, &str),
    {
        if !self.switch_signal.is_empty() {
            let Some(stored) = &self.switch_stored else {
                return Picked::Case(None);
            };
            return match cache.get(stored, read, seen) {
                None => Picked::Waiting,
                Some(v) => Picked::Case(match v.number {
                    Some(n) => self.case_for(n),
                    None => self.cases.0.iter().find(|c| c.when == CaseKey::Else),
                }),
            };
        }
        match read(&self.switch) {
            None => Picked::Waiting,
            Some(Reading::Number { value, max }) => {
                Picked::Case(self.case_for(self.switch_position(value, max)))
            }
            // A selector that reports characters has no position to match,
            // and `problems` says so. Only `else` can claim it.
            Some(Reading::Text(_)) => {
                Picked::Case(self.cases.0.iter().find(|c| c.when == CaseKey::Else))
            }
        }
    }

    /// Whether anything here says how to draw a number: a range, decimal
    /// places, rounding, a wrap or a dropped sign.
    pub fn shapes_a_number(&self) -> bool {
        self.works_out_a_number() || self.abs
    }

    /// Whether anything here works the number out, rather than only drawing
    /// it without its sign: everything [`shapes_a_number`](Self::shapes_a_number)
    /// asks but `abs`. A piece drawing a shared result may drop the sign,
    /// since the result keeps it for the conditions and bands that test it.
    pub fn works_out_a_number(&self) -> bool {
        self.reads.is_some()
            || !self.conversions.is_empty()
            || self.decimals != 0
            || self.digits != 0
            || self.round != Round::Nearest
            || self.wrap.is_some()
    }

    /// Half of the last decimal place a reading is shown to, which is how
    /// close a reading has to be to a band's edge to count as inside it.
    pub(crate) fn tolerance(&self) -> f64 {
        10f64.powi(-i32::from(self.decimals)) / 2.0
    }

    /// Whether the user has finished saying what this part draws.
    ///
    /// An empty part is unfinished work rather than a mistake, but it still
    /// stops the profile loading, so it is said plainly and in those terms. A
    /// gap is finished the moment it exists: drawing nothing is the whole of
    /// what it does. A switch is finished once it names a selector: whether
    /// its cases are is asked of each case.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.source.is_empty()
            && self.signal.is_empty()
            && !self.gap
            && !self.is_switch()
    }

    /// Whether this part carries something the flat shape has nowhere to put.
    ///
    /// The flat shape's `align`, `label` and `label_colour` are the field's,
    /// so a part with its own has to be written as a chain of one rather than
    /// quietly handing its setting to the field. A switch has no flat
    /// spelling at all: no field written before switches has one, so nothing
    /// is lost by writing it as a chain.
    pub fn needs_chain(&self) -> bool {
        self.width > 0
            || self.align != Align::Left
            || self.rule
            || !self.label.is_empty()
            || self.is_switch()
            || !self.cases.is_empty()
    }

    /// Turn a raw signal value into the characters this part should show.
    ///
    /// The text half of [`format_reading`](Self::format_reading), for the
    /// callers that only draw characters: the `listen` log, and every test
    /// that measures what a face reads.
    pub fn format_number(&self, value: u16, max: u16) -> String {
        self.format_reading(value, max).0
    }

    /// Turn a raw signal value into the characters this part should show, and
    /// the band it landed in, whose colour and inverse are the paint's to use.
    ///
    /// `max` is the source's own declared maximum, so a needle at 0..65535 and
    /// a selector at 0..10 go through the same arithmetic. With no `reads` the
    /// range is the signal's own, which makes the conversion an identity: the
    /// number as sent.
    ///
    /// The order is the whole of the design. Convert, round and wrap first, so
    /// a band is matched against the number on the dial rather than the raw
    /// count. Then a band claims the reading and draws its own characters.
    /// Only if none does is the number drawn, and only then is the sign
    /// dropped for `abs`, so a band written for negative readings still
    /// matches on a piece that draws magnitudes.
    ///
    /// A band that draws the reading claims it for its style alone: the
    /// number is drawn exactly as if no band had matched, and the band is
    /// still handed back for its colour and inverse.
    pub fn format_reading(&self, value: u16, max: u16) -> (String, Option<&AliasDraw>) {
        let (reading, half_step) = self.convert(value, max);
        let reading = self.settle(reading, half_step);
        let band = self.band_for(reading);
        if let Some(drawn) = band.filter(|b| !b.reading) {
            return (drawn.text.clone(), Some(drawn));
        }
        let shown = if self.abs { reading.abs() } else { reading };
        (self.format_number_at(shown), band)
    }

    /// The characters this part draws for a stored signal's value, and the
    /// band it landed in, the way [`format_reading`](Self::format_reading)
    /// does for a reading.
    ///
    /// The signal has already shaped the number, so all that is left is the
    /// bands: matched against the value read as a number, to the places it
    /// shows. Characters that are not a number draw as they are. A number no
    /// band claims loses its sign for `abs`, after the bands, as a reading's
    /// does.
    pub fn format_stored(&self, value: &StoredValue) -> (String, Option<&AliasDraw>) {
        let tol = value.tolerance();
        let band = value.number.and_then(|n| {
            self.value_aliases
                .iter()
                .find(|(band, _)| band.matches(n, tol))
                .map(|(_, drawn)| drawn)
        });
        if let Some(drawn) = band.filter(|b| !b.reading) {
            return (drawn.text.clone(), Some(drawn));
        }
        if self.abs && value.number.is_some() {
            return (value.text.replacen('-', "", 1), band);
        }
        (value.text.clone(), band)
    }

    /// Characters held to this part's box, where it has one: padded on the
    /// side `align` says, cropped from the end it anchors away from. The text
    /// half of what [`box_fit`](Self::box_fit) does to glyphs, for a stored
    /// signal's terms, which are characters until a field draws them.
    pub fn fit_text(&self, text: &str) -> String {
        let len = text.chars().count();
        if self.width == 0 || len == self.width {
            return text.to_string();
        }
        if len > self.width {
            let front = self.align.pad_before(self.width, len);
            return text.chars().skip(front).take(self.width).collect();
        }
        let before = self.align.pad_before(len, self.width);
        let after = self.width - len - before;
        format!("{}{text}{}", " ".repeat(before), " ".repeat(after))
    }

    /// What the dial reads at a raw count, before rounding, and half of one
    /// count in those units.
    ///
    /// The half count is there because DCS-BIOS sends the nearest count to
    /// where the drum is, so a drum sitting exactly on its 7 can arrive a hair
    /// short of it, and rounding down must not make that a 6. On a dial with
    /// stretches it is the stretch's own, since a count is worth more of the
    /// face where the marks are crowded.
    fn convert(&self, value: u16, max: u16) -> (f64, f64) {
        if let Some(stretch) = self.conversion_for(value) {
            return stretch.convert(value);
        }
        let [low, high] = self.reads.unwrap_or([0.0, f64::from(max)]);
        if max == 0 {
            return (low, 0.0);
        }
        let travel = f64::from(value) / f64::from(max);
        (
            low + travel * (high - low),
            (high - low).abs() / f64::from(max) / 2.0,
        )
    }

    /// The stretch that converts a raw count, where this dial has stretches.
    ///
    /// The one claiming it, and failing that the nearest, whose end is what a
    /// count past it reads. Two claiming one count is a caution, and the lower
    /// one converts it, the same bargain two bands make.
    pub fn conversion_for(&self, value: u16) -> Option<&Conversion> {
        self.conversions
            .iter()
            .filter(|c| c.claims(value))
            .min_by_key(|c| c.raw[0])
            .or_else(|| {
                self.conversions
                    .iter()
                    .min_by_key(|c| (c.distance(value), c.raw[0]))
            })
    }

    /// What the dial is marked with at each end of its travel: `reads` as it
    /// was written, the lowest and highest a chain of stretches reaches, or
    /// the signal's own range where there is neither.
    pub fn face(&self, max: u16) -> [f64; 2] {
        if self.conversions.is_empty() {
            return self.reads.unwrap_or([0.0, f64::from(max)]);
        }
        let ends = self.face_ends(max);
        let lo = ends.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = ends.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        [lo, hi]
    }

    /// Every reading at the end of a straight stretch of the face, which is
    /// where the widest number it draws has to be.
    fn face_ends(&self, max: u16) -> Vec<f64> {
        if self.conversions.is_empty() {
            return self.face(max).to_vec();
        }
        self.conversions.iter().flat_map(|c| c.reads).collect()
    }

    /// The counts from 0 to `max` no stretch claims, as closed runs.
    ///
    /// Empty for a dial with no stretches, which converts its whole range.
    pub fn unconverted(&self, max: u16) -> Vec<[u16; 2]> {
        if self.conversions.is_empty() {
            return Vec::new();
        }
        let mut runs: Vec<[u16; 2]> = self.conversions.iter().map(|c| c.raw).collect();
        runs.sort();
        let mut out = Vec::new();
        // The next count nothing has claimed yet. u32 so that a stretch ending
        // on 65535 can move it past the end.
        let mut next: u32 = 0;
        for [a, b] in runs {
            if u32::from(a) > next && next <= u32::from(max) {
                out.push([next as u16, (a - 1).min(max)]);
            }
            next = next.max(u32::from(b) + 1);
        }
        if next <= u32::from(max) {
            out.push([next as u16, max]);
        }
        out
    }

    /// Whether the bands between them claim every reading this face can show.
    ///
    /// A reading no band claims draws as a number, so this is what decides
    /// whether the number's width has to be allowed for at all. Exact rather
    /// than cautious, because the editor's answer is how many characters will
    /// be dropped and a maybe would make that a guess.
    ///
    /// Walked as intervals rather than by each band's ends, so a list is its
    /// own values and not the run between them: `"0,2"` leaves 1 to the
    /// number. Neighbours are allowed one step of daylight between them,
    /// because a reading between two bands a step apart is one the face cannot
    /// show once it has been rounded.
    fn bands_cover(&self, low: f64, high: f64) -> bool {
        if self.value_aliases.is_empty() {
            return false;
        }
        let (lo, hi) = (low.min(high), low.max(high));
        let step = 10f64.powi(-i32::from(self.decimals));
        let tol = self.tolerance();
        // A band drawing the reading leaves the number to be drawn, so it
        // covers nothing.
        let mut spans: Vec<(f64, f64)> = self
            .value_aliases
            .iter()
            .filter(|(_, drawn)| !drawn.reading)
            .map(|(band, _)| band)
            .flat_map(|band| match band {
                ValueBand::Range { lo, hi } => vec![(*lo, *hi)],
                other => other.values().into_iter().map(|v| (v, v)).collect(),
            })
            .collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Everything below the face counts as behind us already, so the first
        // band is held to the same test as every other one.
        let mut reach = lo - step;
        for (start, end) in spans {
            if start > reach + step + tol {
                return false;
            }
            reach = reach.max(end);
        }
        reach >= hi - tol
    }

    /// The band claiming a settled reading, where one does.
    ///
    /// Two bands claiming one reading is a caution rather than a refusal, so
    /// this has to settle it, and the first match is what it settles on. Bands
    /// are held in order of where they start, so that is the lower of the two,
    /// every time and on every machine. The order they were written in never
    /// decides anything.
    fn band_for(&self, reading: f64) -> Option<&AliasDraw> {
        let tol = self.tolerance();
        self.value_aliases
            .iter()
            .find(|(band, _)| band.matches(reading, tol))
            .map(|(_, drawn)| drawn)
    }

    /// Round a converted reading the way this part says, then wrap it.
    ///
    /// Rounded before wrapping, so a compass at 359.7 rounds to 360 and then
    /// draws 0, and a drum at 9.8 rounded to the nearest draws 0 rather than
    /// a 10 that does not fit its cell.
    fn settle(&self, reading: f64, half_step: f64) -> f64 {
        let places = self.decimals as usize;
        let rounded = match self.round {
            // Through the formatter, which is how every reading was rounded
            // before this setting existed, so nothing already on the glass
            // moves by so much as a digit.
            Round::Nearest => format!("{reading:.places$}").parse().unwrap_or(reading),
            Round::Down => {
                let scale = 10f64.powi(i32::from(self.decimals));
                ((reading + half_step) * scale).floor() / scale
            }
            // The same allowance the other way: a drum a hair past its 7 has
            // not started towards the 8.
            Round::Up => {
                let scale = 10f64.powi(i32::from(self.decimals));
                ((reading - half_step) * scale).ceil() / scale
            }
        };
        let wrapped = match self.wrap {
            Some(every) if every > 0.0 => rounded.rem_euclid(every),
            _ => rounded,
        };
        // Zero is drawn as 0. Negative zero, from a face that runs up from
        // below it or a remainder taken of one, would otherwise draw as -0.
        if wrapped == 0.0 {
            0.0
        } else {
            wrapped
        }
    }

    /// The glyph this part draws in place of `value`, where it has one.
    pub fn alias<'a>(&'a self, value: &'a str) -> &'a str {
        self.aliases.get(value).map(String::as_str).unwrap_or(value)
    }

    /// Swap each stand-in character for the one it stands for.
    pub fn replace_chars(&self, text: &str) -> String {
        if self.replace.is_empty() {
            return text.to_string();
        }
        text.chars()
            .map(|c| {
                let mut buf = [0u8; 4];
                self.replace
                    .get(c.encode_utf8(&mut buf) as &str)
                    .and_then(|r| r.chars().next())
                    .unwrap_or(c)
            })
            .collect()
    }

    /// The widest this part can ever draw, in cells, where that is knowable.
    ///
    /// `number_max` is the source's own maximum for a numeric source and None
    /// for a string. None back only for a string with no `max_length`, which
    /// is the one case nothing bounds. A number is always known: converted, it
    /// runs between the ends of `reads`, and shown as sent it runs from 0 to
    /// its maximum, the same default `format_number` takes. Everything is
    /// known ahead of a single frame arriving, which is what lets the editor
    /// say how many characters will be dropped rather than warning vaguely
    /// that some might be.
    pub fn widest(&self, max_length: Option<usize>, number_max: Option<u16>) -> Option<usize> {
        // A gap takes what is left over, so it never asks for room of its own
        // and can never be the reason content will not fit. A boxed one does
        // ask: it is a fixed run of blanks, or of dashes.
        if self.gap {
            return Some(self.width);
        }
        // A box is the whole answer: whatever it reads, it draws this many
        // cells.
        if self.width > 0 {
            return Some(self.width);
        }
        if !self.is_signal() {
            return Some(self.text.chars().count());
        }
        let Some(max) = number_max else {
            return max_length;
        };
        let longest_alias = self
            .value_aliases
            .values()
            .filter(|a| !a.reading)
            .map(|a| a.text.chars().count())
            .max()
            .unwrap_or(0);
        let [low, high] = self.face(max);
        // Bands covering the whole face mean no number is ever drawn, so only
        // the bands count. Bands that stop short leave the rest as numbers.
        if self.bands_cover(low, high) {
            return Some(longest_alias);
        }
        let mut ends: Vec<f64> = self
            .face_ends(max)
            .into_iter()
            .map(|end| self.settle(end, 0.0))
            .collect();
        // A reading that starts over somewhere between its ends can draw
        // anything up to the last value before it does, whatever the ends
        // themselves come to.
        if let Some(every) = self.wrap.filter(|&w| w > 0.0) {
            let (a, b) = (low.min(high), low.max(high));
            if b - a >= every || (a / every).floor() != (b / every).floor() {
                ends.push((every - 10f64.powi(-i32::from(self.decimals))).max(0.0));
            }
        }
        let number = ends
            .into_iter()
            // The sign goes before the width is taken, or a face running below
            // zero is measured a cell wider than it ever draws.
            .map(|end| {
                let shown = if self.abs { end.abs() } else { end };
                self.format_number_at(shown).chars().count()
            })
            .max()
            .unwrap_or(0);
        Some(number.max(longest_alias))
    }

    /// The characters this part would draw for one real reading, used to
    /// measure the ends of its range.
    fn format_number_at(&self, reading: f64) -> String {
        let places = self.decimals as usize;
        if self.digits == 0 {
            return format!("{reading:.places$}");
        }
        // Padded as a magnitude, so the zeros go after the sign rather than
        // before it. The reading is already settled, so it is never -0.
        let magnitude = format!("{:.places$}", reading.abs());
        let whole = magnitude.find('.').unwrap_or(magnitude.len());
        let zeros = "0".repeat(usize::from(self.digits).saturating_sub(whole));
        let sign = if reading < 0.0 { "-" } else { "" };
        format!("{sign}{zeros}{magnitude}")
    }

    /// What a gap puts in the cells it was given: blanks, or a rule.
    ///
    /// The rule is drawn by `divider_rule`, the same function a whole field's
    /// divider goes through, so the two cannot drift apart and the editor's
    /// preview keeps working by asking for it rather than working it out.
    ///
    /// An elastic rule is handed whatever the chain left, which may be less
    /// than its label needs. `divider_rule` leaves the label off and draws a
    /// plain line in that case, which is why a label needs a box: in one, the
    /// width is known and `problems` can refuse a label that will not fit
    /// instead of letting it come and go with the reading beside it.
    fn fill(&self, width: usize) -> Vec<Glyph> {
        if !self.rule {
            return vec![Glyph::blank(); width];
        }
        divider_rule(width, &self.label)
            .into_iter()
            .map(|cell| Glyph {
                colour: if cell.label {
                    self.label_colour.or(self.colour)
                } else {
                    self.colour
                },
                text: cell.text,
                small: self.small,
                inverse: self.inverse,
            })
            .collect()
    }

    /// Hold this piece to exactly `width` cells, where it has one.
    ///
    /// The whole point of a box is that the pieces after it do not move, so
    /// this runs before the gaps are measured and a box counts as fixed room
    /// like any typed characters.
    ///
    /// Padded with blanks on the side `align` says, and cropped from the end
    /// `align` anchors away from, which is the same bargain the field makes
    /// with its run: a right aligned box drops its leading pad rather than its
    /// last digit.
    fn box_fit(&self, mut glyphs: Vec<Glyph>) -> Vec<Glyph> {
        if self.width == 0 || glyphs.len() == self.width {
            return glyphs;
        }
        if glyphs.len() > self.width {
            // The same sum read the other way round: the blanks a narrower
            // value would have been given at the front are the glyphs a wider
            // one loses there.
            let front = self.align.pad_before(self.width, glyphs.len());
            glyphs.drain(..front);
            glyphs.truncate(self.width);
            return glyphs;
        }
        let before = self.align.pad_before(glyphs.len(), self.width);
        let mut out = vec![Glyph::blank(); before];
        out.extend(glyphs);
        out.resize(self.width, Glyph::blank());
        out
    }
}

/// One cell's worth of drawn content: what to draw there and how.
///
/// Usually one character. A run of exactly one cell takes the whole value as a
/// single glyph, which is not a convenience: a two-character field really does
/// occupy one cell on this hardware, and its glyph is not the union of the two
/// characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    pub text: String,
    pub colour: Option<Colour>,
    pub small: bool,
    pub inverse: bool,
}

impl Glyph {
    /// A blank cell, which is what a field pads with when its content is
    /// shorter than its run.
    pub fn blank() -> Self {
        Glyph {
            text: " ".to_string(),
            colour: None,
            small: false,
            inverse: false,
        }
    }
}

/// What the state of the world says a signal currently reads.
///
/// The engine looks it up; everything done with it afterwards is policy and
/// belongs here, so a field composes the same way in a test as on the glass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    Text(String),
    Number { value: u16, max: u16 },
}

/// One field of a display, and the content that fills it.
///
/// A field has exactly one owner. Nothing chooses between two sources for the
/// same cells at runtime: on an aircraft that drives its own display, the
/// cockpit has already decided what belongs there, and on any other the user
/// has.
///
/// On disk a field with one part is written flat, with that part's `source` and
/// styling beside the cells, and only a chain of two or more is written as
/// `content`. That is not cosmetic: every profile written before chains existed
/// is in the flat shape, and it has to keep loading, and keep saving back
/// unchanged, or an update would rewrite rows the user owns.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "ReadoutRepr", into = "ReadoutRepr")]
pub struct Readout {
    pub device: String,
    /// Which display on that device, matching a key in `data/displays`.
    pub display: String,
    pub cells: CellRange,
    /// Draw a fixed rule across these cells instead of drawing content.
    ///
    /// A screen only half used needs somewhere for the eye to stop. The Apache
    /// puts its keyboard unit on the bottom line and the A-10C starts ten lines
    /// down, and with the rest of the glass dark the page has no edge. A rule
    /// gives it one. It reads nothing, so it is drawn from the moment the
    /// aircraft loads and never changes afterwards.
    ///
    /// Text grids only, like `colour` and `small`: a segment display draws from
    /// a glyph table, and none of them has a rule in it.
    pub divider: bool,
    /// The rule's colour. A divider is the user's own addition rather than
    /// something the cockpit decided, so unlike a field's colour it is theirs
    /// to choose. Meaningless, and left None, on anything else.
    pub colour: Option<Colour>,
    /// Characters set into the middle of the rule, naming what it divides.
    ///
    /// A rule ends a page; a labelled rule says what the page was. It reads
    /// nothing, like the rest of a divider, so it is on the glass from the
    /// moment the aircraft loads. Empty on anything that is not a divider.
    pub label: String,
    /// The label's colour, which is its own rather than the rule's: a label
    /// drawn in the line's colour reads as part of the line. Falls back to the
    /// rule's colour when nobody has said, which is what a label added to a
    /// rule that already had a colour should look like until it is told
    /// otherwise.
    pub label_colour: Option<Colour>,
    /// Only paint this field from one crew station.
    ///
    /// DCS-BIOS exports the whole cockpit whatever seat you are sitting in, so
    /// a multicrew aircraft publishes both stations at once and a field has no
    /// way to know which one you want. `SEAT_POSITION` is how DCS-BIOS reports
    /// the seat, and it names it the same way in every module that has one, so
    /// this is a convention rather than knowledge of any aircraft.
    ///
    /// Two fields may share cells when their seats differ, which is the point:
    /// the same window shows the pilot one thing and the gunner another. A
    /// field with no seat is always painted, and shares with nothing.
    ///
    /// Only meaningful on a module that reports a seat at all, which is 5 of
    /// the 50 catalogued. Validation rejects it elsewhere rather than silently
    /// never painting.
    pub seat: Option<u32>,
    /// Which end of the run the composed content anchors to.
    pub align: Align,
    /// The parts of this field, drawn end to end. Empty on a divider, which
    /// draws a rule instead.
    pub content: Vec<Span>,
    pub note: String,
    /// The page this field was taken from, when a profile's pages have been
    /// resolved into fields. Never written: on disk a page field lives in its
    /// page file and a profile's own fields have no page.
    ///
    /// It is what tells a field a page put on a screen from one written
    /// loose on it, which version 2 refuses.
    pub page: Option<String>,
}

/// A field as it is written on disk.
///
/// Both shapes at once: the flat one, which is every profile written before
/// chains and every field that still has one part, and `content`, which is a
/// chain. `Readout` converts through this in both directions, so the flat shape
/// cannot be forgotten about by some code path that builds a field by hand.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReadoutRepr {
    // Both left out of a field on a page, which takes its display from the
    // page and its device from the slot that shows it. A profile's own fields
    // always have both, so a profile is written as it always was.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    device: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    display: String,
    cells: CellRange,
    #[serde(default, skip_serializing_if = "is_false")]
    divider: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    signal: String,
    #[serde(default, skip_serializing_if = "is_false")]
    gap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seat: Option<u32>,
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
    #[serde(default, skip_serializing_if = "is_false")]
    abs: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    value_aliases: BTreeMap<ValueBand, AliasDraw>,
    #[serde(default, skip_serializing_if = "is_left")]
    align: Align,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    colour: Option<Colour>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label_colour: Option<Colour>,
    #[serde(default, skip_serializing_if = "is_false")]
    small: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    inverse: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    aliases: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    colours: Option<ColourSource>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    replace: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    content: Vec<Span>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    note: String,
}

impl From<ReadoutRepr> for Readout {
    fn from(r: ReadoutRepr) -> Self {
        // A divider draws a rule and holds no content. Everything else holds
        // either the chain it was written with, or the one part the flat shape
        // describes, which is also what an unfinished field gets: one empty
        // part, so there is a row for the user to finish rather than nothing.
        let mut content = if !r.content.is_empty() {
            r.content
        } else {
            vec![Span {
                text: r.text,
                source: r.source,
                signal: r.signal,
                gap: r.gap,
                reads: r.reads,
                conversions: r.conversions,
                decimals: r.decimals,
                digits: r.digits,
                round: r.round,
                wrap: r.wrap,
                abs: r.abs,
                value_aliases: r.value_aliases,
                aliases: r.aliases,
                format: r.format,
                colour: r.colour,
                small: r.small,
                inverse: r.inverse,
                colours: r.colours,
                replace: r.replace,
                // The flat shape has no spelling for the rest: its `align` and
                // `label` belong to the field, so a part written flat never
                // carried one of its own.
                ..Span::default()
            }]
        };
        // Every way a field arrives, from a file or from the editor, comes
        // through here, so this is where a switch's cases are filled in.
        for span in &mut content {
            span.resolve_cases();
        }
        Readout {
            device: r.device,
            display: r.display,
            cells: r.cells,
            divider: r.divider,
            colour: if r.divider { r.colour } else { None },
            // Only a rule draws a label. Dropped rather than kept on anything
            // else, for the same reason its colour is: it would be a setting
            // the window never shows and nothing ever draws.
            label: if r.divider { r.label } else { String::new() },
            label_colour: if r.divider { r.label_colour } else { None },
            seat: r.seat,
            align: r.align,
            content,
            note: r.note,
            page: None,
        }
    }
}

impl From<Readout> for ReadoutRepr {
    fn from(r: Readout) -> Self {
        // One span goes back flat, which is how it arrived and how every
        // shipped default is written. Only a real chain needs `content`, so
        // opening a profile and saving it changes nothing.
        //
        // A rule draws itself and has no content. What it carries is only kept
        // so that a signal written on one can still be refused rather than
        // quietly dropped, and it goes back beside the cells the way it came.
        //
        // A piece that is boxed, aligned inside its box or drawing a rule goes
        // as a chain of one instead. The flat shape has an `align` and a
        // `label` already and they belong to the field, so writing a piece's
        // own into them would be two different settings sharing a key: a box
        // aligned right inside a field aligned left has no flat spelling. The
        // flat shape is therefore exactly what it always was, and every
        // profile written before this still round trips byte for byte.
        let flat_one = r.content.len() == 1 && !r.content[0].needs_chain();
        let (one, chain) = if r.divider || flat_one {
            (r.content.first().cloned(), Vec::new())
        } else {
            (None, r.content)
        };
        let span = one.unwrap_or_default();
        ReadoutRepr {
            device: r.device,
            display: r.display,
            cells: r.cells,
            divider: r.divider,
            text: span.text,
            source: span.source,
            signal: span.signal,
            gap: span.gap,
            seat: r.seat,
            reads: span.reads,
            conversions: span.conversions,
            decimals: span.decimals,
            digits: span.digits,
            round: span.round,
            wrap: span.wrap,
            abs: span.abs,
            value_aliases: span.value_aliases,
            align: r.align,
            format: span.format,
            colour: if r.divider { r.colour } else { span.colour },
            // A colour for a label that is not there is a setting nothing
            // draws. The window holds on to it while the text is being edited,
            // so clearing it to retype costs nothing, but it stops there.
            label_colour: if r.label.is_empty() {
                None
            } else {
                r.label_colour
            },
            label: r.label,
            small: span.small,
            inverse: span.inverse,
            aliases: span.aliases,
            colours: span.colours,
            replace: span.replace,
            content: chain,
            note: r.note,
        }
    }
}

impl Default for Readout {
    /// An unfinished field on nothing in particular, which is what the editor
    /// starts from and what a test fills in the two or three parts it cares
    /// about. One empty span rather than none, so there is a piece to fill.
    fn default() -> Self {
        Readout {
            device: String::new(),
            display: String::new(),
            cells: CellRange { first: 0, last: 0 },
            divider: false,
            colour: None,
            label: String::new(),
            label_colour: None,
            seat: None,
            align: Align::Left,
            content: vec![Span::default()],
            note: String::new(),
            page: None,
        }
    }
}

impl Readout {
    /// A field of one span reading `source`, which is how nearly every field
    /// in a shipped profile is written.
    pub fn reading(device: &str, display: &str, cells: CellRange, source: &str) -> Self {
        Readout {
            device: device.to_string(),
            display: display.to_string(),
            cells,
            content: vec![Span {
                source: source.to_string(),
                ..Span::default()
            }],
            ..Readout::default()
        }
    }
}

pub(crate) fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

pub(crate) fn is_zero_usize(n: &usize) -> bool {
    *n == 0
}

pub(crate) fn is_left(a: &Align) -> bool {
    *a == Align::Left
}

pub(crate) fn is_nearest(r: &Round) -> bool {
    *r == Round::Nearest
}

impl Readout {
    /// Every signal this field reads, in the order the parts are drawn.
    ///
    /// Includes the second signals, `format` and `colours`, because a profile
    /// that names one the module does not have is as broken as one that names
    /// a missing source.
    ///
    /// A switch's selector is here as well as everything its cases read,
    /// because the engine repaints a field only when something in this list
    /// changes, and turning the selector is a change the field has to show.
    /// So is every signal a stored signal reads, for the same reason.
    pub fn sources(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for span in &self.content {
            if !span.switch.is_empty() {
                out.push(span.switch.as_str());
            }
            if let Some(stored) = &span.switch_stored {
                out.extend(stored.sources());
            }
        }
        for span in self.pieces() {
            if span.is_signal() {
                out.push(span.source.as_str());
            }
            if let Some(stored) = &span.stored {
                out.extend(stored.sources());
            }
            if let Some(f) = &span.format {
                out.push(f.as_str());
            }
            if let Some(c) = &span.colours {
                out.push(c.source.as_str());
            }
        }
        out
    }

    /// Whether anything here reads a signal at all.
    pub fn reads_anything(&self) -> bool {
        self.content.iter().any(Span::is_switch) || self.pieces().any(Span::reads)
    }

    /// Whether any piece of this field is a switch.
    pub fn has_switch(&self) -> bool {
        self.content.iter().any(Span::is_switch)
    }

    /// Every piece this field can draw: its own, with each switch standing
    /// for the pieces of all its cases.
    pub fn pieces(&self) -> impl Iterator<Item = &Span> {
        self.content.iter().flat_map(|span| {
            let own = (!span.is_switch()).then_some(span);
            let cased = span
                .cases
                .0
                .iter()
                .filter(move |_| span.is_switch())
                .flat_map(|c| c.pieces.iter());
            own.into_iter().chain(cased)
        })
    }

    /// This field once for each way its switches can fall, every switch
    /// replaced by the pieces of one case.
    ///
    /// What the checks run on. A switch draws exactly one case at a time, so
    /// a field is as wide as its widest case and as wrong as any case is,
    /// and asking the plain field's questions of each variant answers both
    /// without the checks having to know switches exist. A field without one
    /// is its own single variant.
    ///
    /// A switch with no cases stands for nothing, which is what it draws. A
    /// switch inside a case is left out, since it is refused on its own.
    pub fn variants(&self) -> Vec<Readout> {
        let mut out = vec![Readout {
            content: Vec::new(),
            ..self.clone()
        }];
        for span in &self.content {
            if !span.is_switch() {
                for v in &mut out {
                    v.content.push(span.clone());
                }
                continue;
            }
            if span.cases.is_empty() {
                continue;
            }
            out = out
                .into_iter()
                .flat_map(|v| {
                    span.cases.0.iter().map(move |case| {
                        let mut v = v.clone();
                        v.content
                            .extend(case.pieces.iter().filter(|p| !p.is_switch()).cloned());
                        v
                    })
                })
                .collect();
        }
        out
    }

    /// The rule this divider draws, one glyph per cell, its label marked.
    pub fn divider_cells(&self) -> Vec<RuleCell> {
        divider_rule(self.cells.len(), &self.label)
    }

    /// How many cells the piece at `index` draws, where that never changes.
    ///
    /// A box is the whole answer, and typed characters are their own length.
    /// An elastic gap has one too, but only when every piece it shares the
    /// line with is itself settled: the leftover is what the rest did not
    /// use, so one reading that sheds a digit widens the gaps beside it. On a
    /// line of boxes and typed characters, or on a line the gap has to
    /// itself, the leftover is the same in every frame and a rule there is as
    /// good as boxed.
    ///
    /// None for a piece as wide as whatever it reads, and for a gap on a line
    /// carrying one. Ahead of a single frame arriving, which is what lets the
    /// editor tell a label that will hold still from one that would come and
    /// go.
    pub fn settled_cells(&self, index: usize) -> Option<usize> {
        let span = self.content.get(index)?;
        if !span.gap || span.width > 0 {
            return self.settled_span(span);
        }
        let mut used = 0usize;
        let mut elastic: Vec<usize> = Vec::new();
        for (at, other) in self.content.iter().enumerate() {
            match self.settled_span(other) {
                Some(cells) => used += cells,
                None if other.gap => elastic.push(at),
                None => return None,
            }
        }
        // The same sum `compose` does: the leftover split evenly, the
        // remainder going to the earlier gaps.
        let spare = self.cells.len().saturating_sub(used);
        let rank = elastic.iter().position(|&at| at == index)?;
        Some(spare / elastic.len() + usize::from(rank < spare % elastic.len()))
    }

    /// What one piece takes off the line before the gaps are measured. None
    /// for an elastic gap, which is measured from the leftover, and for a
    /// piece as wide as whatever it reads.
    fn settled_span(&self, span: &Span) -> Option<usize> {
        if span.width > 0 {
            return Some(span.width);
        }
        // A switch is as wide as whichever case is drawing.
        if span.gap || span.is_switch() {
            return None;
        }
        // A one cell run takes a piece's whole value as a single glyph, the
        // same as `compose` does, so its length is the run's.
        if self.cells.len() == 1 {
            return Some(1);
        }
        (!span.reads()).then(|| span.text.chars().count())
    }

    /// The glyphs this field draws right now, one per cell, or None while it
    /// is still waiting on every signal it reads.
    ///
    /// None rather than a run of blanks, so a field whose signal has not
    /// arrived leaves the cells alone instead of writing spaces over them. A
    /// chain that has some of its signals draws what it has: a label should be
    /// on the glass before the reading beside it is.
    pub fn compose<F>(&self, read: F) -> Option<Vec<Glyph>>
    where
        F: Fn(&str) -> Option<Reading>,
    {
        self.compose_seen(read, |_, _, _| {})
    }

    /// [`compose`](Self::compose), told each number it converts: the signal,
    /// the count it arrived as, and the characters it became, before aliases
    /// and replacements. What the converter logs, so the log says what the
    /// glass was given rather than working it out a second time.
    pub fn compose_seen<'a, F, S>(&'a self, read: F, seen: S) -> Option<Vec<Glyph>>
    where
        F: Fn(&str) -> Option<Reading>,
        S: FnMut(&'a str, u16, &str),
    {
        self.compose_cached(read, &StoredCache::default(), seen)
    }

    /// [`compose_seen`](Self::compose_seen), taking stored signals from
    /// `cache`, so a paint works each one out once however many fields draw
    /// it. A signal's terms are told to `seen` the first time only.
    pub fn compose_cached<'a, F, S>(
        &'a self,
        read: F,
        cache: &StoredCache,
        mut seen: S,
    ) -> Option<Vec<Glyph>>
    where
        F: Fn(&str) -> Option<Reading>,
        S: FnMut(&'a str, u16, &str),
    {
        if self.divider {
            return Some(
                self.divider_cells()
                    .into_iter()
                    .map(|cell| Glyph {
                        colour: if cell.label {
                            self.label_colour.or(self.colour)
                        } else {
                            self.colour
                        },
                        text: cell.text,
                        small: false,
                        inverse: false,
                    })
                    .collect(),
            );
        }
        let width = self.cells.len();
        let mut waiting = false;
        // The chain this frame draws: each switch replaced by the pieces of
        // the case its selector is in, so everything below lays out one plain
        // chain and a case's gaps share the line with the pieces around the
        // switch. A selector still to arrive draws nothing yet, like any
        // other signal.
        let mut chain: Vec<&'a Span> = Vec::with_capacity(self.content.len());
        for span in &self.content {
            if !span.is_switch() {
                chain.push(span);
                continue;
            }
            match span.pick_cached(&read, cache, &mut seen) {
                Picked::Waiting => waiting = true,
                Picked::Case(Some(case)) => chain.extend(&case.pieces),
                Picked::Case(None) => {}
            }
        }
        // One group per piece rather than one flat run, because a gap cannot
        // be measured until everything that is not a gap has been laid out.
        let mut groups: Vec<Vec<Glyph>> = Vec::new();
        let mut gaps: Vec<usize> = Vec::new();
        let mut drew = false;
        for &span in &chain {
            if span.gap {
                // A boxed gap knows how wide it is before anything else is
                // laid out, so it is filled here and never asks for a share of
                // the leftover. An elastic one waits for the measuring below.
                if span.width > 0 {
                    drew |= span.rule;
                    groups.push(span.fill(span.width));
                } else {
                    gaps.push(groups.len());
                    groups.push(Vec::new());
                }
                continue;
            }
            let mut glyphs: Vec<Glyph> = Vec::new();
            // The colour the band this reading landed in asked for, if any. It
            // sits between the two that already exist: a `colours` signal is
            // the module speaking about one exact cell and stays the most
            // specific, this is the user speaking about this reading, and the
            // piece's own colour is the user speaking about the whole piece.
            let mut banded: Option<Colour> = None;
            // A band drawn inverse adds to the piece's own, the way a format
            // signal does: any of them asking is enough.
            let mut band_inverse = false;
            // The same for the stretch of the dial the needle is in: its
            // colour gives way to a band's, which names this exact reading,
            // and asking for small is enough whoever asks.
            let mut stretched: Option<Colour> = None;
            let mut small = span.small;
            let value = if let Some(stored) = &span.stored {
                match cache.get(stored, &read, &mut seen) {
                    Some(v) => {
                        let (text, band) = span.format_stored(&v);
                        banded = band.and_then(|b| b.colour);
                        band_inverse = band.is_some_and(|b| b.inverse);
                        small |= band.is_some_and(|b| b.small);
                        text
                    }
                    None => {
                        waiting = true;
                        groups.push(span.box_fit(glyphs));
                        continue;
                    }
                }
            } else if span.is_stored() {
                // A signal the page file does not have, which the checks
                // refuse. Nothing to draw, and nothing coming.
                groups.push(span.box_fit(glyphs));
                continue;
            } else if span.is_signal() {
                match read(&span.source) {
                    Some(Reading::Text(t)) => t,
                    Some(Reading::Number { value, max }) => {
                        let (text, band) = span.format_reading(value, max);
                        seen(&span.source, value, &text);
                        banded = band.and_then(|b| b.colour);
                        band_inverse = band.is_some_and(|b| b.inverse);
                        small |= band.is_some_and(|b| b.small);
                        if let Some(stretch) = span.conversion_for(value) {
                            stretched = stretch.colour;
                            small |= stretch.small;
                        }
                        text
                    }
                    None => {
                        // Room held rather than content: a boxed piece keeps
                        // its cells from the first frame, so what sits beside
                        // it does not jump when the reading finally arrives.
                        waiting = true;
                        groups.push(span.box_fit(glyphs));
                        continue;
                    }
                }
            } else {
                span.text.clone()
            };
            let value = span.replace_chars(&value);
            // Which of this part's own characters draw inverse, and in what
            // colour, where the module says so per character. A second signal
            // that has not arrived leaves the part drawing plainly rather than
            // holding it back.
            let inverse: Vec<bool> = span
                .format
                .as_ref()
                .and_then(|f| match read(f) {
                    Some(Reading::Text(t)) => Some(t),
                    _ => None,
                })
                .map(|t| t.chars().map(|c| c == 'i').collect())
                .unwrap_or_default();
            let colours: Vec<Option<Colour>> = span
                .colours
                .as_ref()
                .and_then(|cs| {
                    let text = match read(&cs.source) {
                        Some(Reading::Text(t)) => t,
                        _ => return None,
                    };
                    Some(
                        text.chars()
                            .map(|c| {
                                let mut buf = [0u8; 4];
                                cs.codes.get(c.encode_utf8(&mut buf) as &str).copied()
                            })
                            .collect(),
                    )
                })
                .unwrap_or_default();
            // A one cell run takes the part's whole value as a single glyph,
            // because a two character field really does occupy one cell here.
            if width == 1 {
                glyphs.push(Glyph {
                    text: span.alias(&value).to_string(),
                    colour: colours
                        .first()
                        .copied()
                        .flatten()
                        .or(banded)
                        .or(stretched)
                        .or(span.colour),
                    small,
                    inverse: span.inverse
                        || band_inverse
                        || inverse.first().copied().unwrap_or(false),
                });
                drew = true;
                groups.push(glyphs);
                continue;
            }
            for (i, ch) in value.chars().enumerate() {
                let one = ch.to_string();
                glyphs.push(Glyph {
                    text: span.alias(&one).to_string(),
                    colour: colours
                        .get(i)
                        .copied()
                        .flatten()
                        .or(banded)
                        .or(stretched)
                        .or(span.colour),
                    small,
                    inverse: span.inverse
                        || band_inverse
                        || inverse.get(i).copied().unwrap_or(false),
                });
            }
            drew |= !glyphs.is_empty();
            groups.push(span.box_fit(glyphs));
        }

        // What the gaps get: whatever the rest of the line did not use, split
        // evenly, the remainder going to the earlier ones. Nothing spare means
        // a gap of nothing, which is a line that has simply filled up rather
        // than a reason to push anything off the end.
        if !gaps.is_empty() {
            let fixed: usize = groups.iter().map(Vec::len).sum();
            let spare = width.saturating_sub(fixed);
            let each = spare / gaps.len();
            let extra = spare % gaps.len();
            for (n, &at) in gaps.iter().enumerate() {
                let take = each + usize::from(n < extra);
                // One group per piece, pushed in order, so a gap's group index
                // is its index in the chain: the piece that says whether this
                // is blanks or a rule, and what the rule is labelled.
                let span = chain[at];
                drew |= span.rule && take > 0;
                groups[at] = span.fill(take);
            }
        }

        let glyphs: Vec<Glyph> = groups.into_iter().flatten().collect();
        // Nothing drawn and still waiting is a field whose signals have not
        // arrived, which leaves its cells alone. Nothing to wait for is an
        // empty field, which draws blanks like any other.
        //
        // Drawn rather than simply present, because a gap and a box both fill
        // cells with blanks before anything has arrived, and writing those
        // over the glass would be the field announcing itself as an empty row
        // and then filling in. A rule counts: it reads nothing and is ready
        // from the moment the aircraft loads.
        if !drew && waiting {
            return None;
        }
        Some(self.fit(glyphs))
    }

    /// Lay composed glyphs across the run, one per cell.
    ///
    /// Longer content is cropped from the end the alignment anchors away from,
    /// which is what makes a right aligned scratchpad drop its leading pad
    /// rather than its last digit. Shorter content is padded with blanks, so a
    /// field that shrinks never leaves the old character behind.
    fn fit(&self, mut glyphs: Vec<Glyph>) -> Vec<Glyph> {
        let width = self.cells.len();
        if width == 1 {
            // Two parts on one cell is not something the hardware can draw, so
            // they are joined and the cell's glyph table decides whether the
            // result exists. It usually does not, and `problems` says so.
            if glyphs.len() > 1 {
                let text: String = glyphs.iter().map(|g| g.text.as_str()).collect();
                let first = glyphs.swap_remove(0);
                return vec![Glyph { text, ..first }];
            }
            return glyphs;
        }
        if glyphs.len() > width {
            let front = self.align.pad_before(width, glyphs.len());
            glyphs.drain(..front);
            glyphs.truncate(width);
            return glyphs;
        }
        let before = self.align.pad_before(glyphs.len(), width);
        let mut out = vec![Glyph::blank(); before];
        out.extend(glyphs);
        out.resize(width, Glyph::blank());
        out
    }
}

/// Where a readout's per-character colours come from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColourSource {
    /// Catalogue signal id, a string one character per cell.
    pub source: String,
    /// Each letter the module sends, and the colour it means.
    pub codes: BTreeMap<String, Colour>,
}
