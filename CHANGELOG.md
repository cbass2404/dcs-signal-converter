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
- **A switch no longer has a reading shared by its cases.** Each case
  builds what it draws from its own pieces, a reading, a shared result or
  text, so the reading under the switch is gone. A switch that had one
  opens with it copied into each case and draws the same. Its colour is
  still shared.
- **An always on lamp no longer has "Use a signal instead".** Clear it
  with its × and pick any of the choices, as on every other lamp.

### Fixes

- **Lamp rows see shared conditions as soon as they change.** A lamp lit
  by shared conditions now shows a renamed set by its new name, and a new
  set is offered in its menu, without reopening the profile.
- **An empty note closes again.** Opened with "Add a note" and left
  empty, the box goes back to the button once you click elsewhere,
  instead of staying open until the profile is reopened.
