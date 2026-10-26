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

## 1.0.0-beta.016

### Fixes

- **Reset on a profile now resets its pages too.** It used to put back only
  the lamps and slots, leaving any page you had edited as you left it.
  Reset now also puts the aircraft's shipped pages back as they shipped,
  including any you deleted. Pages you made yourself are kept. The pages
  are shared by every profile on the aircraft, and the Reset question names
  those profiles.

### Editor

- **Reset page.** A shipped page open in the page editor has a Reset page
  button that puts its name and every field back as shipped, without
  touching the rest of the profile. Save page writes it.
- **A lamp lit by shared conditions can add its own.** Under the shared
  conditions, "+ Add condition (and)" adds tests that must hold as well,
  and "+ Add alternative (or)" adds other ways for the lamp to light. Use
  one or the other: once you add to one, only that one extends. A throttle
  A/G lamp can light by a shared BOMB READY, or by master arm with rockets
  armed.
- **"+ Add condition" reads "+ Add condition (and)"** under every group of
  conditions, to match "+ Add alternative (or)".

### Profiles

- **I-16: the Takeoff panel, throttle and master caution follow the
  aircraft.** LEFT and RIGHT follow the main wheels, lit up and blinking
  down, and the gear handle light comes on while either wheel is between
  up and down. FLAPS lights while the flaps are out, HALF at takeoff and
  FULL at landing. JETT lights by new BOMB READY shared conditions: a
  count set on the rockets/bombs counter with the rockets/bombs master
  arm on. On the Orion throttle, A/G lights by BOMB READY too. Master
  caution, with FAIL on the MCDU and each PFP, now lights with the fuel
  gauge at a fifth or below, in place of the panel glow lights.
  Backlights follow the cockpit light rheostat. If you have changed a
  TAKEOFF_PLANEL_2 row, the Orion_Throttle_Base_II A/G row, the
  CarrierAce_MFD_C INST_PNL_Backlight row, or the Master Caution shared
  conditions, reset it to pick this up.
- **FW-190D9: the Takeoff panel, throttle and master caution follow the
  aircraft.** LEFT and RIGHT follow the main wheels, lit up and blinking
  down, with NOSE for both together, and the gear handle light comes on
  while either wheel is between up and down. FLAPS lights while the flaps
  are out, HALF at takeoff and FULL at landing. LI, LO, RI and RO show the
  bomb fusing selector, and CTR its off position. JETT lights by new BOMB
  READY shared conditions: a bomb lamp with the fusing selector off CTR.
  On the Orion throttle, A/A lights with any gun lamp, and A/G by BOMB
  READY or the main rocket switch on. Master caution, with FAIL on the
  MCDU and each PFP, now lights with the fuel gauge at a fifth or below,
  in place of the forward low fuel lamp. Backlights follow the instrument
  lights. If you have changed a TAKEOFF_PLANEL_2 row, the
  Orion_Throttle_Base_II A/A or A/G row, the CarrierAce_MFD_C
  INST_PNL_Backlight row, or the Master Caution shared conditions, reset
  it to pick this up.
- **FW-190A8: the Takeoff panel, throttle and master caution follow the
  aircraft.** LEFT and RIGHT follow the main wheels, lit up and blinking
  down, with NOSE for both together, and the gear handle light comes on
  while either wheel is between up and down. FLAPS lights while the flaps
  are out, HALF at takeoff and FULL at landing. LI, LO, RI and RO show the
  bomb fusing selector with the ZSK 244 ready, and CTR its off position,
  where the ZSK 244 stays dark. JETT lights by new BOMB READY shared conditions: a bomb lamp,
  the ZSK 244 ready and Master Arm on. On the Orion throttle, A/A lights
  with any gun lamp, and A/G by BOMB READY or the main rocket switch on.
  Master caution, with FAIL on the MCDU and each PFP, now lights with the
  fuel gauge at a fifth or below, in place of the forward low fuel lamp.
  Backlights follow the instrument lights. If you have changed a
  TAKEOFF_PLANEL_2 row, the Orion_Throttle_Base_II A/A or A/G row, the
  CarrierAce_MFD_C INST_PNL_Backlight row, or the Master Caution shared
  conditions, reset it to pick this up.
- **Christen Eagle II: backlights only.** The aircraft has no warning
  lights to show, so the placeholder Master Caution, which followed the
  left position light, is gone, and Master Caution and FAIL stay dark.
  Backlights hold at a fixed level. If you have changed the
  TAKEOFF_PLANEL_2 Master_Caution row, or a FAIL row on the MCDU or a PFP,
  reset it to pick this up.
- **New profile: Yak-52.** The takeoff panel shows each gear leg, lit up
  and blinking down, with NOSE for the nose wheel, and the gear handle
  light comes on while any wheel is between up and down. FLAPS lights
  while the flaps are down. The stall warning is master caution, and FAIL on the
  MCDU and each PFP. Backlights hold at a fixed level.
- **New profile: La-7.** The takeoff panel shows each gear leg, lit up and
  blinking down, with NOSE for the tail wheel, and the gear handle light
  blinks while any wheel is moving with the gear indicator switched on.
  FLAPS lights while the flaps are out, HALF near half travel and FULL at
  full. Low fuel is master caution, and FAIL on the MCDU and each PFP.
  Backlights follow the cockpit light rheostat.
- **F4U-1D: the Takeoff panel shows the gear, flaps, hook and bombs.**
  LEFT, RIGHT and NOSE follow the left, right and tail wheels, lit steady
  at one end of their travel and blinking slowly at the other, and the gear
  handle light blinks while any wheel is moving. FLAPS lights while the
  flaps are out, HALF near half travel and FULL at full. HOOK lights while
  the tail hook is down. CTR, LI and RI follow the pylon, left wing and
  right wing release switches, and LO and RO the arming switch at NOSE and
  TAIL. JETT lights by new BOMB READY shared conditions: a release
  selected, the arming switch off SAFE and Master Arm on.
- **F4U-1D: the Orion throttle A/A and A/G lamps show the weapons are
  ready.** A/A lights with Master Arm on, both gun charge switches on and
  all three gun stations selected. A/G lights by BOMB READY, or with the
  rocket ready light on, rockets armed and Master Arm on.
- **F4U-1D: the CDU screen backlight follows the MFD backlight.**

These F4U-1D rows were unbound. If you have changed any TAKEOFF_PLANEL_2
row, the Orion_Throttle_Base_II A/A or A/G row, or the CDU_Kneeboard
Screen_Backlight row, reset it to pick this up.
