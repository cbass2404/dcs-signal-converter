#!/usr/bin/env python3
"""DSC device tool - talk to a board running the DscDevice library.

  python tools/dsc_probe.py self-test
  python tools/dsc_probe.py list
  python tools/dsc_probe.py describe --port COM5
  python tools/dsc_probe.py describe --hid
  python tools/dsc_probe.py set  --port COM5 MASTER_CAUTION=1 BACKLIGHT=128
  python tools/dsc_probe.py walk --hid [--hold 0.5]
  python tools/dsc_probe.py off  --port COM5
  python tools/dsc_probe.py state --port COM5 MASTER_CAUTION=1 BACKLIGHT=128

For testing a board before the converter drives it. Needs pyserial for
--port and hidapi for --hid (pip install pyserial hidapi).

Protocol: docs/PROTOCOL-DSC.md. Messages are a type byte and its fields.
Serial frames are COBS(message + CRC-8/SMBUS) followed by 0x00. HID reports
are report id 1, a length byte, the message, padding to 64 bytes, on usage
page 0xFFD5 usage 0x01.
"""
import argparse
import sys
import time

HELLO, DESCRIBE, STATE, SET_LAMPS, ALL_OFF = 0x01, 0x02, 0x03, 0x10, 0x11
HELLO_REPLY, LAMP, STATE_REPLY, ERROR = 0x81, 0x82, 0x83, 0xFF
VERSION = 1
MAX_MESSAGE = 62
USAGE_PAGE, USAGE = 0xFFD5, 0x01
REPORT_ID = 0x01
ERRORS = {1: "unknown message type", 2: "lamp index past the end", 3: "message too short"}


def crc8(data):
    """CRC-8/SMBUS: polynomial 0x07, initial 0, no reflection, no final XOR."""
    crc = 0
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = ((crc << 1) ^ 0x07) & 0xFF if crc & 0x80 else (crc << 1) & 0xFF
    return crc


def crc16(data):
    """CRC-16/CCITT-FALSE: polynomial 0x1021, initial 0xFFFF, no reflection, no final XOR."""
    crc = 0xFFFF
    for b in data:
        crc ^= b << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def cobs_encode(data):
    out = bytearray([0])
    code_at, code = 0, 1
    for b in data:
        if b == 0:
            out[code_at] = code
            code_at, code = len(out), 1
            out.append(0)
        else:
            out.append(b)
            code += 1
            if code == 0xFF:
                out[code_at] = code
                code_at, code = len(out), 1
                out.append(0)
    out[code_at] = code
    return bytes(out)


def cobs_decode(data):
    out, i = bytearray(), 0
    while i < len(data):
        code = data[i]
        i += 1
        if code == 0 or i + code - 1 > len(data):
            return None
        out += data[i : i + code - 1]
        i += code - 1
        if code < 0xFF and i < len(data):
            out.append(0)
    return bytes(out)


def self_test():
    assert crc8(b"123456789") == 0xF4, "CRC-8/SMBUS check value"
    assert crc16(b"123456789") == 0x29B1, "CRC-16/CCITT-FALSE check value"
    for sample in (b"\x01", b"\x00", b"\x10\x02\x00\x01\x03\xff", bytes(range(63))):
        assert cobs_decode(cobs_encode(sample)) == sample, sample
        assert 0 not in cobs_encode(sample), sample
    print("self-test ok: CRC check values 0xF4 and 0x29B1, COBS round trips")


class SerialLink:
    def __init__(self, port):
        import serial

        self.port = serial.Serial(port, 115200, timeout=0.05)
        self.pending = bytearray()

    def send(self, msg):
        self.port.write(cobs_encode(msg + bytes([crc8(msg)])) + b"\x00")

    def receive(self, timeout):
        until = time.monotonic() + timeout
        while time.monotonic() < until:
            self.pending += self.port.read(64)
            while 0 in self.pending:
                end = self.pending.index(0)
                frame, self.pending = bytes(self.pending[:end]), self.pending[end + 1 :]
                raw = cobs_decode(frame) if frame else None
                if raw and len(raw) >= 2 and crc8(raw[:-1]) == raw[-1]:
                    return raw[:-1]
        return None


class HidLink:
    def __init__(self, path):
        import hid

        self.dev = hid.device()
        self.dev.open_path(path)

    @staticmethod
    def find():
        import hid

        return [d for d in hid.enumerate() if d["usage_page"] == USAGE_PAGE and d["usage"] == USAGE]

    def send(self, msg):
        report = bytes([REPORT_ID, len(msg)]) + msg
        self.dev.write(report + bytes(64 - len(report)))

    def receive(self, timeout):
        data = self.dev.read(64, int(timeout * 1000))
        if not data:
            return None
        # Windows hands back the report id first.
        if data[0] == REPORT_ID and len(data) == 64:
            data = data[1:]
        n = data[0]
        return bytes(data[1 : 1 + n]) if 0 < n <= MAX_MESSAGE else None


def strings(msg, at, count):
    out = []
    for _ in range(count):
        n = msg[at]
        out.append(msg[at + 1 : at + 1 + n].decode("ascii", "replace"))
        at += 1 + n
    return out


def ask(link, msg, want, timeout=0.5):
    link.send(msg)
    reply = link.receive(timeout)
    if reply and reply[0] == ERROR:
        raise SystemExit(f"device error on 0x{reply[1]:02x}: {ERRORS.get(reply[2], reply[2])}")
    if not reply or reply[0] != want:
        return None
    return reply


def hello(link, wait):
    # Opening the port resets most serial boards, so ask until they answer.
    until = time.monotonic() + wait
    while time.monotonic() < until:
        reply = ask(link, bytes([HELLO, VERSION]), HELLO_REPLY, 0.25)
        if reply:
            vendor, model, unit, firmware = strings(reply, 4, 4)
            return {"version": reply[1], "lamps": reply[2], "vendor": vendor,
                    "model": model, "unit": unit, "firmware": firmware}
    raise SystemExit("no HELLO_REPLY: is the sketch running, and is it this port?")


def describe(link, count):
    lamps = []
    for i in range(count):
        reply = ask(link, bytes([DESCRIBE, i]), LAMP)
        if not reply:
            raise SystemExit(f"no answer describing lamp {i}")
        name, label = strings(reply, 5, 2)
        lamps.append({"index": reply[1], "kind": "dimmer" if reply[2] == 0 else "indicator",
                      "max": reply[3], "backlight": bool(reply[4] & 1),
                      "name": name, "label": label or name})
    return lamps


def set_lamps(link, pairs):
    for start in range(0, len(pairs), 30):
        chunk = pairs[start : start + 30]
        msg = bytes([SET_LAMPS, len(chunk)]) + bytes(b for p in chunk for b in p)
        link.send(msg)
        # Silence is success; an error comes back at once.
        reply = link.receive(0.05)
        if reply and reply[0] == ERROR:
            raise SystemExit(f"device error: {ERRORS.get(reply[2], reply[2])}")


def open_link(args):
    if args.port:
        return SerialLink(args.port), 3.0
    found = HidLink.find()
    if not found:
        raise SystemExit("no HID device on usage page 0xFFD5")
    if len(found) > 1:
        print(f"{len(found)} found, using the first; see `list`")
    return HidLink(found[0]["path"]), 1.0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("command", choices=["self-test", "list", "describe", "set", "walk", "off", "state"])
    p.add_argument("pairs", nargs="*", help="NAME=VALUE for set and state")
    how = p.add_mutually_exclusive_group()
    how.add_argument("--port", help="serial port, such as COM5")
    how.add_argument("--hid", action="store_true", help="the first HID device on usage page 0xFFD5")
    p.add_argument("--hold", type=float, default=0.5, help="seconds each lamp stays lit in walk")
    args = p.parse_args()

    if args.command == "self-test":
        self_test()
        return
    if args.command == "list":
        for d in HidLink.find():
            print(f"HID  vid 0x{d['vendor_id']:04x} pid 0x{d['product_id']:04x}  "
                  f"{d['manufacturer_string']} {d['product_string']}")
        try:
            from serial.tools import list_ports

            for port in list_ports.comports():
                print(f"port {port.device}  {port.description}")
        except ImportError:
            print("pyserial not installed: no ports listed")
        return
    if not (args.port or args.hid):
        p.error("say --port COMn or --hid")

    link, wait = open_link(args)
    who = hello(link, wait)
    print(f"{who['vendor']} {who['model']}"
          f"{' unit ' + who['unit'] if who['unit'] else ''}, firmware {who['firmware']}, "
          f"protocol {who['version']}, {who['lamps']} lamps")
    lamps = describe(link, who["lamps"])
    by_name = {lamp["name"]: lamp for lamp in lamps}

    if args.command == "describe":
        for lamp in lamps:
            print(f"  {lamp['index']:3}  {lamp['name']:<20} {lamp['kind']:<9} max {lamp['max']:<3} "
                  f"{'backlight ' if lamp['backlight'] else ''}{lamp['label']}")
    elif args.command in ("set", "state"):
        pairs = []
        for item in args.pairs:
            name, _, value = item.partition("=")
            if name not in by_name:
                raise SystemExit(f"no lamp {name!r}; see describe")
            pairs.append((by_name[name]["index"], int(value)))
        if args.command == "state":
            # From all off, so the board's STATE can be compared with what the
            # converter would expect after sending these.
            link.send(bytes([ALL_OFF]))
        set_lamps(link, pairs)
        if args.command == "state":
            values = [0] * len(lamps)
            for index, value in pairs:
                values[index] = min(value, lamps[index]["max"])
            reply = ask(link, bytes([STATE]), STATE_REPLY)
            if not reply:
                raise SystemExit("no STATE_REPLY")
            held, want = reply[1] | reply[2] << 8, crc16(bytes(values))
            print(f"STATE 0x{held:04x}, expected 0x{want:04x}: {'match' if held == want else 'DIFFERENT'}")
    elif args.command == "walk":
        # One lamp at a time, gently, so a wiring fault shows as the one that
        # stays dark.
        set_lamps(link, [(lamp["index"], 0) for lamp in lamps])
        for lamp in lamps:
            print(f"  {lamp['index']:3}  {lamp['name']}")
            set_lamps(link, [(lamp["index"], lamp["max"])])
            time.sleep(args.hold)
            set_lamps(link, [(lamp["index"], 0)])
    elif args.command == "off":
        link.send(bytes([ALL_OFF]))


if __name__ == "__main__":
    sys.exit(main())
