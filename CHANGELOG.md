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

## 1.0.0-beta.002

### Shipped profile changes

Each change below reaches you on its own if you have not touched that row,
slot or setting. If you have, it is left as you made it, and Reset is how to
take the new one.

**AH-64D**

- **The KU page now shows the radios and flight instruments above the
  scratchpad**, on every MCDU. Rows 3 to 7 give the VHF, UHF, FM1, FM2 and
  HF frequencies. Under a SPEED and BARO heading, row 11 shows the pilot's
  airspeed in knots and barometric altitude in feet, and row 12 the
  altimeter setting in inches of mercury. Airspeed is red below 20 knots,
  amber from 20 to 30, and red again above 120.
