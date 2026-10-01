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

## 1.0.0-beta.006

### Screens

- **A reading can now depend on a knob.** A new kind of piece, a switch,
  reads a selector and draws a different case for each position. The
  Huey's ADF needle sends the same numbers in every band, and with a switch
  on the band selector it shows 190 to 400, 400 to 850 or 850 to 1750 kHz
  as the band says. Each case has every setting a reading has: its own
  range, rows for an uneven dial, words and colours, or a short chain with a
  label or unit of its own.
- **Settings every case shares are set once,** on the switch: the signal,
  the decimals, the colour. A case sets only what differs, and the editor
  shows what it takes from the switch in grey. Emptying a setting takes the
  switch's back, and a shared wrap, padding or set of words can be turned
  off for one case.
- **Positions with no case draw nothing,** unless the switch has an else
  case. Two cases naming one position, a case the selector never reaches,
  and positions with no case are cautions on the field, not refusals.
- **An alias can now keep the number and only colour it.** Tick the
  reading on an alias row and the reading draws as usual, in that row's
  colour or inverse, while it is in that band: a radar altimeter that turns
  red below 50. An alias with an empty box and the reading unticked still
  draws nothing, as before.
- **An alias can draw in the small font** on the MCDU and the PFPs, the
  same small font a piece of text can use.
