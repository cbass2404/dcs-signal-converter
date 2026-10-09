#include "DscDevice.h"

#include <stdlib.h>

namespace {

enum : uint8_t {
  HELLO = 0x01,
  DESCRIBE = 0x02,
  STATE = 0x03,
  SET_LAMPS = 0x10,
  ALL_OFF = 0x11,
  HELLO_REPLY = 0x81,
  LAMP = 0x82,
  STATE_REPLY = 0x83,
  ERROR = 0xFF,
};

enum : uint8_t {
  UNKNOWN_TYPE = 0x01,
  NO_SUCH_LAMP = 0x02,
  TOO_SHORT = 0x03,
};

// CRC-8/SMBUS: polynomial 0x07, initial 0, no reflection, no final XOR.
uint8_t crc8(const uint8_t *data, uint8_t len) {
  uint8_t crc = 0;
  for (uint8_t i = 0; i < len; i++) {
    crc ^= data[i];
    for (uint8_t bit = 0; bit < 8; bit++) {
      crc = (crc & 0x80) ? (uint8_t)((crc << 1) ^ 0x07) : (uint8_t)(crc << 1);
    }
  }
  return crc;
}

// CRC-16/CCITT-FALSE: polynomial 0x1021, initial 0xFFFF, no reflection, no
// final XOR.
uint16_t crc16(const uint8_t *data, uint8_t len) {
  uint16_t crc = 0xFFFF;
  for (uint8_t i = 0; i < len; i++) {
    crc ^= (uint16_t)data[i] << 8;
    for (uint8_t bit = 0; bit < 8; bit++) {
      crc = (crc & 0x8000) ? (uint16_t)((crc << 1) ^ 0x1021) : (uint16_t)(crc << 1);
    }
  }
  return crc;
}

// Decode one COBS frame without its 0x00 into at most `most` bytes. Returns
// the decoded length, or 0 for a frame that is not valid COBS or will not fit.
uint8_t cobsDecode(const uint8_t *in, uint8_t len, uint8_t *out, uint8_t most) {
  uint8_t i = 0;
  uint8_t o = 0;
  while (i < len) {
    uint8_t code = in[i++];
    if (code == 0) {
      return 0;
    }
    for (uint8_t k = 1; k < code; k++) {
      if (i >= len || o >= most) {
        return 0;
      }
      out[o++] = in[i++];
    }
    if (code < 0xFF && i < len) {
      if (o >= most) {
        return 0;
      }
      out[o++] = 0;
    }
  }
  return o;
}

// Append a string as its length then its bytes, cut to at most `most`.
uint8_t putString(uint8_t *out, uint8_t at, const char *s, uint8_t most) {
  uint8_t len = 0;
  if (s) {
    while (len < most && s[len]) {
      len++;
    }
  }
  out[at++] = len;
  if (len) {
    memcpy(out + at, s, len);
  }
  return at + len;
}

}  // namespace

// ------------------------------------------------------------------ serial

uint8_t DscSerial::receive(uint8_t *buf) {
  while (stream_.available() > 0) {
    uint8_t b = (uint8_t)stream_.read();
    if (b != 0) {
      if (fill_ < sizeof(frame_)) {
        frame_[fill_++] = b;
      } else {
        overflow_ = true;
      }
      continue;
    }
    // End of a frame.
    uint8_t got = fill_;
    bool bad = overflow_;
    fill_ = 0;
    overflow_ = false;
    if (bad || got == 0) {
      continue;
    }
    uint8_t raw[DSC_MAX_MESSAGE + 1];
    uint8_t n = cobsDecode(frame_, got, raw, sizeof(raw));
    // At least a type byte and the CRC, and no longer than a message can be.
    if (n < 2 || n > DSC_MAX_MESSAGE + 1) {
      continue;
    }
    if (crc8(raw, n - 1) != raw[n - 1]) {
      continue;
    }
    memcpy(buf, raw, n - 1);
    return n - 1;
  }
  return 0;
}

void DscSerial::send(const uint8_t *msg, uint8_t len) {
  if (len == 0 || len > DSC_MAX_MESSAGE) {
    return;
  }
  uint8_t raw[DSC_MAX_MESSAGE + 1];
  memcpy(raw, msg, len);
  raw[len] = crc8(msg, len);
  len++;

  // COBS: each code byte says how far to the next zero. A message is far
  // shorter than 254 bytes, so no group ever reaches 0xFF.
  uint8_t out[DSC_MAX_MESSAGE + 3];
  uint8_t codeAt = 0;
  uint8_t o = 1;
  uint8_t code = 1;
  for (uint8_t i = 0; i < len; i++) {
    if (raw[i] == 0) {
      out[codeAt] = code;
      codeAt = o++;
      code = 1;
    } else {
      out[o++] = raw[i];
      code++;
    }
  }
  out[codeAt] = code;
  out[o++] = 0;
  stream_.write(out, o);
}

// ------------------------------------------------------------------ device

DscDevice::DscDevice(const char *vendor, const char *model, const char *unit,
                     const char *firmware, const DscLamp *lamps, uint8_t count)
    : vendor_(vendor),
      model_(model),
      unit_(unit),
      firmware_(firmware),
      lamps_(lamps),
      count_(count) {}

void DscDevice::begin(DscTransport &transport) {
  transport_ = &transport;
  if (!values_ && count_) {
    values_ = (uint8_t *)calloc(count_, 1);
  }
  for (uint8_t i = 0; i < count_; i++) {
    if (!writer_ && lamps_[i].pin != DSC_NO_PIN) {
      pinMode(lamps_[i].pin, OUTPUT);
    }
    set(i, 0);
  }
}

void DscDevice::poll() {
  if (!transport_) {
    return;
  }
  // The PC gone, unplugged or asleep: lamps off, so a board on its own
  // power does not hold a cockpit nobody is flying. Not a timer: a still
  // cockpit keeps its link.
  bool linked = transport_->linked();
  if (linked_ && !linked) {
    for (uint8_t i = 0; i < count_; i++) {
      set(i, 0);
    }
  }
  linked_ = linked;

  uint8_t m[DSC_MAX_MESSAGE];
  uint8_t n;
  while ((n = transport_->receive(m)) > 0) {
    handle(m, n);
  }
}

// Every case checks the message is long enough for its fields before
// reading one; bytes after them are from a newer version and ignored.
void DscDevice::handle(const uint8_t *m, uint8_t n) {
  if (n == 0) {
    return;
  }
  switch (m[0]) {
    case HELLO:
      if (n < 2) {
        error(m[0], TOO_SHORT);
        return;
      }
      hello();
      return;

    case DESCRIBE:
      if (n < 2) {
        error(m[0], TOO_SHORT);
        return;
      }
      describe(m[1]);
      return;

    case STATE:
      state();
      return;

    case SET_LAMPS: {
      if (n < 2 || n < 2 + 2 * m[1]) {
        error(m[0], TOO_SHORT);
        return;
      }
      // Pairs before a bad index still apply.
      for (uint8_t i = 0; i < m[1]; i++) {
        uint8_t index = m[2 + 2 * i];
        if (index >= count_) {
          error(m[0], NO_SUCH_LAMP);
          return;
        }
        set(index, m[3 + 2 * i]);
      }
      return;
    }

    case ALL_OFF:
      for (uint8_t i = 0; i < count_; i++) {
        set(i, 0);
      }
      return;

    default:
      error(m[0], UNKNOWN_TYPE);
      return;
  }
}

void DscDevice::hello() {
  uint8_t out[DSC_MAX_MESSAGE];
  out[0] = HELLO_REPLY;
  out[1] = DSC_PROTOCOL_VERSION;
  out[2] = count_;
  out[3] = 0;
  uint8_t at = 4;
  at = putString(out, at, vendor_, 12);
  at = putString(out, at, model_, 16);
  at = putString(out, at, unit_, 8);
  at = putString(out, at, firmware_, 12);
  transport_->send(out, at);
}

void DscDevice::describe(uint8_t index) {
  if (index >= count_) {
    error(DESCRIBE, NO_SUCH_LAMP);
    return;
  }
  const DscLamp &lamp = lamps_[index];
  uint8_t out[DSC_MAX_MESSAGE];
  out[0] = LAMP;
  out[1] = index;
  out[2] = lamp.kind;
  out[3] = lamp.max ? lamp.max : 1;
  out[4] = lamp.flags;
  uint8_t at = 5;
  at = putString(out, at, lamp.name, 20);
  at = putString(out, at, lamp.label, 30);
  transport_->send(out, at);
}

void DscDevice::state() {
  if (!values_) {
    error(STATE, UNKNOWN_TYPE);
    return;
  }
  uint16_t crc = crc16(values_, count_);
  uint8_t out[3] = {STATE_REPLY, (uint8_t)crc, (uint8_t)(crc >> 8)};
  transport_->send(out, 3);
}

void DscDevice::set(uint8_t index, uint8_t value) {
  const DscLamp &lamp = lamps_[index];
  uint8_t max = lamp.max ? lamp.max : 1;
  if (value > max) {
    value = max;
  }
  if (values_) {
    values_[index] = value;
  }
  if (writer_) {
    writer_(index, value);
    return;
  }
  if (lamp.pin == DSC_NO_PIN) {
    return;
  }
  if (lamp.kind == DSC_INDICATOR) {
    digitalWrite(lamp.pin, value ? HIGH : LOW);
  } else {
    // Linear across the lamp's range; a pin without PWM is on from half.
    analogWrite(lamp.pin, (uint16_t)value * 255 / max);
  }
}

void DscDevice::error(uint8_t type, uint8_t code) {
  uint8_t out[3] = {ERROR, type, code};
  transport_->send(out, 3);
}
