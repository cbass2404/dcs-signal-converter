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
- **A switch can be decided by a gauge as well as a knob.** Read the
  signal that decides as converted, and cases are written in the gauge's
  units, so typed text can change colour by a reading's range: FUEL in
  amber below 3000 lb and in red below 1000.
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
- **Gauge tables are built into the editor.** A converted reading has a
  second menu with every gauge table the site lists for the aircraft, and
  picking one copies its rows into the field. Choosing a gauge that has a
  table starts it converted by that table. The rows stay hidden while
  they are the table's, and choosing custom opens them to change.
- **Shared results: a value made once, drawn on any page.** A new
  section at the top of the profile, Shared Results, holds the aircraft's
  shared results. Each reads one or more signals, shaped the way a
  reading is, laid side by side: three fuel drums, each rounded down on
  its own, read as one number without a rolling drum counting twice. Pick
  "a shared result" as a piece of a field to draw one, and give it words
  and colours there. Each has a note, is saved on its own, is shared
  by every profile on the aircraft, and travels with an exported page.
- **A reading can round up** as well as down or to the nearest.
- **Shared conditions.** In their own section under Shared Results, write
  a lamp's logic once, seat choice and all, and point any number of lamps
  at it with "Use shared conditions", on any panel: the CH-47's master
  caution on every CDU and the PTO2 from one set of conditions. Each lamp
  keeps its own brightness.
- **A switch can be decided by a shared result**, in its units: a FUEL
  label red at or below 999 and green from 1000.
- **Switch cases read like lamp tests**: is exactly, is one of, is
  between, is at least, is at most, or is anything else. At least and at
  most work in alias bands too.
- **Lamps can flash.** Each group of conditions ends with a choice of
  as DCS shows it, flashing slowly (twice a second) or flashing fast
  (three times a second), on a lamp's own conditions, on each alternative,
  and on shared conditions, where it applies to every lamp lit by them.
  Every lamp at one rate flashes in step, lit and dark for equal halves.
  An alternative shown as DCS shows it that also holds keeps the lamp lit.
  Flash only a lamp DCS keeps steady: where the cockpit lamp flashes, the
  panel already follows it. Nothing changes until you set one.
- **Readings shaped the same way twice are offered as one shared result.**
  When one signal is shaped the same way in two or more readings on the
  aircraft's pages, a banner at the top of Shared Results offers to make
  it one shared result and point every one of them at it. Each reading
  keeps its own words, colour and box. It asks first, listing every
  reading and every slot it changes, then saves the result and the pages
  together, or none of them. The banner goes once they are made one, or
  when you close it with its ×.
- **Lamps lit the same way are offered as shared conditions.** When two
  or more lamps in a profile light by exactly the same conditions, a
  banner at the top of Shared Conditions offers to write them once. It
  lists the lamps first, then saves the conditions and switches the lamps
  to them, each keeping its own brightness; save the profile to keep the
  change. Closed with its ×, or gone once they are made one.
- **A shared result is worked out only when one of its parts moves.**

### Profiles

- **The shipped profiles now use shared results and shared conditions.**
  Lamps that lit by the same conditions point at one set of shared
  conditions, and a reading shaped the same way on more than one page
  draws one shared result. Every lamp and reading shows what it did
  before; the logic is now written once, at the top of the profile. A row
  you have changed is left as you have it. The rows that moved:
  - **A-10C and A-10C II:** the master caution on the MCDU FAIL, the PFPs'
    FAIL key and the PTO2 Master Caution.
  - **AH-64D:** the master warning on the MCDU STATUS and the PFPs' MSG
    key, and the master caution on the PFPs' FAIL key, both by seat. On
    the UFC Backup and DED Backup pages, the altimeter drums and the
    standby airspeed.
  - **CH-47F:** the master caution, by seat, on the MCDU FAIL and the PTO2
    Master Caution, and on the PFPs' FAIL key.
  - **F-14 and F-14BU:** the master caution on the MCDU
    FAIL, the PFPs' FAIL key and the PTO2 Master Caution.
  - **F-16C:** the master caution on the MCDU FAIL, the PFPs' FAIL key and
    the PTO2 Master Caution; the ECM light on the MCDU STATUS and the
    PFPs' EXEC key; the radio mode knobs on the PFPs' MSG key. On the CDU
    Flight and UFC Flight pages, the fuel totalizer and the three trims.
  - **F/A-18C:** the master caution on the MCDU FAIL, the PFPs' FAIL key
    and the PTO2 Master Caution.
  - **Mi-24P:** on the UFC Flight and DED Flight pages, the radar altitude
    and both airspeeds.
  - **Mosquito FB Mk VI:** the transmitter Type F light on the MCDU FM and
    the PFPs' MSG key. The tachometers on the War and Cruise pages, and
    the sight range and span on both Sight Settings pages.

### Editor

- **The check's banners close with an ×.** The problems, the cautions and
  the DCS-BIOS nightly note at the top of a profile each have one. A closed
  banner comes back if the check later finds something different.
- **FC3 is last in the profile list,** after the aircraft with their own
  module, and "No aircraft" is no longer listed: it has nothing to edit,
  and still lights the panels when you are in no aircraft.
- **Collapsing a panel closes its page editor.** With unsaved changes to
  the page, you are asked first: Continue discards them, Cancel keeps the
  panel and the editor open. Collapse all asks once for every page.
