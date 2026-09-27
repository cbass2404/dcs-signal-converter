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

## 1.0.0-beta.001

The first beta. It installs and updates as the alphas did, and an alpha
install is offered it like any other release.

### Shipped profile changes

Each change below reaches you on its own if you have not touched that row,
slot or setting. If you have, it is left as you made it, and Reset is how to
take the new one.

**Mi-24P**

- **The radar altimeter on the UFC reads what the dial reads.** It was
  taken as one straight line from 0 to 750 m, but the dial's marks bunch up
  low, so at 100 m it showed about 340. It now follows the dial's marks, and
  has an R in front of it. Above 750 m, where the needle leaves the scale,
  it shows --- until you come back down. The page is now named Flight
  rather than Radios.

**Mosquito FB VI**

- **The Flight page is now the War page**, on every MCDU and PFP. Its fuel
  rows move to the bottom, under a FUEL rule. Above them it now shows GUNS
  and ROCKETS, SAFE in green or ARMED in red; RELEASE, CINEMA in green or
  BOMB in red; the rocket SALVO switch, SINGLE or MULTI; and for each engine
  the RPM, the boost, and the radiator flap, OPEN or CLOSED in red. Boost is
  green from +6 to +10, and amber from 0 to +6 and from +10 to +14.
- **A new Cruise page on every MCDU and PFP**, which opens first, with the
  War page after it. It shows airspeed in MPH, climb or descent in feet a
  minute, the altimeter and its pressure setting in hPa, the magnetic
  compass heading and the course set on the repeater compass. Below that,
  each engine's RPM, boost, oil and radiator temperatures and radiator flap,
  and the fuel as on the War page.
- **A new Sight Settings page on the ICP**: the reflector sight's power, and
  the gunsight's range and base setting.
- **Known issue: GUNS always reads ARMED.** DCS-BIOS reports the Mosquito's
  gun master switch as armed in both positions, so the War page cannot tell
  them apart until DCS-BIOS is fixed.
