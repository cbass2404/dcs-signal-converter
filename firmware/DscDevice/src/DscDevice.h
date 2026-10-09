// DscDevice: lamps driven by DCS Signal Converter.
//
// The board says what lamps it has and sets what it is told; the converter
// does the aircraft logic. One sketch serves every aircraft. The protocol is
// docs/PROTOCOL-DSC.md in the DCS Signal Converter repository.

#pragma once

#include <Arduino.h>

#define DSC_PROTOCOL_VERSION 1

// Longest message either side sends, type byte included.
#define DSC_MAX_MESSAGE 62

// What a lamp is. A dimmer takes any value 0 to its max, an indicator 0 or
// its max.
enum DscKind : uint8_t {
  DSC_DIMMER = 0,
  DSC_INDICATOR = 1,
};

// Lamp flags. A backlight lights legends or a feature rather than signalling,
// and the converter's shipped profiles dim every backlight together.
#define DSC_BACKLIGHT 0x01

// A lamp with no pin of its own, set by the sketch's onLamp writer instead.
#define DSC_NO_PIN 0xFF

// One lamp. Its index is its row in the table, counting from 0.
struct DscLamp {
  uint8_t pin;
  DscKind kind;
  // Highest value the lamp takes, 1 to 255. Indicators usually 1, dimmers 255.
  uint8_t max;
  uint8_t flags;
  // What profiles store: up to 20 of A-Z, a-z, 0-9 and _, unique on the board.
  // Rename it and every binding to it is lost, so choose once.
  const char *name;
  // What the editor shows, up to 30 characters. nullptr shows the name.
  const char *label;
};

#define DSC_COUNT(table) (sizeof(table) / sizeof((table)[0]))

// How messages reach the board. DscSerial is here; DscHid is in DscHid.h.
class DscTransport {
public:
  // The next whole message into buf (DSC_MAX_MESSAGE bytes), returning its
  // length, or 0 when none is waiting. Never blocks.
  virtual uint8_t receive(uint8_t *buf) = 0;
  virtual void send(const uint8_t *msg, uint8_t len) = 0;
};

// Messages over a serial port: COBS frames ending in 0x00, each message
// followed by a CRC-8/SMBUS byte. Begin the port at 115200 yourself.
class DscSerial : public DscTransport {
public:
  explicit DscSerial(Stream &stream) : stream_(stream) {}
  uint8_t receive(uint8_t *buf) override;
  void send(const uint8_t *msg, uint8_t len) override;

private:
  Stream &stream_;
  // A frame as it arrives, still encoded: the message, its CRC and the COBS
  // code byte.
  uint8_t frame_[DSC_MAX_MESSAGE + 2];
  uint8_t fill_ = 0;
  // Too long to be a frame; dropped up to the next 0x00.
  bool overflow_ = false;
};

// Sets one lamp in place of the built-in pin handling, for shift registers,
// LED drivers or anything else. Called with a value already within the
// lamp's max.
typedef void (*DscLampWriter)(uint8_t index, uint8_t value);

class DscDevice {
public:
  // vendor up to 12 characters, model up to 16, unit up to 8 (empty unless
  // two of the same model share a PC), firmware up to 12. Vendor, model and
  // unit are the board's identity: keep them the same between flashes.
  DscDevice(const char *vendor, const char *model, const char *unit,
            const char *firmware, const DscLamp *lamps, uint8_t count);

  // Use writer for every lamp instead of the pins. Call before begin.
  void onLamp(DscLampWriter writer) { writer_ = writer; }

  // Lamps off, then listen on transport.
  void begin(DscTransport &transport);

  // Handle every message waiting. Call from loop() as often as it runs.
  void poll();

private:
  void handle(const uint8_t *m, uint8_t n);
  void hello();
  void describe(uint8_t index);
  void set(uint8_t index, uint8_t value);
  void error(uint8_t type, uint8_t code);

  const char *vendor_;
  const char *model_;
  const char *unit_;
  const char *firmware_;
  const DscLamp *lamps_;
  uint8_t count_;
  DscLampWriter writer_ = nullptr;
  DscTransport *transport_ = nullptr;
};
