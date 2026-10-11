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

## 1.0.0-beta.013

- **Panels in the profile editor show they open.** Each panel's title now
  has a chevron that points right while it is closed and down while it is
  open.
- **More buttons explain themselves.** Hovering "drive this panel", "Assign
  a signal", Learn, "Edit page", "+ New page" or any of the buttons that
  add a piece to a field (a reading, a shared result, a switch, text, a gap
  or a rule) says what it does, as the other choices on a lamp already did.

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
