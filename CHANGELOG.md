# Changelog

What changed in the release being prepared, for the people running it. Only
the current version is kept here: the release pipeline puts this section at
the top of that release's notes, and each earlier release already carries its
own. Anything written here reaches users; development detail belongs in
[docs/STATUS.md](docs/STATUS.md) instead.

**When a shipped profile changes**, say so here and name the rows. An update
never rewrites anything you have changed: a lamp row, a display field, or a
setting such as which panel follows which. Anything still exactly as the last
release shipped it is brought up to the new one for you. A fix to something
you have touched only reaches you if you reset it, and you cannot decide to
unless this says what moved.

## 1.0.0-alpha.011

### Added

- **Gauges with uneven marks read correctly.** Some dials have marks that
  bunch up, like the Mosquito's fuel gauges, where the first 20 gallons take
  up far more of the dial than the last 20. A converted reading can now be
  given as one row per section between two marks, saying what the dial reads
  there, instead of one straight line from end to end. If your rows don't
  reach the end of the dial yet, the editor offers a button to add the rest.
- **A colour and small text for each section of a dial.** Each row of a
  converted reading can draw in its own colour and in small text, so the last
  few gallons of a tank can show in red. An alias's colour still wins where
  one claims the reading.
- **Leading zeros on a number.** Set digits to fill a reading with zeros, so
  a 000 to 999 counter at 1 shows 001 rather than 1.

### Shipped profile changes

Each change below reaches you on its own if you have not touched that row,
slot or setting. If you have, it is left as you made it, and Reset is how to
take the new one.

**A-10C and A-10C2**

- **The amber rule on row 4 of the A-10C CDU and A-10C2 CDU pages** is now a
  rule piece across the row instead of a divider field. It draws the same
  line.

**Mosquito FB VI** (new)

- **Fuel on the MCDU and every PFP**, in gallons, read from the dials'
  uneven marks: the port and starboard inner tanks, the No 10 centre tank,
  and the port and starboard No 4 wing tanks. Each reads red when empty and
  amber in its lowest stretch. The inner tanks show OFF, and the others go
  blank, while the needles rest off the scale. The No 12 centre tank is not
  shown, since DCS does not report it.
- **PTO2**: LEFT, NOSE and RIGHT light with the port wheel, tail wheel and
  starboard wheel on the ground; FLAPS with any flap, HALF at half flap and
  FULL at full; HOOK once the bomb doors are fully open, from the cockpit's
  bomb door lamp. With the bomb panel cover open, LI and RI show the wing
  ordnance 1 and 2 switches and LO and RO the fuselage bombs 3 and 4
  switches.
- Every backlight follows the MFD C's, held on.

### Fixed

- **A profile opens at the top.** Picking a profile from further down the
  list opened its page scrolled down as far as the list had been.
- **The editor uses a wide window.** Pages grow with the window instead of
  stopping at a fixed width, and a converted reading's options take fewer
  lines when there is room. A row's colour and small setting always stay
  together.
- **A reading's signal stays put.** The signal box, and a text piece's box,
  now sit at the top left of the piece at one width, with the other
  controls below. Before, resizing the window moved them from beside the
  controls to above them and could squeeze Learn over the signal name. In a
  narrow window the page no longer runs off the right-hand side.
- **A rule from an older page shows and edits like any other.** A page
  saved before rules were pieces kept its rules as a different kind of
  field, whose preview drew wrongly. The editor now turns each one into a
  rule piece as the page opens, keeping its colour, size and label, and
  saves it that way. It draws the same line on the panel.
