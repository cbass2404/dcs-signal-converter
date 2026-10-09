#!/usr/bin/env python3
"""List every shipped profile row and page field that moved since a release.

  python tools/changed_defaults.py                 working tree against the last tag
  python tools/changed_defaults.py --ref HEAD      what a tag of HEAD would ship
  python tools/changed_defaults.py --since v1.0.0-beta.010

The raw material for "Shipped profile changes" in CHANGELOG.md, run by
tools/release.cmd before it asks whether the notes are written. An update never
rewrites a lamp row the user has changed, and corrects a display field or page
slot only while it is still exactly as the last release shipped it, so a fix to
something a user has touched reaches them only if the notes name it. This says
what to name; the wording is still a person's.

Everything is matched on what it is, never on where it sits in the file, the
same keys the update reconciles on: a lamp row on device and lamp, a follow on
device, a page slot on device and slot number, a page on its id, a page field
on its cells, a stored signal on its id. Rows reordered, devices added and
profiles appearing all show as only what changed. Files are matched by
snapshot.py's stem, so a page file renamed only by case is the same file.

Compared as parsed JSON, so key order and line endings are not changes.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import snapshot  # noqa: E402

ROOT = snapshot.ROOT
PROFILES = "defaults"
PAGES = "default-pages"

# Longest value shown before it is cut, so a note or a replace table does not
# take over the listing. The full value is in the file.
WIDE = 90


def files_on_disk(current):
    folder = os.path.join(ROOT, "data", current)
    out = {}
    for name in snapshot.names(folder):
        try:
            out[snapshot.stem(name)] = (name, snapshot.load(os.path.join(folder, name)))
        except (OSError, ValueError) as e:
            print("ERROR: could not read data/%s/%s (%s)" % (current, name, e))
            sys.exit(1)
    return out


def files_at(ref, current):
    """Every .json file of data/<current> at a git ref, by stem."""
    out = {}
    for name in snapshot.shipped_at(ref, current):
        try:
            data = snapshot.at_tag(ref, current, name)
        except ValueError as e:
            print("ERROR: could not parse data/%s/%s at %s (%s)" % (current, name, ref, e))
            sys.exit(1)
        if data is not None:
            out[snapshot.stem(name)] = (name, data)
    return out


def show(value):
    """A value on one line, cut to WIDE."""
    text = json.dumps(value, ensure_ascii=False, sort_keys=True)
    return text if len(text) <= WIDE else text[: WIDE - 3] + "..."


def keyed_changes(was, now):
    """`key: old -> new` for each top-level key of two dicts that differs."""
    out = []
    for key in sorted(set(was) | set(now)):
        if was.get(key) == now.get(key):
            continue
        if key not in was:
            out.append("%s added: %s" % (key, show(now[key])))
        elif key not in now:
            out.append("%s removed (was %s)" % (key, show(was[key])))
        else:
            out.append("%s: %s -> %s" % (key, show(was[key]), show(now[key])))
    return out


def by(items, key):
    """A list of dicts as {key(item): item}. A key seen twice keeps the first
    and is reported, since the update would match only one of them."""
    out, dups = {}, []
    for item in items or []:
        k = key(item)
        if k in out:
            dups.append(k)
        else:
            out[k] = item
    return out, dups


class Report:
    def __init__(self):
        self.lines = []

    def heading(self, text):
        self.lines.append("")
        self.lines.append("**%s**" % text)
        self.lines.append("")

    def item(self, text, depth=0):
        self.lines.append("%s- %s" % ("  " * depth, text))


def page_names(*libraries):
    """Page id -> "name (id)" across both sides, newest name last wins."""
    out = {}
    for lib in libraries:
        for _, data in lib.values():
            for page in data.get("pages") or []:
                if "id" in page:
                    out[page["id"]] = "%s (%s)" % (page.get("name", "?"), page["id"])
    return out


def slot_text(slot, names):
    if slot is None:
        return "disabled"
    page = slot.get("page")
    text = "Blank" if page is None else names.get(page, page)
    if slot.get("key") is not None:
        text += " key %s" % show(slot["key"])
    return text


# Profile keys diffed on their own below; everything else is compared whole.
PROFILE_KEYED = {"bindings", "disabled_devices", "follows", "screens"}


def diff_profile(was, now, names):
    """Lines for one profile present on both sides, grouped by device."""
    general = keyed_changes(
        {k: v for k, v in was.items() if k not in PROFILE_KEYED},
        {k: v for k, v in now.items() if k not in PROFILE_KEYED},
    )
    devices = {}

    def at(device):
        return devices.setdefault(device, [])

    def row_key(b):
        return (b.get("device"), b.get("led"))

    old_rows, d1 = by(was.get("bindings"), row_key)
    new_rows, d2 = by(now.get("bindings"), row_key)
    for k in sorted(set(d1) | set(d2), key=str):
        general.append("duplicate row %s %s: the update matches only the first" % k)
    for k in sorted(set(old_rows) | set(new_rows), key=str):
        device, led = k
        a, b = old_rows.get(k), new_rows.get(k)
        if a == b:
            continue
        if a is None:
            at(device).append("%s added: %s" % (led, show(strip(b))))
        elif b is None:
            at(device).append("%s removed" % led)
        else:
            changes = keyed_changes(strip(a), strip(b))
            only_note = all(c.startswith("note") for c in changes)
            at(device).append("%s%s" % (led, " (note only)" if only_note else ""))
            for c in changes:
                at(device).append("    " + c)

    off_was = set(was.get("disabled_devices") or [])
    off_now = set(now.get("disabled_devices") or [])
    for device in sorted(off_now - off_was):
        at(device).append("now not driven")
    for device in sorted(off_was - off_now):
        at(device).append("now driven")

    f_was, f_now = was.get("follows") or {}, now.get("follows") or {}
    for device in sorted(set(f_was) | set(f_now)):
        a, b = f_was.get(device), f_now.get(device)
        if a == b:
            continue
        if a is None:
            at(device).append("now follows %s" % b)
        elif b is None:
            at(device).append("no longer follows %s" % a)
        else:
            at(device).append("follows %s, was %s" % (b, a))

    s_was, s_now = was.get("screens") or {}, now.get("screens") or {}
    for device in sorted(set(s_was) | set(s_now)):
        a, b = s_was.get(device) or {}, s_now.get(device) or {}
        if a == b:
            continue
        if not a:
            at(device).append("page slots added")
        elif not b:
            at(device).append("page slots removed")
        if a.get("start") != b.get("start"):
            at(device).append("start page: slot %s -> slot %s" % (a.get("start"), b.get("start")))
        old_slots, new_slots = a.get("slots") or [], b.get("slots") or []
        for i in range(max(len(old_slots), len(new_slots))):
            x = old_slots[i] if i < len(old_slots) else None
            y = new_slots[i] if i < len(new_slots) else None
            if x != y:
                at(device).append(
                    "slot %d: %s -> %s" % (i + 1, slot_text(x, names), slot_text(y, names))
                )
        rest = keyed_changes(
            {k: v for k, v in a.items() if k not in ("start", "slots")},
            {k: v for k, v in b.items() if k not in ("start", "slots")},
        )
        at(device).extend(rest)

    return general, devices


def strip(row):
    """A row without the keys that name it."""
    return {k: v for k, v in row.items() if k not in ("device", "led")}


def changes(general, devices):
    """A profile's diff as a set of single changes, (device, lines), where
    lines is one change and its indented detail. A profile-wide change has
    device None. The unit the grouping below compares."""
    out = []
    for line in general:
        out.append((None, (line,)))
    for device, lines in devices.items():
        for line in lines:
            if line.startswith("    ") and out and out[-1][0] == device:
                out[-1] = (device, out[-1][1] + (line,))
            else:
                out.append((device, (line,)))
    return out


def write_changes(report, items):
    """Profile-wide changes, then each device's, with devices whose changes
    are the same merged under one line: the three PFP Captains gaining the
    same rows read once."""
    per = {}
    for device, lines in items:
        if device is None:
            report.item(lines[0])
        else:
            per.setdefault(device, []).append(lines)
    merged = {}
    for device in sorted(per):
        merged.setdefault(tuple(per[device]), []).append(device)
    for block, devices in sorted(merged.items(), key=lambda e: e[1]):
        report.item(", ".join(devices))
        for lines in block:
            report.item(lines[0], 1)
            for detail in lines[1:]:
                report.item(detail.strip(), 2)


def diff_profiles(report, was, now, names):
    """Each changed profile, with every change made the same way in several
    profiles pulled out first under one heading, so a device added to every
    profile reads once rather than once per aircraft."""
    both = sorted(set(was) & set(now))
    changed = {}
    for key in both:
        a, b = was[key], now[key]
        if a[1] != b[1]:
            changed[key] = changes(*diff_profile(a[1], b[1], names))

    def name(key):
        return (now.get(key) or was.get(key))[1].get("name", key)

    seen = {}
    for key, items in changed.items():
        for item in items:
            seen.setdefault(item, []).append(key)

    by_keys = {}
    for item, keys in seen.items():
        if len(keys) > 1:
            by_keys.setdefault(tuple(keys), []).append(item)
    for keys in sorted(by_keys, key=lambda k: (-len(k), k)):
        if len(keys) == len(both):
            report.heading("Every profile")
        else:
            report.heading("In %d profiles: %s" % (len(keys), ", ".join(name(k) for k in keys)))
        write_changes(report, by_keys[keys])

    for key in sorted(set(was) | set(now)):
        a, b = was.get(key), now.get(key)
        if a is None:
            report.heading("%s (new, %s)" % (name(key), b[0]))
            report.item("aircraft: %s" % ", ".join(b[1].get("aircraft") or []))
            continue
        if b is None:
            report.heading("%s (no longer shipped, was %s)" % (name(key), a[0]))
            continue
        own = [i for i in changed.get(key, []) if len(seen[i]) == 1]
        if own:
            report.heading("%s (%s)" % (name(key), b[0]))
            write_changes(report, own)


def diff_pages(report, was, now):
    for key in sorted(set(was) | set(now)):
        a, b = was.get(key), now.get(key)
        if a is not None and b is not None and a[1] == b[1]:
            continue
        module = (b or a)[1].get("module", key)
        if a is None:
            report.heading("Pages: %s (new, %s)" % (module, b[0]))
            for page in b[1].get("pages") or []:
                report.item("page %s on %s" % (label(page), page.get("display")))
            for sig in b[1].get("signals") or []:
                report.item("signal %s" % label(sig))
            continue
        if b is None:
            report.heading("Pages: %s (no longer shipped, was %s)" % (module, a[0]))
            continue
        report.heading("Pages: %s (%s)" % (module, b[0]))
        general = keyed_changes(
            {k: v for k, v in a[1].items() if k not in ("pages", "signals")},
            {k: v for k, v in b[1].items() if k not in ("pages", "signals")},
        )
        for line in general:
            report.item(line)
        diff_page_list(report, a[1].get("pages"), b[1].get("pages"))
        diff_signals(report, a[1].get("signals"), b[1].get("signals"))


def label(item):
    return "%s (%s)" % (item.get("name", "?"), item.get("id", "?"))


def diff_page_list(report, was, now):
    old, d1 = by(was, lambda p: p.get("id"))
    new, d2 = by(now, lambda p: p.get("id"))
    for k in sorted(set(d1) | set(d2), key=str):
        report.item("duplicate page id %s: the update matches only the first" % k)
    for pid in sorted(set(old) | set(new), key=str):
        a, b = old.get(pid), new.get(pid)
        if a == b:
            continue
        if a is None:
            report.item("page %s added, on %s" % (label(b), b.get("display")))
            continue
        if b is None:
            report.item("page %s removed" % label(a))
            continue
        report.item("page %s" % label(b))
        for line in keyed_changes(
            {k: v for k, v in a.items() if k != "fields"},
            {k: v for k, v in b.items() if k != "fields"},
        ):
            report.item(line, 1)
        f_old, d3 = by(a.get("fields"), lambda f: f.get("cells"))
        f_new, d4 = by(b.get("fields"), lambda f: f.get("cells"))
        for k in sorted(set(d3) | set(d4), key=str):
            report.item("duplicate field at cells %s: the update matches only the first" % k, 1)
        for cells in sorted(set(f_old) | set(f_new), key=cell_order):
            x, y = f_old.get(cells), f_new.get(cells)
            if x == y:
                continue
            if x is None:
                report.item("field %s added: %s" % (cells, show(no_cells(y))), 1)
            elif y is None:
                report.item("field %s removed" % cells, 1)
            else:
                changes = keyed_changes(no_cells(x), no_cells(y))
                only_note = all(c.startswith("note") for c in changes)
                report.item("field %s%s" % (cells, " (note only)" if only_note else ""), 1)
                for c in changes:
                    report.item(c, 2)


def no_cells(field):
    return {k: v for k, v in field.items() if k != "cells"}


def cell_order(cells):
    """Fields in the order they sit on the glass: "96-119" by its first cell."""
    head = str(cells).split("-")[0].split(",")[0].strip()
    return (0, int(head), str(cells)) if head.isdigit() else (1, 0, str(cells))


def diff_signals(report, was, now):
    old, d1 = by(was, lambda s: s.get("id"))
    new, d2 = by(now, lambda s: s.get("id"))
    for k in sorted(set(d1) | set(d2), key=str):
        report.item("duplicate signal id %s: the update matches only the first" % k)
    for sid in sorted(set(old) | set(new), key=str):
        a, b = old.get(sid), new.get(sid)
        if a == b:
            continue
        if a is None:
            report.item("signal %s added" % label(b))
        elif b is None:
            report.item("signal %s removed" % label(a))
        else:
            report.item("signal %s" % label(b))
            for line in keyed_changes(a, b):
                report.item(line, 1)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument(
        "--since",
        help="the release to compare against (default: the newest v* tag)",
    )
    ap.add_argument(
        "--ref",
        help="read what ships from this git ref instead of the working tree",
    )
    args = ap.parse_args()

    since = args.since or snapshot.last_release_tag()
    if since is None:
        print("  no released tag yet, so there is nothing to compare against.")
        return 0
    if snapshot.git("rev-parse", "--verify", "-q", since + "^{commit}") is None:
        print("ERROR: %s is not a commit here." % since)
        return 1

    if args.ref:
        if snapshot.git("rev-parse", "--verify", "-q", args.ref + "^{commit}") is None:
            print("ERROR: %s is not a commit here." % args.ref)
            return 1
        profiles_now = files_at(args.ref, PROFILES)
        pages_now = files_at(args.ref, PAGES)
        what = args.ref
    else:
        profiles_now = files_on_disk(PROFILES)
        pages_now = files_on_disk(PAGES)
        what = "the working tree"
    profiles_was = files_at(since, PROFILES)
    pages_was = files_at(since, PAGES)

    report = Report()
    names = page_names(pages_was, pages_now)
    diff_profiles(report, profiles_was, profiles_now, names)
    diff_pages(report, pages_was, pages_now)

    print("Shipped profiles and pages, %s against %s:" % (what, since))
    if not report.lines:
        print("")
        print("  nothing changed.")
        return 0
    for line in report.lines:
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
