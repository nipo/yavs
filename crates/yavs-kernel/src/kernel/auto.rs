//! `--threads=auto` (`Config::auto`): the threads are judged while the run goes, in windows of
//! wall time. In a window that has them on, the batches that go to the pool (or want to and get
//! no token) add their estimate inline and the pooled ones their measured time; probes (batches
//! that would pool and run inline) say what the estimate is worth inline. A window wins when the
//! saving is at least `GAIN_PERCENT` of its time. Windows that lose in a row turn the threads off
//! (`workers` 0: the code paths of `--threads=1`, tokens given back), and a trial comes again
//! after a pause that doubles with every trial that does not win. Where a batch runs changes no
//! output (DESIGN.md "Parallel evaluation"), so neither does a decision.

use super::*;
use std::time::{Duration, Instant};

/// Fewer processes that may run in parallel than this: no batch can be worth the threads, so
/// they stay off for the whole run, without a trial (the static estimate).
// ponytail: a count, not the processes' weights; a few very heavy processes stay on one thread.
pub(super) const MIN_PROCS: usize = 8;
/// A window with the threads on: long enough for the costs of a design of thousands of
/// processes to be timed (`COST_EVERY` batches), short enough that a design that does not gain
/// pays its trial cost on a small part of a short run.
const WINDOW: Duration = Duration::from_millis(25);
/// The saving that wins a window, in percent of its wall time: above what being on costs a design
/// whose batches stay inline (1 to 2 percent).
const GAIN_PERCENT: u64 = 4;
/// Losing windows in a row that turn the threads off: one in which no batch wanted the pool is
/// enough; otherwise two, since the first window of a trial also pays for starting the threads;
/// four once the run (or the last one) won a window, since on a loaded host single pooled batches
/// are slow and the kernel then keeps batches inline for a while by itself.
const STRIKES: u32 = 2;
const STRIKES_PROVEN: u32 = 4;
/// The first trial starts this long after the build: a test that ends sooner pays nothing for the
/// trial (timing a sample of the activations, the estimate of each batch).
const START: Duration = Duration::from_millis(10);
/// The pause after the first trial that does not win; it doubles up to `PAUSE_MAX`. Once a window
/// won, in this run or the last (`Config::auto_hint`), the pause starts at `PAUSE_GAINED`, so a
/// design whose batches go back to gaining is found again soon.
const PAUSE: Duration = Duration::from_secs(1);
const PAUSE_GAINED: Duration = Duration::from_millis(100);
const PAUSE_MAX: Duration = Duration::from_secs(16);
/// The kernel looks at the clock every this many cycles (a mask of `SimControl::cycle`).
pub(super) const TICK_MASK: u64 = 0xff;

/// What `--threads=auto` decided, for the run's notes and the memory of the next run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoVerdict {
    /// Too few processes run in parallel: one thread for the whole run, no trial.
    Static,
    /// Windows judged, and how many of them won.
    Judged { windows: u64, gained: u64 },
    /// The run ended before its first window did (or it started off from the memory and no
    /// trial came).
    Unjudged,
}

pub(super) struct Auto {
    /// The workers of `--threads`; `SimControl::workers` is 0 while the threads are off.
    workers: usize,
    on: bool,
    since: Instant,
    /// While off: the time until the next trial.
    pause: Duration,
    /// A window won in this run or the last one.
    proven: bool,
    /// This window: the inline estimate of the batches that went to the pool or wanted to, and
    /// of the pooled ones, and the measured time of those (ns).
    want: u64,
    pooled_est: u64,
    pooled: u64,
    /// The pooled time over its estimate, in 1024ths, of the last window that pooled (512 until
    /// one did: a design is given the threads until its pooled batches are measured).
    ratio: u64,
    /// The probes: their estimate and measured time, halved at each window so recent ones count
    /// most. The estimate of a tiny process is mostly the clock read that timed it.
    inline_est: u64,
    inline: u64,
    /// Losing windows in a row.
    lost: u32,
    /// `workers` is limited to the CPUs (`SimControl::auto_cpus`).
    pub(super) counted: bool,
    windows: u64,
    gained: u64,
}

impl Auto {
    /// `hint`: what the last run of the same command line found (`SimControl::set_threads_hint`):
    /// one that gained starts on, one that did not starts off with a long pause.
    pub(super) fn new(workers: usize, hint: Option<bool>) -> Auto {
        Auto {
            workers,
            on: hint == Some(true),
            since: Instant::now(),
            pause: match hint {
                Some(true) => PAUSE_GAINED,
                Some(false) => PAUSE_MAX / 4,
                None => START,
            },
            proven: hint == Some(true),
            want: 0,
            pooled_est: 0,
            pooled: 0,
            ratio: 512,
            inline_est: 0,
            inline: 0,
            lost: 0,
            counted: false,
            windows: 0,
            gained: 0,
        }
    }

    /// The workers to start with.
    pub(super) fn start(&self) -> usize {
        if self.on { self.workers } else { 0 }
    }

    /// A batch that would go to the pool, estimated `est` ns inline; `ns` its measured time when
    /// it went (`None`: it ran inline for want of a token).
    pub(super) fn batch(&mut self, est: u64, ns: Option<u64>) {
        self.want += est;
        if let Some(ns) = ns {
            self.pooled_est += est;
            self.pooled += ns;
        }
    }

    /// A probe: a batch that would pool ran inline, estimated `est` ns, in `ns`.
    pub(super) fn inline(&mut self, est: u64, ns: u64) {
        self.inline_est += est;
        self.inline += ns;
    }

    pub(super) fn verdict(&self) -> AutoVerdict {
        if self.windows == 0 {
            AutoVerdict::Unjudged
        } else {
            AutoVerdict::Judged {
                windows: self.windows,
                gained: self.gained,
            }
        }
    }

    /// End a window of `ns` with the threads on: whether they stay on, and whether it won.
    fn judge(&mut self, ns: u64) -> (bool, bool) {
        if let Some(r) = self
            .pooled
            .saturating_mul(1024)
            .checked_div(self.pooled_est)
        {
            self.ratio = r;
        }
        // Inline time over the estimate, in 1024ths: even until a probe was timed.
        let inline = self
            .inline
            .saturating_mul(1024)
            .checked_div(self.inline_est)
            .unwrap_or(1024);
        (self.inline_est, self.inline) = (self.inline_est / 2, self.inline / 2);
        let saved = self.want.saturating_mul(inline.saturating_sub(self.ratio)) / 1024;
        let won = saved.saturating_mul(100) >= ns.saturating_mul(GAIN_PERCENT);
        self.lost = if won { 0 } else { self.lost + 1 };
        let strikes = match (self.proven, self.want) {
            (true, _) => STRIKES_PROVEN,
            (false, 0) => 1,
            (false, _) => STRIKES,
        };
        if won {
            self.proven = true;
            self.pause = PAUSE_GAINED;
        }
        (self.want, self.pooled_est, self.pooled) = (0, 0, 0);
        (self.lost < strikes, won)
    }
}

impl SimControl {
    /// From `tick`: end a window with the threads on, or start a trial after a pause. `flip`
    /// (`Config::auto_flip`, tests) turns them the other way at every tick.
    pub(super) fn auto_tick(&mut self) {
        let flip = self.cfg.auto_flip;
        let Some(a) = self.auto.as_deref_mut() else {
            return;
        };
        let now = Instant::now();
        let elapsed = now.duration_since(a.since);
        if a.on {
            if elapsed < WINDOW && !flip {
                return;
            }
            let (on, won) = a.judge(elapsed.as_nanos().min(u128::from(u64::MAX)) as u64);
            let won = won && !flip;
            a.on = on && !flip;
            a.since = now;
            a.windows += 1;
            a.gained += u64::from(won);
            self.stats.threads_windows += 1;
            self.stats.threads_gained += u64::from(won);
            if !a.on {
                self.workers = 0;
                self.give_tokens();
            }
        } else if elapsed >= a.pause || flip {
            a.pause = match a.pause {
                START => PAUSE,
                p => (p * 2).min(PAUSE_MAX),
            };
            a.on = true;
            a.lost = 0;
            a.since = now;
            self.workers = a.workers;
        }
    }

    /// What the last run of the same command line found, gained or not (`--threads=auto` past the
    /// static estimate; call it before the run): one that did not gain starts on one thread.
    pub fn set_threads_hint(&mut self, hint: Option<bool>) {
        if let Some(a) = self.auto.as_deref_mut() {
            *a = Auto::new(a.workers, hint);
            self.workers = a.start();
        }
    }

    /// The first batch that wants the threads: no more workers than the CPUs the process may run
    /// on (asked only now, so a run that never wants them does not ask).
    #[cold]
    pub(super) fn auto_cpus(&mut self) {
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
        let Some(a) = self.auto.as_deref_mut() else {
            return;
        };
        a.counted = true;
        a.workers = a.workers.min(cpus - 1);
        self.workers = a.workers;
        if a.workers == 0 {
            // One CPU: one thread for the rest of the run.
            self.auto = None;
        }
    }

    /// What `--threads=auto` decided; `None` without it.
    pub fn threads_verdict(&self) -> Option<AutoVerdict> {
        match &self.auto {
            Some(a) => Some(a.verdict()),
            None if self.cfg.auto => Some(AutoVerdict::Static),
            None => None,
        }
    }
}
