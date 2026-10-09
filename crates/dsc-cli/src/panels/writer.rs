//! A thread per panel, so writing to one never holds up anything else.
//!
//! Every report to a panel blocks for about a millisecond, USB's polling
//! interval, and a text screen is 16 of them. Written from the main loop,
//! that was time in which no other panel was written and no datagram read:
//! under a heavy stream, two thirds of every second. Here each panel's
//! writes happen on its own thread, and the main loop only leaves them in
//! the panel's mailbox.
//!
//! The mailbox keeps the latest of everything and nothing older: a lamp by
//! its id, a screen piece by where it goes. A lamp value and a screen piece
//! are each whole, never a change from the last, so a panel that falls
//! behind skips straight to the newest state rather than working through a
//! queue of old ones. A screen that is not ready for its next paint, as the
//! MCDU's text grid is not for 40 ms after each, leaves its paint in the
//! mailbox, where a newer one replaces it, until it is.
//!
//! A panel that can check its device is still in step says how often, and
//! its writer calls the check on that beat, idle or not, on the same thread
//! as its writes so the two never cross.

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use dsc_engine::{LcdWrite, LedWrite};

use super::{Panel, Sent};

/// A lamp, by part and index.
type LampKey = (u32, u8);
/// A screen piece, by part, group and where in the screen it starts.
type ScreenKey = (u32, u8, usize);

#[derive(Default)]
struct Mailbox {
    lamps: BTreeMap<LampKey, LedWrite>,
    screens: BTreeMap<ScreenKey, LcdWrite>,
    /// Stop once everything left has been written.
    closing: bool,
    /// Why the writer stopped, if a write failed.
    failed: Option<String>,
    /// Screen pieces replaced by a newer one before they were written.
    superseded: u64,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
    /// What the panel had been sent at the end of its last round.
    sent: Mutex<Sent>,
}

/// One panel, written from its own thread.
pub struct Writer {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    /// Start a writer for `panel`, named for the device so a profiler can
    /// tell it apart.
    pub fn start(key: &str, panel: Box<dyn Panel>) -> Result<Writer> {
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox::default()),
            wake: Condvar::new(),
            sent: Mutex::new(Sent::default()),
        });
        let theirs = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name(format!("write {key}"))
            .spawn(move || run(panel, &theirs))?;
        Ok(Writer {
            shared,
            thread: Some(thread),
        })
    }

    /// Leave a batch's lamps and screen pieces for this panel, replacing any
    /// not yet written, and return at once.
    pub fn post<'a>(
        &self,
        lamps: impl IntoIterator<Item = &'a LedWrite>,
        screens: impl IntoIterator<Item = &'a LcdWrite>,
    ) {
        let mut mb = self.shared.mailbox.lock().expect("mailbox poisoned");
        for w in lamps {
            mb.lamps.insert((w.id.part_id, w.id.index), w.clone());
        }
        for w in screens {
            if mb
                .screens
                .insert((w.part_id, w.group, w.offset), w.clone())
                .is_some()
            {
                mb.superseded += 1;
            }
        }
        drop(mb);
        self.shared.wake.notify_one();
    }

    /// Why the writer stopped, if it has.
    pub fn failed(&self) -> Option<String> {
        self.shared
            .mailbox
            .lock()
            .expect("mailbox poisoned")
            .failed
            .clone()
    }

    /// Everything sent so far, as of the writer's last round.
    pub fn sent(&self) -> Sent {
        let mut sent = *self.shared.sent.lock().expect("sent poisoned");
        sent.superseded = self
            .shared
            .mailbox
            .lock()
            .expect("mailbox poisoned")
            .superseded;
        sent
    }

    /// Write whatever is left and stop, waiting at most `limit`, and say
    /// what was sent in all. On the way out, where the last thing posted is
    /// the panel being cleared.
    pub fn finish(mut self, limit: Duration) -> Result<Sent> {
        self.close();
        let Some(thread) = self.thread.take() else {
            return Ok(self.sent());
        };
        let deadline = Instant::now() + limit;
        while !thread.is_finished() {
            if Instant::now() >= deadline {
                return Err(anyhow!("still writing after {} ms", limit.as_millis()));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        thread
            .join()
            .map_err(|_| anyhow!("the writer thread panicked"))?;
        match self.failed() {
            Some(why) => Err(anyhow!(why)),
            None => Ok(self.sent()),
        }
    }

    fn close(&self) {
        self.shared
            .mailbox
            .lock()
            .expect("mailbox poisoned")
            .closing = true;
        self.shared.wake.notify_one();
    }
}

impl Drop for Writer {
    /// A writer dropped without [`finish`](Writer::finish), as when the
    /// converter stops on an error, is told to stop but not waited for: a
    /// panel that has stopped answering must not hold up the exit.
    fn drop(&mut self) {
        self.close();
    }
}

fn run(mut panel: Box<dyn Panel>, shared: &Shared) {
    let every = panel.check_every();
    let mut next_check = every.map(|e| Instant::now() + e);
    loop {
        let (lamps, screens) = {
            let mut mb = shared.mailbox.lock().expect("mailbox poisoned");
            loop {
                let now = Instant::now();
                // The soonest a held screen piece is ready, if none is now.
                let mut soonest: Option<Instant> = None;
                let mut any_ready = false;
                for w in mb.screens.values() {
                    match panel.ready_at(w) {
                        Some(t) if t > now => soonest = Some(soonest.map_or(t, |s| s.min(t))),
                        _ => any_ready = true,
                    }
                }
                if !mb.lamps.is_empty() || any_ready {
                    break;
                }
                if mb.closing && mb.screens.is_empty() {
                    return;
                }
                if next_check.is_some_and(|t| now >= t) {
                    break;
                }
                // Until a held screen is ready or a check is due.
                mb = match soonest.into_iter().chain(next_check).min() {
                    Some(t) => {
                        shared
                            .wake
                            .wait_timeout(mb, t.saturating_duration_since(now))
                            .expect("mailbox poisoned")
                            .0
                    }
                    None => shared.wake.wait(mb).expect("mailbox poisoned"),
                };
            }
            let lamps = std::mem::take(&mut mb.lamps);
            let ready: Vec<ScreenKey> = mb
                .screens
                .iter()
                .filter(|(_, w)| panel.ready_at(w).is_none_or(|t| t <= Instant::now()))
                .map(|(k, _)| *k)
                .collect();
            let screens: Vec<LcdWrite> =
                ready.iter().filter_map(|k| mb.screens.remove(k)).collect();
            (lamps, screens)
        };

        // Lamps before screens, as the main loop always sent them.
        let result = (|| -> Result<()> {
            if !lamps.is_empty() || !screens.is_empty() {
                for w in lamps.values() {
                    panel.set_lamp(w)?;
                }
                for w in &screens {
                    panel.write_display(w)?;
                }
                // Lamps too: a panel may hold them for one message per batch.
                panel.flush()?;
            }
            if let (Some(e), Some(t)) = (every, next_check) {
                if Instant::now() >= t {
                    panel.check()?;
                    next_check = Some(Instant::now() + e);
                }
            }
            Ok(())
        })();
        *shared.sent.lock().expect("sent poisoned") = panel.sent();
        if let Err(e) = result {
            let mut mb = shared.mailbox.lock().expect("mailbox poisoned");
            mb.failed = Some(format!("{e:#}"));
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsc_config::Transport;
    use dsc_engine::LedId;
    use std::sync::mpsc;

    /// A panel that records what it is sent. Its text screen is ready only
    /// from a given moment, as the MCDU's is 40 ms after a paint, and its
    /// lamp writes can be held open to keep the writer busy.
    #[derive(Default)]
    struct Fake {
        log: Arc<Mutex<Vec<String>>>,
        text_ready: Option<Instant>,
        /// Said when a lamp write starts; the write then waits for `go`.
        entered: Option<mpsc::Sender<()>>,
        go: Option<mpsc::Receiver<()>>,
        unplugged: bool,
        /// Checked this often, failing once `check_fails`.
        check: Option<Duration>,
        check_fails: bool,
    }

    impl Panel for Fake {
        fn set_lamp(&mut self, w: &LedWrite) -> Result<()> {
            if let (Some(entered), Some(go)) = (&self.entered, &self.go) {
                let _ = entered.send(());
                let _ = go.recv();
            }
            anyhow::ensure!(!self.unplugged, "unplugged");
            let line = format!("lamp {} = {}", w.id.index, w.value);
            self.log.lock().unwrap().push(line);
            Ok(())
        }
        fn write_display(&mut self, w: &LcdWrite) -> Result<()> {
            let line = format!("screen {}", String::from_utf8_lossy(&w.bytes));
            self.log.lock().unwrap().push(line);
            Ok(())
        }
        fn flush(&mut self) -> Result<()> {
            self.log.lock().unwrap().push("flush".into());
            Ok(())
        }
        fn ready_at(&self, w: &LcdWrite) -> Option<Instant> {
            match w.transport {
                Transport::Text => self.text_ready.filter(|t| *t > Instant::now()),
                _ => None,
            }
        }
        fn sent(&self) -> Sent {
            Sent::default()
        }
        fn check_every(&self) -> Option<Duration> {
            self.check
        }
        fn check(&mut self) -> Result<()> {
            anyhow::ensure!(!self.check_fails, "stopped answering");
            self.log.lock().unwrap().push("check".into());
            Ok(())
        }
    }

    fn lamp(index: u8, value: u8) -> LedWrite {
        LedWrite {
            id: LedId {
                device: "PANEL".into(),
                part_id: 1,
                index,
            },
            value,
        }
    }

    fn text(s: &str) -> LcdWrite {
        LcdWrite {
            device: "PANEL".into(),
            part_id: 1,
            transport: Transport::Text,
            display: "MCDU".into(),
            group: 0,
            offset: 0,
            bytes: s.as_bytes().to_vec(),
            font: None,
        }
    }

    const LIMIT: Duration = Duration::from_secs(2);

    #[test]
    fn lamps_go_before_screens_and_a_batch_is_flushed_once() {
        let fake = Fake::default();
        let log = Arc::clone(&fake.log);
        let w = Writer::start("PANEL", Box::new(fake)).unwrap();
        w.post(&[lamp(1, 5), lamp(2, 7)], &[text("A")]);
        w.finish(LIMIT).unwrap();
        assert_eq!(
            *log.lock().unwrap(),
            ["lamp 1 = 5", "lamp 2 = 7", "screen A", "flush"]
        );
    }

    /// While the panel is busy, everything posted for it is replaced by
    /// whatever comes after, so it catches up in one step rather than
    /// working through old states.
    #[test]
    fn a_busy_panel_skips_to_the_latest_state() {
        let (entered_tx, entered) = mpsc::channel();
        let (go, go_rx) = mpsc::channel();
        let fake = Fake {
            entered: Some(entered_tx),
            go: Some(go_rx),
            ..Fake::default()
        };
        let log = Arc::clone(&fake.log);
        let w = Writer::start("PANEL", Box::new(fake)).unwrap();

        w.post(&[lamp(1, 1)], &[]);
        entered.recv().unwrap(); // the writer is inside that write now
        w.post(&[lamp(1, 2)], &[text("B")]);
        w.post(&[lamp(1, 3)], &[text("C")]);
        go.send(()).unwrap();
        go.send(()).unwrap();
        let sent = w.finish(LIMIT).unwrap();

        assert_eq!(
            *log.lock().unwrap(),
            ["lamp 1 = 1", "flush", "lamp 1 = 3", "screen C", "flush"]
        );
        assert_eq!(sent.superseded, 1);
    }

    /// The MCDU's case: a screen not ready for its next paint keeps only the
    /// latest one until it is, and the panel's lamps do not wait for it.
    #[test]
    fn a_screen_not_ready_holds_its_latest_paint_and_nothing_else_waits() {
        let fake = Fake {
            text_ready: Some(Instant::now() + Duration::from_millis(150)),
            ..Fake::default()
        };
        let log = Arc::clone(&fake.log);
        let w = Writer::start("PANEL", Box::new(fake)).unwrap();

        w.post(&[lamp(1, 1)], &[text("A")]);
        w.post(&[], &[text("B")]);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(*log.lock().unwrap(), ["lamp 1 = 1", "flush"]);

        let sent = w.finish(LIMIT).unwrap();
        assert_eq!(
            *log.lock().unwrap(),
            ["lamp 1 = 1", "flush", "screen B", "flush"]
        );
        assert_eq!(sent.superseded, 1);
    }

    /// A DSC board sends its lamps in one message at the flush, so a batch
    /// with no screen in it is flushed too.
    #[test]
    fn a_batch_of_lamps_alone_is_flushed() {
        let fake = Fake::default();
        let log = Arc::clone(&fake.log);
        let w = Writer::start("PANEL", Box::new(fake)).unwrap();
        w.post(&[lamp(1, 5)], &[]);
        w.finish(LIMIT).unwrap();
        assert_eq!(*log.lock().unwrap(), ["lamp 1 = 5", "flush"]);
    }

    #[test]
    fn a_panel_that_checks_is_checked_while_idle_and_nothing_flushed() {
        let fake = Fake {
            check: Some(Duration::from_millis(20)),
            ..Fake::default()
        };
        let log = Arc::clone(&fake.log);
        let w = Writer::start("PANEL", Box::new(fake)).unwrap();
        std::thread::sleep(Duration::from_millis(90));
        w.finish(LIMIT).unwrap();
        let log = log.lock().unwrap();
        assert!(log.len() >= 2, "{log:?}");
        assert!(log.iter().all(|l| l == "check"), "{log:?}");
    }

    #[test]
    fn a_failed_check_stops_the_writer_as_a_failed_write_does() {
        let fake = Fake {
            check: Some(Duration::from_millis(10)),
            check_fails: true,
            ..Fake::default()
        };
        let w = Writer::start("PANEL", Box::new(fake)).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert!(w.failed().is_some_and(|e| e.contains("stopped answering")));
    }

    #[test]
    fn a_failed_write_is_reported_and_stops_the_writer() {
        let fake = Fake {
            unplugged: true,
            ..Fake::default()
        };
        let w = Writer::start("PANEL", Box::new(fake)).unwrap();
        w.post(&[lamp(1, 1)], &[]);
        let err = w.finish(LIMIT).unwrap_err();
        assert!(format!("{err:#}").contains("unplugged"), "{err:#}");
    }
}
