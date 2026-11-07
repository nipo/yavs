//! `--threads=auto` (`Config::auto`): the threads are judged while the run goes, in windows of
//! wall time. In a window that has them on, the batches that go to the pool (or want to and get
//! no token) add their estimate inline and the pooled ones their measured time; probes (batches
//! that would pool and run inline) say what the estimate is worth inline. A window wins when the
//! saving is at least `GAIN_PERCENT` of its time. Windows that lose in a row turn the threads off
//! (`workers` 0: the code paths of `--threads=1`, tokens given back), and a trial comes again
//! after a pause that doubles with every trial that does not win. Now and then a window runs on
//! half the threads (or all again) and the faster count is kept. Where a batch runs changes no
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
/// A window in which no batch wanted the threads ends this early: the usual design that does not
/// gain pays its trial on 5 ms.
const WINDOW_EMPTY: Duration = Duration::from_millis(5);
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
/// Windows that won between two tries of the other thread count (all of `--threads=auto`'s, or
/// half of them): the first try follows the first window that won.
const EXPLORE_EVERY: u32 = 16;
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
    /// The workers a pooled batch may use: `workers`, or about half of the threads when that was
    /// faster (`SimControl::helpers`). Hyperthreads or a shared cache make some designs faster on
    /// fewer threads; the other count is tried every `EXPLORE_EVERY` windows that won.
    pub(super) limit: usize,
    /// The window now runs at the other count; the pooled time over estimate of the one before.
    trying: Option<u64>,
    since_try: u32,
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
            limit: workers,
            trying: None,
            since_try: EXPLORE_EVERY - 1,
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

    /// After a window that won with pooled time over estimate `this`: keep the thread count that
    /// pooled faster, and now and then try the other one.
    fn explore(&mut self, this: Option<u64>) {
        // Half the threads: workers + 1 of them, the kernel thread counted.
        let half = self.workers.div_ceil(2) - 1;
        let Some(r) = this.filter(|_| half > 0) else {
            return;
        };
        if let Some(before) = self.trying.take() {
            if r >= before {
                self.limit = if self.limit == self.workers {
                    half
                } else {
                    self.workers
                };
            }
            self.since_try = 0;
        } else {
            self.since_try += 1;
            if self.since_try >= EXPLORE_EVERY {
                self.trying = Some(r);
                self.limit = if self.limit == self.workers {
                    half
                } else {
                    self.workers
                };
            }
        }
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
        let this = self
            .pooled
            .saturating_mul(1024)
            .checked_div(self.pooled_est);
        if let Some(r) = this {
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
            self.explore(this);
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
            if !flip && (elapsed < WINDOW_EMPTY || elapsed < WINDOW && a.want > 0) {
                return;
            }
            let limit = a.limit;
            let (on, won) = a.judge(elapsed.as_nanos().min(u128::from(u64::MAX)) as u64);
            let resized = a.limit != limit;
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
            if resized {
                // A pool of the new size: idle workers of a bigger one slow the hand-over.
                self.pool = None;
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
            // Called before the run: the CPUs are not counted yet.
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
        a.limit = a.workers;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A window of 1 ms whose batches pooled at `ratio` (1024ths of their estimate).
    fn window(a: &mut Auto, ratio: u64) -> (bool, bool) {
        a.batch(1_000_000, Some(ratio * 1_000_000 / 1024));
        a.judge(1_000_000)
    }

    #[test]
    fn the_faster_thread_count_is_kept() {
        let mut a = Auto::new(7, None);
        // The first window that wins tries half the threads (4 of 8: 3 workers).
        assert_eq!(window(&mut a, 600), (true, true));
        assert_eq!(a.limit, 3);
        // Half pooled faster: kept until the next try.
        assert_eq!(window(&mut a, 500), (true, true));
        assert_eq!(a.limit, 3);
        for _ in 0..EXPLORE_EVERY - 1 {
            window(&mut a, 500);
            assert_eq!(a.limit, 3);
        }
        // The try of all threads is slower: back to half.
        window(&mut a, 500);
        assert_eq!(a.limit, 7);
        window(&mut a, 700);
        assert_eq!(a.limit, 3);
    }

    /// Pooled batches on half the threads still teach the pool threshold (`learn_pooled`).
    #[test]
    fn batches_on_half_the_threads_learn_the_threshold() {
        let mut b = crate::Builder::new();
        for i in 0..64 {
            let f = |c: &mut Ctx<'_>| c.wait_for(Time::ns(1));
            b.process(&format!("p{i}"), crate::ProcFlags::default(), &[], f);
        }
        let cfg = Config {
            threads: 8,
            auto: true,
            ..Config::default()
        };
        let mut sim = b.build(cfg).unwrap();
        sim.set_threads_hint(Some(true));
        let a = sim.auto.as_deref_mut().unwrap();
        (a.counted, a.limit) = (true, 3);
        // No window is judged (a slow machine could end one): inline first, so every process has
        // a cost, then every batch pools.
        sim.tick_mask = u64::MAX;
        sim.pool_ns = u64::MAX;
        sim.run_until(Time::ns(100)).unwrap();
        sim.pool_ns = 0;
        sim.run_until(Time::ns(400)).unwrap();
        assert_eq!(sim.auto.as_ref().unwrap().limit, 3);
        assert!(sim.stats().pooled_batches > 0, "{:?}", sim.stats());
        assert_ne!(sim.pool_ratio, 512, "no pooled batch was learnt from");
    }

    #[test]
    fn losing_windows_turn_the_threads_off() {
        let mut a = Auto::new(7, None);
        // Nothing wanted the threads: off at once.
        assert_eq!(a.judge(1_000_000), (false, false));
        let mut a = Auto::new(7, None);
        // Pooled no faster than inline: off after two windows.
        assert_eq!(window(&mut a, 1100), (true, false));
        assert_eq!(window(&mut a, 1100), (false, false));
        // After a window that won it takes four.
        let mut a = Auto::new(7, Some(true));
        for _ in 0..3 {
            assert_eq!(window(&mut a, 1100), (true, false));
        }
        assert_eq!(window(&mut a, 1100), (false, false));
    }
}
