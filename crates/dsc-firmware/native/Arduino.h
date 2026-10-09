// Just enough of the Arduino core to build DscDevice on a PC, for tests. Pin
// writes are recorded by shim.cpp, and time stands still.

#pragma once

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define OUTPUT 1
#define HIGH 1
#define LOW 0

class Stream {
public:
  virtual ~Stream() {}
  virtual int available() = 0;
  virtual int read() = 0;
  virtual size_t write(const uint8_t *buf, size_t len) = 0;
};

void pinMode(uint8_t pin, uint8_t mode);
void digitalWrite(uint8_t pin, uint8_t value);
void analogWrite(uint8_t pin, int value);
unsigned long millis();
void delay(unsigned long ms);
void yield();
