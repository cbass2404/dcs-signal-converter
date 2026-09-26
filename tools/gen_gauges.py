#!/usr/bin/env python3
"""Regenerate docs/gauges.json, the uneven gauges and their conversions.

  python tools/gen_gauges.py [--dcs "D:/Eagle Dynamics/DCS World"] [--bios DIR]

Each DCS module lists its gauges in Cockpit/Scripts/mainpanel_init.lua: an
`input` table of values and an `output` table of needle positions, joined by
straight lines. DCS-BIOS sends that needle position scaled to 0-65535 by the
limits in its own module file, so a gauge's breakpoints in DCS-BIOS numbers are

  raw = round((output - lo) / (hi - lo) * 65535)

This reads both, keeps the gauges whose marks are uneven and that DCS-BIOS
sends, and writes their `conversions` rows per aircraft. docs/gauges.html reads
the JSON and draws the tables, so only the JSON changes when this reruns.

`--bios` is DCS-BIOS's lib/modules/aircraft_modules. It defaults to the pinned
nightly in target/dcs-bios-pin (tools/fetch_bios.py), then to Saved Games.

It is parsed data. Units are read off the dial by hand in `UNITS`, and a
module update can move any breakpoint, so rerun this after either updates and
check the gauges people use against Learn.
"""
import argparse
import json
import math
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "docs", "gauges.json")
BIOS_PIN = os.path.join(ROOT, "target", "dcs-bios-pin", "DCS-BIOS", "lib", "modules", "aircraft_modules")
BIOS_SAVED = os.path.join(os.path.expanduser("~"), "Saved Games", "DCS", "Scripts", "DCS-BIOS",
                          "lib", "modules", "aircraft_modules")

# DCS module folder: (name on the page, DCS-BIOS module file). A-10C_2 is the
# A-10C II; its gauges are the A-10C's.
MODULES = {
    "A-10C_2": ("A-10C / A-10C II", "A-10C"),
    "AJS37": ("AJS37 Viggen", "AJS37"),
    "Bf-109K-4": ("Bf 109 K-4", "Bf-109K-4"),
    "C130J": ("C-130J", "C-130J"),
    "CH-47F": ("CH-47F", "CH-47F"),
    "Christen Eagle II": ("Christen Eagle II", "Christen Eagle II"),
    "F-5E 2024": ("F-5E", "F-5E-3"),
    "F-86": ("F-86F Sabre", "F-86F Sabre"),
    "F4U-1D": ("F4U-1D Corsair", "F4U-1D"),
    "FA-18C": ("F/A-18C", "FA-18C_hornet"),
    "FW-190A8": ("Fw 190 A-8", "FW-190A8"),
    "FW-190D9": ("Fw 190 D-9", "FW-190D9"),
    "Ka-50_3": ("Ka-50", "Ka-50"),
    "L-39C": ("L-39", "L-39"),
    "MB-339": ("MB-339", "MB-339"),
    "Mi-24P": ("Mi-24P", "Mi-24P"),
    "Mi-8MTV2": ("Mi-8MTV2", "Mi-8MT"),
    "MiG-15bis": ("MiG-15bis", "MiG-15bis"),
    "MIG-21bis": ("MiG-21bis", "MiG-21Bis"),
    "MosquitoFBMkVI": ("Mosquito FB VI", "Mosquito"),
    "OH-58D": ("OH-58D", "OH-58D"),
    "P-47D-30": ("P-47D", "P-47D"),
    "P-51D": ("P-51D / TF-51D", "P-51D"),
    "SA342": ("SA342 Gazelle", "SA342"),
    "SpitfireLFMkIX": ("Spitfire LF Mk IX", "SpitfireLFMkIX"),
    "Uh-1H": ("UH-1H", "UH-1H"),
    "Yak-52": ("Yak-52", "Yak-52"),
}

KT, KMH, MPH, FPM, DEG = 1.943844, 3.6, 2.236936, 196.850394, 180 / math.pi

# (module, DCS gauge name): (unit on the dial, factor, offset, note). DCS
# values are multiplied by the factor, then the offset added, so the rows
# read in the dial's own units. A gauge missing here reads in DCS's numbers.
UNITS = {
    ("A-10C_2", "Variometer"): ("ft/min",),
    ("A-10C_2", "EngineLeftFanSpeed"): ("%",),
    ("A-10C_2", "EngineRightFanSpeed"): ("%",),
    ("AJS37", "IndicatedAirSpeed"): ("km/h", KMH, 0, "DCS sends m/s"),
    ("AJS37", "IndicatedAirSpeedBackup"): ("km/h", KMH, 0, "DCS sends m/s"),
    ("Bf-109K-4", "Fuel_Tank_Fuselage"): ("L",),
    ("Christen Eagle II", "IAS_needle"): ("mph", MPH, 0, "DCS sends m/s"),
    ("Christen Eagle II", "ACCEL_CURRENT_needle"): ("g",),
    ("Christen Eagle II", "ACCEL_MAX_needle"): ("g",),
    ("Christen Eagle II", "ACCEL_MIN_needle"): ("g",),
    ("Christen Eagle II", "OIL_temperature"): ("\u00b0F",),
    ("Christen Eagle II", "FUEL_flow"): ("gal/h",),
    ("Christen Eagle II", "EGT"): ("\u00b0F", 1.8, 32, "DCS sends \u00b0C"),
    ("Christen Eagle II", "EGT_max"): ("\u00b0F", 1.8, 32, "DCS sends \u00b0C"),
    ("F-5E 2024", "Airspeed"): ("kt",),
    ("F-5E 2024", "MaxAirspeed"): ("kt",),
    ("F-5E 2024", "MachIndicator"): ("Mach",),
    ("F-5E 2024", "Variometer"): ("ft/min",),
    ("F-5E 2024", "EGT_Left"): ("\u00b0C",),
    ("F-5E 2024", "EGT_Right"): ("\u00b0C",),
    ("F-86", "AirspeeedM1"): ("kt", KT, 0, "DCS sends m/s"),
    ("F-86", "Variometer"): ("ft/min", FPM, 0, "DCS sends m/s"),
    ("F-86", "MachNumber"): ("Mach",),
    ("F4U-1D", "IAS_INSTRUMENT"): ("kt", KT, 0, "DCS sends m/s"),
    ("F4U-1D", "VARIOMETER"): ("ft/min", FPM, 0, "DCS sends m/s"),
    ("F4U-1D", "ACCEL_CURRENT_g"): ("g",),
    ("F4U-1D", "CYLINDER_HEAD_TEMPERATURE"): ("\u00b0C",),
    ("F4U-1D", "FUEL_TANK_MAIN"): ("gal",),
    ("FA-18C", "Airspeed"): ("kt",),
    ("FA-18C", "Variometer"): ("ft/min",),
    ("FA-18C", "Altitude_Pointer_ID2163A"): ("ft",),
    ("FW-190A8", "Variometer"): ("m/s",),
    ("FW-190A8", "Engine_RPM"): ("rpm",),
    ("FW-190A8", "Coolant_Temperature"): ("\u00b0C",),
    ("FW-190A8", "Oil_Temperature"): ("\u00b0C",),
    ("FW-190A8", "FuelScaleUpper"): ("L",),
    ("FW-190D9", "AirspeedNeedle"): ("km/h",),
    ("FW-190D9", "Variometer"): ("m/s",),
    ("FW-190D9", "Engine_RPM"): ("rpm",),
    ("FW-190D9", "Coolant_Temperature"): ("\u00b0C",),
    ("FW-190D9", "Oil_Temperature"): ("\u00b0C",),
    ("FW-190D9", "FuelScaleUpper"): ("L",),
    ("FW-190D9", "TargetDist"): ("m",),
    ("Ka-50_3", "VM_15PV_BaroPressure"): ("mmHg",),
    ("Ka-50_3", "A_036_RALT"): ("m",),
    ("Ka-50_3", "A_036_DangerRALT_index"): ("m",),
    ("L-39C", "RV_5_RALT"): ("m",),
    ("L-39C", "RV_5_2_RALT"): ("m",),
    ("L-39C", "Variometer"): ("m/s",),
    ("L-39C", "Variometer_2"): ("m/s",),
    ("L-39C", "IAS"): ("km/h",),
    ("L-39C", "TAS"): ("km/h",),
    ("L-39C", "IAS_2"): ("km/h",),
    ("L-39C", "TAS_2"): ("km/h",),
    ("L-39C", "Fuel_Quantity"): ("L",),
    ("Mi-24P", "Variometer"): ("m/s",),
    ("Mi-24P", "GMeter"): ("g",),
    ("Mi-24P", "IAS_Pilot"): ("km/h",),
    ("Mi-24P", "IAS_Operator"): ("km/h",),
    ("Mi-24P", "UV_5_RALT"): ("m",),
    ("Mi-24P", "AntiIceCurrent"): ("A",),
    ("Mi-24P", "ELEC_Volt_AC"): ("V",),
    ("Mi-8MTV2", "Variometer_L"): ("m/s",),
    ("Mi-8MTV2", "Variometer_R"): ("m/s",),
    ("Mi-8MTV2", "IAS_L"): ("km/h", KMH, 0, "DCS sends m/s"),
    ("Mi-8MTV2", "IAS_R"): ("km/h", KMH, 0, "DCS sends m/s"),
    ("Mi-8MTV2", "APU_temperature"): ("\u00b0C",),
    ("Mi-8MTV2", "FuelScaleUpper"): ("L",),
    ("Mi-8MTV2", "SalonTemperature"): ("\u00b0C",),
    ("MiG-15bis", "Variometer"): ("m/s",),
    ("MiG-15bis", "FuelQuantity"): ("L",),
    ("MIG-21bis", "RADIO_ALTIMETER_indicator"): ("m",),
    ("MIG-21bis", "UUA_indicator"): ("\u00b0", DEG, 0, "DCS sends radians"),
    ("MIG-21bis", "DA200_VerticalVelocity"): ("m/s",),
    ("MIG-21bis", "ACCELEROMETER"): ("g",),
    ("MIG-21bis", "ENGINE_TEMP"): ("\u00b0C",),
    ("MIG-21bis", "ASP_DISTANCE_MISSILE"): ("km",),
    ("MIG-21bis", "COCKPIT_PRESSURE"): ("kgf/cm\u00b2",),
    ("MosquitoFBMkVI", "PortBoostGauge"): ("lb/in\u00b2",),
    ("MosquitoFBMkVI", "StbdBoostGauge"): ("lb/in\u00b2",),
    ("MosquitoFBMkVI", "PortRadTempGauge"): ("\u00b0C",),
    ("MosquitoFBMkVI", "StbdRadTempGauge"): ("\u00b0C",),
    ("MosquitoFBMkVI", "FuelGaugeInnerPort"): ("gal",),
    ("MosquitoFBMkVI", "FuelGaugeInnerStbd"): ("gal",),
    ("MosquitoFBMkVI", "FuelGaugeOuterPort"): ("gal",),
    ("MosquitoFBMkVI", "FuelGaugeOuterStbd"): ("gal",),
    ("MosquitoFBMkVI", "FuelGaugeCentral"): ("gal",),
    ("MosquitoFBMkVI", "FuelGaugeLongRange"): ("gal",),
    ("OH-58D", "IAS_Needle"): ("kt",),
    ("OH-58D", "External_Temp_Needle"): ("\u00b0C",),
    ("P-47D-30", "Carbair"): ("\u00b0C",),
    ("P-51D", "Fuel_Tank_Left"): ("gal",),
    ("P-51D", "Fuel_Tank_Right"): ("gal",),
    ("P-51D", "Fuel_Tank_Fuselage"): ("gal",),
    ("SA342", "Radar_Altimeter"): ("ft",),
    ("SA342", "DangerRALT_index"): ("ft",),
    ("SA342", "QComb"): ("L",),
    ("SA342", "Voltmetre"): ("V",),
    ("SA342", "TQuatre"): ("\u00b0C",),
    ("Uh-1H", "AIRSPEED_Nose"): ("kt",),
    ("Uh-1H", "AIRSPEED_Roof"): ("kt",),
    ("Uh-1H", "EngOilTemp"): ("\u00b0C",),
    ("Uh-1H", "TransmOilTemp"): ("\u00b0C",),
    ("Uh-1H", "VertVelocPilot"): ("ft/min",),
    ("Uh-1H", "VertVelocCopilot"): ("ft/min",),
    ("Yak-52", "ManifoldTemperatureGauge"): ("\u00b0C",),
}

# Gauges left out by hand, with the reason the page gives.
SKIP = {
    ("F-86", "Throttle"): "the throttle lever, not a gauge",
    ("F4U-1D", "FUEL_FLOW"): "shares argument 21 with cylinder head temperature in DCS, so DCS-BIOS has no fuel flow of its own",
    ("FW-190A8", "FuelScaleLower"): "a second scale on the fuel gauge's needle; the upper scale is listed",
    ("FW-190D9", "FuelScaleLower"): "a second scale on the fuel gauge's needle; the upper scale is listed",
    ("Mi-8MTV2", "FuelScaleLower"): "a second scale on the fuel gauge's needle; the upper scale is listed",
}

NUM = r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?"
LINEAR = 0.02  # a needle within 2% of a straight line counts as even


def strip_comments(text):
    text = re.sub(r"--\[(=*)\[.*?\]\1\]", "", text, flags=re.S)
    return re.sub(r"--[^\n]*", "", text)


def numbers(text, env):
    out = []
    for part in text.split(","):
        part = part.strip()
        if not part:
            continue
        try:
            out.append(float(eval(part, {"__builtins__": {}, "math": math}, env)))
        except Exception:
            return None
    return out


def find_mainpanel(dcs, folder):
    for root, _, files in os.walk(os.path.join(dcs, "Mods", "aircraft", folder, "Cockpit")):
        for f in files:
            if f.lower() == "mainpanel_init.lua":
                return os.path.join(root, f)
    return None


def arg_names(mainpanel):
    """The arg_int table some modules name their arguments from."""
    cockpit = mainpanel
    while os.path.basename(cockpit).lower() != "cockpit":
        cockpit = os.path.dirname(cockpit)
    path = os.path.join(cockpit, "arg_int.lua")
    if not os.path.exists(path):
        return {}
    with open(path, encoding="utf-8", errors="replace") as f:
        return {m.group(1): int(m.group(2)) for m in re.finditer(r"(\w+)\s*=\s*(\d+)", strip_comments(f.read()))}


def dcs_gauges(mainpanel):
    names = arg_names(mainpanel)
    with open(mainpanel, encoding="utf-8", errors="replace") as f:
        text = strip_comments(f.read())
    env = {m.group(1): float(m.group(2))
           for m in re.finditer(r"^\s*(?:local\s+)?([A-Za-z_]\w*)\s*=\s*(" + NUM + r")\s*$", text, re.M)}
    parts = re.split(r"\n\s*([\w\.\[\]\"']+)\s*=\s*CreateGauge\s*\([^)]*\)", text)
    gauges = []
    for name, body in zip(parts[1::2], parts[2::2]):
        arg = re.search(r"\.arg_number\s*=\s*(\d+)", body)
        if arg:
            arg = int(arg.group(1))
        else:
            named = re.search(r"\.arg_number\s*=\s*arg_int\.(\w+)", body)
            arg = names.get(named.group(1)) if named else None
        xs = re.search(r"\.input\s*=\s*\{([^}]*)\}", body)
        ys = re.search(r"\.output\s*=\s*\{([^}]*)\}", body)
        if arg is None or not xs or not ys:
            continue
        xs, ys = numbers(xs.group(1), env), numbers(ys.group(1), env)
        if xs and ys and len(xs) == len(ys):
            gauges.append((name.strip(), arg, list(zip(xs, ys))))
    return gauges


def bios_floats(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        text = f.read()
    found = {}
    pat = re.compile(r":define(?:8Bit)?Float\(\s*\"([^\"]+)\"\s*,\s*(\d+)\s*,\s*\{\s*(" + NUM + r")\s*,\s*("
                     + NUM + r")\s*\}\s*,\s*(?:\"[^\"]*\"|[\w\.]+)\s*,\s*\"([^\"]*)\"")
    for m in pat.finditer(text):
        found.setdefault(int(m.group(2)), []).append(
            (m.group(1), float(m.group(3)), float(m.group(4)), m.group(5)))
    return found


def chord_off(points):
    (x0, y0), (x1, y1) = points[0], points[-1]
    if x1 == x0 or y1 == y0:
        return 1.0
    return max(abs(y0 + (y1 - y0) * (x - x0) / (x1 - x0) - y) for x, y in points) / abs(y1 - y0)


def shape(points):
    """even, uneven, wraps, folded, or switch.

    Rows only need the needle in order; what it reads can jump between them.
    So a needle that runs the other way, or wraps past the end of the dial
    once, is still rows. One that points the same way for two values (an
    ammeter that can't show the sign) is folded, and one that jumps about is
    a switch's positions rather than a gauge.
    """
    pts = sorted(points)
    if len({x for x, _ in pts}) != len(pts):
        return "step"
    ys = [y for _, y in pts]
    if not (all(b >= a for a, b in zip(ys, ys[1:])) or all(b <= a for a, b in zip(ys, ys[1:]))):
        if len(set(ys)) != len(ys):
            return "folded"
        ranks = [i for i, _ in sorted(enumerate(ys), key=lambda p: p[1])]
        breaks = sum(1 for a, b in zip(ranks, ranks[1:]) if abs(b - a) != 1)
        return "wraps" if breaks <= 1 else "switch"
    if len(pts) < 3 or chord_off(pts) < LINEAR:
        return "even"
    for lo, hi in ((1, 0), (0, 1), (1, 1)):
        sub = pts[lo:len(pts) - hi]
        if len(sub) >= 2 and (len(sub) < 3 or chord_off(sub) < LINEAR):
            return "even"  # straight apart from a peg at one end
    return "uneven"


def pick(defs, gauge):
    """DCS-BIOS sometimes gives one argument several names; take the likeliest."""
    words = set(re.findall(r"[a-z]+", gauge.lower()))
    return max(defs, key=lambda d: len(words & set(re.findall(r"[a-z]+", (d[0] + " " + d[3]).lower()))))


def tidy(v):
    v = round(v, 3 if abs(v) < 10 else 1 if abs(v) < 1000 else 0)
    return int(v) if v == int(v) else v


def breakpoints(points, lo, hi, unit):
    """(raw, reads, place in DCS's table) per point, in the order of raw."""
    factor, offset = (unit[1], unit[2]) if len(unit) > 1 else (1, 0)
    pts = []
    for rank, (x, y) in enumerate(sorted(points)):
        raw = round(min(1.0, max(0.0, (y - lo) / (hi - lo))) * 65535)
        pts.append((raw, tidy(x * factor + offset), rank))
    pts.sort(key=lambda p: p[0])
    # Values past DCS-BIOS's limits all send the same number: keep the one
    # nearest the scale.
    out = []
    for p in pts:
        if out and out[-1][0] == p[0]:
            if p[0] == 0:
                out[-1] = p
            continue
        out.append(p)
    return out


def pegged(bps):
    """Which ends are a peg: a stretch of needle travel worth almost nothing."""
    def slope(a, b):
        return abs(b[1] - a[1]) / max(1, b[0] - a[0])
    ends = []
    if len(bps) > 3 and slope(bps[0], bps[1]) * 5 < slope(bps[1], bps[2]):
        ends.append("first")
    if len(bps) > 3 and slope(bps[-2], bps[-1]) * 5 < slope(bps[-3], bps[-2]):
        ends.append("last")
    return ends


def straight_off(bps):
    (r0, v0, _), (r1, v1, _) = bps[0], bps[-1]
    span = abs(v1 - v0) or 1
    return max(abs(v0 + (v1 - v0) * (r - r0) / (r1 - r0) - v) for r, v, _ in bps) / span * 100


def rows(bps):
    """A row between two points next to each other in DCS's table too.

    Where a needle wraps, the two points either side of the jump are next to
    each other in raw but not in the table, and the needle never points
    between them, so that stretch is left without a row.
    """
    out = []
    for (ra, va, ka), (rb, vb, kb) in zip(bps, bps[1:]):
        if abs(kb - ka) != 1:
            continue
        start = ra + 1 if out and out[-1]["raw"][1] == ra else ra
        out.append({"raw": [start, rb], "reads": [va, vb]})
    return out


def collect(dcs, bios_dir):
    found, left_out, empty = [], [], []
    for folder, (title, bios_name) in MODULES.items():
        mainpanel = find_mainpanel(dcs, folder)
        bios_path = os.path.join(bios_dir, bios_name + ".lua")
        if not mainpanel or not os.path.exists(bios_path):
            continue
        floats = bios_floats(bios_path)
        seen, gauges = set(), []
        for name, arg, points in dcs_gauges(mainpanel):
            if (arg, name) in seen:
                continue
            seen.add((arg, name))
            kind = shape(points)
            if kind == "folded":
                # The needle can't show the sign, so it reads the magnitude.
                points = [p for p in points if p[0] >= 0]
                kind = shape(points)
            if kind == "even" or kind == "step":
                continue
            defs = floats.get(arg)
            why = SKIP.get((folder, name))
            if not why and kind == "switch":
                why = "a switch's positions, not a gauge; draw them with value_aliases"
            if not why and not defs:
                why = "DCS-BIOS sends no gauge for argument %d" % arg
            if why:
                left_out.append({"aircraft": title, "dcs_gauge": name, "why": why})
                continue
            ident, lo, hi, desc = pick(defs, name)
            unit = UNITS.get((folder, name), ("DCS units",))
            bps = breakpoints(points, lo, hi, unit)
            notes = []
            if len(unit) > 3:
                notes.append(unit[3] + ", converted")
            if (lo, hi) != (0, 1):
                notes.append("DCS-BIOS limits {%g, %g}" % (lo, hi))
            ends = pegged(bps) if kind == "uneven" else []
            if kind == "wraps":
                notes.append("The needle wraps past the end of the dial; the counts it never shows have no row")
            if ends:
                notes.append("%s row is the needle's rest off the scale" % " and ".join(ends).capitalize())
            gauges.append({"id": ident, "description": desc, "unit": unit[0], "notes": notes,
                           "straight_off_pct": round(straight_off(bps)), "dcs_gauge": name, "argument": arg,
                           "conversions": rows(bps)})
        if gauges:
            found.append({"aircraft": title, "bios_module": bios_name, "gauges": gauges})
        else:
            empty.append(title)
    return found, left_out, empty


def dcs_version(dcs):
    try:
        with open(os.path.join(dcs, "autoupdate.cfg"), encoding="utf-8") as f:
            return json.load(f).get("version", "unknown")
    except (OSError, ValueError):
        return "unknown"


def bios_version(bios_dir):
    config = os.path.normpath(os.path.join(bios_dir, "..", "..", "..", "BIOSConfig.lua"))
    try:
        with open(config, encoding="utf-8") as f:
            found = re.search(r'version\s*=\s*"([^"]+)"', f.read())
        return found.group(1) if found else "unknown"
    except OSError:
        return "unknown"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dcs", default="D:/Eagle Dynamics/DCS World", help="the DCS World install")
    ap.add_argument("--bios", help="DCS-BIOS lib/modules/aircraft_modules")
    args = ap.parse_args()
    bios = args.bios or (BIOS_PIN if os.path.isdir(BIOS_PIN) else BIOS_SAVED)
    found, left_out, empty = collect(args.dcs, bios)
    doc = {
        "dcs": dcs_version(args.dcs),
        "dcs_bios": bios_version(bios),
        "aircraft": found,
        "left_out": left_out,
        "none_found": empty,
    }
    with open(OUT, "w", encoding="utf-8", newline="\r\n") as f:
        f.write(dumps(doc) + "\n")
    print("%d gauges in %d aircraft, %d left out -> %s"
          % (sum(len(a["gauges"]) for a in found), len(found), len(left_out), os.path.relpath(OUT, ROOT)))


def dumps(doc):
    """Indented, but each conversions row on one line, as the profiles read."""
    text = json.dumps(doc, indent=2, ensure_ascii=False)
    return re.sub(r'\{\s+"raw": \[\s+([-\d.]+),\s+([-\d.]+)\s+\],\s+"reads": \[\s+([-\d.]+),\s+([-\d.]+)\s+\]\s+\}',
                  r'{"raw": [\1, \2], "reads": [\3, \4]}', text)


if __name__ == "__main__":
    main()
