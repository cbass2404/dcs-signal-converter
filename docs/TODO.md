# TODO

What is still outstanding, in one place, so nothing has to be reconstructed by
reading [STATUS.md](STATUS.md) end to end. This file is the checklist only: why
a thing is shaped the way it is, and what flying it taught, belong in
`STATUS.md`, and anything a user would notice belongs in
[CHANGELOG.md](../CHANGELOG.md) under the version in
[VERSION.md](../VERSION.md), as it lands.

Links into `STATUS.md` name the section or phrase to search for, since line
numbers move and the words do not.

## Next

- [x] ~~**Decide on skipping unchanged key reports.**~~ Kept 2026-09-30:
      the readers went from 0.42 to 0.50% of a core to 0.28 to 0.37%, the
      daemon at idle from 0.54 to 0.41 to 0.49%. Small, but it costs nothing
      and adds up on a machine DCS already loads. What is left is the 100
      wakes a second per panel; batching them was left undone. Since
      beta.011 a reader runs only while its panel is driven.
      [PERFORMANCE.md](PERFORMANCE.md), "The page key readers run only in
      flight"

- [x] ~~**Pick up the idle CPU in PERFORMANCE.md.**~~ Done 2026-09-30: the
      spikes were Windows charging whole 15.6 ms ticks to the page key
      readers, not load. The UFC, ICP and MCDU each send 100 identical
      reports a second; `bench_daemon.py` now counts exact cycles, splits
      CPU by thread and pins cores with `--affinity`; PERFORMANCE.md is
      re-measured with an i5-12400F estimate.
      [STATUS.md](STATUS.md), "the performance pass"

- [x] ~~**Name the pages, then move the shipped defaults onto them.**~~
      Done 2026-09-23: every shipped profile is version 2, and the six
      aircraft with MCDU content each have one page in slot 1. Checked with a
      dry run and the tests, not yet seen on the panel.
      [STATUS.md](STATUS.md), "Built 2026-09-23: MCDU pages"

- [x] ~~**Build page swapping.**~~ Built 2026-09-23 on
      `feature/mcdu-page-selection-inputs`, with the Settings dialog behind
      the gear. [STATUS.md](STATUS.md), "Built 2026-09-23: page swapping"

- [x] ~~**Swap pages on the panel.**~~ Flown 2026-09-23 in the A-10C: odd
      slots a page, even slots blank, the last disabled. Every combination of
      Ctrl, Shift and Alt, left and right, with the modifier changed in
      Settings mid-flight and run through again. Pages swapped, blanks went
      dark and the disabled key did nothing, each only when intended.
      [CONFIG.md](CONFIG.md), "Swapping"

- [x] ~~**Settings and import with pages.**~~ Proven 2026-09-23: the
      modifier, the three themes (the first look at the light theme) and the
      theme surviving a restart, Import profile... and Manage Converter...
      from the gear, a version 1 profile refused on import, a whole import
      offering its pages, and a partial merge from an F-14BU export bringing
      its page slot into the F-14.

- [x] ~~**Fly pages on the UFC and the ICP.**~~ Flown 2026-09-24: pages on
      the UFC and the ICP's DED swap from the mapped page keys (A/P to BCN,
      COM 1 to A-G), with the modifier chosen in Settings, and only with that
      modifier. [STATUS.md](STATUS.md), "pages on the UFC and the ICP"

- [x] **Finish the UFC and ICP page checks.** Not covered by the flight
      above: a blank slot taking the screen dark and a disabled slot's key
      doing nothing on these two screens, and the A-10C CMSC and Mi-24P
      Radios pages on the glass.

- [x] ~~**Check an update still carries changes to a panel set to not
      drive.**~~ Checked 2026-09-25: `merge_new` reconciles every row
      whatever `disabled_devices` says, and keeps the user's own entry in
      it, so a panel turned back on later has current rows. Pinned by
      `a_panel_the_user_stopped_driving_still_takes_the_update` in
      `crates/dsc-config/tests/update_lamps_and_settings.rs`.

- [x] **Fly the A-10C split.** Upgrade an alpha.007 install whose A-10C
      profile is untouched: the A-10C II should land on the new A-10C2 profile
      with the ARC-210 CDU page, and the A-10C keep its profile with the VHF AM
      CDU page. Then fly each and watch the PTO2 NMSP lamps (EGI, STEER PT,
      TCN, ANCHR, ILS). [STATUS.md](STATUS.md), "a shipped profile can split"

- [x] ~~**Give the A-10C CDU page a readable id before alpha.008 ships.**~~
      Dropped 2026-09-25: `i63dn3` stays, and new shipped pages keep the id
      the editor generates. [STATUS.md](STATUS.md), "Shipped ids"

- [x] **Decide the F-14BU's ICP: disabled, or a Blank slot.** alpha.007
      shipped it disabled; the page move took that out. Then the changelog
      entry under F-14BU stands or goes.

- [x] ~~**Bring the A-10C PTO2 notes up to date.**~~ Done 2026-09-25: CTR,
      LI, LO, RI and RO in `a-10c.json` and `a-10c2.json` name the NMSP lamp
      each shows. A changed default row, so it goes in CHANGELOG.md at ship.

- [x] ~~**Decide on the dead field-reset code.**~~ Decided 2026-09-25:
      pages got a reset of their own. Reset this field and "+ the field that
      shipped here" now work from the shipped page files, and every field and
      lamp gained Undo unsaved changes. Built and type-checked, not yet
      clicked through.

- [x] ~~**Remove line merging from `merge.rs`.**~~ Removed 2026-09-25,
      with the line checkboxes in the merge dialog: a screen merges a slot
      at a time. Merging part of one page into another would be a feature
      of its own.

- [x] ~~**Site page for UFC and ICP pages.**~~ Done 2026-09-24, with the
      PFPs and the steady backlights, ahead of flying the UFC and ICP pages.
      The screenshots are Cory's to retake.

- [x] **See the pages on the panel.** Fly one aircraft per page file and check
      the MCDU looks as it did before the move: A-10C CDU, AH-64D KU, CH-47F
      CDU, F-14BU CDNU, F-16 Flight, F/A-18 IFEI.

- [x] **Open the page editor in the window.** Built and type-checked, not yet
      clicked through: the six slots (Disabled, Blank, pages), Edit page and
      New page, Save page, Save as new page, Delete page, and export, import
      and merge with pages. The shipped defaults are version 2 now, so any
      of the six aircraft with a page will do.

- [x] ~~**Text output fields.**~~ Built and flown 2026-09-20, on the panel
      with DCS feeding it: a chain on the A-10C's free rows from the A-10C II
      fuel strings, pieced together with typed text, and labels put on rules.
      A field is a chain of pieces, each characters the user typed or a signal,
      each with its own colour and size. Font selection came with it, and the
      editor now lists every area of every screen in the order it sits on the
      glass. See [CHANGELOG.md](../CHANGELOG.md) and "Content: what fills a
      field" in [CONFIG.md](CONFIG.md).

- [x] **Watch the panels stay lit through the options menu.** Built
      2026-09-22, not yet seen in DCS. Mid-mission, sit in the options or
      controls menu for more than 20 seconds: every lamp and screen should
      keep the last cockpit and come back without a blank and a rebuild. Then
      quit DCS: the panels should clear and the daemon exit on its own.
      [STATUS.md](STATUS.md), "the panels stay lit through a quiet stream"

- [x] **Look at the UFC and DED glass for the preview.** Two things only the
      panel can answer. What colour each glass is, since both previews are
      drawn white and a colour per display is a small change in `paintInk`.
      And whether each UFC segment sits where `art` in
      `data/displays/ufc1.json` draws it, which was read out of the glyph
      table rather than captured; a photograph of a few lit cells settles it.
      [STATUS.md](STATUS.md), "the UFC and the DED are previewed too"

- [x] ~~**Point the defaults' backlights at one lamp across panels.**~~ Done
      2026-09-21: every backlight in every default matches the MFD C's
      `INST_PNL_Backlight`, moved in the editor. The changed rows are named in
      CHANGELOG.md.

- [x] ~~**Fly a cross-panel `same_as`.**~~ Proven on the panels 2026-09-21: a
      dimmer pointed at a dimmer on another device follows it.

- [x] ~~**Decide whether shipped defaults use `follows`.**~~ Decided
      2026-09-21: they do. In every default the MFD L and R use the MFD C and
      the MCDU Co-Pilot and Observer use the Captain; their own rows are kept.

- [x] ~~**Decide whether the IFEI page ships as the Hornet's MCDU default.**~~
      It did, on the MCDU Captain, in 1.0.0-alpha.004.

- [x] ~~**Watch the update reconcile on a real upgrade.**~~ Seen on the
      alpha.003 and later upgrades: untouched rows corrected, rows changed by
      hand left alone.

- [x] ~~**Fly the #30 display work.**~~ Flown 2026-09-22: the F-16 MCDU
      flight page (drums, CMDS aliases, trim bands), the A-10C MCDU radio
      rows and DED countermeasures page, and a per-seat copy of a field.

## Release

- [x] ~~**Say beta, not alpha, in the docs and the site, at ship only.**~~
      Done 2026-09-27 for 1.0.0-beta.001: the badge and heading in
      `docs/index.html` and the note in `README.md`. The site and README go
      live on merge while installed copies are still alpha, so this waited
      for the commit that gets tagged beta.001.
      Known places: the Alpha badge and "Read this bit: it is an alpha" in
      `docs/index.html`, and the **Alpha.** note in `README.md`. Grep for
      "alpha" first; the ones in `STATUS.md` and this file are history.
      The update banner offers alpha.010 users the beta, whatever the
      version numbers say: `alpha_to_beta_is_offered_though_the_number_drops`
      in `editor/src-tauri/src/update.rs`.

- [x] ~~**The release-notes list of changed default rows.**~~ Done
      2026-10-09: `tools/changed_defaults.py` lists every lamp row, follow,
      page slot, page field and stored signal that moved since the last tag,
      matched on what it is rather than where it sits, with a change made the
      same way in several profiles listed once. `release.cmd` runs it at step
      0. [STATUS.md](STATUS.md), "Release notes"

## Smaller

- [ ] **Open a DCS-BIOS PR for the Mosquito `GUN_MASTER`.** Found
      2026-09-27: it never leaves 1 (ARMED). `Mosquito.lua:120` defines it as
      `defineTumb(..., 121, 2, { -1, 1 }, ...)`, but the module's
      `clickabledata.lua` moves argument 121 over `{0, 1}`, so safe (0)
      rounds `(0 + 1) / 2 = 0.5` up to 1 as well. The fix is
      `defineToggleSwitch("GUN_MASTER", 5, 3003, 121, "Main Panel", "Gun
    Firing Master Switch")`. Nothing on our side can work around it.
- [x] ~~**Move the DED glyph generator into `tools/`.**~~ Done 2026-09-25:
      `tools/gen_ded.py` pairs the two fixtures itself, so there is no
      glyph cache, and rewrites only the font and its two notes in
      `data/displays/ded.json`. A fresh run leaves the file byte for byte.
- [x] ~~**Replace the drawn DED glyphs.**~~ Done 2026-09-25. A second flight
      through every DED page captured 10 of the 27. The other 17 never show on
      the F-16's DED, so they come from SimAppPro's font file, which every
      captured glyph matches. [STATUS.md](STATUS.md), "The whole font is
      SimAppPro's"
- [x] ~~**Name a display from a device spec**, so a part can carry one.~~
      Already done, and the note was out of date: a part names its display in
      `data/devices.json` (`"display": "DED"`), `DeviceSpec::displays` reads it,
      and `every_declared_display_has_a_map` checks each name has a map.

## Blocked on hardware

- [ ] **VIRPIL backlights.** Blocked until the gear arrives, expected around
      January 2027. The seam it plugs into is already in: every device names a
      protocol, and `crates/dsc-cli/src/panels/` holds the `Protocol` and
      `Panel` traits with `wctrl` as the only implementation. Adding a brand is
      a module there and a name in `panels::all()`; nothing above it changes.

      Do not start writing a backend before there is a capture. In order:

      1. **Answer the one question that decides feasibility.** USBPcap and
         Wireshark on the VPC Configuration Tool: does moving the LED
         brightness slider produce traffic immediately, or only on save to
         device? Persistent-only means flash writes at signal rate and this
         cannot be built on it.
      2. **Read the ids and the descriptors off the real hardware** rather than
         trusting a remembered vendor id. `declares_output` in `wctrl-hid`
         already says whether an interface declares host-to-device writes.
      3. **Then** the backend, backlights only, fixed colour per lamp so the
         engine keeps sending one byte of brightness.

      Colour as a bindable signal is a separate, much larger feature and is not
      part of this. See "What VIRPIL will need decided" in STATUS.md for why
      each of those is in that order. Tracked in #124.

- [ ] **Fly `follows` on two MCDUs.** Built 2026-09-21 and tested, not yet on
      a panel. Point the Co-Pilot unit at the Captain in the Hornet and check
      that both show the IFEI page and dim together.
      [STATUS.md](STATUS.md), "one panel under several names shares a setup"

- [ ] **See a PFP on a real panel.** Built 2026-09-24 from WwDevicesDotnet
      alone; nobody here owns one. When a PFP owner reports back, confirm the
      part id (a lamp lights at all), the five lamps, the LSK page keys, and
      whether the 31px rows sit acceptably against the keys or the 32px
      fonts are worth a per-part glyph height. Then mark `verified` in
      `winctrl.json`. [STATUS.md](STATUS.md), "Built 2026-09-24: the PFP-3N"

- [ ] **Fly DSC boards before `dev_only` comes off.** Everything is built
      and tested on the PC against the real firmware (`crates/dsc-firmware`);
      nothing has driven a board yet. On an Uno (serial), a Leonardo (HID) and
      a Pico (HID, TinyUSB):
      - [ ] Leonardo and Pico on their own power: pull the cable while lamps
            are lit, and the lamps go out.
      - [ ] Uno: stall `loop()` while driven; the converter logs "stopped
            answering" and catches the board up when it recovers.
      - [ ] `tools/dsc_probe.py describe`, `walk` and `state` on each board.
      - [ ] Fly a profile bound to a board in DCS, light by light.
      - [ ] Then take `dev_only` out of `editor/src-tauri/src/boards.rs`.

      What each case should do, and why: "Recovery" in
      [PROTOCOL-DSC.md](PROTOCOL-DSC.md). Tracked in #132, under #123.

## Deferred, not scheduled

- [ ] **Profile inheritance.** Leaning no for v1.
      [STATUS.md](STATUS.md), "Open threads"
- [ ] **Backlight contention** with SimAppPro's "Sync with DCS": the one lamp
      both applications may drive. Detect and warn.
      [STATUS.md](STATUS.md), "Open threads"
- [ ] **A perceptual response curve for dimmers.** Linear PWM feels wrong at
      the bottom. [STATUS.md](STATUS.md), "Open threads"
- [ ] **Arduino boards as user-defined devices.** A generic sketch flashed
      once, the board and its lamps added in the editor, bound per aircraft
      like any panel. Needs our own protocol (HID, serial or both), a
      user-owned device file and an Add a device form. Tracked in #123.

      The inventory is now a folder, `data/devices`, one file per maker
      (`winctrl.json`, `virtual.json`), read by `DeviceInventory::load_dir`.
      A user's boards go in a `devices` folder of the same shape in the
      writable folder, read after the shipped one; a key in both is refused
      today and should become a warning once users can add devices.

      Spec drafted 2026-10-09 in [PROTOCOL-DSC.md](PROTOCOL-DSC.md): HID and
      serial with one message set, devices describe their lamps with names,
      and a described device is saved to the user's devices folder so it can
      be bound unplugged. The reference library is `firmware/DscDevice`
      (serial, HID on 32u4 and RP2040 TinyUSB, a shift register example),
      compiled but untested until boards arrive; `tools/dsc_probe.py` drives
      one without the converter. The `dsc` backend and the editor's Boards
      section in Settings are built; the section, and every command behind
      it, works only in a dev checkout until a real board has been flown
      (`boards.rs`, `dev_only`). Added boards go in `user-devices/boards.json`.

      Hardened 2026-10-09: version negotiation, identity conflicts, malformed
      messages and recovery are written into the spec with their reasons. A
      driven board is checked with `STATE` every 2 s and caught up when it
      drifts or stops answering; native USB boards turn their lamps off when
      the link goes. `crates/dsc-firmware` builds the real library on the PC
      against stand-ins and tests the host against it (`DSC_ASAN=1` adds
      AddressSanitizer; the MSVC bin folder must be on PATH to run it).
      Waiting on boards: "Fly DSC boards" under Blocked on hardware, #132.

      A profile binding a device the inventory lacks is still refused
      (`UnknownLed`), so boards never reach one that lacks them: an export
      leaves out the boards this PC added (`Profile::without_devices`), and
      removing a board takes it out of every profile that names it first,
      after a confirm naming them. A dev checkout flies data/defaults, so a
      board bound while testing lands in a shipped default:
      `tools/shipped_devices.py` fails CI and the release on that, and on a
      `dsc` device in data/devices.

      **Lamp names and labels in flash on AVR**, before anyone builds a big
      panel on an Uno. AVR copies constant data from flash into SRAM at
      startup, so each lamp's name and label (up to about 50 bytes) comes out
      of the Uno's 2 KB: around 30 fully labelled lamps would run out. Keep
      them in flash with `PROGMEM` and read them with `pgm_read_byte` when
      answering `DESCRIBE`. One code path for every board: the RP2040 core
      defines `PROGMEM` as nothing and `pgm_read_byte` as a plain read, since
      its constants already stay in flash, so only AVR changes. The protocol
      and the converter do not change. Check it with the RAM figure `arduino-cli`
      prints for a large table, and give the PC stand-in in
      `crates/dsc-firmware` the same macros so its tests still run.

      **Then a Build Sketch screen**, once the backend has driven a real
      board: board, transport, model and unit, then rows of pin, name, label,
      kind and backlight, with a 74HC595 chain as an option, saved as an
      `.ino` for the Arduino IDE to flash. The value is in per-board pin
      tables (which pins exist, which dim, which to avoid such as the serial
      pins 0 and 1), taken from the boards' own documentation and checked on
      hardware, and in validating names against the protocol's limits.
      Flashing from the app is out: it would mean bundling arduino-cli and
      the cores. Configuring lamps over the protocol into EEPROM is out too:
      the spec writes nothing persistent. A "these pins are buttons" option
      fits here too, adding a game controller on native USB boards.

      **Maybe a protocol v2: inputs over serial.** A serial board cannot be a
      game controller and cannot share its COM port with a DCS-BIOS sketch,
      so today its switches need a second board. v2 would have the board
      report raw input events and a profile map them to DCS-BIOS commands,
      keeping aircraft logic out of the sketch. A whole input-binding feature
      in the engine and editor; wait for someone to ask. See "Buttons and
      switches" in PROTOCOL-DSC.md.

      The same protocol can serve small commercial makers. Total Controls
      (Apache MPDs and other panels) is being asked (2026-10-08) about putting
      it in their firmware; their MPDs have no host brightness control today,
      only key combos. So the spec comes first and is written as its own
      document beside PROTOCOL-WINCTRL.md, with a capability report (the
      device says what lamps and channels it has) and a version byte from
      the start, because outside firmware will ship against it. A
      self-describing device then needs no device file; our sketch can
      report itself the same way and shrink the form. Backend is one module
      in `crates/dsc-cli/src/panels/`, the seam VIRPIL uses.
- [x] ~~**The 175 ms pass at mission start.**~~ Gone 2026-09-30: it was
      the screens being written from the main loop. Each panel now has its
      own writer thread, and the longest pass in the minute the Mosquito
      loaded was 2 ms. [STATUS.md](STATUS.md), "longest pass 175 ms"
