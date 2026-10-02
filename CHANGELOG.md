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

## 1.0.0-beta.007

### Profiles

- **New profile for the UH-60L mod**, read through the MH-60R module in
  DCS-BIOS:
  - **Backlights:** every panel's backlight is lit whenever you are in the
    aircraft.
  - **MCDU, PFP-3N, PFP-4 and PFP-7:** FAIL lights on the Master Caution.
  - **PTO2:** Master Caution lights on the Master Caution.
  - All three take it from one set of shared conditions, Master Caution
    Light.
  - **No screen pages.** The module sends no gauge, radio or engine
    readings worth showing, so the MCDU, UFC and ICP are left blank.

- **New profile for the UH-1H**, which also loads for the Bell 47:
  - **Backlights:** every panel's backlight is lit whenever you are in the
    aircraft.
  - **Master Caution:** FAIL on the MCDU, PFP-3N, PFP-4 and PFP-7, and
    Master Caution on the PTO2, all from one set of shared conditions,
    MASTER CAUTION.
  - **MCDU and PFPs:** RDY on the MCDU and DSPY on the PFPs light while
    the radar altimeter is powered.
  - **PTO2:** NOSE, LEFT and RIGHT light with weight on the skids, JETT
    while the jettison cover is open, and HOOK with the cargo release
    safety switch.
  - **Orion Throttle Base II:** A/A lights with master arm ARMED and the
    7.62 guns selected, A/G with master arm ARMED, 2.75 rockets selected
    and a rocket pair set.
  - **MCDU page, CDU Radios:** the transmit selector, which picks the
    radio you talk on, then UHF, VHF AM, VHF FM, VHF NAV and ADF, each
    with its frequency, power and volume. UHF adds its preset, and the ADF
    its band and a signal strength bar.
  - **No UFC or ICP pages yet.** Their slots hold empty pages.

- **F-16C: the fuel totalizer on the CDU Flight page is coloured by
  fuel left,** red at 2,000 lbs or less, amber up to 3,000 lbs and green
  above. It used to be amber throughout. Changed: the totalizer and the
  trailing 0 beside it on the CDU Flight page.

### Editor

- **The flash choice "steady" is now "as DCS shows it".** It never held a
  lamp steady: a lamp DCS flashes still flashes. Only the name changed.
- **Collapsing a panel closes its page editor.** With unsaved changes to
  the page, you are asked first: Continue discards them, Cancel keeps the
  panel and the editor open. Collapse all asks once for every page.
- **Profile cards name only the aircraft.** The DCS-BIOS module is
  gone from the line under each profile's name, where it repeated one of
  the aircraft.
- **A lamp on shared conditions shows just their name,** with "shared
  conditions" under it, instead of "Lights by" and the name.
- **A lamp that follows another shows just that lamp,** with "lit the
  same as that lamp" under it, instead of "Matches" and "follows it".
- **A switch no longer has settings shared by its cases.** Each case
  builds what it draws from its own pieces, a reading, a shared result or
  text, each with its own colour, so the reading and the colour under the
  switch are gone. A switch that had them opens with them copied into
  each case and draws the same.
- **A switch offers an else case only while a position has none.** Once
  every position has a case, else could never draw, so it is no longer
  offered.
- **A shared result can have symbol parts.** Beside its readings, a
  part can be digits, a point or a sign, laid down as typed: a radio's
  megahertz, a ".", then its hundredths at two digits read 30.50. The
  result stays a number, so bands still match it.
- **A shared result's part can give each switch position a number.**
  Choose "a number for each position" on a part reading a switch, and a
  switch sending 0 and 1 can read 1 and 5, each drawn exactly as typed.
- **An always on lamp no longer has "Use a signal instead".** Clear it
  with its × and pick any of the choices, as on every other lamp.

### Fixes

- **A failed save no longer lingers.** Once a profile, page or shared
  result saves, the error from its last failed attempt goes, and trying
  again replaces the error rather than adding another.

- **Lamp rows see shared conditions as soon as they change.** A lamp lit
  by shared conditions now shows a renamed set by its new name, and a new
  set is offered in its menu, without reopening the profile.
- **An empty note closes again.** Opened with "Add a note" and left
  empty, the box goes back to the button once you click elsewhere,
  instead of staying open until the profile is reopened.
