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

## 1.0.0-beta.003

### Shipped profile changes

Each change below reaches you on its own if you have not touched that row,
slot or setting. If you have, it is left as you made it, and Reset is how to
take the new one.

**F/A-18**, checked lamp by lamp and page by page in the jet.

- **The ViperAce ICP is on**, where before the profile turned it off. Its
  starting page is a new DED-UFC page: the UFC scratchpad on the top line,
  the five option windows down the right with their cueing, and COMM 1 and
  COMM 2 on the bottom line, all drawn inverse.
- **FAIL follows the master caution** on the MCDU, PFP-3N, PFP-4 and PFP-7.
- **The MCDU's RDY lamp follows APU READY.**
- **The PTO2's JETT lamp has a note** saying why it lights: only when the
  selective jettison knob and stations are set so that pressing it would
  jettison something. How it lights has not changed.

**Mosquito FB Mk VI**, checked lamp by lamp and page by page in the aircraft.

- **The ICP's reflector sight page is redrawn**, now called DED Sight
  Settings: a REFLECTOR SIGHT title, POWER, RANGE and BASE spelled out, and
  the values under them drawn inverse. BASE now reads the span scale as the
  sight marks it, 100 down to 30, where before it ran straight from 32 to
  100.
- **The UFC has a starting page**, a new UFC Sight Settings page: the
  reflector sight switch, range and base on one line.
- **FM on the MCDU and MSG on the PFP-3N, PFP-4 and PFP-7 follow the
  transmitter Type F light.**
- **The throttle's A/A lamp follows the gun master switch**, and A/G lights
  with the rockets master switch or the cine camera changeover switch.
- **The key backlights on the PFP-3N, PFP-4, PFP-7 and ICP, and the UFC's
  LCD backlight, are held on.** The UFC's panel backlight follows the MFD C
  panel backlight.
- **The MFD L and MFD R follow the MFD C**, and the Co-Pilot and Observer
  MCDU, PFP-3N, PFP-4 and PFP-7 follow their Captain.
- **The PTO2's JETT lamp has a note** saying why it lights: when a selected
  bomb could be released. The profile cannot tell which bombs are on which
  pylon, so with several selected it can light when nothing would drop.

### Converter

- **Screens are redrawn only when something on them changes**, and at most
  30 times a second. A burst of updates is drawn once, from the latest
  values, so a screen never runs behind the cockpit.
- **A panel that is not plugged in is treated like one turned off.** Its
  lamps and fields are no longer worked out on every change.
- **The log follows only what reaches a panel.** A device turned off or not
  plugged in, and the page a page key has just replaced, no longer have
  their signals logged. A reading on a screen is logged as it was drawn,
  rather than converted a second time for the log.

### Editor

- **A reading can be drawn inverse**, on glass that draws inverse such as
  the DED. Before, only typed text had the box. While a piece is inverse its
  highlighting signal is not offered, since there is nothing left to mark.
- **Each screen can have its own page open at once**, so the DED page can
  stay open while you build a UFC page beside it. Before, a page open on
  one screen locked Edit page on every other. A screen with a page open
  still has to close it before opening another, and one page cannot be
  open on two screens at once.
