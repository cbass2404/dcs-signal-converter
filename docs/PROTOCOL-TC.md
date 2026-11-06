# Total Controls panels

The Apache MPD frames from Total Controls, driven by the `tc` backend in
`crates/dsc-cli/src/panels/tc.rs`. Everything here was observed on the
hardware or given by the maker; nothing is inferred.

## Identity

| Panel             | VID      | PID      | Product string         |
| ----------------- | -------- | -------- | ---------------------- |
| Apache MPD Left   | `0x04D8` | `0xE7ED` | `TC Apache MPD Left`   |
| Apache MPD Right  | `0x04D8` | `0xE7EC` | `TC Apache MPD Right`  |
| Apache MPD Center | `0x04D8` | `0xE7C1` | `TC Apache MPD Center` |

`0x04D8` is Microchip's vendor id, which other makers use too, so a device is
only taken as Total Controls when its manufacturer string also says
`Total Controls`. One unit can be any of the three: a button combo reboots it
into another identity. Units ship as left.

Each is a joystick (34 buttons, X and Y axes 0 to 255, a 10-byte input
report) with no output report and one 8-byte vendor feature report on usage
page `0xFF00`, with no report id. Reading that report fails (error 31), so it
is write-only.

## The backlight command

Given by the maker, 2026-11-06: the feature report `'B'` (`0x42`), then the
level as a raw byte.

| Level | Light  |
| ----- | ------ |
| 0     | off    |
| 1     | dim    |
| 2     | medium |
| 3     | full   |

On Windows the write is report id 0 and then the declared eight bytes:
`00 42 n 00 00 00 00 00 00`. Checked 2026-11-06 on all three identities: each
level shows at once, and a write to one panel leaves the others alone.

Nothing else is ever written into that report. Other first bytes are
undocumented and may switch the panel's identity or start its bootloader.

## Why it is written sparingly

A level set this way survives a power cycle (set dim, unplugged 30 s, came
back dim), so the panel stores it. Whether it stores every write, and in
EEPROM or flash, is not known, so the backend assumes every write is stored
and wears the part:

- The engine's brightness maps onto the four levels: 0 is off, and 1 to 255
  in thirds is dim, medium and full.
- A level is written only once it has held for 500 ms, and only if it is not
  the level last written. Sweeping a dimmer from full to off is one write.
- The exit's clearing write (off) goes out at once, not after the wait.
- The benchmark runs with `--skip-protocol tc`, so a perf run never writes to
  them.

The panel's own combo still changes the level and reports nothing to the
host, so after a combo press the next level the cockpit asks for wins.
