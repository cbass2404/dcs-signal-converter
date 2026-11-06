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
  shipped profile lights them from the same source as its other backlights.
  They go dark when the converter stops, as the other panels do.
