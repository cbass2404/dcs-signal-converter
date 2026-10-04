# Performance and install size

The daemon was measured 2026-10-04, at 1.0.0-beta.008 in development, on an
i9-12900KF with 64 GB, Windows 11, release builds from rustc 1.98.1, with
nothing else open: no browser, no chat. Every figure is from the panels
driven for real; the benchmark has no dry run, since writing to the panels
costs more than deciding what to send and a dry run cannot see it. The editor
figures are from 2026-09-18 and the install sizes from the 1.0.0-beta.002
release.

## Summary

- The daemon uses about 45 MB of private memory and about 1.3% of one core
  in flight, 0.3% deciding what to send and the rest writing to the panels
  and reading their keys. Waiting for DCS it uses about 0.5%. Under a
  stream far heavier than DCS produces, every lamp and screen changing
  every frame, it uses 3.6%. On an i5-12400F, the most common gaming CPU,
  expect about 1.2 times these figures (estimated, see
  [Other CPUs](#other-cpus)).
- The editor uses about 165 MB, almost all of it WebView2. It uses no CPU while
  idle.
- The installer is 3.5 MB. Installed, the program comes to 16 MB, and the
  catalogue it builds on first run adds about 11 MB.

## The daemon

`dcs-signal run` with the A-10C II profile, against a synthetic DCS-BIOS
stream, with every panel connected and driven. Each scenario was measured
for 60 seconds, after 3 seconds of warmup that absorb the startup and
module-load flood, in one pass. Peaks are the busiest 1-second sample.

| Scenario | Frames/s | CPU avg | CPU peak | Working set | Private |
|---|---|---|---|---|---|
| idle | 0 | 0.47% | 0.56% | 50.5 MB | 44.6 MB |
| typical | 30 | 1.28% | 2.20% | 51.4 MB | 45.6 MB |
| stress | 60 | 3.62% | 4.18% | 54.1 MB | 47.9 MB |

Where the CPU goes, by thread, same pass:

| Scenario | Main loop | Panel writers | Page key readers | Windows' threads |
|---|---|---|---|---|
| idle | 0.13% | 0% | 0.33% | 0% |
| typical | 0.33% | 0.46% | 0.49% | 0% |
| stress | 1.07% | 1.63% | 0.92% | 0% |

- **idle:** no stream at all, which is the daemon waiting for DCS. It still
  reads page keys, and writes nothing.
- **typical:** 30 frames a second. Each frame moves 20 integer outputs and
  one text field, and the whole map is re-exported every 300 ms, the same
  cycle DCS-BIOS uses (see `crates/dsc-bios`).
- **stress:** 60 frames a second, and every output in the module takes a new
  random value in every frame. Every bound lamp and every display field
  changes every frame, which no real cockpit does.

CPU is a percentage of one core, from exact cycle counts. Other programs
open make less difference than they did, since cycles are counted per
thread, but measure on a quiet machine anyway, so that one run compares
with the next.

**Why cycles and not process times.** Figures before 2026-09-30 came from
`GetProcessTimes`, which does not time threads: at each 15.6 ms clock tick
Windows charges the whole tick to whichever thread is running. Each page key
reader wakes 100 times a second, for every input report its panel sends,
changed or not, and runs for microseconds. When those wakes happened to line
up with the tick, a reader was charged whole ticks for them, and when they
did not, it was charged nothing. That is why idle once read anywhere from
0.05 to 0.52% and peaked at 4.68% with no stream at all. The same noise hid
what writing to the panels costs, which is why the benchmark dropped live
runs on 2026-09-22 and took them back once cycles were counted. The tool now
reads `QueryProcessCycleTime` and `QueryThreadCycleTime`, which count every
cycle a thread runs.

**The page key readers are most of idle.** The UFC, ICP and MCDU each send
100 identical input reports a second with nothing pressed, and each reader
wakes for every one, whether DCS is running or not.

- **Repeats are passed over.** A reader used to ask Windows which buttons
  every report held. Now it compares the report with the last one and asks
  only when they differ, which cannot lose a press: the same bytes hold the
  same keys. Measured before and after, the readers came down by about
  0.1% of a core together.
- **What is left is waking up.** At idle, 0.09 to 0.13% of a core each for
  the UFC, ICP and MCDU. Windows buffers reports, so a reader could wake 30
  times a second and take several at once, for up to 33 ms more before a
  page key acts. Not done: it would save perhaps another 0.2%.
- **They climb while their panel is written to**, since its input reports
  then change and a repeat can no longer be passed over: the MCDU's to
  0.21% at typical and 0.33% under stress, the ICP's to 0.51% under stress.

**Lamps and screens are sent at most once a frame**, 40 ms, 25 times a
second. The first change after a quiet spell goes out at once, and the rest
of that frame goes out together, from the latest state. Before, a lamp was
resolved and written on every datagram that moved it, so under stress a
flickering lamp was rewritten 60 times a second.

### How the builds compare

One pass each, quiet, panels driven: 51b5f6b, in which the MCDU paused the
whole converter for 40 ms after every screen; 922ddac, which paced the MCDU
without pausing but still wrote every panel from the main loop; beta.004,
which writes each panel from its own thread; and beta.005, which changed
profiles and nothing that runs per frame.

| Live | 51b5f6b | 922ddac | beta.004 | beta.005 |
|---|---|---|---|---|
| idle | 0.51% | 0.49% | 0.47% | 0.47% |
| typical | 1.29% | 1.25% | 1.35% | 1.31% |
| stress | 2.53% | 3.81% | 3.85% | 3.43% |
| typical, main loop | 0.77% | 0.74% | 0.30% | 0.33% |
| stress, main loop | 1.74% | 2.85% | 1.00% | 1.05% |
| typical, datagrams read of about 1,893 sent | 1,889 | 1,889 | 1,890 | 1,890 |
| stress, datagrams read of about 3,785 sent | 843 | 3,779 | 3,779 | 3,778 |
| longest main loop pass, typical | not recorded | 771 ms | 13 ms | 0 ms |

- **51b5f6b fell behind under stress.** It read 843 of 3,785 datagrams.
  Each MCDU screen held the whole converter for 40 ms, the stream queued up
  behind it and most of it was dropped, and on the panel the screen
  changed, froze and jumped. The same stall is what made a constantly
  changing MCDU page, the Mosquito's, lag and then catch up in flight.
  Every later build reads everything; their CPU under stress is higher
  because they do the work 51b5f6b threw away.
- **Writing moved off the main loop.** Every report to a panel blocks for
  about 1 ms, USB's polling interval, and 922ddac spent that time in the
  main loop, two thirds of every second under stress. Now each panel's
  writes happen on its own thread, `write <device>`, and the main loop only
  reads, decodes and decides what to send. Total CPU is about the same;
  handing each batch to the writers costs about 0.1% at typical.
- **The pause at aircraft load is gone.** The MCDU's font, about 600
  reports, used to hold the main loop for about 0.8 s when an aircraft
  loaded. It now goes out on the MCDU's own thread. The longest pass seen
  since, 94 ms once under stress on beta.004, is probably the sweep at
  aircraft load, when every lamp and screen is worked out at once while the
  stream is still pouring in; not confirmed.
- **Stress costs more than typical because every send carries
  everything.** The frame holds sends to 25 a second however fast the
  stream runs, but under stress every lamp and screen changes every frame,
  so each send carries all of them: 94 reports a second to the PTO2
  against 4 at typical. No cockpit changes everything at once.

What each panel was sent, per second, on beta.008. Busy is the share of
each second its writer spent blocked on USB, which holds up nothing else:

| Panel | Typical | Busy | Stress | Busy |
|---|---|---|---|---|
| MCDU Captain | 173 reports | 17% | 393 reports | 39% |
| ViperAce ICP | 19 reports | 1.9% | 224 reports | 23% |
| PTO2 | 4 reports | 0.5% | 95 reports | 10% |
| Orion Throttle | 0.8 reports | 0.1% | 17 reports | 2% |
| CarrierAce UFC | 0.4 reports | 0% | 0.4 reports | 0% |
| Orion Rudder Pedals | 0.1 reports | 0% | 0.1 reports | 0% |

- **The MCDU costs most because a text screen is always sent whole**, 16
  reports. Under stress it paints about 24 screens a second against a cap
  of 25.
- **A panel that falls behind skips to the latest.** Its mailbox keeps only
  the newest of each lamp and screen, and about 7 MCDU screens a second
  under stress were replaced by a newer one before they went. The MCDU is
  ready again 40 ms after its last screen finished; a paint that comes
  sooner waits in the mailbox, and the panel's lamps do not wait for it.

**Real flight matches typical.** Two minutes in the Mosquito on beta.004,
from the log's status lines: the MCDU took 185 to 207 reports a second,
about 12 screens, against typical's 185, with its writer busy 18 to 21% of
each second. The longest main loop pass was 2 ms, aircraft load included,
so the 94 ms pass under stress belongs to that scenario and not to flying.

**Memory is up about 11 MB since beta.005, almost all of it in
beta.006.** Idle passes of each release, back to back on 2026-10-04, read
29.1 MB private for beta.005, 40.0 MB for beta.006, 42.2 MB for beta.007
and 44.8 MB for beta.008. beta.006 added switch pieces, stored signals and
flashing lamps. The shipped data grew 0.3 MB over the same releases and the
catalogue not at all (51 files, 11 MB). Not profiled.

**Memory is up about 11 MB since 2026-09-22.** The catalogue has not grown
(still 51 files, 11 MB), so it is the profiles, pages and screens added
since then, and about 1 MB for the open panels. Not profiled.

**What the numbers show.** CPU follows how much actually changes, not how many
frames arrive. That matches the engine's design: `BiosState::apply` reports
whether a word moved, and only words that moved are resolved against the
profile. Memory rises about 3 MB under stress, holds there, and did not grow
during any run.

**Where the memory goes.** This has not been profiled. The daemon loads every
module in `data/catalogue` at startup (51 files, 11 MB of JSON), and that is
the likeliest owner of most of it. Loading only the active aircraft's module
was considered and turned down: learn mode and the startup profile checks need
every module, and they are worth far more than a few megabytes.

## Release runs

Every release is benchmarked before it ships, and gets a row here whether
or not anything changed, so a later change can be dated to the releases
between which it appeared. Live, quiet machine, A-10C II profile, one 60 s
pass per scenario. Memory is private memory at idle; the 2026-10-04 column
was measured for every release back to back, so those compare with each
other.

| Release | Run | Idle | Typical | Stress | Memory at the run | Memory, 2026-10-04 | Result |
|---|---|---|---|---|---|---|---|
| beta.004 | 2026-09-30 | 0.47% | 1.35% | 3.85% | not kept | not measured | First run with panel writer threads |
| beta.005 | 2026-09-30 | 0.47% | 1.31% | 3.43% | 34.3 MB | 29.1 MB | Baseline for the tables until beta.008 |
| beta.006 | before release | not kept | not kept | not kept | not kept | 40.0 MB | No notable change in CPU |
| beta.007 | before release | not kept | not kept | not kept | not kept | 42.2 MB | No notable change in CPU |
| beta.008 | 2026-10-04 | 0.47% | 1.28% | 3.62% | 44.6 MB | 44.8 MB | No notable change; memory up about 11 MB since beta.005, mostly beta.006, still small |

- **beta.005's two memory figures differ** because the first is a
  development build before release, and the second is the release build.
  Compare memory down the 2026-10-04 column only.
- **Stress moves by about 0.2% between passes** of builds that change
  nothing per frame, as beta.004 to beta.005 shows, so a difference that
  size is not a change.

## Other CPUs

Estimated, not measured: the only machine measured is the i9-12900KF above.

| Scenario | i9-12900KF, measured | i5-12400F, estimated |
|---|---|---|
| idle | 0.47% | about 0.6% |
| typical | 1.28% | about 1.5% |
| stress | 3.62% | about 4.3% |

- **How.** The daemon's threads each do a little, on whichever core is
  free, and none comes near filling one, so what it costs follows how fast a
  single core is, not how many there are. Both chips have the same P-cores
  (Golden Cove), and the 12400F boosts to 4.4 GHz against the 12900KF's 5.1,
  so the same work takes about 1.16 times as long. The estimate is the
  measured figure times 1.2.
- **Measured on P-cores, which is what the 12400F has.** The 12900KF also
  has slower E-cores, which the 12400F does not. Pinned to the P-cores
  (`--affinity 0xff`), a pass matched the unpinned ones, so Windows already
  runs the daemon on P-cores. Pinned to the E-cores (`--affinity 0xff00`) it
  cost about 1.5 times as much at stress, which is about the most an older
  or slower core would add.
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
python tools/bench_daemon.py --seconds 60     # the tables above
python tools/bench_daemon.py --scenario stress
python tools/bench_daemon.py --module F-16C_50 --aircraft F-16C_50
python tools/bench_daemon.py --affinity 0xff  # P-cores only on a 12900K
python tools/bench_daemon.py --exe ..\dsc-baseline\target\release\dcs-signal.exe
```

The last line measures another build, such as the commit before a change,
checked out with `git worktree add ..\dsc-baseline <commit>` and built
there, so the two are measured by the same script.

The tool needs only the Python standard library, and DCS does not need to be
running. It plays the synthetic stream onto 239.255.50.10:5010 from the
generated catalogue, starts the daemon, which drives the panels, with its
output sent to NUL and its log to a temporary folder, and samples it with
`QueryProcessCycleTime`, `QueryThreadCycleTime` and `GetProcessMemoryInfo`.
The second table it prints splits the CPU between the main loop, the panel
writers (`write <device>`), the page key readers (`keys <device>`) and
Windows' own threads. The
third compares the datagrams it sent with those the daemon read, and a
fourth says what each panel was sent, read from the daemon's
status lines before its log is deleted.

- **Plug the panels in, and expect them to light.** With none connected
  the daemon has nothing to drive and exits. Lamps and screens show random
  values during typical and stress and are cleared at the end.
- **Close other DCS-BIOS clients first.** Anything else reading the multicast
  group, such as `dcs-signal listen` or the editor's learn mode, sees the
  synthetic stream too.
- **Close other work.** Browsers, chat and anything else that can be closed.
  A build or a video in the background still skews the figures, even
  counted in cycles, and before-and-after comparisons are only fair when
  both ran on the same quiet machine.
- **Stop the editor's daemon first.** Only one daemon can hold the
  panels, and a run that finds another exits at once.

The editor figures were taken by hand. The tool does not measure the editor.
