// The board's side of the tests: the DscDevice library as a sketch would use
// it, over a serial stream and over the TinyUSB stand-in, with C entry points
// for Rust.

#include <deque>
#include <vector>

#include <Adafruit_TinyUSB.h>
#include <Arduino.h>

#include "DscDevice.h"
#include "DscHid.h"

namespace {

// Each pin's last value, -1 until written. Per thread, as Rust runs tests on
// several at once.
thread_local std::vector<int> pins(256, -1);

const DscLamp lamps[] = {
    {2, DSC_INDICATOR, 1, 0, "FIRE", "Fire"},
    {3, DSC_DIMMER, 255, DSC_BACKLIGHT, "BACKLIGHT", nullptr},
    {4, DSC_DIMMER, 100, 0, "GAUGE", "Gauge"},
};

class Wire : public Stream {
public:
  std::deque<uint8_t> in;
  std::vector<uint8_t> out;

  int available() override { return (int)in.size(); }
  int read() override {
    if (in.empty()) {
      return -1;
    }
    int b = in.front();
    in.pop_front();
    return b;
  }
  size_t write(const uint8_t *buf, size_t len) override {
    out.insert(out.end(), buf, buf + len);
    return len;
  }
};

struct SerialBoard {
  Wire wire;
  DscSerial link{wire};
  DscDevice device{"Arduino", "Test Panel", "", "1.0", lamps, DSC_COUNT(lamps)};
};

// TinyUSB keeps one HID interface for the whole program, and so does
// DscHid's queue, so the HID board is one global as in a sketch.
dsc_set_report_cb setReport = nullptr;
std::deque<std::vector<uint8_t>> sentReports;
DscHid hidLink;
DscDevice hidDevice("Arduino", "Test Panel", "", "1.0", lamps,
                    DSC_COUNT(lamps));

}  // namespace

void pinMode(uint8_t, uint8_t) {}
void digitalWrite(uint8_t pin, uint8_t value) { pins[pin] = value ? 1 : 0; }
void analogWrite(uint8_t pin, int value) { pins[pin] = value; }
unsigned long millis() { return 0; }
void delay(unsigned long) {}
void yield() {}

Adafruit_USBD_Device TinyUSBDevice;

void Adafruit_USBD_HID::setReportCallback(dsc_get_report_cb,
                                          dsc_set_report_cb set) {
  setReport = set;
}

bool Adafruit_USBD_HID::sendReport(uint8_t id, const void *report,
                                   uint16_t len) {
  std::vector<uint8_t> r{id};
  const uint8_t *bytes = static_cast<const uint8_t *>(report);
  r.insert(r.end(), bytes, bytes + len);
  sentReports.push_back(r);
  return true;
}

static size_t takeInto(std::vector<uint8_t> &from, uint8_t *out, size_t cap) {
  size_t n = from.size() < cap ? from.size() : cap;
  memcpy(out, from.data(), n);
  from.erase(from.begin(), from.begin() + n);
  return n;
}

extern "C" {

void *dsc_serial_new() {
  SerialBoard *b = new SerialBoard;
  b->device.begin(b->link);
  return b;
}

void dsc_serial_free(void *board) { delete static_cast<SerialBoard *>(board); }

void dsc_serial_feed(void *board, const uint8_t *bytes, size_t len) {
  SerialBoard *b = static_cast<SerialBoard *>(board);
  b->wire.in.insert(b->wire.in.end(), bytes, bytes + len);
  b->device.poll();
}

size_t dsc_serial_take(void *board, uint8_t *out, size_t cap) {
  return takeInto(static_cast<SerialBoard *>(board)->wire.out, out, cap);
}

int dsc_pin(uint8_t pin) { return pins[pin]; }

void dsc_hid_begin() {
  static bool begun = false;
  if (!begun) {
    hidLink.begin();
    hidDevice.begin(hidLink);
    begun = true;
  }
  sentReports.clear();
}

void dsc_hid_report(uint8_t id, const uint8_t *data, uint16_t len) {
  setReport(id, HID_REPORT_TYPE_OUTPUT, data, len);
  hidDevice.poll();
}

size_t dsc_hid_take(uint8_t *out, size_t cap) {
  if (sentReports.empty()) {
    return 0;
  }
  size_t n = takeInto(sentReports.front(), out, cap);
  sentReports.pop_front();
  return n;
}

}  // extern "C"
