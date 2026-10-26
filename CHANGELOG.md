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

## 1.0.0-beta.016

### Fixes

- **Reset on a profile now resets its pages too.** It used to put back only
  the lamps and slots, leaving any page you had edited as you left it.
  Reset now also puts the aircraft's shipped pages back as they shipped,
  including any you deleted. Pages you made yourself are kept. The pages
  are shared by every profile on the aircraft, and the Reset question names
  those profiles.

### Editor

- **Reset page.** A shipped page open in the page editor has a Reset page
  button that puts its name and every field back as shipped, without
  touching the rest of the profile. Save page writes it.
