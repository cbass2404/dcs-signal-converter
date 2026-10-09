// Just enough of Adafruit TinyUSB to build DscHid on a PC, for tests. The
// report callback is kept so a test can hand the firmware output reports as
// the USB stack would, and sent reports are recorded by shim.cpp.

#pragma once

#include <Arduino.h>

typedef enum {
  HID_REPORT_TYPE_INVALID,
  HID_REPORT_TYPE_INPUT,
  HID_REPORT_TYPE_OUTPUT,
  HID_REPORT_TYPE_FEATURE,
} hid_report_type_t;

#define HID_ITF_PROTOCOL_NONE 0

typedef uint16_t (*dsc_get_report_cb)(uint8_t, hid_report_type_t, uint8_t *,
                                      uint16_t);
typedef void (*dsc_set_report_cb)(uint8_t, hid_report_type_t, uint8_t const *,
                                  uint16_t);

class Adafruit_USBD_HID {
public:
  Adafruit_USBD_HID(const uint8_t *, uint16_t, uint8_t, uint8_t, bool) {}
  void setReportCallback(dsc_get_report_cb get, dsc_set_report_cb set);
  bool begin() { return true; }
  bool ready() { return true; }
  bool sendReport(uint8_t id, const void *report, uint16_t len);
};

class Adafruit_USBD_Device {
public:
  bool isInitialized() { return true; }
  bool begin(uint8_t) { return true; }
  bool mounted() { return false; }
  bool detach() { return true; }
  bool attach() { return true; }
};

extern Adafruit_USBD_Device TinyUSBDevice;
