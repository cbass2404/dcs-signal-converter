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

## 1.0.0-beta.005

This release checks each shipped profile in the aircraft and fills in the
lamps and pages it was missing. Every profile it changes is listed here with
the rows that moved.

### Profiles

#### F-16

- **PTO2:** JETT lights when Master Arm is on and the jet is off the
  ground, and HOOK follows the hook light. CTR, LI, RI, LO and RO show ECM
  programs 1 to 5 (CTR 1, LI 2, RI 3, LO 4, RO 5), lit while that
  program's S, A or T lamp is on.
- **MCDU:** FAIL follows Master Caution, FM1 and FM2 light while COMM 1 or
  COMM 2 is on, and STATUS follows the ECM light.
- **PFP-3N, PFP-4 and PFP-7:** FAIL follows Master Caution, MSG lights
  while either COMM radio is on, and EXEC follows the ECM light.
- **The Flight page is now CDU Flight.** Fuel reads to 10 lb instead of
  100, and the trim values round down instead of to the nearest tenth.
- **New UFC Flight page** for the UFC: fuel, heading, EHSI course and the
  three trims.
