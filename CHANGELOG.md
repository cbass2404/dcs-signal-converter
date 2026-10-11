# Changelog

What changed in the release being prepared, for the people running it. Only
the current version is kept here: the release pipeline puts this section at
the top of that release's notes, and each earlier release already carries its
own. Anything written here reaches users; development detail belongs in
[docs/STATUS.md](docs/STATUS.md) instead.

**When a shipped profile changes**, say so here and name the rows. An update
never rewrites anything you have changed: a lamp row, a display field, or a
setting such as which panel follows which. Anything still exactly as it was
shipped to you is brought up to the new one for you. A fix to something
you have touched only reaches you if you reset it, and you cannot decide to
unless this says what moved.

## 1.0.0-beta.013

- **Panels in the profile editor show they open.** Each panel's title now
  has a chevron that points right while it is closed and down while it is
  open.
- **More buttons explain themselves.** Hovering "drive this panel", "Assign
  a signal", Learn, "Edit page", "+ New page" or any of the buttons that
  add a piece to a field (a reading, a shared result, a switch, text, a gap
  or a rule) says what it does, as the other choices on a lamp already did.
- **Updates reach you even if you skipped a release.** An update brings
  forward any row you never changed, but it only recognised rows as the
  release just before it shipped them. If you skipped a release, every
  default changed in the one you missed looked like your own edit and was
  kept, so those fixes never arrived. Each profile and page file now
  remembers the default it was last given, and updates compare against
  that. Rows you changed are still never touched. The first update to
  this release or any up to beta.017 compares against beta.012 once, so
  if you were already behind beta.012, a fix from a release you skipped
  before it reaches you only through a reset.

### Profiles

- **Mosquito: the altimeter setting on the MCDU Cruise page shows a
  decimal.** It read whole hPa only, so 1013.2 showed as 1013. It now shows
  one decimal place. If you have changed that page field (the hPa line,
  cells 72-95), reset it to pick this up.
- **Mosquito: the altitude on the MCDU Cruise page no longer jumps ahead.**
  Its ten-thousands and thousands digits rounded to the nearest, so they
  turned over before the hundreds did: 950 ft read as 1950. Every altitude
  digit now rounds down, and so does the climb rate beside them. If you
  have changed that page field (the airspeed line, cells 48-71), reset it
  to pick this up.
- **P-51D: the MFD backlight follows the cockpit lighting.** The centre
  CarrierAce MFD, and the two that follow it, stayed fully lit. They now
  dim and brighten with the left fluorescent light. If you have changed
  the CarrierAce_MFD_C INST_PNL_Backlight row, reset it to pick this up.
- **P-51D: the Orion throttle A/A and A/G lamps show weapon state.** A/A
  lights while the gun switch is on guns. A/G blinks slowly while bombs are
  selected and a bomb arm switch is armed, and lights steady while rockets
  are selected with a release mode set. Both rows were empty before.
- **P-51D: the TakeOff panel landing gear lamp shows the gear lights.** It
  lights steady with the green gear light and blinks slowly with the red.
  The row was empty before.
