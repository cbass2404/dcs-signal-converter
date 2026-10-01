#!/usr/bin/env python3
"""Regenerate docs/gauges.json, the uneven gauges and their conversions.

  python tools/gen_gauges.py [--dcs "D:/Eagle Dynamics/DCS World"] [--bios DIR]

Each DCS module lists its gauges in Cockpit/Scripts/mainpanel_init.lua: an
`input` table of values and an `output` table of needle positions, joined by
straight lines. Some set them field by field, others pass them to a helper
function; both are read. Mods installed in Saved Games are read too. DCS-BIOS sends that needle position scaled to 0-65535 by the
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
MODS_SAVED = os.path.join(os.path.expanduser("~"), "Saved Games", "DCS", "Mods", "aircraft")

# DCS module folder: (name on the page, DCS-BIOS module). A-10C_2 is the
# A-10C II; its gauges are the A-10C's. Folders are looked for in DCS's
# Mods/aircraft, then in Saved Games for mods installed there.
MODULES = {
    "A-10C_2": ("A-10C / A-10C II", "A-10C"),
    "A-4E-C": ("A-4E-C Skyhawk community mod", "A-4E-C"),
    "AH-64D": ("AH-64D", "AH-64D"),
    "AJS37": ("AJS37 Viggen", "AJS37"),
    "Bf-109K-4": ("Bf 109 K-4", "Bf-109K-4"),
    "C130J": ("C-130J", "C-130J"),
    "CH-47F": ("CH-47F", "CH-47F"),
    "Christen Eagle II": ("Christen Eagle II", "Christen Eagle II"),
    "F-16C": ("F-16C", "F-16C_50"),
    "F-5E 2024": ("F-5E", "F-5E-3"),
    "F-86": ("F-86F Sabre", "F-86F Sabre"),
    "F4U-1D": ("F4U-1D Corsair", "F4U-1D"),
    "FA-18C": ("F/A-18C / CJS Super Hornet mod", "FA-18C_hornet"),
    "FW-190A8": ("Fw 190 A-8", "FW-190A8"),
    "FW-190D9": ("Fw 190 D-9", "FW-190D9"),
    "Ka-50_3": ("Ka-50", "Ka-50"),
    "L-39C": ("L-39", "L-39"),
    "MB-339": ("MB-339", "MB-339"),
    "Mi-24P": ("Mi-24P", "Mi-24P"),
    "Mi-8MTV2": ("Mi-8MTV2", "Mi-8MT"),
    "MiG-15bis": ("MiG-15bis", "MiG-15bis"),
    "MIG-21bis": ("MiG-21bis", "MiG-21Bis"),
    "MiG-29-Fulcrum": ("MiG-29 Fulcrum", "MiG-29 Fulcrum"),
    "MosquitoFBMkVI": ("Mosquito FB VI", "Mosquito"),
    "OH-58D": ("OH-58D", "OH-58D"),
    "P-47D-30": ("P-47D", "P-47D"),
    "P-51D": ("P-51D / TF-51D", "P-51D"),
    "SA342": ("SA342 Gazelle", "SA342"),
    "SpitfireLFMkIX": ("Spitfire LF Mk IX", "SpitfireLFMkIX"),
    "Uh-1H": ("UH-1H", "UH-1H"),
    "Yak-52": ("Yak-52", "Yak-52"),
}

# DCS-BIOS modules whose Lua file is named otherwise. The module's own name is
# the signal catalogue's, which the editor matches a profile's tables by.
BIOS_FILES = {
    "MiG-29 Fulcrum": "MiG-29A",
}

# Installed modules this can't read, with the reason the page gives. The
# DCS-BIOS file, where there is one, keeps it off the not-installed list.
COMPILED = "works its gauges out in compiled code, so no Lua file holds their marks"
NOT_COVERED = {
    "C-101": ("C-101", "C-101", COMPILED),
    "F-4E": ("F-4E Phantom II", "F-4E", COMPILED),
    "F14": ("F-14A / F-14B / F-14BU", "F-14", COMPILED),
    "I-16": ("I-16", "I-16", COMPILED),
    "JF-17": ("JF-17", "JF-17", COMPILED),
    "Mirage-F1": ("Mirage F1", "MirageF1", COMPILED),
    "uh60l": ("UH-60L community mod", "MH-60R", "DCS-BIOS runs it under the MH-60R module, which sends no gauges"),
    "F-100D": ("F-100D", None, "DCS-BIOS has no module for it"),
    "La-7": ("La-7", None, "DCS-BIOS has no module for it"),
    "F-15C": ("F-15C", "FC3", "DCS-BIOS sends Flaming Cliffs aircraft's readings in real units, so they need no rows"),
    "Su-25T": ("Su-25T", "FC3", "DCS-BIOS sends Flaming Cliffs aircraft's readings in real units, so they need no rows"),
    "Su-33": ("Su-33", "FC3", "DCS-BIOS sends Flaming Cliffs aircraft's readings in real units, so they need no rows"),
}

# Installed folders that need no entry: another folder covers them, or they
# are not aircraft. The CJS Super Hornet mod builds on the FA-18C and its
# gauge tables are the FA-18C's; DCS-BIOS runs it under FA-18C_hornet.
COVERED_ELSEWHERE = {"A-10C", "TF-51D", "F14BU", "NS430", "GCBase", "CJS Super Hornet Mod v2.4 Core Module",
                     "CJS Super Hornet Mod v2.4 Player Module"}

# DCS-BIOS modules that are not one aircraft's cockpit.
NOT_AIRCRAFT = {"CommonData", "FC3", "NS430", "VNAO_Room"}

KT, KMH, MPH, FPM, DEG = 1.943844, 3.6, 2.236936, 196.850394, 180 / math.pi
LBH = 3600 / 0.45359237  # kg/s to lb/h

# (module, DCS gauge name): (unit on the dial, factor, offset, note). DCS
# values are multiplied by the factor, then the offset added, so the rows
# read in the dial's own units. A gauge missing here reads in DCS's numbers.
UNITS = {
    ("A-10C_2", "Variometer"): ("ft/min",),
    ("A-10C_2", "EngineLeftFanSpeed"): ("%",),
    ("A-10C_2", "EngineRightFanSpeed"): ("%",),
    ("A-10C_2", "OxygenPress"): ("psi",),
    ("A-4E-C", "Engine_Fuel_Flow"): ("lb/h", LBH, 0, "DCS sends kg/s"),
    ("A-4E-C", "RadarAltimeter"): ("ft",),
    ("A-4E-C", "LAWS_indexer"): ("ft",),
    ("A-4E-C", "VerticalVelocity"): ("ft/min", FPM, 0, "DCS sends m/s"),
    ("AH-64D", "ias"): ("kt",),
    ("AJS37", "IndicatedAirSpeed"): ("km/h", KMH, 0, "DCS sends m/s"),
    ("AJS37", "IndicatedAirSpeedBackup"): ("km/h", KMH, 0, "DCS sends m/s"),
    ("Bf-109K-4", "Fuel_Tank_Fuselage"): ("L",),
    ("Bf-109K-4", "Coolant_Temperature"): ("\u00b0C",),
    ("Bf-109K-4", "Oil_Temperature"): ("\u00b0C",),
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
    ("F-5E 2024", "SAI_Pitch"): ("\u00b0", DEG, 0, "DCS sends radians"),
    ("F-5E 2024", "FlowPressure"): ("psi",),
    ("F-16C", "Airspeed"): ("kt",),
    ("F-16C", "MaxAirspeed"): ("kt",),
    ("F-16C", "MachIndicator"): ("Mach",),
    ("F-16C", "VVI"): ("ft/min",),
    ("F-16C", "SysA_Pressure"): ("psi",),
    ("F-16C", "SysB_Pressure"): ("psi",),
    ("F-16C", "EngineTachometer"): ("%",),
    ("F-16C", "EngineFTIT"): ("\u00b0C",),
    ("F-16C", "OxygenPressure"): ("psi",),
    ("F-86", "AirspeeedM1"): ("kt", KT, 0, "DCS sends m/s"),
    ("F-86", "Variometer"): ("ft/min", FPM, 0, "DCS sends m/s"),
    ("F-86", "MachNumber"): ("Mach",),
    ("F-86", "Tachometer"): ("%", 100, 0, "DCS sends a fraction"),
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
    ("L-39C", "Oil_Press"): ("kgf/cm\u00b2",),
    ("L-39C", "Oil_Press_2"): ("kgf/cm\u00b2",),
    ("Mi-24P", "Variometer"): ("m/s",),
    ("Mi-24P", "GMeter"): ("g",),
    ("Mi-24P", "IAS_Pilot"): ("km/h",),
    ("Mi-24P", "IAS_Operator"): ("km/h",),
    ("Mi-24P", "UV_5_RALT"): ("m",),
    ("Mi-24P", "AntiIceCurrent"): ("A",),
    ("Mi-24P", "ELEC_Volt_AC"): ("V",),
    ("Mi-24P", "G_Meter_Min"): ("g",),
    ("Mi-24P", "oils_temp_intermediate_reductor"): ("\u00b0C",),
    ("Mi-24P", "oils_t_left_engine"): ("\u00b0C",),
    ("Mi-24P", "oils_t_right_engine"): ("\u00b0C",),
    ("Mi-24P", "RAM_Temp"): ("\u00b0C",),
    ("Mi-8MTV2", "Variometer_L"): ("m/s",),
    ("Mi-8MTV2", "Variometer_R"): ("m/s",),
    ("Mi-8MTV2", "IAS_L"): ("km/h", KMH, 0, "DCS sends m/s"),
    ("Mi-8MTV2", "IAS_R"): ("km/h", KMH, 0, "DCS sends m/s"),
    ("Mi-8MTV2", "APU_temperature"): ("\u00b0C",),
    ("Mi-8MTV2", "FuelScaleUpper"): ("L",),
    ("Mi-8MTV2", "SalonTemperature"): ("\u00b0C",),
    ("Mi-8MTV2", "oils_temp_intermediate_reductor"): ("\u00b0C",),
    ("Mi-8MTV2", "oils_t_left_engine"): ("\u00b0C",),
    ("Mi-8MTV2", "oils_t_right_engine"): ("\u00b0C",),
    ("Mi-8MTV2", "RAM_Temp"): ("\u00b0C",),
    ("Mi-8MTV2", "G_Meter"): ("g",),
    ("MiG-15bis", "Variometer"): ("m/s",),
    ("MiG-15bis", "FuelQuantity"): ("L",),
    ("MiG-15bis", "Altimeter_Pressure"): ("mmHg",),
    ("MiG-15bis", "MACH"): ("Mach",),
    ("MiG-15bis", "PRV_46_RAlt"): ("m",),
    ("MiG-15bis", "PressureDifference"): ("kgf/cm\u00b2",),
    ("MIG-21bis", "RADIO_ALTIMETER_indicator"): ("m",),
    ("MIG-21bis", "UUA_indicator"): ("\u00b0", DEG, 0, "DCS sends radians"),
    ("MIG-21bis", "DA200_VerticalVelocity"): ("m/s",),
    ("MIG-21bis", "ACCELEROMETER"): ("g",),
    ("MIG-21bis", "ENGINE_TEMP"): ("\u00b0C",),
    ("MIG-21bis", "ASP_DISTANCE_MISSILE"): ("km",),
    ("MIG-21bis", "COCKPIT_PRESSURE"): ("kgf/cm\u00b2",),
    ("MiG-29-Fulcrum", "EgtPointerLeft"): ("\u00b0C",),
    ("MiG-29-Fulcrum", "EgtPointerRight"): ("\u00b0C",),
    ("MiG-29-Fulcrum", "AOApointer"): ("\u00b0",),
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
    ("MosquitoFBMkVI", "PortOilTempGauge"): ("\u00b0C",),
    ("MosquitoFBMkVI", "StbdOilTempGauge"): ("\u00b0C",),
    ("MosquitoFBMkVI", "AirTemperatureGauge"): ("\u00b0C",),
    ("OH-58D", "IAS_Needle"): ("kt",),
    ("OH-58D", "External_Temp_Needle"): ("\u00b0C",),
    ("P-47D-30", "Carbair"): ("\u00b0C",),
    ("P-47D-30", "TriGaugeOilTemperature"): ("\u00b0C",),
    ("P-51D", "Fuel_Tank_Left"): ("gal",),
    ("P-51D", "Fuel_Tank_Right"): ("gal",),
    ("P-51D", "Fuel_Tank_Fuselage"): ("gal",),
    ("SA342", "Radar_Altimeter"): ("ft",),
    ("SA342", "DangerRALT_index"): ("ft",),
    ("SA342", "QComb"): ("L",),
    ("SA342", "Voltmetre"): ("V",),
    ("SA342", "TQuatre"): ("\u00b0C",),
    ("SpitfireLFMkIX", "FuelReserveGauge"): ("gal",),
    ("Uh-1H", "AIRSPEED_Nose"): ("kt",),
    ("Uh-1H", "AIRSPEED_Roof"): ("kt",),
    ("Uh-1H", "EngOilTemp"): ("\u00b0C",),
    ("Uh-1H", "TransmOilTemp"): ("\u00b0C",),
    ("Uh-1H", "VertVelocPilot"): ("ft/min",),
    ("Uh-1H", "VertVelocCopilot"): ("ft/min",),
    ("Uh-1H", "FuelPress"): ("psi",),
    ("Yak-52", "ManifoldTemperatureGauge"): ("\u00b0C",),
    ("Yak-52", "ForeOilPressureGauge"): ("kgf/cm\u00b2",),
    ("Yak-52", "AftOilPressureGauge"): ("kgf/cm\u00b2",),
    ("Yak-52", "ForeOilTemperatureGauge"): ("\u00b0C",),
    ("Yak-52", "AftOilTemperatureGauge"): ("\u00b0C",),
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


# Lua's math library, as far as gauge tables use it.
LUA_MATH = type("LuaMath", (), {"pi": math.pi, "huge": math.inf, "rad": staticmethod(math.radians),
                                "deg": staticmethod(math.degrees), "abs": staticmethod(abs),
                                "floor": staticmethod(math.floor), "sqrt": staticmethod(math.sqrt)})


def numbers(text, env):
    out = []
    for part in text.split(","):
        part = part.strip()
        if not part:
            continue
        try:
            out.append(float(eval(part, {"__builtins__": {}, "math": LUA_MATH}, env)))
        except Exception:
            return None
    return out


def find_mainpanel(roots, folder):
    for base in roots:
        for root, _, files in os.walk(os.path.join(base, folder, "Cockpit")):
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


def helpers(text):
    """Functions that build a gauge from their parameters, as
    {name: (arg, input, output) parameter positions}. Some modules write every
    gauge as one call to such a function rather than field by field."""
    found = {}
    for m in re.finditer(r"function\s+(\w+)\s*\(([^)]*)\)(.*?)\n\s*end\b", text, re.S):
        params = [p.strip() for p in m.group(2).split(",")]
        pos = []
        for field in ("arg_number", "input", "output"):
            f = re.search(r"\.%s\s*=\s*(\w+)\s*$" % field, m.group(3), re.M)
            pos.append(params.index(f.group(1)) if f and f.group(1) in params else None)
        if None not in pos:
            found[m.group(1)] = pos
    return found


def call_args(text, start):
    """The top-level arguments of the call whose `(` is at `start`."""
    depth, parts, cur = 0, [], start + 1
    for i in range(start, len(text)):
        c = text[i]
        if c in "({":
            depth += 1
        elif c in ")}":
            depth -= 1
            if depth == 0:
                parts.append(text[cur:i].strip())
                return parts
        elif c == "," and depth == 1:
            parts.append(text[cur:i].strip())
            cur = i + 1
    return None


def dcs_gauges(mainpanel):
    names = arg_names(mainpanel)
    with open(mainpanel, encoding="utf-8", errors="replace") as f:
        text = strip_comments(f.read())
    env = {m.group(1): float(m.group(2))
           for m in re.finditer(r"^\s*(?:local\s+)?([A-Za-z_]\w*)\s*=\s*(" + NUM + r")\s*$", text, re.M)}
    # One-line wrappers such as `rad_(v)` for math.rad(v).
    for m in re.finditer(r"function\s+(\w+)\s*\(\s*(\w+)\s*\)\s*return\s+math\.(\w+)\s*\(\s*\2\s*\)\s*end", text):
        if hasattr(LUA_MATH, m.group(3)):
            env[m.group(1)] = getattr(LUA_MATH, m.group(3))
    tables = {m.group(1): m.group(2)
              for m in re.finditer(r"^\s*(?:local\s+)?([A-Za-z_]\w*)\s*=\s*\{([^{}]*)\}", text, re.M)}

    def arg_of(expr):
        if re.fullmatch(r"\d+", expr):
            return int(expr)
        named = re.fullmatch(r"arg_int\.(\w+)", expr)
        return names.get(named.group(1)) if named else None

    def table_of(expr):
        if expr.startswith("{") and expr.endswith("}"):
            return numbers(expr[1:-1], env)
        return numbers(tables[expr], env) if expr in tables else None

    found = []
    parts = re.split(r"\n\s*([\w\.\[\]\"']+)\s*=\s*CreateGauge\s*\([^)]*\)", text)
    for name, body in zip(parts[1::2], parts[2::2]):
        arg = re.search(r"\.arg_number\s*=\s*(\w+(?:\.\w+)?)", body)
        xs = re.search(r"\.input\s*=\s*(\{[^}]*\}|\w+)", body)
        ys = re.search(r"\.output\s*=\s*(\{[^}]*\}|\w+)", body)
        if arg and xs and ys:
            found.append((name, arg_of(arg.group(1)), table_of(xs.group(1)), table_of(ys.group(1))))
    for helper, (a, i, o) in helpers(text).items():
        for m in re.finditer(r"^\s*([\w\.\[\]\"']+)\s*=\s*%s\s*\(" % helper, text, re.M):
            args = call_args(text, m.end() - 1)
            if args and len(args) > max(a, i, o):
                found.append((m.group(1), arg_of(args[a]), table_of(args[i]), table_of(args[o])))
    gauges = []
    for name, arg, xs, ys in found:
        if arg is not None and xs and ys and len(xs) == len(ys):
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
        # Two points left are always straight, so that proves nothing.
        if len(sub) >= 3 and chord_off(sub) < LINEAR:
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
    if len(bps) >= 3 and slope(bps[0], bps[1]) * 5 < slope(bps[1], bps[2]):
        ends.append("first")
    if len(bps) >= 3 and slope(bps[-2], bps[-1]) * 5 < slope(bps[-3], bps[-2]):
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


def installed(roots):
    return {d for base in roots if os.path.isdir(base) for d in os.listdir(base)
            if os.path.isdir(os.path.join(base, d, "Cockpit"))}


def collect(roots, bios_dir):
    found, left_out, empty, missing = [], [], [], []
    have = installed(roots)
    for folder in sorted(have - set(MODULES) - set(NOT_COVERED) - COVERED_ELSEWHERE):
        print("warning: %s is installed but in neither MODULES nor NOT_COVERED" % folder)
    for folder, (title, bios_name) in MODULES.items():
        mainpanel = find_mainpanel(roots, folder)
        bios_path = os.path.join(bios_dir, BIOS_FILES.get(bios_name, bios_name) + ".lua")
        if not mainpanel or not os.path.exists(bios_path):
            missing.append(bios_name)
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
    not_covered = [{"aircraft": title, "why": why} for folder, (title, _, why) in NOT_COVERED.items()
                   if folder in have]
    # DCS-BIOS modules for aircraft not installed where this ran, so not read.
    known = {BIOS_FILES.get(b, b) for _, b in MODULES.values()} | {b for _, b, _ in NOT_COVERED.values() if b}
    others = {os.path.splitext(f)[0] for f in os.listdir(bios_dir) if f.endswith(".lua")}
    missing += sorted(others - known - NOT_AIRCRAFT)
    missing += [b for folder, (_, b, _) in NOT_COVERED.items() if b and folder not in have]
    return found, left_out, empty, not_covered, sorted(set(missing), key=str.lower)


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
    ap.add_argument("--mods", default=MODS_SAVED, help="Saved Games Mods/aircraft, for mods installed there")
    args = ap.parse_args()
    bios = args.bios or (BIOS_PIN if os.path.isdir(BIOS_PIN) else BIOS_SAVED)
    roots = [os.path.join(args.dcs, "Mods", "aircraft"), args.mods]
    found, left_out, empty, not_covered, missing = collect(roots, bios)
    doc = {
        "dcs": dcs_version(args.dcs),
        "dcs_bios": bios_version(bios),
        "aircraft": found,
        "left_out": left_out,
        "none_found": empty,
        "not_covered": not_covered,
        "not_installed": missing,
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
