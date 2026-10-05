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

## 1.0.0-beta.009

### Profiles

- **New profile: A-4E-C** (the community mod). The takeoff panel shows
  gear, flaps, hook and each armed station, with the glareshield fire light
  as master caution and on FAIL. The throttle's A/A lamp lights with the gun
  armed. The MCDU and PFPs get a CDU Flight page with UHF and TACAN
  frequencies and the APN-153 navigation readings.
- **Backlights follow the cockpit again.** Every panel backlight dims with a
  console or instrument lighting knob instead of being held on, in every
  profile. The row that sets it is the INST_PNL_Backlight row on the
  CarrierAce MFD C; the others follow it. The knob is:
  - **A-10C and A-10C II:** left console.
  - **AH-64D:** primary lighting of the seat you are in.
  - **AJS37:** console lighting.
  - **C-130J-30:** pilot or copilot panel backlighting, whichever was
    turned last.
  - **CH-47F:** centre console dimmer.
  - **F-14A/B and F-14B Upgrade:** pilot or RIO console, whichever was
    turned last.
  - **F-16C:** primary consoles.
  - **F/A-18C:** consoles dimmer.
  - **Mi-24P:** the seat you are in.
  - **Mi-8MTV2:** left red lighting.
  - **Mosquito:** left bomb panel dimmer.
  - **OH-58D:** front overhead console.
  - **UH-1H:** the seat you are in.
  - **UH-60L:** pilot or copilot instrument lighting, whichever is brighter.
- **AH-64D:** the backup altimeter is one reading on the KU, UFC Backup and
  DED Backup pages, in feet to the nearest 10. Each drum now holds its digit
  until it has fully turned over, so the thousands no longer step up early.
  On the KU it is green.
  The shared result Pilot Alitimeter Needle is now spelled Pilot Altimeter
  Needle.
- **F-16C:** fuel on the CDU Flight page is one reading in pounds, red under
  2000 and amber to 3000, so the trailing 0 changes colour with the rest.
- **AJS37:** every lamp that follows the right master caution light (FAIL on
  the MCDU and each PFP, and the PTO2 master caution) reads it from one
  shared result, Master Caution Light right (red).
- **Mosquito:** ON on the DED Sight Settings page is as wide as OFF, so its
  highlight no longer jumps.
- **UH-1H:** the empty UFC Flight and DED Flight pages are gone. **A-10C and
  A-10C II** drop an empty UFC screen entry. Neither showed anything.

### Editor

- **The page modifier can match a modifier you set up in DCS**, on any of
  your controllers, instead of Ctrl, Shift or Alt. In Settings, pick
  **Controller button + page key...** and press the button within ten
  seconds. Hold it with a page key to swap, as you would Ctrl. It is the
  same button DCS sees, JOY_BTN12 being button 12, so with it set as a
  modifier in DCS's controls, it and a page key stay a combination of their
  own there too. **Press another button...** changes it.
- **If that controller is not connected**, the log and Settings say
  `<name> not found. Page swapping is disabled until it's back or another
  modifier is selected.` Plug it back in and pages swap again within a few
  seconds, without restarting anything.
