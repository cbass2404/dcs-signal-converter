// Compiled into every sketch that uses the library, so on a board without
// native USB it must be empty rather than include DscHid.h, which refuses it.
#include <Arduino.h>

#if (defined(ARDUINO_ARCH_AVR) && defined(USBCON)) || defined(USE_TINYUSB)

#include "DscHid.h"

namespace {

const uint8_t REPORT_ID = 0x01;
// The report after its ID: a length byte, then the message, then padding.
const uint8_t PAYLOAD = 63;

#define DSC_REPORT_DESCRIPTOR                                       \
  0x06, 0xD5, 0xFF, /* Usage Page (0xFFD5, vendor)              */  \
  0x09, 0x01,       /* Usage (0x01)                             */  \
  0xA1, 0x01,       /* Collection (Application)                 */  \
  0x85, 0x01,       /*   Report ID (1)                          */  \
  0x15, 0x00,       /*   Logical Minimum (0)                    */  \
  0x26, 0xFF, 0x00, /*   Logical Maximum (255)                  */  \
  0x75, 0x08,       /*   Report Size (8)                        */  \
  0x95, 0x3F,       /*   Report Count (63)                      */  \
  0x09, 0x02,       /*   Usage (0x02)                           */  \
  0x81, 0x02,       /*   Input (Data, Variable, Absolute)       */  \
  0x09, 0x03,       /*   Usage (0x03)                           */  \
  0x91, 0x02,       /*   Output (Data, Variable, Absolute)      */  \
  0xC0              /* End Collection                           */

// A payload into a message: its length byte, checked, then the message.
uint8_t unpack(const uint8_t *payload, uint8_t size, uint8_t *buf) {
  if (size < 2) {
    return 0;
  }
  uint8_t n = payload[0];
  if (n == 0 || n > DSC_MAX_MESSAGE || n > size - 1) {
    return 0;
  }
  memcpy(buf, payload + 1, n);
  return n;
}

}  // namespace

// ------------------------------------------------------- ATmega32u4 boards
#if defined(ARDUINO_ARCH_AVR)

#include <HID.h>
#include <PluggableUSB.h>

namespace {

const uint8_t reportDescriptor[] PROGMEM = {DSC_REPORT_DESCRIPTOR};

struct Descriptor {
  InterfaceDescriptor interface;
  HIDDescDescriptor hid;
  EndpointDescriptor in;
  EndpointDescriptor out;
};

// One HID interface with an interrupt endpoint each way. Windows writes to
// the OUT endpoint when a device has one, so output reports never come as
// control transfers here.
class Usb : public PluggableUSBModule {
public:
  Usb() : PluggableUSBModule(2, 1, types_) {
    types_[0] = EP_TYPE_INTERRUPT_IN;
    types_[1] = EP_TYPE_INTERRUPT_OUT;
    PluggableUSB().plug(this);
  }
  uint8_t in() const { return pluggedEndpoint; }
  uint8_t out() const { return pluggedEndpoint + 1; }

protected:
  int getInterface(uint8_t *count) override {
    *count += 1;
    Descriptor d = {
        D_INTERFACE(pluggedInterface, 2, USB_DEVICE_CLASS_HUMAN_INTERFACE,
                    HID_SUBCLASS_NONE, HID_PROTOCOL_NONE),
        D_HIDREPORT(sizeof(reportDescriptor)),
        D_ENDPOINT(USB_ENDPOINT_IN(pluggedEndpoint),
                   USB_ENDPOINT_TYPE_INTERRUPT, USB_EP_SIZE, 0x01),
        D_ENDPOINT(USB_ENDPOINT_OUT(pluggedEndpoint + 1),
                   USB_ENDPOINT_TYPE_INTERRUPT, USB_EP_SIZE, 0x01),
    };
    return USB_SendControl(0, &d, sizeof(d));
  }

  int getDescriptor(USBSetup &setup) override {
    if (setup.bmRequestType != REQUEST_DEVICETOHOST_STANDARD_INTERFACE ||
        setup.wValueH != HID_REPORT_DESCRIPTOR_TYPE ||
        setup.wIndex != pluggedInterface) {
      return 0;
    }
    return USB_SendControl(TRANSFER_PGM, reportDescriptor,
                           sizeof(reportDescriptor));
  }

  bool setup(USBSetup &setup) override {
    if (setup.wIndex != pluggedInterface) {
      return false;
    }
    if (setup.bmRequestType == REQUEST_HOSTTODEVICE_CLASS_INTERFACE) {
      // Windows sets idle on every HID interface it opens; nothing to keep.
      return setup.bRequest == HID_SET_IDLE ||
             setup.bRequest == HID_SET_PROTOCOL;
    }
    return false;
  }

private:
  uint8_t types_[2];
};

// Made by the first DscHid, which a sketch declares as a global, so it is
// plugged in before USB starts. Not a global here, or every sketch using the
// library on a 32u4 would grow a HID interface.
Usb &usb() {
  static Usb u;
  return u;
}

}  // namespace

DscHid::DscHid() { usb(); }

void DscHid::begin() {}

uint8_t DscHid::receive(uint8_t *buf) {
  uint8_t ep = usb().out();
  if (USB_Available(ep) == 0) {
    return 0;
  }
  uint8_t report[1 + PAYLOAD];
  int got = USB_Recv(ep, report, sizeof(report));
  if (got < 2 || report[0] != REPORT_ID) {
    return 0;
  }
  return unpack(report + 1, (uint8_t)(got - 1), buf);
}

void DscHid::send(const uint8_t *msg, uint8_t len) {
  if (len == 0 || len > DSC_MAX_MESSAGE) {
    return;
  }
  uint8_t report[1 + PAYLOAD] = {0};
  report[0] = REPORT_ID;
  report[1] = len;
  memcpy(report + 2, msg, len);
  USB_Send(usb().in() | TRANSFER_RELEASE, report, sizeof(report));
}

// ------------------------------------------------------------ TinyUSB boards
#else

#include <Adafruit_TinyUSB.h>

namespace {

const uint8_t reportDescriptor[] = {DSC_REPORT_DESCRIPTOR};

// Output reports arrive in TinyUSB's callback and wait here for poll(). A
// lamp batch can be several reports back to back, so there is room for a few.
const uint8_t QUEUE = 8;
uint8_t queue[QUEUE][PAYLOAD];
uint8_t sizes[QUEUE];
volatile uint8_t head = 0;
volatile uint8_t tail = 0;

uint16_t onGetReport(uint8_t, hid_report_type_t, uint8_t *, uint16_t) {
  return 0;
}

void onSetReport(uint8_t id, hid_report_type_t, const uint8_t *data,
                 uint16_t size) {
  // From the OUT endpoint the report ID is still the first byte; from a
  // control transfer it has been taken off and passed as `id`.
  if (id == 0) {
    if (size < 1 || data[0] != REPORT_ID) {
      return;
    }
    data++;
    size--;
  } else if (id != REPORT_ID) {
    return;
  }
  uint8_t next = (uint8_t)((head + 1) % QUEUE);
  if (next == tail) {
    return;
  }
  uint8_t n = size < PAYLOAD ? (uint8_t)size : PAYLOAD;
  memcpy(queue[head], data, n);
  sizes[head] = n;
  head = next;
}

Adafruit_USBD_HID &hid() {
  static Adafruit_USBD_HID h(reportDescriptor, sizeof(reportDescriptor),
                             HID_ITF_PROTOCOL_NONE, 1, true);
  return h;
}

}  // namespace

DscHid::DscHid() {}

void DscHid::begin() {
  if (!TinyUSBDevice.isInitialized()) {
    TinyUSBDevice.begin(0);
  }
  hid().setReportCallback(onGetReport, onSetReport);
  hid().begin();
  // Already enumerated without the HID interface: make the PC look again.
  if (TinyUSBDevice.mounted()) {
    TinyUSBDevice.detach();
    delay(10);
    TinyUSBDevice.attach();
  }
}

uint8_t DscHid::receive(uint8_t *buf) {
  if (tail == head) {
    return 0;
  }
  uint8_t n = unpack(queue[tail], sizes[tail], buf);
  tail = (uint8_t)((tail + 1) % QUEUE);
  return n;
}

void DscHid::send(const uint8_t *msg, uint8_t len) {
  if (len == 0 || len > DSC_MAX_MESSAGE) {
    return;
  }
  uint8_t payload[PAYLOAD] = {0};
  payload[0] = len;
  memcpy(payload + 1, msg, len);
  // The host is waiting for this answer, so the endpoint frees quickly.
  uint32_t until = millis() + 100;
  while (!hid().ready()) {
    if ((int32_t)(millis() - until) > 0) {
      return;
    }
    yield();
  }
  hid().sendReport(REPORT_ID, payload, sizeof(payload));
}

#endif
#endif
