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

## 1.0.0-beta.017

### Panels

- **Total Controls Apache MPDs.** The left, right and center MPD frames'
  backlights follow the cockpit's lighting, like every other panel's
  backlight. The frame has four steps, off, dim, medium and full, so the
  light changes in steps, about half a second after the knob stops. Every
  shipped profile lights them from the same source as its other backlights,
  and points the center and right MPDs at the left, so a setup with two
  needs only the left one bound. They go dark when the converter stops, as
  the other panels do.

### Editor

- **"is not" test.** A condition can light its lamp on every value but one,
  such as a station light that is on but not green.

### Profiles

- **New profile: Mirage F1.** Covers the F1BE, F1CE and F1EE. LEFT, RIGHT
  and NOSE light while each wheel is on the ground. JETT lights with the
  jettison selector cover open and the armament master on, and CTR, LI and
  RI show the jettison selector position while its cover is open. Either
  master failure light is master caution, and FAIL on the MCDU and each
  PFP. Backlights follow whichever seat's instrument lighting knob moved
  last.
- **F-4E: the Takeoff panel and throttle follow the aircraft.** LEFT, RIGHT
  and NOSE follow the gear position indicators, and the gear handle light
  comes on with the light in the gear lever. FLAPS lights while the flaps
  are out, and HALF and FULL follow the flaps and slats switch at half and
  at full. CTR, LI, LO, RI and RO show the selected stations, CTR also
  with the gun selected. JETT lights with the jettison selector off its safe position,
  and HOOK while the hook is down. On the Orion throttle, A/A and A/G light
  with Master Arm on and the HUD in its air-to-air or air-to-ground mode.
  Backlights follow whichever seat's instrument panel lights knob moved
  last, in place of holding at a fixed level. If you have changed a
  TAKEOFF_PLANEL_2 row, the Orion_Throttle_Base_II A/A or A/G row, or the
  CarrierAce_MFD_C INST_PNL_Backlight row, reset it to pick this up.
- **L-39: the Takeoff panel follows the aircraft.** LEFT, RIGHT and NOSE
  light while each wheel is on the ground, and the gear handle light comes
  on with either seat's gear transit lamp. FULL and HALF follow either
  seat's flaps down and takeoff lamps. JETT lights with either seat's
  emergency jettison cover open. Backlights follow whichever seat's
  instrument lighting knob moved last, in place of holding at a fixed
  level. If you have changed a TAKEOFF_PLANEL_2 row or the CarrierAce_MFD_C
  INST_PNL_Backlight row, reset it to pick this up.
- **MB-339: the Takeoff panel follows the aircraft.** LEFT, RIGHT and NOSE
  follow the gear lights, and the gear handle light comes on with the light
  in the gear handle. FLAPS lights while the flaps are out, and FULL and
  HALF follow either seat's flap lever. LO, LI, RI and RO light while their
  station's light is on but not green, and CTR with station 3 or 4. HOOK
  lights with Master Arm on, and JETT stays on. Backlights follow
  whichever seat's instrument lighting knob moved last, in place of holding
  at a fixed level. If you have changed a TAKEOFF_PLANEL_2 row or the
  CarrierAce_MFD_C INST_PNL_Backlight row, reset it to pick this up.
- **F-14: the F-14A-95-GR flies the F-14 profile.** It shares every signal
  with the F-14A and F-14B. If you flew it before this release, it already
  has a starter profile of its own, and that one keeps flying it.
