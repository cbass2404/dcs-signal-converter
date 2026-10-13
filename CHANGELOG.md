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
- **Gauge values show the range they really read.** The AJS37, Bf-109K-4,
  F-86F and P-51D send some gauges as the reading itself, such as coolant
  temperature in degrees. Their signal details said 0 to 65535, the same as
  a needle position. They now show the real range, such as 0 to 130 for
  the Bf-109's `COOLANT_TEMPERATURE_VALUE`, and a condition on one is
  checked against it.

### Profiles

- **Mosquito: the MFD backlight follows the left cockpit lamp dimmer the
  right way round.** The centre CarrierAce MFD, and the panels that follow
  it, got brighter as the dimmer knob turned up, but the cockpit lamp
  comes on at full and dims from there. The backlight now stays dark until
  the lamp comes on and dims with it. If you have changed the
  CarrierAce_MFD_C INST_PNL_Backlight row, reset it to pick this up.
- **Mosquito: the MCDU Cruise page shows the climb rate and heading
  better.** The climb rate now reads to the nearest 10 ft/min, lined up on
  the right with a fixed last zero, so it no longer flickers through the
  ones digit. The heading now comes from the repeater compass needle
  instead of the compass card. If you have changed that page field (the
  airspeed line, cells 48-71), reset it to pick this up.
