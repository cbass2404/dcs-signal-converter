// Sixteen indicators on two chained 74HC595 shift registers, over USB serial.
// The pattern for any pit with more lamps than pins: the table names the
// lamps, and onLamp sets them however the hardware wants.

#include <DscDevice.h>

const uint8_t DATA_PIN = 11;
const uint8_t CLOCK_PIN = 13;
const uint8_t LATCH_PIN = 10;

// Index n is output n of the chain: QA of the first register is 0.
const DscLamp lamps[] = {
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "MASTER_CAUTION", "Master caution"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "FIRE_LEFT", "Left engine fire"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "FIRE_RIGHT", "Right engine fire"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "APU_FIRE", "APU fire"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "GEAR_NOSE", "Nose gear"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "GEAR_LEFT", "Left gear"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "GEAR_RIGHT", "Right gear"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "GEAR_HANDLE", "Gear handle"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "HOOK", "Hook"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "FLAPS", "Flaps"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "SPEEDBRAKE", "Speed brake"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "LOW_FUEL", "Low fuel"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "MASTER_ARM", "Master arm"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "AA", "Air to air"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "AG", "Air to ground"},
    {DSC_NO_PIN, DSC_INDICATOR, 1, 0, "NWS", "Nosewheel steering"},
};

DscDevice panel("Arduino", "Warning Panel", "", "1.0", lamps, DSC_COUNT(lamps));
DscSerial link(Serial);

uint16_t bits = 0;
bool changed = false;

void setLamp(uint8_t index, uint8_t value) {
  uint16_t mask = (uint16_t)1 << index;
  bits = value ? (bits | mask) : (bits & ~mask);
  changed = true;
}

void shiftOutBits() {
  digitalWrite(LATCH_PIN, LOW);
  shiftOut(DATA_PIN, CLOCK_PIN, MSBFIRST, highByte(bits));
  shiftOut(DATA_PIN, CLOCK_PIN, MSBFIRST, lowByte(bits));
  digitalWrite(LATCH_PIN, HIGH);
  changed = false;
}

void setup() {
  pinMode(DATA_PIN, OUTPUT);
  pinMode(CLOCK_PIN, OUTPUT);
  pinMode(LATCH_PIN, OUTPUT);
  Serial.begin(115200);
  panel.onLamp(setLamp);
  panel.begin(link);
  shiftOutBits();
}

void loop() {
  // A batch of changes goes out to the registers once, after it is all in.
  panel.poll();
  if (changed) {
    shiftOutBits();
  }
}
