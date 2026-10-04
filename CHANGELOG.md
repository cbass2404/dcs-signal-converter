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
- **New profile for the Mi-8MT and Mi-8MTV2:**
  - **Backlights:** every panel's backlight is lit whenever you are in the
    aircraft.
  - **PTO2:** NOSE, LEFT and RIGHT light while that wheel carries weight,
    HOOK while the emergency cargo release cover is open, and the master
    caution while the weapon safe/armed switch is on, standing in for the
    red cabin light that switch turns on.
  - **MCDU and PFP page, CDU Flight:** the radio selector; the R-863, R-828
    and Jadro-1A, each with its power, channel or frequency, tuning and
    volume; the ARC-9 ADF's main and backup frequencies with its mode,
    signal and volume; and a flight data line with airspeed, barometric and
    radar altitude and vertical speed. Airspeed under 50 shows `---` in red,
    and is amber from 50 to 70. Radar altitude is red under 20 m and shows
    `---` above 700 m. Vertical speed reads like the dial, 100 for each
    mark, so a climb of one mark shows +100. It is red descending past 300,
    amber descending at 300 or less, and green level or climbing.
  - **ICP page, DED Doppler Nav:** the DISS-15's coordinates in km with
    their drift, map angle, power, land or sea mode, drift angle, heading
    and course.
  - **No UFC page yet.**
- **New profile for the AJS37 Viggen:**
  - **Backlights:** every panel's backlight is lit whenever you are in the
    aircraft.
  - **PTO2:** NOSE, LEFT and RIGHT light with that gear's green lamp, the
    landing gear light with the yellow gear lamp, and the master caution
    with the right master caution light. FLAPS lights with the AFK lamp,
    HALF with attitude hold and FULL with altitude hold. JETT lights while
    the external tank release cover is open, and HOOK with the stores
    released lamp.
  - **MCDU and PFP:** FAIL lights with the right master caution light.
  - **Orion Throttle Base II:** A/A lights with the weapon selector at GUN
    or IR, A/G with the master mode selector at ANF.
  - **MCDU and PFP page, CDU Flight:** the master mode in its own colour,
    weapon selector, interval and release modes; the FR 22's group, manual
    frequency, base channel and buttons, each button green while pressed;
    range in km or mil, fuel, destination, airspeed and Mach; and the CK37
    data panel's selector, readout and IN or UT.
  - **ICP page, DED CK47:** the CK37 data panel's selector, readout and IN
    or UT, with range, fuel, destination, airspeed and Mach.
  - **UFC page, UFC Flight:** the CK37 readout, selector and IN or UT, with
    destination, range and airspeed.

### Log

- **The minute status line says when its slowest pass happened and what it
  was doing**, for example `longest pass 76 ms at 21:38:12.304 (frame +
  profile check)`. A slow moment in a log you send can now be matched to a
  stutter, a menu or an aircraft loading. See the log section of
  [docs/CLI.md](docs/CLI.md).

### Editor

- **The profile header shows only the aircraft under the name**, as the
  profile cards already do. It no longer repeats the DCS-BIOS module.
- **A shared result can add a part** to the parts before it instead of
  laying it beside them. Each part after the first chooses **laid beside
  the parts before** or **added to the parts before**, and the chain runs
  left to right, so the two mix: the Mi-8's ADF lays its hundreds and tens
  knobs side by side, 1 and 50 for 150, and adds the fine tuning to that.
  The Mi-8's course arrow turns with the compass card, so DCS-BIOS sends
  its angle on the card rather than the course: the heading plus that
  angle, each total wrapping at 360, is the course you set. Totals can be
  padded and wrapped, and an added symbol part adds a fixed number, such as
  180 for a reciprocal.
- **Picking a gauge's signal fills in what it reads, for every gauge**, not
  only uneven ones. An even dial arrives converted across its real range,
  such as 0 to 10,000 m on the Mi-8's altimeter, instead of 0 to 100 for you
  to set. The numbers are parsed from DCS's own files, so a line under them
  says so and asks you to check them against the dial; where nobody has read
  the unit off the dial, it says the unit is not checked. The line goes once
  you change the numbers.
- **41 more gauges get section rows**, ones that were wrongly taken for even
  before: among them the A-10C's airspeed, the F/A-18C's pressure altimeter,
  the Mi-8's radar altimeter and the MiG-21's fuel quantity.
- **Signals DCS-BIOS has retired are marked.** In the search list and in
  Learn they are tagged "replaced by" the signal to use, and sort after the
  rest. One in a row gets a line saying so, with a button that switches to
  the replacement. It still works today; a later DCS-BIOS may drop it.
- **A lamp's signal details show its colour** where DCS-BIOS gives one, as
  it does for most F-14, C-130J and AH-64D lamps.
- **The signal catalogue is rebuilt once on first start** to pick these up.
- **A piece drawing a shared result can draw it without its sign.** Offered
  when the result can go below zero. The result keeps its sign, so its
  aliases and conditions still tell left from right, and the screen shows
  only the size: a drift of -12.5 draws as 12.5.
