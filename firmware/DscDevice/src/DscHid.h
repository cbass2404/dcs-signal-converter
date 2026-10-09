// DscHid: messages over USB HID, for boards with native USB.
//
// ATmega32u4 boards (Leonardo, Pro Micro, Micro) use the core's own USB. An
// RP2040 board needs the Adafruit TinyUSB stack: in the Arduino IDE, Tools >
// USB Stack > Adafruit TinyUSB. Serial still works alongside on both.
//
// The converter finds the board by the HID usage page 0xFFD5, usage 0x01, so
// it keeps whatever USB ids the board already has.

#pragma once

#include "DscDevice.h"

#if (defined(ARDUINO_ARCH_AVR) && defined(USBCON)) || defined(USE_TINYUSB)
#define DSC_HAS_HID 1
#else
#define DSC_HAS_HID 0
#endif

#if DSC_HAS_HID

class DscHid : public DscTransport {
public:
  // Declare it as a global, so the HID interface is there before the PC
  // first asks the board what it is.
  DscHid();
  // Call once from setup().
  void begin();
  uint8_t receive(uint8_t *buf) override;
  void send(const uint8_t *msg, uint8_t len) override;
};

#else
#error "DscHid needs native USB: a Leonardo, Pro Micro or Micro, or an RP2040 with Tools > USB Stack > Adafruit TinyUSB. Other boards use DscSerial."
#endif
