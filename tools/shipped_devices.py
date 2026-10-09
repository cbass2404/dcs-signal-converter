#!/usr/bin/env python3
"""Check that nothing added while testing a board ships.

  python tools/shipped_devices.py [--ref HEAD]

A CI and release step. Boards a user adds live in their own folder and are
never shipped, but a development checkout flies data/defaults as its active
profiles, so binding a test board there writes it into a shipped default.
Shipped, that profile names a device nobody else has, and every user's copy
of it is refused. So this refuses:

  * a device in data/devices on the "dsc" protocol: those describe
    themselves and are added by the user, so one here is a test board
  * a default profile in data/defaults naming a device data/devices lacks,
    in a binding, a lamp it mirrors, a display field, the disabled list,
    follows or screens

Reads the working tree, or with --ref the files as a commit holds them.
Exits 1 and lists each problem.
"""
import argparse
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEVICES = "data/devices"
DEFAULTS = "data/defaults"


def files(folder, ref):
    """Every .json file directly in `folder`, as (name, text)."""
    if ref:
        out = subprocess.run(["git", "ls-tree", "--name-only", ref, folder + "/"],
                             cwd=ROOT, capture_output=True, check=True, text=True).stdout
        names = [n for n in out.splitlines() if n.endswith(".json")]
        return [(n, subprocess.run(["git", "show", "%s:%s" % (ref, n)], cwd=ROOT,
                                   capture_output=True, check=True).stdout.decode("utf-8-sig"))
                for n in names]
    path = os.path.join(ROOT, folder)
    return [(folder + "/" + n, open(os.path.join(path, n), encoding="utf-8-sig").read())
            for n in sorted(os.listdir(path)) if n.endswith(".json")]


def named(profile):
    """Every device a profile names, and where."""
    for b in profile.get("bindings", []):
        yield b.get("device", ""), "lamp %s" % b.get("led", "?")
        if b.get("same_as") and b.get("same_as_device"):
            yield b["same_as_device"], "the mirror on lamp %s" % b.get("led", "?")
    for r in profile.get("readouts", []):
        if r.get("device"):
            yield r["device"], "a display field"
    for d in profile.get("disabled_devices", []):
        yield d, "the disabled list"
    for follower, source in profile.get("follows", {}).items():
        yield follower, "follows"
        yield source, "follows"
    for d in profile.get("screens", {}):
        yield d, "screens"


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--ref", help="check the files as this commit holds them")
    args = p.parse_args()

    problems = []
    known = set()
    for name, text in files(DEVICES, args.ref):
        for d in json.loads(text).get("devices", []):
            known.add(d["key"])
            if d.get("protocol") == "dsc":
                problems.append("%s: %s is a dsc board; boards are added by the user, never shipped"
                                % (name, d["key"]))

    for name, text in files(DEFAULTS, args.ref):
        profile = json.loads(text)
        seen = set()
        for device, where in named(profile):
            if device and device not in known and (device, where) not in seen:
                seen.add((device, where))
                problems.append("%s names %s (%s), which data/devices does not ship"
                                % (name, device, where))

    if not problems:
        print("ok, every shipped device is ours and every default names only those.")
        return
    print("shipped files that came from testing:")
    for line in problems:
        print("  " + line)
    print()
    print("Take the board out in the editor's Settings, which strips it from every")
    print("profile, or remove those rows by hand, then commit.")
    sys.exit(1)


if __name__ == "__main__":
    main()
