# Performance and install size

The daemon was measured 2026-09-30, at 1.0.0-beta.4 in development, on an
i9-12900KF with 64 GB, Windows 11, release builds from rustc 1.98.1, with
nothing else open: no browser, no chat. The live figures are from the build
that writes each panel from its own thread, sends at most 25 times a second
and paces the MCDU without pausing anything else; the dry figures are from
51b5f6b, which sent at most 30 times a second and wrote from the main loop.
The editor figures are from 2026-09-18 and the install sizes from the
1.0.0-beta.002 release.

## Summary

- The daemon uses about 35 MB of private memory and about 1.35% of one core
  in flight with the panels driven, 0.3% deciding what to send and the rest
  writing to the panels and reading their keys. Waiting for DCS it uses
  about 0.5%. Under a stream far heavier than DCS produces, every lamp and
  screen changing every frame, it uses 3.9%. On an i5-12400F, the most
  common gaming CPU, expect about 1.2 times these figures (estimated, see
  [Other CPUs](#other-cpus)).
- The editor uses about 165 MB, almost all of it WebView2. It uses no CPU while
  idle.
- The installer is 3.5 MB. Installed, the program comes to 16 MB, and the
  catalogue it builds on first run adds about 11 MB.

## The daemon

`dcs-signal run --dry-run` with the A-10C profile, against a synthetic
DCS-BIOS stream, with the UFC, ICP and Captain MCDU connected. Each scenario
was measured for 60 seconds, after 3 seconds of warmup that absorb the
startup and module-load flood, in three passes. Averages are the range across
the passes; peaks are the highest.

| Scenario | Frames/s | CPU avg | CPU peak | Working set | Private |
|---|---|---|---|---|---|
| idle | 0 | 0.35 to 0.47% | 0.56% | 40.5 to 40.9 MB | 34.2 to 34.4 MB |
| typical | 30 | 0.66 to 0.72% | 0.85% | 40.7 to 40.8 MB | 34.2 to 34.3 MB |
| stress | 60 | 1.43 to 1.46% | 1.75% | 40.6 to 40.8 MB | 34.1 to 34.4 MB |

Where the CPU goes, by thread, same passes:

| Scenario | Main loop | Page key readers | Windows' threads |
|---|---|---|---|
| idle | 0.09 to 0.11% | 0.26 to 0.36% | 0% |
| typical | 0.31 to 0.36% | 0.33 to 0.38% | 0% |
| stress | 1.10 to 1.15% | 0.31 to 0.34% | 0% |

- **idle:** no stream at all, which is the daemon waiting for DCS. It still
  reads page keys.
- **typical:** 30 frames a second. Each frame moves 20 integer outputs and
  one text field, and the whole map is re-exported every 300 ms, the same
  cycle DCS-BIOS uses (see `crates/dsc-bios`).
- **stress:** 60 frames a second, and every output in the module takes a new
  random value in every frame. Every bound lamp and every display field
  changes every frame, which no real cockpit does.

CPU is a percentage of one core, from exact cycle counts. Peaks are the
busiest 1-second sample. The three passes agree to within 0.12% in every
scenario, and no run was thrown out. Other programs open make less
difference than they did, since cycles are counted per thread: the build
before the lamp cap measured within a few hundredths of a percent with
Chrome and Discord open and closed. Measure on a quiet machine anyway, so
that one run compares with the next.

**Why these replace the 2026-09-29 figures.** Those came from
`GetProcessTimes`, which does not time threads: at each 15.6 ms clock tick
Windows charges the whole tick to whichever thread is running. Each page key
reader wakes 100 times a second, for every input report its panel sends,
changed or not, and runs for microseconds. When those wakes happened to line
up with the tick, a reader was charged whole ticks for them, and when they
did not, it was charged nothing. That is why idle read anywhere from 0.05 to
0.52%, peaked at 4.68% (3 ticks in one second) with no stream at all, and
why the 2026-09-22 runs, before page keys existed, were steady. The tool
now reads `QueryProcessCycleTime` and `QueryThreadCycleTime`, which count
every cycle a thread runs.

**The page key readers are most of idle.** The UFC, ICP and MCDU each send
100 identical input reports a second with nothing pressed, and each reader
wakes for every one. The cost is the same in every scenario, since the
reports come whether DCS is running or not.

- **Passing over repeats saves about 0.1%.** A reader used to ask Windows
  which buttons every report held. Now it compares the report with the last
  one and asks only when they differ. Measured before and after with the
  same programs open, the readers went from 0.42 to 0.50% to 0.28 to
  0.37%, and the whole daemon at idle from 0.54 to 0.41 to 0.49%. The main
  loop did not change.
- **What is left is waking up.** 0.05 to 0.11% of a core each for the UFC
  and MCDU, 0.14 to 0.17% for the ICP. Windows buffers reports, so a
  reader could wake 30 times a second and take several at once, for up to
  33 ms more before a page key acts. Not done: it would save perhaps
  another 0.2%.

**Lamps are sent at most once a frame, like the screens.** Before, a lamp
was resolved and written on every datagram that moved it, so under stress
a flickering lamp was rewritten 60 times a second. Now lamps and screens
share one frame, 33 ms when this was measured and 40 ms since (see
[Driving the panels](#driving-the-panels)): the first change after a quiet
spell goes out at once, and the rest of that frame goes out together, from
the latest state.
The build before it (babd51e), measured quiet with the same script, came
to 0.43 to 0.48% idle, 0.66 to 0.69% typical and 1.53 to 1.61% at stress,
with the main loop at 1.17 to 1.23%. So stress is about 0.1% lower and
nothing else moved, which is all a dry run can show: what the cap saves
most is lamp writes to the panels, half of them under stress, and a dry
run writes none.

### Driving the panels

The tables above are dry runs, which find the panels but never open them.
`--live` drives them, in every scenario, so that a regression under load is
seen. One pass each, quiet, three builds: 51b5f6b, in which the MCDU paused
the whole converter for 40 ms after every screen; 922ddac, which paced the
MCDU without pausing but still wrote every panel from the main loop; and
this build, which writes each panel from its own thread.

| Live | 51b5f6b | 922ddac | This build |
|---|---|---|---|
| idle | 0.51% | 0.49% | 0.47% |
| typical | 1.29% | 1.25% | 1.35% |
| stress | 2.53% | 3.81% | 3.85% |
| typical, main loop | 0.77% | 0.74% | 0.30% |
| stress, main loop | 1.74% | 2.85% | 1.00% |
| typical, datagrams read of about 1,892 sent | 1,889 | 1,889 | 1,890 |
| stress, datagrams read of 3,785 sent | 843 | 3,779 | 3,779 |
| longest main loop pass, typical | not recorded | 771 ms | 13 ms |

- **51b5f6b fell behind under stress.** It read 843 of 3,785 datagrams.
  Each MCDU screen held the whole converter for 40 ms, the stream queued up
  behind it and most of it was dropped, and on the panel the screen
  changed, froze and jumped. The same stall is what made a constantly
  changing MCDU page, the Mosquito's, lag and then catch up in flight.
  Both later builds read everything; their CPU under stress is higher
  because they do the work 51b5f6b threw away.
- **Writing moved off the main loop.** Every report to a panel blocks for
  about 1 ms, USB's polling interval, and 922ddac spent that time in the
  main loop, two thirds of every second under stress. Now each panel's
  writes happen on its own thread, `write <device>` (0.57% of a core at
  typical, 1.90% under stress, all panels together), and the main loop only
  reads, decodes and decides what to send. Total CPU is about the same;
  handing each batch to the writers costs about 0.1% at typical.
- **The pause at aircraft load is gone.** The MCDU's font, about 600
  reports, used to hold the main loop for about 0.8 s when an aircraft
  loaded. It now goes out on the MCDU's own thread. The longest pass left,
  94 ms once under stress, is probably the sweep at aircraft load, when
  every lamp and screen is worked out at once while the stream is still
  pouring in; not confirmed.
- **Stress costs more than typical because every send carries
  everything.** The frame holds sends to 25 a second however fast the
  stream runs, but under stress every lamp and screen changes every frame,
  so each send carries all of them: 94 reports a second to the PTO2
  against 4 at typical. No cockpit changes everything at once.

What each panel was sent, per second, on this build. Busy is the share of
each second its writer spent blocked on USB, which no longer holds up
anything else:

| Panel | Typical | Busy | Stress | Busy |
|---|---|---|---|---|
| MCDU Captain | 181 reports | 18% | 380 reports | 38% |
| ViperAce ICP | 16 reports | 1.6% | 224 reports | 23% |
| PTO2 | 4 reports | 0.5% | 94 reports | 10% |
| Orion Throttle | 1 report | 0.1% | 16 reports | 2% |
| CarrierAce UFC | 0.4 reports | 0% | 0.4 reports | 0% |

- **The MCDU costs most because a text screen is always sent whole**, 16
  reports. Under stress it now paints about 24 screens a second against a
  cap of 25, up from about 20 when other panels' writes held it up.
- **A panel that falls behind skips to the latest.** Its mailbox keeps only
  the newest of each lamp and screen, and 7 MCDU screens a second under
  stress were replaced by a newer one before they went. The MCDU is ready
  again 40 ms after its last screen finished; a paint that comes sooner
  waits in the mailbox, and the panel's lamps do not wait for it.
- **Key readers climb while their panel is written to**, since its input
  reports then change and a repeat can no longer be passed over: the
  MCDU's to about 0.22 to 0.31%, and the ICP's to about 0.56% under stress.
- **Private memory is about 1 MB higher live**, for the open panels.

**Real flight matches typical.** Two minutes in the Mosquito on this build,
from the log's status lines: the MCDU took 185 to 207 reports a second,
about 12 screens, against typical's 181, with its writer busy 18 to 21% of
each second. The longest main loop pass was 2 ms, aircraft load included,
so the 94 ms pass under stress belongs to that scenario and not to flying.

**The paint cap does not show up here.** Measured 2026-09-29 with the old
tick-sampled timing, and not repeated since. The build before it (52f1120)
was run alternately with 255bb80, same machine, same hour, three pairs of
idle and stress. Stress came to 2.03, 4.53 and 1.35% before and 1.98, 2.39 and
1.04% after; idle was under 0.2% for both in all but one run. That is inside
the noise. What the change saves most is HID traffic: a screen is sent at
most once a frame, and only when something on it moved. A dry run never
sends anything, so a dry run cannot measure it (see above). The main loop
waking once a frame, down from every 100 ms, costs nothing measurable at
idle.

**Memory is up about 11 MB since 2026-09-22.** Nearly all of it came before
the paint change: the build before it used 33.1 MB private, this one 34.2 MB.
The catalogue has not grown (still 51 files, 11 MB), so it is the profiles,
pages and screens added since then. Not profiled.

**What the numbers show.** CPU follows how much actually changes, not how many
frames arrive. That matches the engine's design: `BiosState::apply` reports
whether a word moved, and only words that moved are resolved against the
profile. Memory is flat across all three scenarios and did not grow during any
run.

**Where the memory goes.** This has not been profiled. The daemon loads every
module in `data/catalogue` at startup (51 files, 11 MB of JSON), and that is
the likeliest owner of most of it. Loading only the active aircraft's module
was considered and turned down: learn mode and the startup profile checks need
every module, and they are worth far more than a few megabytes.

## Other CPUs

Estimated, not measured: the only machine measured is the i9-12900KF above.

| Scenario | i9-12900KF, measured | i5-12400F, estimated |
|---|---|---|
| idle | 0.35 to 0.49% | about 0.6% |
| typical, panels driven | 1.35% | about 1.6% |
| stress, panels driven | 3.85% | about 4.6% |
| typical, dry run | 0.66 to 0.72% | about 0.9% |
| stress, dry run | 1.43 to 1.46% | about 1.8% |

- **How.** The daemon's threads each do a little, on whichever core is
  free, and none comes near filling one, so what it costs follows how fast a
  single core is, not how many there are. Both chips have the same P-cores
  (Golden Cove), and the 12400F boosts to 4.4 GHz against the 12900KF's 5.1,
  so the same work takes about 1.16 times as long. The estimate is the top
  of the measured range times 1.2.
- **Measured on P-cores, which is what the 12400F has.** The 12900KF also
  has slower E-cores, which the 12400F does not. On the build before
  repeats were passed over, pinned to the P-cores (`--affinity 0xff`), one
  pass matched the unpinned passes: idle 0.58%,
  typical 0.78%, stress 1.71%, so Windows already runs the daemon on
  P-cores. Pinned to the E-cores (`--affinity 0xff00`) it came to 0.73,
  1.01 and 2.64%. That is about the most an older or slower core would add,
  roughly 1.5 times at stress.
- **Not counted.** The 12400F has 18 MB of L3 cache against 30 MB, and runs
  hotter when DCS loads it. Neither should matter much at these loads, but
  neither has been checked.

## The editor

The editor idle with a profile open, measured across its whole process tree
after 10 seconds, on 2026-09-18.

| Process | Private memory |
|---|---|
| editor | 4.1 MB |
| msedgewebview2.exe (6 processes) | 161.2 MB |
| **Total** | **165.3 MB** |

Idle CPU was 0%.

Adding up the working sets gives 344 MB, but that overcounts: the WebView2
processes share their DLL pages, and each process's working set counts them
again. Private memory is the fair figure. WebView2's cost comes with Tauri and
does not depend on what the editor does.

## Install size

From the 1.0.0-beta.002 release, installed per user.

| Part | Size |
|---|---|
| The installer, `DCS-Signal-Converter-1.0.0-beta.002-setup.exe` | 3.5 MB |
| `DCS Signal Converter.exe`, the editor with its frontend embedded | 12.6 MB |
| `dcs-signal.exe`, the daemon | 3.4 MB |
| `data`: devices, displays, MCDU fonts, defaults and their snapshot | 2.0 MB |
| **Installed, `%LOCALAPPDATA%\DCS Signal Converter`** | **about 16 MB** |
| The catalogue, built on first run in `Saved Games\DCS Signal Converter` | about 11 MB |

- **The catalogue is not shipped.** It is generated from the DCS-BIOS
  installed on each machine, because one from another DCS-BIOS release reads
  the wrong addresses (see STATUS.md). The daemon and the editor both build it
  when it is missing or out of date, so it needs nothing else installed.
- **WebView2** is not counted. It ships with Windows 11. On Windows 10 machines
  without it, the NSIS bootstrapper downloads about 2 MB, and the runtime then
  takes about 150 MB of its own.

## Reproducing

```powershell
cargo build --release --bin dcs-signal
python tools/bench_daemon.py                  # all three scenarios, 30 s each
python tools/bench_daemon.py --seconds 60     # the tables above, three times
python tools/bench_daemon.py --scenario stress
python tools/bench_daemon.py --module F-16C_50 --aircraft F-16C_50
python tools/bench_daemon.py --affinity 0xff  # P-cores only on a 12900K
python tools/bench_daemon.py --live           # every scenario, panels driven
python tools/bench_daemon.py --exe ..\dsc-baseline\target\release\dcs-signal.exe
```

The last line measures another build, such as the commit before a change,
checked out with `git worktree add ..\dsc-baseline <commit>` and built
there, so the two are measured by the same script.

The tool needs only the Python standard library, and DCS does not need to be
running. It plays the synthetic stream onto 239.255.50.10:5010 from the
generated catalogue, starts the daemon (a dry run unless `--live`) with its
output sent to NUL and its log to a temporary folder, and samples it with
`QueryProcessCycleTime`, `QueryThreadCycleTime` and `GetProcessMemoryInfo`.
The second table it prints splits the CPU between the main loop, the panel
writers (`write <device>`), the page key readers (`keys <device>`) and
Windows' own threads. The
third compares the datagrams it sent with those the daemon read, and on a
live run a fourth says what each panel was sent, read from the daemon's
status lines before its log is deleted.

- **Plug the panels in.** A dry run never opens them, but it still looks for
  them, and with none connected it has nothing to drive and exits.
- **Close other DCS-BIOS clients first.** Anything else reading the multicast
  group, such as `dcs-signal listen` or the editor's learn mode, sees the
  synthetic stream too.
- **Close other work.** Browsers, chat and anything else that can be closed.
  A build or a video in the background still skews the figures, even
  counted in cycles, and before-and-after comparisons are only fair when
  both ran on the same quiet machine.
- **Stop the editor's daemon before `--live`.** Only one daemon can hold the
  panels, and a live run that finds another exits at once.

The editor figures were taken by hand. The tool does not measure the editor.
