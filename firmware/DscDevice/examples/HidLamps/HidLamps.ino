// The same caution panel over USB HID, found by DCS Signal Converter on its
// own with no port to choose.
//
// Needs native USB: a Leonardo, Pro Micro or Micro, or an RP2040 board with
// Tools > USB Stack > Adafruit TinyUSB.

#include <DscDevice.h>
#include <DscHid.h>

// One row per lamp: pin, kind, highest value, flags, name, label.
const DscLamp lamps[] = {
    {2, DSC_INDICATOR, 1, 0, "MASTER_CAUTION", "Master caution"},
    {4, DSC_INDICATOR, 1, 0, "FIRE", "Fire"},
    {7, DSC_INDICATOR, 1, 0, "GEAR_DOWN", "Gear down"},
    {9, DSC_DIMMER, 255, DSC_BACKLIGHT, "BACKLIGHT", "Panel backlight"},
};

DscDevice panel("Arduino", "Caution Panel", "", "1.0", lamps, DSC_COUNT(lamps));
// A global, so the HID interface exists before the PC enumerates the board.
DscHid link;

void setup() {
  link.begin();
  panel.begin(link);
}

void loop() {
  panel.poll();
}
