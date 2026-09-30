#!/usr/bin/env python3
"""Measure what `dcs-signal run` costs in CPU and memory.

  python tools/bench_daemon.py                          # A-10C, all scenarios
  python tools/bench_daemon.py --aircraft F-16C_50 --module F-16C_50
  python tools/bench_daemon.py --scenario stress --seconds 60
  python tools/bench_daemon.py --live                   # all scenarios, panels driven

No DCS needed. The tool plays a synthetic DCS-BIOS export stream onto the
multicast group (239.255.50.10:5010), shaped like the real one: a frame every
1/--hz seconds carrying the words that moved, plus the whole module map
re-exported every 300 ms. Values are random within each output's mask and
max_value, taken from the generated catalogue, so the engine sees every kind of
signal it would in flight.

Scenarios:

    idle     no stream at all; the daemon waiting for DCS
    typical  30 Hz, 20 integer outputs and 1 text field moving per frame
    stress   60 Hz, every output in the module rewritten every frame

The daemon runs with --dry-run unless --live is given. A dry run finds the
panels but never opens or writes to them, so they must be plugged in, or it
exits with nothing to drive. --live drives them for real, to check now and
then that the HID writes cost what the dry runs assume: the lamps and screens
show random values for the length of the run and are cleared at the end.
A live run covers stress along with typical, so a regression under load is
seen: the frame cap holds lamps and screens to 25 sends a second however fast
the stream runs, so stress costs the panels no more writes than typical. A
live run needs the panels to itself, so stop any daemon the editor started
first.

The daemon's own output goes to NUL, since printing it would be the thing
measured, and its log to a temporary folder, so a run never rotates the log
of the last flight.

CPU is reported as a percentage of one core, from QueryProcessCycleTime and
QueryThreadCycleTime deltas, and memory from GetProcessMemoryInfo. Cycle counts
are exact. GetProcessTimes is not: Windows charges a whole 15.6 ms clock tick
to whichever thread is running when the tick lands, so the page key readers,
which wake 100 times a second each for microseconds, read anywhere from zero
to several percent depending on how their wakes line up with the tick. A
second table splits the CPU between the main loop, each page key reader and
Windows' own threads. The first --warmup seconds are dropped so the
module-load flood and startup do not skew the steady state.
"""
import argparse
import ctypes as C
import ctypes.wintypes as W
import json
import os
import random
import re
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GROUP = ("239.255.50.10", 5010)
SYNC = b"\x55\x55\x55\x55"

SCENARIOS = {
    "idle": None,
    "typical": dict(hz=30, changes=20, strings=1, stress=False),
    "stress": dict(hz=60, changes=0, strings=0, stress=True),
}

# ---------------------------------------------------------------- stream


class Stream:
    """The module's address space and the frames that write into it."""

    def __init__(self, catalogue, aircraft):
        cat = json.load(open(catalogue))
        self.ints, self.strs = [], []
        for s in cat["signals"]:
            for o in s["outputs"]:
                if o["type"] == "integer":
                    self.ints.append(o)
                elif o["type"] == "string":
                    self.strs.append(o)
        self.mem = {}
        name = aircraft.encode().ljust(24, b"\0")
        for i in range(12):
            self.mem[2 * i] = name[2 * i] | name[2 * i + 1] << 8
        for o in self.ints:
            self.mem.setdefault(o["address"] & ~1, 0)
        for o in self.strs:
            for off in range(o["max_length"] + 1):
                self.mem.setdefault((o["address"] + off) & ~1, 0x2020)
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM, socket.IPPROTO_UDP)
        self.sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 1)
        self.sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_LOOP, 1)
        self.frames = 0
        self.datagrams = 0

    def set_int(self, o):
        v = random.randint(0, o["max_value"])
        w = self.mem[o["address"]]
        self.mem[o["address"]] = (w & ~o["mask"]) | ((v << o["shift"]) & o["mask"])
        return [o["address"]]

    def set_str(self, o):
        n = o["max_length"]
        text = bytes(random.choice(b"ABCDEFGHIJ0123456789 ") for _ in range(n))
        touched = []
        for i, b in enumerate(text):
            byte_addr = o["address"] + i
            addr = byte_addr & ~1
            w = self.mem[addr]
            if byte_addr % 2 == 0:
                self.mem[addr] = (w & 0xFF00) | b
            else:
                self.mem[addr] = (w & 0x00FF) | b << 8
            touched.append(addr)
        return touched

    def send(self, addrs):
        # Contiguous words go in one block, as DCS-BIOS itself packs them.
        # Datagrams stay under 4 KB; the listener's buffer is 8 KB.
        runs, run = [], []
        for ad in sorted(set(addrs)):
            if run and ad != run[-1] + 2:
                runs.append(run)
                run = []
            run.append(ad)
        if run:
            runs.append(run)
        pkt = SYNC
        for r in runs:
            block = struct.pack("<HH", r[0], 2 * len(r))
            block += b"".join(struct.pack("<H", self.mem[x]) for x in r)
            if len(pkt) + len(block) > 4000:
                self.sock.sendto(pkt, GROUP)
                self.datagrams += 1
                pkt = SYNC
            pkt += block
        self.sock.sendto(pkt, GROUP)
        self.datagrams += 1

    def run(self, hz, changes, strings, stress, stop):
        period = 1 / hz
        start = time.perf_counter()
        last_full = -1.0
        while not stop.is_set():
            now = time.perf_counter()
            touched = []
            if stress:
                for o in self.ints:
                    touched += self.set_int(o)
                for o in self.strs:
                    touched += self.set_str(o)
            else:
                for o in random.sample(self.ints, min(changes, len(self.ints))):
                    touched += self.set_int(o)
                for o in random.sample(self.strs, min(strings, len(self.strs))):
                    touched += self.set_str(o)
            if now - last_full >= 0.3:
                touched = list(self.mem)
                last_full = now
            self.send(touched)
            self.frames += 1
            lag = start + self.frames * period - time.perf_counter()
            if lag > 0:
                time.sleep(lag)


# ---------------------------------------------------------------- sampling

k32 = C.WinDLL("kernel32", use_last_error=True)
psapi = C.WinDLL("psapi", use_last_error=True)


class PMC(C.Structure):
    _fields_ = [
        ("cb", W.DWORD),
        ("PageFaultCount", W.DWORD),
        ("PeakWorkingSetSize", C.c_size_t),
        ("WorkingSetSize", C.c_size_t),
        ("QuotaPeakPagedPoolUsage", C.c_size_t),
        ("QuotaPagedPoolUsage", C.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", C.c_size_t),
        ("QuotaNonPagedPoolUsage", C.c_size_t),
        ("PagefileUsage", C.c_size_t),
        ("PeakPagefileUsage", C.c_size_t),
        ("PrivateUsage", C.c_size_t),
    ]


class THREADENTRY32(C.Structure):
    _fields_ = [
        ("dwSize", W.DWORD),
        ("cntUsage", W.DWORD),
        ("th32ThreadID", W.DWORD),
        ("th32OwnerProcessID", W.DWORD),
        ("tpBasePri", W.LONG),
        ("tpDeltaPri", W.LONG),
        ("dwFlags", W.DWORD),
    ]


k32.GetCurrentThread.restype = W.HANDLE
k32.OpenThread.restype = W.HANDLE
k32.CreateToolhelp32Snapshot.restype = W.HANDLE
k32.QueryProcessCycleTime.argtypes = [W.HANDLE, C.POINTER(C.c_ulonglong)]
k32.QueryThreadCycleTime.argtypes = [W.HANDLE, C.POINTER(C.c_ulonglong)]
k32.GetThreadTimes.argtypes = [W.HANDLE] + [C.POINTER(W.FILETIME)] * 4
k32.GetThreadDescription.argtypes = [W.HANDLE, C.POINTER(C.c_wchar_p)]
k32.LocalFree.argtypes = [C.c_void_p]
THREAD_QUERY_LIMITED_INFORMATION = 0x0800
TH32CS_SNAPTHREAD = 0x4


def cycle_rate():
    """Cycles per second the cycle counters run at. Windows counts them on
    the time stamp counter, which runs at a fixed rate whatever the core's
    clock, so this is measured once by spinning. The fastest of three spins
    wins, since one preempted spin reads low."""
    here = k32.GetCurrentThread()
    best = 0
    for _ in range(3):
        a, b = C.c_ulonglong(), C.c_ulonglong()
        k32.QueryThreadCycleTime(here, C.byref(a))
        t = time.perf_counter()
        while time.perf_counter() - t < 0.2:
            pass
        k32.QueryThreadCycleTime(here, C.byref(b))
        best = max(best, (b.value - a.value) / (time.perf_counter() - t))
    return best


def process_cycles(handle):
    """Every cycle the process's threads have run, exited ones included.
    Exact, unlike GetProcessTimes, which charges a whole 15.6 ms clock tick
    to whichever thread is running when the tick lands."""
    n = C.c_ulonglong()
    if not k32.QueryProcessCycleTime(handle, C.byref(n)):
        raise C.WinError(C.get_last_error())
    return n.value


class Threads:
    """The daemon's threads, each with its cycle count, named by what it
    does: the one started first is the main loop, a named one keeps its name
    (the page key readers are "keys <device>", the panel writers "write
    <device>"), and the rest are Windows' own, such as the thread pool."""

    def __init__(self, pid):
        self.pid = pid
        self.handles = {}  # thread id -> (handle, name, created)

    def refresh(self):
        snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
        e = THREADENTRY32()
        e.dwSize = C.sizeof(e)
        ok = k32.Thread32First(snap, C.byref(e))
        while ok:
            tid = e.th32ThreadID
            if e.th32OwnerProcessID == self.pid and tid not in self.handles:
                h = k32.OpenThread(THREAD_QUERY_LIMITED_INFORMATION, False, tid)
                if h:
                    self.handles[tid] = (h, self._description(h), self._created(h))
            ok = k32.Thread32Next(snap, C.byref(e))
        k32.CloseHandle(snap)

    @staticmethod
    def _description(h):
        p = C.c_wchar_p()
        if k32.GetThreadDescription(h, C.byref(p)) < 0 or not p.value:
            return ""
        name = p.value
        k32.LocalFree(C.cast(p, C.c_void_p))
        return name

    @staticmethod
    def _created(h):
        c, e, k, u = W.FILETIME(), W.FILETIME(), W.FILETIME(), W.FILETIME()
        k32.GetThreadTimes(h, C.byref(c), C.byref(e), C.byref(k), C.byref(u))
        return c.dwHighDateTime << 32 | c.dwLowDateTime

    def cycles(self):
        """Cycles so far by role: "main", each named thread, and "windows"."""
        self.refresh()
        first = min(self.handles.values(), key=lambda v: v[2])[0]
        out = {}
        for h, name, _ in self.handles.values():
            n = C.c_ulonglong()
            k32.QueryThreadCycleTime(h, C.byref(n))
            role = "main" if h == first else (name or "windows")
            out[role] = out.get(role, 0) + n.value
        return out

    def close(self):
        for h, _, _ in self.handles.values():
            k32.CloseHandle(h)


def memory(handle):
    m = PMC()
    m.cb = C.sizeof(PMC)
    if not psapi.GetProcessMemoryInfo(handle, C.byref(m), m.cb):
        raise C.WinError(C.get_last_error())
    return m


def measure(args, scenario):
    exe = args.exe
    logs = tempfile.mkdtemp(prefix="dcs-signal-bench-")
    cmd = [exe, "run", "--seconds", str(int(args.seconds + args.warmup) + 2), "--log-dir", logs]
    if not args.live:
        cmd.append("--dry-run")
    launched = time.perf_counter()
    proc = subprocess.Popen(
        cmd, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    )
    handle = int(proc._handle)
    if args.affinity:
        # Applies to every thread, those already running and those to come.
        if not k32.SetProcessAffinityMask(W.HANDLE(handle), C.c_size_t(args.affinity)):
            raise C.WinError(C.get_last_error())

    stop = threading.Event()
    stream = None
    if SCENARIOS[scenario]:
        stream = Stream(args.catalogue, args.aircraft)
        t = threading.Thread(target=stream.run, args=(*SCENARIOS[scenario].values(), stop))
        t.start()

    def exited_early():
        stop.set()
        hint = ""
        if args.live:
            hint = "; another daemon may have the panels, stop it first"
        sys.exit(f"dcs-signal exited early with code {proc.returncode}{hint}")

    rate = args.rate
    threads = Threads(proc.pid)
    time.sleep(args.warmup)
    if proc.poll() is not None:
        exited_early()
    by_role0 = threads.cycles()
    cpu0, t0 = process_cycles(handle), time.perf_counter()
    frames0 = stream.frames if stream else 0
    samples = []
    prev_cpu, prev_t = cpu0, t0
    while time.perf_counter() - t0 < args.seconds:
        time.sleep(args.interval)
        if proc.poll() is not None:
            exited_early()
        cpu, now = process_cycles(handle), time.perf_counter()
        m = memory(handle)
        samples.append(
            (100 * (cpu - prev_cpu) / rate / (now - prev_t), m.WorkingSetSize, m.PrivateUsage)
        )
        prev_cpu, prev_t = cpu, now
    cpu1, t1 = process_cycles(handle), time.perf_counter()
    by_role1 = threads.cycles()
    threads.close()
    m = memory(handle)
    frames = (stream.frames - frames0) if stream else 0
    # A thread that started inside the window counts from zero.
    percent = lambda cycles: 100 * cycles / rate / (t1 - t0)
    roles = {r: percent(n - by_role0.get(r, 0)) for r, n in by_role1.items()}

    # Let --seconds run out rather than killing it, so the daemon shuts down
    # the way it does in use.
    stop.set()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.terminate()
        proc.wait()
    ran = time.perf_counter() - launched
    status = read_status(logs)
    shutil.rmtree(logs, ignore_errors=True)

    mb = 1 / (1024 * 1024)
    return dict(
        scenario=scenario,
        fps=frames / (t1 - t0),
        cpu_avg=percent(cpu1 - cpu0),
        cpu_peak=max(s[0] for s in samples),
        roles=roles,
        ws_avg=sum(s[1] for s in samples) / len(samples) * mb,
        ws_peak=m.PeakWorkingSetSize * mb,
        private=max(s[2] for s in samples) * mb,
        ran=ran,
        sent=stream.datagrams if stream else 0,
        status=status,
    )


STATUS_TALLY = re.compile(
    r"status\s+(\d+) frame\(s\), \d+ word\(s\) in, \d+ lamp write\(s\), "
    r"\d+ paint\(s\), longest pass (\d+) ms"
)
STATUS_PANEL = re.compile(
    r"status\s+(\S+)\s+(\d+) report\(s\), (\d+) KB, writing (\d+) ms, (\d+) superseded"
)


def read_status(logs):
    """The daemon's status lines over the whole run, summed: datagrams read,
    the longest pass of its main loop, and what each panel was sent. Lines
    go out once a minute and at exit, so together they cover the run."""
    out = dict(read=0, longest=0, panels={})
    for name in os.listdir(logs):
        with open(os.path.join(logs, name), encoding="utf-8", errors="replace") as f:
            for line in f:
                if m := STATUS_TALLY.search(line):
                    out["read"] += int(m[1])
                    out["longest"] = max(out["longest"], int(m[2]))
                elif m := STATUS_PANEL.search(line):
                    p = out["panels"].setdefault(m[1], [0, 0, 0, 0])
                    for i in range(4):
                        p[i] += int(m[i + 2])
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--scenario", choices=[*SCENARIOS, "all"], default="all")
    ap.add_argument("--aircraft", default="A-10C_2", help="name written to _ACFT_NAME")
    ap.add_argument("--module", default="A-10C", help="catalogue file to draw outputs from")
    ap.add_argument("--seconds", type=float, default=30, help="measured window per scenario")
    ap.add_argument("--warmup", type=float, default=3)
    ap.add_argument("--interval", type=float, default=1, help="sample period for peaks")
    ap.add_argument(
        "--affinity",
        type=lambda s: int(s, 0),
        default=0,
        help="cores the daemon may run on, as a mask; 0xff is the P-cores of a 12900K",
    )
    ap.add_argument(
        "--live",
        action="store_true",
        help="drive the panels for real",
    )
    ap.add_argument(
        "--exe",
        default=os.path.join(ROOT, "target", "release", "dcs-signal.exe"),
        help="daemon to measure, such as an older build from a worktree",
    )
    args = ap.parse_args()
    args.catalogue = os.path.join(ROOT, "data", "catalogue", args.module + ".json")

    if not os.path.exists(args.exe):
        sys.exit(f"{args.exe} missing - build it first: cargo build --release --bin dcs-signal")
    if not os.path.exists(args.catalogue):
        sys.exit(f"{args.catalogue} missing - build it with: cargo run --bin dcs-signal -- catalogue")

    names = list(SCENARIOS) if args.scenario == "all" else [args.scenario]
    if args.live:
        print("LIVE: the panels are driven for real. Lamps and screens show random")
        print("values for the length of each run and are cleared at the end.\n")

    args.rate = cycle_rate()
    mode = "live" if args.live else "dry run"
    print(f"dcs-signal run ({mode}), {args.aircraft}, {args.seconds:g}s per scenario, "
          f"cycle counter at {args.rate / 1e6:.0f} MHz\n")
    print(f"{'scenario':<9} {'frames/s':>8} {'CPU avg':>8} {'CPU peak':>9} "
          f"{'WS avg':>8} {'WS peak':>8} {'private':>8}")
    results = []
    for name in names:
        r = measure(args, name)
        results.append(r)
        print(f"{r['scenario']:<9} {r['fps']:>8.1f} {r['cpu_avg']:>7.2f}% {r['cpu_peak']:>8.2f}% "
              f"{r['ws_avg']:>6.1f}MB {r['ws_peak']:>6.1f}MB {r['private']:>6.1f}MB", flush=True)

    # CPU by thread: the main loop decodes and works out what to send, each
    # "write" thread writes one panel, each "keys" thread reads one panel's
    # page keys, and "windows" is everything else, including threads that
    # came and went between samples.
    def named(prefix):
        return sorted({k for r in results for k in r["roles"] if k.startswith(prefix)})

    keys, writes = named("keys "), named("write ")
    print(f"\n{'scenario':<9} {'main':>7} {'write':>7} {'keys':>7}  {'windows':>7}  "
          f"keys by panel")
    for r in results:
        main_ = r["roles"].get("main", 0)
        writers = sum(r["roles"].get(k, 0) for k in writes)
        readers = sum(r["roles"].get(k, 0) for k in keys)
        rest = max(0.0, r["cpu_avg"] - main_ - writers - readers)
        each = ", ".join(f"{k[5:]} {r['roles'].get(k, 0):.3f}%" for k in keys)
        print(f"{r['scenario']:<9} {main_:>6.3f}% {writers:>6.3f}% {readers:>6.3f}% "
              f"{rest:>7.3f}%  {each}")

    # The stream against what the daemon read, over the whole run. Fewer read
    # than sent means datagrams still queued when it exited, or dropped: the
    # main loop fell behind the cockpit. Then what each panel was sent, per
    # second of the run: writing is time its writer thread was blocked on
    # USB, and superseded counts screen pieces replaced by a newer one
    # before they went, because the panel was busy or its screen not ready.
    print(f"\n{'scenario':<9} {'sent':>6} {'read':>6} {'longest pass':>13}")
    for r in results:
        s = r["status"]
        print(f"{r['scenario']:<9} {r['sent']:>6} {s['read']:>6} {s['longest']:>10} ms")
    if any(r["status"]["panels"] for r in results):
        print(f"\n{'scenario':<9} {'panel':<22} {'reports/s':>9} {'KB/s':>6} "
              f"{'writing':>10} {'  sup/s':>8}")
        for r in results:
            for key, (reports, kb, writing, superseded) in sorted(r["status"]["panels"].items()):
                per = lambda n: n / r["ran"]
                print(f"{r['scenario']:<9} {key:<22} {per(reports):>9.1f} {per(kb):>6.1f} "
                      f"{per(writing):>5.1f} ms/s {per(superseded):>8.1f}")

    print("\nCPU is percent of one core, from exact cycle counts (QueryProcessCycleTime),")
    print("not the 15.6 ms ticks GetProcessTimes counts in. WS is working set; private")
    print("is committed memory.")


if __name__ == "__main__":
    main()
