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

## 1.0.0-beta.012

- **Page keys are only read while a press can swap the page.** A panel whose
  screen has fewer than two page slots in use is no longer read at all, nor
  is the page modifier while no panel needs it. A blank slot counts as in
  use; a slot set to off does not.

### Profiles

- **Mosquito: the panel backlights follow the L.H. Flood Light Dimmer.**
  The INST_PNL_Backlight row on the CarrierAce MFD C read the Bomb Panel
  Flood Dimmer instead. Every other panel backlight follows that
  row, so they all move with it. If you have changed that row, set its
  source to L_SIDE_L_DIM yourself.
- **UH-1H: the ADF gain bar on the CDU Radios page fills the right way.**
  It showed a full green bar with the GAIN knob turned all the way down and
  an empty red one at full gain. It now fills as the knob turns up. If you
  have changed that page field (the ADF line, cells 288-311), reset it to
  pick this up.
