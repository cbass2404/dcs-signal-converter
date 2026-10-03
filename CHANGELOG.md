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

## 1.0.0-beta.008

### Profiles

- **New profile for the OH-58D Kiowa Warrior:**
  - **Backlights:** every panel's backlight is lit whenever you are in the
    aircraft.
  - **MCDU:** RDY lights steady with the laser armed and blinks slowly with
    it on but not armed. IND lights while the copilot fires the armed laser,
    and STATUS with the MMS mode selector past its second position.
  - **PTO2:** JETT lights while either pylon jettison guard is open, and
    HOOK with the CMWS armed and on, or with the IR jammer on and
    transmitting.
  - **Orion Throttle Base II:** A/A lights with the master switch at ARM
    and the ARMED lamp lit, A/G with the gun switch at its middle position
    and the ARMED lamp lit.
  - **ICP page, DED Radio:** the Remote Frequency Indicator's five radios,
    each with its channel, a C while it ciphers, its frequency, and an
    arrow marking the radio each seat has selected.
  - **No MCDU or UFC pages yet.** Their slots hold empty pages.

### Log

- **The minute status line says when its slowest pass happened and what it
  was doing**, for example `longest pass 76 ms at 21:38:12.304 (frame +
  profile check)`. A slow moment in a log you send can now be matched to a
  stutter, a menu or an aircraft loading. See the log section of
  [docs/CLI.md](docs/CLI.md).

### Editor

- **The profile header shows only the aircraft under the name**, as the
  profile cards already do. It no longer repeats the DCS-BIOS module.
- **A piece drawing a shared result can draw it without its sign.** Offered
  when the result can go below zero. The result keeps its sign, so its
  aliases and conditions still tell left from right, and the screen shows
  only the size: a drift of -12.5 draws as 12.5.
