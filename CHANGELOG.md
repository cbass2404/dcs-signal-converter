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

## 1.0.0-beta.004

### Converter

- **Page keys cost less to read.** The UFC, ICP and MCDU each report their
  buttons 100 times a second, pressed or not. A report the same as the one
  before is now passed over instead of read again, which takes about a
  sixth off the converter's CPU while it waits for DCS.
- **Lamps are sent at most 30 times a second, as the screens already are.**
  A lamp change after a quiet spell still goes out at once. Changes that
  follow within the same thirtieth of a second go out together, from the
  latest values, and a lamp that goes on and off again inside that time is
  not rewritten at all.
