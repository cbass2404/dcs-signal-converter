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

- **The MCDU screen no longer falls behind.** A page that changes
  constantly, such as the Mosquito's, could lag and then jump to catch up,
  because the converter stopped everything for 40 ms after each MCDU
  screen. It now keeps the MCDU's gap without waiting for it: a screen
  that comes too soon is held and sent, as the latest one, when the MCDU
  is ready, and the other panels and the stream carry on meanwhile.
- **Each panel is written from its own thread.** Every write to a panel
  takes about a millisecond, and the converter used to wait for each one
  before doing anything else, so a busy screen held up the other panels
  and the reading of DCS's data. Now no panel waits on another, a panel
  that falls behind skips straight to the latest lamps and screens, and
  loading an aircraft no longer pauses everything for the MCDU's font.
- **Lamps and screens are sent at most 25 times a second**, where before
  screens went at most 30 and lamps on every change. A change after a quiet
  spell still goes out at once. Changes that follow within the same
  twenty-fifth of a second go out together, from the latest values, and a
  lamp that goes on and off again inside that time is not rewritten at all.
- **Page keys cost less to read.** The UFC, ICP and MCDU each report their
  buttons 100 times a second, pressed or not. A report the same as the one
  before is now passed over instead of read again, which takes about a
  sixth off the converter's CPU while it waits for DCS.
- **The log's status line says what each panel was sent**: reports, bytes,
  the time spent writing to it, and how many screens were replaced by a
  newer one before they went.
