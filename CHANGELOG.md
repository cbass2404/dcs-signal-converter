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

#### F-14 and F-14BU

Both profiles carry the same rows.

- **PTO2:** CTR, LI, LO, RO and RI follow the pilot's steering buttons:
  TACAN, DEST, VEC, MAN and AWL.
- **MCDU, PFP-3N, PFP-4 and PFP-7:** the key backlight is held at full
  instead of following the centre MFD, and FAIL lights on either seat's
  Master Caution.
- **New UFC Flight page** for the UFC, now its starting page: the UHF and
  V/UHF remote channel displays. The V/UHF field shows channels up to 20
  and is blank above that.

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

#### CH-47F

- **PTO2:** CAUTION follows the Master Caution lamp of the seat you are
  in, and JETT lights while that seat's cargo hook release cover is open.
  LI, RI, LO, RO and CTR show the hook selected while Hook Control is at
  ARM (LI forward, RI middle, LO aft, RO tandem, CTR all). HOOK lights
  while the hoist runs: the OUT/IN knob off centre with Hoist Control at
  REMOTE or PLT. The gear lamp lights while the swivel switch is off its
  first position.
- **MCDU, PFP-3N, PFP-4 and PFP-7:** FAIL follows the Master Caution
  lamp of the seat you are in.
- **New UFC Backup page** for the UFC, now its starting page: the ARC-186
  frequency, radar altitude and both engine FIRE handles.
- **New DED Backup page** for the ICP, now its starting page: the same
  readings, with each FIRE handle drawn inverse while it is lit.

#### AH-64D

- **PTO2:** LEFT, RIGHT and NOSE light while the left, right and tail
  wheels carry weight.
- **MCDU:** FAIL follows the Master Caution of the seat you are in, and
  STATUS that seat's Master Warning.
- **PFP-3N, PFP-4 and PFP-7:** FAIL follows the Master Caution of the
  seat you are in, and MSG that seat's Master Warning.
- **New UFC Backup page** for the UFC, now its starting page: barometric
  altitude, airspeed, and the flare and chaff counts.
- **New DED Backup page** for the ICP, now its starting page: the same
  readings, labelled.

#### A-10C and A-10C2

Both profiles carry the same rows.

- **MCDU, PFP-3N, PFP-4 and PFP-7:** FAIL follows the Master Caution
  light.
- **A-10C only:** the Co-Pilot and Observer names of the MCDU, PFP-3N,
  PFP-4 and PFP-7 had the A-10C2 CDU page in slot 1, with radios the
  A-10C does not have. They now show the A-10C CDU, as the Captain does.
  While a Co-Pilot or Observer follows its Captain, as shipped, it already
  showed the Captain's page, so this matters only to one set to drive on
  its own.

### Fixes

- The converter no longer stops with "Overlapped I/O operation is in
  progress (os error 997)". Windows sometimes reports a quiet moment on
  the export stream that way, and it is now treated as the pause it is.
- Deleting a page now warns when the page is in a slot. The slots using
  it are listed in red, with a note that a screen showing it moves to the
  next set slot and how to page back.
