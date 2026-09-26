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

### Fixed

- **A profile opens at the top.** Picking a profile from further down the
  list opened its page scrolled down as far as the list had been.
