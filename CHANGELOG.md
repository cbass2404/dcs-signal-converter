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

## 1.0.0-beta.010

### Profiles

- **New profile: MiG-15bis.** The takeoff panel shows each gear leg, lit up
  and blinking down, the flaps at take-off or landing, the flaps and gear
  warning lights, and JETT with the emergency release cover open. The
  300 litre fuel warning is master caution, and FAIL on the MCDU and each
  PFP. Backlights follow the panel lighting knob. The MCDU and PFPs get a
  CDU Radio page with the RSI-6K and ARC-5 radios and flight readings, and
  the ICP a DED Gunsight page with the ASP-3N's power, mode, base and
  distance.
