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

## 1.0.0-beta.014

- **A scale can dim as the knob turns up.** Some warbird rheostats go
  from off straight to full and dim from there as the knob keeps
  turning. A lamp scaled across a dimmer now takes where it is dimmest
  and where it is fully bright in either order, and an optional Dark
  below that keeps it off under a value. Scales you already have work
  and save exactly as before.
- **The session log shows the signal behind a copied lamp.** A lamp set
  to copy another one logged its brightness but not the cockpit signal
  driving it when the panel it copies was not plugged in. That signal is
  now logged as it moves.
