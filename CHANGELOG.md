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
- **New profile: AV-8B Night Attack.** The takeoff panel shows each gear leg
  while its wheel is on the ground, the gear lever light, the flaps light,
  FULL in STOL and HALF in CRUISE, and LO to RO from the jettison station
  lights. JETT lights with the jettison selector at CMBT or FUEL, or at STA or
  STOR with a station selected. Master caution or master warning is master
  caution, and FAIL on the MCDU and each PFP. A/G on the throttle follows the
  A/G master mode light, and A/A lights when no master mode is selected.
  Backlights follow the console lights knob.
- **New profile: F-15E.** Each lamp follows either seat. The takeoff panel
  shows each gear leg, lit when down and blinking in transit, the flaps lit
  when down and blinking while moving, the gear handle or unsafe light, and
  HOOK when the hook is down. Master caution is master caution, and FAIL on
  the MCDU and each PFP. A/A and A/G on the throttle follow the HUD master
  mode lights. Backlights follow whichever console lights knob moved last.
- **New profile: Mirage 2000C**, also for the 2000D. The takeoff panel shows
  each gear leg while its wheel is on the ground, the gear lever light, and
  LO to RO from the PCA station buttons. JETT lights with the selective
  jettison cover open, master arm on and a station selected. Either PANNE
  light is master caution, and FAIL on the MCDU and each PFP. With master arm
  on, A/A lights for MAGIC or PCA weapon button 1 or 5, and A/G for weapon
  button 2 or 4; A/G also lights for either gun mode. Backlights follow the
  console panel lights knob.
- **New profile: MiG-19P.** The takeoff panel shows each gear leg, lit up and
  blinking down, the gear in transit light, the flaps down, at take-off or
  landing, and JETT with the jettison button cover open. ENGINE FIRE is master
  caution, and FAIL on the MCDU and each PFP. A/A and A/G on the throttle
  follow the ASP-5 sight's operating and aiming modes. Backlights follow the
  left UV lamp knob.
- **New profile: JF-17.** The takeoff panel shows each gear leg, the gear
  lever light, and JETT with the emergency jettison cover open. The red
  warning light is master caution, and FAIL on the MCDU and each PFP.
  Backlights follow the console light knob.
- **New profile: MiG-21bis.** The takeoff panel shows each gear leg, lit up
  and blinking down, the check gear light, the flaps light, FULL at landing
  and HALF at take-off, and JETT with any pylon or drop tank jettison cover
  open. Master caution is master caution, and FAIL on the MCDU and each PFP.
  Backlights follow the instrument lighting knob.
- **New profile: MiG-29 Fulcrum.** The takeoff panel shows each gear leg, the
  gear warning light, FULL with both flaps at landing and HALF with both at
  take-off, and JETT with the emergency jettison cover open. Master caution
  is master caution, and FAIL on the MCDU and each PFP. Backlights follow the
  console lights knob.
