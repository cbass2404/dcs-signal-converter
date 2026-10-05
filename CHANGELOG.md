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

## 1.0.0-beta.009

### Editor

- **The page modifier can match a modifier you set up in DCS**, on any of
  your controllers, instead of Ctrl, Shift or Alt. In Settings, pick
  **Controller button + page key...** and press the button within ten
  seconds. Hold it with a page key to swap, as you would Ctrl. It is the
  same button DCS sees, JOY_BTN12 being button 12, so with it set as a
  modifier in DCS's controls, it and a page key stay a combination of their
  own there too. **Press another button...** changes it.
- **If that controller is not connected**, the log and Settings say
  `<name> not found. Page swapping is disabled until it's back or another
  modifier is selected.` Plug it back in and pages swap again within a few
  seconds, without restarting anything.
