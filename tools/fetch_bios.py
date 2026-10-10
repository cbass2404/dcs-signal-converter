#!/usr/bin/env python3
"""Fetch the current DCS-BIOS nightly.

  python tools/fetch_bios.py [--zip PATH] [--build-catalogue]

A pipeline step. DCS-BIOS publishes nightlies as one rolling `latest`
pre-release, each replacing the last. The shipped defaults follow the nightly
as it moves, so this takes whatever `latest` holds today, unpacks it to
`target/dcs-bios`, and prints the version inside. Nothing pins it: a nightly
that drops or renames a signal the defaults read fails the tests, which is the
point of running them against it.

`--build-catalogue` then builds `data/catalogue` from it, which the tests and
`tools/nightly_only.py` read. The catalogue remembers where it was built from,
so on a development machine this repoints it away from the DCS-BIOS installed
in Saved Games; `dcs-signal --bios <that doc/json> catalogue --rebuild` puts it
back.

`--zip` unpacks a local nightly zip instead of downloading.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.request
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPO = "DCS-Skunkworks/dcs-bios"
OUT = os.path.join(ROOT, "target", "dcs-bios")


def latest_asset():
    url = "https://api.github.com/repos/%s/releases/tags/latest" % REPO
    headers = {"Accept": "application/vnd.github+json"}
    # The pipeline passes its token: unauthenticated calls share a small
    # hourly limit across every job on the runner's address.
    if os.environ.get("GITHUB_TOKEN"):
        headers["Authorization"] = "Bearer " + os.environ["GITHUB_TOKEN"]
    req = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(req) as resp:
        rel = json.load(resp)
    assets = [a for a in rel["assets"]
              if a["name"].startswith("DCS-BIOS") and a["name"].endswith(".zip")]
    if not assets:
        sys.exit("the DCS-BIOS latest release has no DCS-BIOS zip")
    return assets[0]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--zip", help="a local nightly zip instead of downloading")
    ap.add_argument("--build-catalogue", action="store_true",
                    help="build data/catalogue from it afterwards")
    args = ap.parse_args()

    shutil.rmtree(OUT, ignore_errors=True)
    os.makedirs(OUT)
    archive = args.zip
    if not archive:
        asset = latest_asset()
        archive = os.path.join(OUT, asset["name"])
        print("fetching %s" % asset["browser_download_url"], flush=True)
        urllib.request.urlretrieve(asset["browser_download_url"], archive)

    with zipfile.ZipFile(archive) as z:
        z.extractall(OUT)
    config = os.path.join(OUT, "DCS-BIOS", "BIOSConfig.lua")
    with open(config, encoding="utf-8") as f:
        found = re.search(r'version\s*=\s*"([^"]+)"', f.read())
    if not found:
        sys.exit("%s names no version" % config)

    bios_json = os.path.join(OUT, "DCS-BIOS", "doc", "json")
    print("DCS-BIOS %s at %s" % (found.group(1), bios_json), flush=True)

    if args.build_catalogue:
        cmd = ["cargo", "run", "--quiet", "--locked", "--bin", "dcs-signal", "--",
               "--bios", bios_json, "catalogue", "--rebuild"]
        # DSC_DATA names the checkout's `data` outright: a Tauri build copies
        # `data/devices` beside `target/*/dcs-signal.exe`, which then
        # takes itself for an installed copy and would build elsewhere.
        env = dict(os.environ, DSC_DATA=os.path.join(ROOT, "data"))
        out = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
        # Only the first line: the rest is a summary of every module.
        if out.returncode != 0:
            sys.stderr.write(out.stdout + out.stderr)
            sys.exit(out.returncode)
        print(out.stdout.splitlines()[0] if out.stdout else "catalogue built")


if __name__ == "__main__":
    main()
