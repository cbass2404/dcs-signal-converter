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

## 1.0.0-beta.015

### Profiles

- **MiG-21bis: a CDU Flight page.** The MCDU, CDU and PFP screens show
  airspeed and altitude, heading, course, the RSBN/ADF needle, RSBN
  range, the barometric altimeter setting, the RSBN and PRMG channels
  with volume, and the ARC-10 channel, mode, squelch and volume.
- **MiG-21bis: the Orion throttle A/A and A/G lamps follow the weapons
  AA/AG mode switch.** They were unbound. If you have changed the
  Orion_Throttle_Base_II A/A or A/G row, reset it to pick this up.
