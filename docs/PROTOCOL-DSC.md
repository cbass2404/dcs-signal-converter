# DSC device protocol

**Draft, version 1.** How a device tells DCS Signal Converter what lamps it has,
and how the converter drives them. Open for anyone to implement: DIY boards
running our reference sketch, and commercial panels that want their lamps driven
from DCS without a program of their own.

The converter does the aircraft logic. A device only says what it has and sets
what it is told, so one firmware serves every aircraft and changing what a lamp
means never needs a reflash.

Version 1 covers lamps: indicators and dimmers. Displays come later, the way the
UFC followed the lamps. Buttons and switches are not carried in version 1; see
"Buttons and switches" for where they go instead.

## In one paragraph

The host sends `HELLO`; the device answers with who it is and how many lamps it
has. The host asks for each lamp with `DESCRIBE`; the device answers with its
index, kind, highest value and names. From then on the host sends `SET_LAMPS`
with index and value pairs, and the device sets them. No acknowledgements on the
hot path; the device answers with `ERROR` only when something is wrong. Every
two seconds the host asks for `STATE`, a checksum of what the device holds, and
sends every lamp again if it is not what the host sent. "Recovery" says why.

## Transports

The same messages travel over either transport. A message is a type byte
followed by its fields, never more than 62 bytes.

### HID

For native USB boards (Leonardo, Pro Micro, RP2040, ESP32-S2 and S3) and for
commercial panels that are already HID devices.

| Item            | Value                                       |
| --------------- | ------------------------------------------- |
| Usage page      | `0xFFD5` (vendor defined)                   |
| Usage           | `0x01`                                      |
| Report ID       | `0x01`, input and output                    |
| Report size     | 64 bytes including the ID, so 63 of payload |
| Feature reports | none                                        |

The host finds devices by that usage page and usage, not by vendor or product
id, so a maker adds one top-level collection to an existing descriptor and
keeps their own ids. A joystick and this collection can share a device.

Payload byte 0 is the message length `n` (1 to 62), bytes 1 to `n` are the
message, and the rest is padding the receiver ignores. Host messages go out as
output reports and device messages come back as input reports. A device sends
input reports only in answer to a host message, never on its own, so an idle
device costs the bus one `STATE` question and answer every two seconds.

### Serial

For boards without native USB (Uno, Nano, Mega) and anything with a USB serial
bridge. 115200 baud, 8N1, no flow control.

Each frame is the message followed by one CRC-8 byte, COBS-encoded and ended by
a `0x00`:

```text
COBS( message bytes ... , crc8 ) 0x00
```

The CRC is CRC-8/SMBUS (polynomial `0x07`, initial value `0x00`, no reflection,
no final XOR) over the message bytes. Its check value for the ASCII string
`123456789` is `0xF4`. A frame whose CRC fails, or which decodes to more than 63
bytes, is dropped without an answer. The host notices the silence where it
waits for an answer, and the next `STATE` catches a lost `SET_LAMPS`.

Opening the port resets most of these boards. The host waits for the device: it
sends `HELLO` every 250 ms for up to 3 seconds before giving up on the port.

**The host never probes serial ports on its own.** A COM port can be a
DCS-BIOS Arduino, a GPS or a flight controller, and bytes it did not expect can
upset it. The user chooses the port in the editor once; from then on the host
opens only ports it was given, and knows the board by its `HELLO` answer. A
board that comes back on another port is not followed there; "Recovery" says
why.

## Messages

All multi-byte numbers are little-endian. A string is one length byte followed
by that many bytes of ASCII, no terminator. Message types from the host are
`0x01` to `0x7F`; answers from the device set the top bit.

| Type   | Name          | Direction     |
| ------ | ------------- | ------------- |
| `0x01` | `HELLO`       | host → device |
| `0x81` | `HELLO_REPLY` | device → host |
| `0x02` | `DESCRIBE`    | host → device |
| `0x82` | `LAMP`        | device → host |
| `0x03` | `STATE`       | host → device |
| `0x83` | `STATE_REPLY` | device → host |
| `0x10` | `SET_LAMPS`   | host → device |
| `0x11` | `ALL_OFF`     | host → device |
| `0xFF` | `ERROR`       | device → host |

A device answers a type it does not know with `ERROR` code `0x01` and otherwise
carries on in the version it agreed. The host sends only the messages of the
agreed version, so this is a fallback for firmware that leaves out part of its
version, not the way a host finds out what a device can do. A message the
device refused had no effect, and the host does not count on it.

### Malformed messages

The serial CRC catches noise on the wire. It says nothing about whether a
message is laid out right, and HID has no CRC at all, so every receiver checks
each message against its layout before reading a field.

- A frame or report that breaks its transport's rules is dropped without an
  answer, as under "Transports": bad COBS, a failed CRC, a frame longer than
  64 bytes before its `0x00`, a report with another ID, or a length byte of 0,
  above 62 or past the end of the report.
- A receiver never reads past the end of a message. A device answers a message
  too short for its fields with `ERROR` code `0x03`, and nothing in it
  applies: a `SET_LAMPS` whose `count` promises more pairs than it holds sets
  no lamp at all.
- A string longer than its field allows makes the message invalid, as does a
  lamp name outside its characters. The host refuses a device that sends one,
  saying which field, and does not drive it.
- Bytes after the known fields are not malformed. They are from a newer
  version, and "Versions" says to ignore them.
- A sender never sends what a receiver would refuse. The host refuses its own
  message longer than 62 bytes rather than send it.

### `HELLO` and `HELLO_REPLY`

```text
HELLO        01  version:u8
HELLO_REPLY  81  version:u8  lamps:u8  flags:u8
                 vendor:str  model:str  unit:str  firmware:str
```

`version` is the highest protocol version the sender speaks, from 1 up; 0 is
not a version. Both sides then speak the lower of the two, the agreed version,
with no further message to settle it: the host knows it on reading
`HELLO_REPLY`, the device on reading `HELLO`. This document is version 1.

A device speaks version 1 from reset until its first `HELLO`, and each `HELLO`
sets the agreed version again: the host repeats `HELLO` while a serial board
resets, and a later host can be a different program. The device sends nothing
from a version newer than the agreed one, neither a message type nor anything
it would send unasked. `HELLO_REPLY` goes out before both sides agree, so its
fields up to `firmware` keep this layout in every version.

A device never stops speaking an older version, which the rule under
"Versions" makes free, so there is always a version both sides speak. Only the
host may decide a version is too old. It then leaves the device closed and says
why in its log, for example that the firmware speaks protocol 1 and the
converter needs 2. A device answering 0 is treated the same way. There is no
message for a device to refuse a host; it always answers `HELLO`.

`lamps` is how many lamps the device has, numbered 0 to `lamps - 1`. `flags` is
0 in version 1; a receiver ignores bits it does not know.

| Field      | Up to    | What it is                                                            |
| ---------- | -------- | --------------------------------------------------------------------- |
| `vendor`   | 12 bytes | Who made it: `Arduino` for our sketch, the maker's name otherwise.    |
| `model`    | 16 bytes | What it is, fixed by the firmware: `MPD Left`, `Caution Panel`.       |
| `unit`     | 8 bytes  | Tells two of the same model apart. Empty when there is only ever one. |
| `firmware` | 12 bytes | The firmware's own version, for the log. Free text.                   |

Vendor, model and unit together are the device's identity: the converter makes
its device key from them, and profiles bind lamps by that key. They must not
change between plugs, and a firmware update should keep them. On our sketch the
builder sets `model` and `unit` in the same table as the lamps.

Two devices answering with the same identity are a conflict the host reports
and does not guess at: it drives the one it found first, leaves the other
alone, and names both in its log. Which it finds first can change when ports
are renumbered, so the fix is a `unit` on one of them, never the order.

Nothing in the identity is tied to the hardware, by design. A board replaced
by another running the same firmware is the same device to the host, and every
profile binding it carries on unchanged. A new serial board may come up on
another COM port; the user adds it in the editor as they did the first, since
the host opens only ports it was given.

### `DESCRIBE` and `LAMP`

```text
DESCRIBE  02  index:u8
LAMP      82  index:u8  kind:u8  max:u8  flags:u8  name:str  label:str
```

The host sends one `DESCRIBE` per lamp, 0 to `lamps - 1`, and waits for each
answer before the next. An index past the end is `ERROR` code `0x02`.

| Field   | Meaning                                                                                         |
| ------- | ----------------------------------------------------------------------------------------------- |
| `kind`  | `0` dimmer, takes any value 0 to `max`. `1` indicator, takes 0 or `max`.                        |
| `max`   | Highest value the lamp accepts, 1 to 255. An indicator usually says 1; a dimmer usually 255.    |
| `flags` | Bit 0: a backlight, lighting legends or a feature rather than signalling. Other bits 0.         |
| `name`  | Up to 20 bytes of `A`-`Z`, `a`-`z`, `0`-`9` and `_`. Unique on the device. What profiles store. |
| `label` | Up to 30 bytes of printable ASCII. What the editor shows. Empty means use `name`.               |

The backlight flag matters: every shipped profile drives all backlights from
one source so the whole pit dims together, and a lamp flagged here joins in.

A name is the lamp's identity, as the model is the device's. Renaming a lamp in
a firmware update leaves any binding to the old name pointing at nothing, which
the editor reports. Renumbering lamps while keeping their names loses no
binding, but the host drives lamps by index and checks the numbering when it
opens the device, so it waits until the board is added again.

### `SET_LAMPS`

```text
SET_LAMPS  10  count:u8  (index:u8 value:u8) * count
```

Up to 30 pairs per message. The host sends only lamps whose value changed, and
splits a larger change across several messages.

`value` is 0 to the lamp's `max`. For a dimmer it is linear brightness as the
host intends it; mapping that onto PWM, gamma or a driver chip is the device's
business. For an indicator anything above 0 means on. A device clamps a value
above `max` rather than ignoring it.

No answer on success. An index past the end is `ERROR` code `0x02`, and the
pairs before it still apply.

The host sends at most one batch per DCS-BIOS update, and no lamps at all
while the cockpit is still. A batch is up to 9 messages back to back, for 255
lamps, and a device must take them all without dropping any: a sketch's
`loop()` can be busy while they arrive, so the device queues them or holds the
bus until it has room. Our sketch queues 15 on TinyUSB, and on a 32u4 USB holds
the next report until the last is read.

### `ALL_OFF`

```text
ALL_OFF  11
```

Every lamp to 0. The host sends it when it stops driving the device: the
aircraft changes to one whose profile leaves the device alone, or DCS exits.
No answer.

Lamps hold their value until told otherwise. A device must not turn them off
by itself on a timer: a still cockpit sends no lamps for minutes, and a device
that changed on its own would differ from what the host sent, which the next
`STATE` would put back.

Losing the link is different. A device that can tell its USB link is gone,
unplugged or the PC asleep, turns every lamp off: on its own power it would
otherwise hold a cockpit nobody is flying, with nothing left to clear it. Our
sketch does this on native USB boards. A board behind a USB serial chip cannot
tell, so it should be powered from its USB cable.

### `STATE` and `STATE_REPLY`

```text
STATE        03
STATE_REPLY  83  crc:u16
```

`crc` is CRC-16/CCITT-FALSE (polynomial `0x1021`, initial value `0xFFFF`, no
reflection, no final XOR) over every lamp's value as the device holds it, one
byte each, from lamp 0 to `lamps - 1`. A value is held as applied, clamped to
the lamp's `max`. From reset, and after `ALL_OFF`, every value is 0. The check
value for the ASCII string `123456789` is `0x29B1`.

The host computes the same over what it sent. They differ after a lost
message, a device reset or a batch partly taken, and then the host sends every
lamp again. Why a checksum and not the values: 255 lamps do not fit in one
message, and the answer stays three bytes for any device. Why 16 bits, when the
serial frame makes do with 8: a missed difference stays wrong until that lamp
next changes, perhaps the whole flight, so the 1 in 256 chance of CRC-8 is too
high.

A device that cannot hold its values answers `ERROR` code `0x01`, and the host
drives it unchecked.

### `ERROR`

```text
ERROR  FF  type:u8  code:u8
```

`type` is the message that failed.

| Code   | Meaning                        |
| ------ | ------------------------------ |
| `0x01` | Unknown message type           |
| `0x02` | Lamp index past the end        |
| `0x03` | Message too short for its type |

The host logs these and carries on driving the device.

## What the host does

For implementers of firmware this section is background; it is what the
converter promises.

- **Found, then described once.** On finding a device the host sends `HELLO`,
  then a `DESCRIBE` per lamp, then `ALL_OFF`. Only after that does it send
  `SET_LAMPS`.
- **Remembered while unplugged.** The description is saved to the user's own
  device folder, under the writable folder beside their profiles, so the
  device can be bound in the editor with it unplugged. A later description
  that differs replaces the saved one, and the editor points at any binding
  that lost its lamp.
- **Only while in use.** A device is opened when the profile for the aircraft
  in DCS drives it and closed otherwise, as every other panel is.
- **Never anything persistent.** Version 1 has no message that writes to a
  device's flash or EEPROM, and later versions keep it that way: lamp state is
  volatile and belongs to the converter.
- **Checked while in use.** Every 2 seconds the host sends `STATE` and waits
  half a second for the answer. A difference sends every lamp again in one
  batch. No answer is logged once, asked again on the same beat, and caught up
  when the device answers. Only a write that fails, as when the device is
  unplugged, closes it.

## Recovery

What happens when things go wrong, and why it was built that way. The reasons
are here so a later change does not have to find them again.

**Why not acknowledgements.** The hot path stays one way: a batch goes out and
the next DCS-BIOS update is not held up waiting for an answer. Without answers
the host cannot see a message that went missing, so a slow check covers what
acknowledgements would have: `STATE` every two seconds costs a few bytes and
puts anything wrong right within that time. Two seconds is short enough that a
wrong lamp is a blink, not a flight, and long enough to cost nothing. Half a
second to answer is far more than any device needs over USB, so a device that
misses it is stopped, not slow.

**A device disconnects while its lamps are lit.** The host sees the USB device
go, or a write fail, and forgets the device; nothing is sent to it again.
When it comes back it is described again and every lamp is sent in full, as
for a device seen for the first time, since it starts from nothing the host
sent. On the device, a native USB board turns its lamps off when it loses the
link (see `ALL_OFF`). A board behind a USB serial chip cannot tell, so on its
own power it holds its last lamps until it is reset. Powering such a board from
USB is the answer: the spec forbids a timer, and the chip gives the board no
other sign.

**A USB serial adapter comes back on another COM port.** The device is not
found there, and the editor shows the port it was added on as not plugged in.
The host does not follow it, because it could only do that by opening ports it
was not given, which the spec forbids. Following it by USB identity was
considered and dropped: Windows already keeps the COM number for an adapter
with a USB serial number, so the case that happens is a clone adapter moved to
another socket, and a clone has no unique identity to follow. The user adds
the board again on its new port.

**The firmware stops responding after it was found.** The host sees it at the
next `STATE`. On native USB a stopped device usually stops taking reports too,
a write fails, and it is handled as unplugged. Behind a USB serial chip the
chip keeps taking bytes for a sketch that no longer reads them, so writes never
fail and `STATE` is the only sign. The host logs it, keeps asking every two
seconds without blocking anything else, and sends every lamp again once the
device answers. A device that reset without its USB link dropping, a watchdog
or a brownout behind a serial chip, answers with every lamp off, and the same
check puts its lamps back.

**A `SET_LAMPS` is lost, or a batch only partly taken.** Each message stands
alone and is applied whole or not at all, so no message leaves its own lamps
half set. A whole message can still go: a frame dropped for its CRC, a serial
buffer that overflowed while `loop()` was busy, or a report that found a full
queue. Without answers the host does not know, and because it sends only
changes, those lamps would stay wrong until each next changed. The next `STATE`
differs, and every lamp is sent again.

## Buttons and switches

Version 1 carries lamps only. A board with buttons or switches gives them to
DCS another way, and which way depends on the board.

- **Native USB** (Leonardo, Pro Micro, RP2040, ESP32-S2 and S3, and any HID
  panel): the firmware adds a game controller beside this protocol's
  collection, on the same USB cable. DCS binds its buttons like any stick or
  button box, and the converter is not involved. A toggle switch is a button
  per position, which DCS's "switch to position" bindings cover for most
  modules. On Arduino this is the Joystick library on a 32u4, or a TinyUSB
  gamepad on an RP2040.
- **Serial only** (Uno, Nano, Mega): the board cannot be a game controller,
  and the usual DIY answer, the DCS-BIOS Arduino library sending commands over
  the COM port, cannot share the port with the converter: one program holds a
  COM port at a time. Today such a pit uses two boards, one running a DCS-BIOS
  sketch for its inputs and one running this protocol for its lamps.

A later version may carry inputs over serial: the board would report raw
events, input 5 on or off, and a profile would turn them into DCS-BIOS
commands, which keeps the aircraft logic in the converter. It would arrive as
new message types, which version 1 boards already answer with `ERROR` code
`0x01`, so nothing built against version 1 breaks.

## Versions

| Version | Changes      |
| ------- | ------------ |
| 1       | First draft. |

A later version adds message types or appends fields at the end of an
existing message, never changes the meaning of one already here. A receiver
ignores bytes after the fields it knows. So every version includes the ones
before it, and a device needs no reflash when the host moves on, unless the
host decides its version is too old (see `HELLO`).
