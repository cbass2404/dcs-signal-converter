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

## 1.0.0-beta.011

- **Kneeboard CDU: the CDU screen in OpenKneeboard or a browser**, for
  anyone without a CDU of their own. Turn on **Kneeboard CDU** in a profile,
  fill its six slots with CDU pages and pick the start page, as on any MCDU.
  Its section shows a file, with a **Copy** button, to add to OpenKneeboard as
  a **Single file** tab (OpenKneeboard 1.10 or later) or open in a browser on
  this PC. It says it is waiting until a mission starts, then shows the
  screen, drawing each page in the aircraft's own CDU font and the page's
  colours on a dark screen. The converter answers it on this PC only, and only
  once a profile turns the Kneeboard CDU on.
- **The Kneeboard CDU's screen backlight** can follow a cockpit lighting knob,
  like the MCDU's. Left unassigned, it stays at full brightness.
- **Swap the Kneeboard CDU's pages with the page modifier and 1 to 6** on the
  keyboard's top row, without leaving DCS. DCS still sees the keystroke, so
  pick a modifier whose number combinations DCS does not use. With a
  controller button as the modifier, DCS sees the bare number.
- **Page keys are only read while there is a page to swap to.** A panel the
  profile turns off, one whose screen has fewer than two slots in use, and
  every panel while no aircraft is loaded, is not read at all.
- **A panel unplugged in flight no longer stops the converter.** The other
  panels carry on, and the unplugged one comes back on its own when it is
  plugged in again, lamps, screen and page keys, showing the cockpit as it
  is. A panel plugged in after the converter started is picked up the same
  way. Before, pulling a panel stopped every panel until the next mission.
- **A controller button page modifier is found the moment it is plugged
  back in**, instead of within a few seconds, and an unplugged one is in the
  log as it goes.
- **Merging page slots from a file says what becomes of each page.** Each
  slot says whether its page is already here or comes in new, and a page
  shown in several slots is said to come in once. It always did; the list
  only made it look like a copy per slot.

### Profiles

- **Every profile has the Kneeboard CDU, turned off.** Turn it on in the
  profile to use it. Its screen backlight row follows the INST_PNL_Backlight
  row on the CarrierAce MFD C, as every other panel backlight does, and is
  full bright while the lighting knob reads zero.
- **Its slots start with the CDU pages the profile already shows on the MCDU
  and PFPs**, on the same start page: the A-10C, A-10C II, A-4E-C, AH-64D,
  AJS37, CH-47F, F-14B Upgrade, F-16, F/A-18, JF-17, Mi-24P, Mi-8, MiG-15bis,
  MiG-21bis, MiG-29, Mosquito and UH-1H. The other profiles leave its slots
  empty, as they leave the MCDU's.
- **Placeholder profiles for the aircraft DCS-BIOS supports that had none
  yet:** A-29B, AH-6J, Alphajet, Bf-109K-4, C-101, Christen Eagle II,
  Edge540, F-22A, F-4E-45MC, F-5E-3, F-86F Sabre, F4U-1D, FW-190A8,
  FW-190D9, I-16, Ka-50, L-39, MB-339, P-47D, P-51D, SA342 and Spitfire LF
  Mk IX. They are not finished. Each has a Master Caution shared condition
  that lights the PTO 2 Master Caution and the MCDU and PFP FAIL lamps; in
  some it reads a stand-in lamp until the profile is done. The MFD C
  backlight is held on, every other panel backlight follows it, and the
  Kneeboard CDU's screen is held at full. Every other lamp is left for you.
  They ship now so the finished versions arrive later as ordinary updates:
  anything you change in one stays yours. If you already have a profile of
  your own for one of these aircraft, it is kept and the placeholder does
  not come in for that aircraft.
