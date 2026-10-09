# DscDevice

An Arduino library for lamps driven from DCS World by DCS Signal Converter. Flash
it once; what each lamp means is set per aircraft in the converter's editor,
never in the sketch.

**Status: not yet tested on hardware.** It compiles for the boards below, and
the converter cannot drive it yet.

## Boards

| Board                      | Transport   | Example                                               |
| -------------------------- | ----------- | ----------------------------------------------------- |
| Uno, Nano, Mega            | Serial      | `SerialLamps`                                         |
| Leonardo, Pro Micro, Micro | Serial, HID | `SerialLamps`, `HidLamps`                             |
| RP2040 (Pico and others)   | Serial, HID | `HidLamps` needs Tools > USB Stack > Adafruit TinyUSB |
| More lamps than pins       | Serial      | `ShiftRegisterLamps`                                  |

HID boards are found on their own. A serial board's port is chosen once in the
editor; the converter never opens a port it was not given.

## Install

Copy this `DscDevice` folder into `Documents\Arduino\libraries`, restart the
Arduino IDE, and open an example from File > Examples > DscDevice.

## The lamp table

```cpp
const DscLamp lamps[] = {
    // pin, kind, highest value, flags, name, label
    {13, DSC_INDICATOR, 1, 0, "MASTER_CAUTION", "Master caution"},
    {9, DSC_DIMMER, 255, DSC_BACKLIGHT, "BACKLIGHT", "Panel backlight"},
};
DscDevice panel("Arduino", "Caution Panel", "", "1.0", lamps, DSC_COUNT(lamps));
```

- **Name** is what profiles store. Rename a lamp and every binding to it is
  lost, so choose names once.
- **Vendor, model and unit** are the board's identity. Two boards of the same
  model need different units, such as `"L"` and `"R"`.
- **`DSC_BACKLIGHT`** joins the lamp to the pit-wide backlight dimming.
- **`DSC_NO_PIN`** with `panel.onLamp(writer)` hands every lamp to your own
  code, for shift registers or LED drivers.

## Buttons and switches

This library drives lamps only. On a native USB board, add a game controller
to the same sketch (the Joystick library on a Leonardo or Pro Micro, a TinyUSB
gamepad on an RP2040) and bind its buttons in DCS like any button box. A
serial board (Uno, Nano, Mega) cannot share its COM port with a DCS-BIOS
sketch, so its switches go on a second board running DCS-BIOS. See "Buttons
and switches" in the protocol document.

## Testing a board

`tools/dsc_probe.py` in the repository talks to a board without the converter:
`describe` lists its lamps, `walk` lights each in turn, `set` sets one. The wire
protocol is [docs/PROTOCOL-DSC.md](../../docs/PROTOCOL-DSC.md).
