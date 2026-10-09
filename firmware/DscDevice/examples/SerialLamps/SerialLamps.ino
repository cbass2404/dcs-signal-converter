// A small caution panel on any Arduino, driven over USB serial.
//
// Edit the table for your wiring, flash once, then choose the board's port in
// DCS Signal Converter and bind its lamps per aircraft. Changing what a lamp
// means is done there, never here.

#include <DscDevice.h>

// One row per lamp: pin, kind, highest value, flags, name, label.
// Names are what profiles store, so pick them once and leave them.
const DscLamp lamps[] = {
    {13, DSC_INDICATOR, 1, 0, "MASTER_CAUTION", "Master caution"},
    {12, DSC_INDICATOR, 1, 0, "FIRE", "Fire"},
    {11, DSC_INDICATOR, 1, 0, "GEAR_DOWN", "Gear down"},
    {9, DSC_DIMMER, 255, DSC_BACKLIGHT, "BACKLIGHT", "Panel backlight"},
};

// Vendor, model, unit and firmware version. Give each board of the same model
// its own unit, such as "L" and "R".
DscDevice panel("Arduino", "Caution Panel", "", "1.0", lamps, DSC_COUNT(lamps));
DscSerial link(Serial);

void setup() {
  Serial.begin(115200);
  panel.begin(link);
}

void loop() {
  panel.poll();
}
