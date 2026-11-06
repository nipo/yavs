//! `Builder::guard_process`: GUARD changes in the cycle of the event on a signal of its condition,
//! and a guard whose condition reads another GUARD follows in the same cycle.

mod common;

use common::*;
use yavs_kernel::*;
use yavs_logic::StdULogic;

const NONE: ProcFlags = ProcFlags {
    postponed: false,
    serial: false,
    several_waits: false,
    concurrent_call: false,
};

fn model(kernel_guard: bool) -> Vec<String> {
    let mut b = Builder::new();
    let one = Range::new(0, 1);
    let v = b.signal("v", BYTE, &[0]);
    let bool_sig = |b: &mut Builder, name: &str, init: StdULogic| {
        b.signal(name, SignalType::logic(1), &LogicVec::new(1, init))
    };
    let g1 = bool_sig(&mut b, "g1", StdULogic::One);
    let g2 = bool_sig(&mut b, "g2", StdULogic::One);
    let (dv, d1, d2) = (b.driver(v), b.driver(g1), b.driver(g2));
    // The testbench: v := 3 at 0 ns (delta 1), 10 at 1 ns.
    let mut step = 0;
    b.process(
        "stim",
        NONE,
        &[dv],
        p(move |c| {
            c.assign(
                dv,
                None,
                &[(Time::ZERO, &[[3u8, 10][step % 2]][..])],
                DelayMode::INERTIAL,
            )?;
            step += 1;
            if step < 2 {
                c.wait_for(ns(1))?;
            }
            Ok(())
        }),
    );
    // g1 <= v < 10; g2 <= g1 and v < 5, each waiting for its condition's signals.
    let guard = |dst: DriverId,
                 reads: Vec<SignalId>,
                 f: fn(&Ctx<'_>, SignalId, SignalId) -> bool,
                 g1: SignalId| {
        p(move |c| {
            let on = f(c, v, g1);
            let code = if on { StdULogic::One } else { StdULogic::Zero } as u8;
            c.assign_code_now(dst, one, code);
            for &r in &reads {
                if !c.wait_same() {
                    c.wait_on(r)?;
                }
            }
            Ok(())
        })
    };
    let p1 = b.process(
        "_guard1",
        NONE,
        &[d1],
        guard(d1, vec![v], |c, v, _| c.value(v).unwrap()[0] < 10, g1),
    );
    let p2 = b.process(
        "_guard2",
        NONE,
        &[d2],
        guard(
            d2,
            vec![v, g1],
            |c, v, g1| {
                c.elem_code(g1, 0).unwrap() == StdULogic::One as u8 && c.value(v).unwrap()[0] < 5
            },
            g1,
        ),
    );
    if kernel_guard {
        b.guard_process(p1, &[(v, Range::new(0, 1))]);
        b.guard_process(p2, &[(v, Range::new(0, 1)), (g1, one)]);
    }
    let (mut sim, trace) = build(
        b,
        Config {
            edge_triggers: false,
            ..Config::default()
        },
    );
    sim.run_until(ns(5)).unwrap();
    trace.take()
}

#[test]
fn guard_changes_in_the_cycle_of_its_condition() {
    let late = model(false);
    let same = model(true);
    // As a process, GUARD follows `v` a delta later and the second guard another one later.
    let at = |t: &[String], key: &str| {
        t.iter()
            .filter(|l| l.contains(key) && !l.starts_with("0 fs +0"))
            .cloned()
            .collect::<Vec<_>>()
    };
    let v = at(&same, "v[");
    let g1 = at(&same, "g1[");
    let g2 = at(&same, "g2[");
    let stamp = |l: &String| l.split(' ').take(3).collect::<Vec<_>>().join(" ");
    assert!(!g1.is_empty() && !g2.is_empty(), "{same:?}");
    // The same time and delta as the event on `v`, for both guards.
    assert_eq!(stamp(&g1[0]), stamp(&v[1]), "{same:?}");
    assert_eq!(stamp(&g2[0]), stamp(&v[1]), "{same:?}");
    let g1_late = at(&late, "g1[");
    assert_ne!(stamp(&g1_late[0]), stamp(&at(&late, "v[")[1]), "{late:?}");
    // And the values are the same either way.
    let values = |t: &[String]| {
        t.iter()
            .filter(|l| l.contains("g1[") || l.contains("g2["))
            .map(|l| l.rsplit('=').next().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(values(&late), values(&same));
}

/// `Builder::guard_init`: at initialisation the guard processes run first, in order, and what they
/// assign is GUARD's initial value (LRM 14.7.5.2): a process that runs before them in the batch
/// order sees it, with no event, no activity and `'last_value` the same; a guard on another GUARD
/// sees the outer one's; nothing changes a delta later.
#[test]
fn guard_init_gives_the_initial_value() {
    for kernel_guard in [false, true] {
        let mut b = Builder::new();
        let one = Range::new(0, 1);
        let zero = LogicVec::new(1, StdULogic::Zero);
        let g1 = b.signal("g1", SignalType::logic(1), &zero);
        let g2 = b.signal("g2", SignalType::logic(1), &zero);
        let (d1, d2) = (b.driver(g1), b.driver(g2));
        b.process(
            "reader",
            NONE,
            &[],
            p(move |c| {
                let msg = format!(
                    "g1={} g2={} event={} active={} last_event={} last_value={}",
                    c.elem_code(g1, 0)?,
                    c.elem_code(g2, 0)?,
                    c.event(g1)?,
                    c.active(g1)?,
                    c.last_event(g1)? == Time::MAX,
                    c.last_value(g1)?
                );
                c.report(Severity::Note, msg);
                c.wait_on(g1)?;
                c.wait_on(g2)
            }),
        );
        let p1 = b.process(
            "_guard1",
            NONE,
            &[d1],
            p(move |c| {
                c.assign_code_now(d1, one, StdULogic::One as u8);
                Ok(())
            }),
        );
        let p2 = b.process(
            "_guard2",
            NONE,
            &[d2],
            p(move |c| {
                let code = c.elem_code(g1, 0)?;
                c.assign_code_now(d2, one, code);
                c.wait_on(g1)
            }),
        );
        b.guard_init(p1);
        b.guard_init(p2);
        if kernel_guard {
            b.guard_process(p2, &[(g1, one)]);
        }
        let (mut sim, trace) = build(b, Config::default());
        sim.run_until(ns(1)).unwrap();
        let one = StdULogic::One as u8;
        assert_eq!(
            trace.take(),
            [format!(
                "0 fs +0 p0 Note: g1={one} g2={one} event=false active=false last_event=true last_value=\"1\""
            )],
            "kernel_guard {kernel_guard}"
        );
    }
}

/// A guard runs in every cycle in which a signal part of its condition is active (LRM 14.7.4),
/// also for a transaction that keeps the value: GUARD on `s(0)'event` turns FALSE at `s(0) <= s(0)`,
/// and a held transaction on `s(1)`, which the condition does not read, changes nothing; also
/// where the kernel skips unchanged assignments that nothing can see.
#[test]
fn guard_runs_when_a_signal_of_its_condition_is_active() {
    for skip_unchanged in [false, true] {
        guard_on_activity(skip_unchanged);
    }
}

fn guard_on_activity(skip_unchanged: bool) {
    let mut b = Builder::new();
    let (e0, e1) = (Range::new(0, 1), Range::new(1, 1));
    let s = b.signal(
        "s",
        SignalType::logic(2),
        &LogicVec::new(2, StdULogic::Zero),
    );
    let g = b.signal(
        "g",
        SignalType::logic(1),
        &LogicVec::new(1, StdULogic::Zero),
    );
    let (ds, dg) = (b.driver(s), b.driver(g));
    let one = StdULogic::One as u8;
    let mut step = 0;
    b.process(
        "stim",
        NONE,
        &[ds],
        p(move |c| {
            // 1 ns: s(0) <= '1' (an event); 2 ns: s(0) <= '1' (held); 3 ns: s(1) <= '0' (held).
            if step > 0 {
                let (r, code) = [(e0, one), (e0, one), (e1, StdULogic::Zero as u8)][step - 1];
                // The direct path where it is taken, which keeps no activity nothing reads.
                if !c.assign_code_now(ds, r, code) {
                    c.assign_code(ds, Some(r), Time::ZERO, code, DelayMode::INERTIAL)?;
                }
            }
            step += 1;
            if step < 4 {
                c.wait_for(ns(1))?;
            }
            Ok(())
        }),
    );
    let pg = b.process(
        "_guard",
        NONE,
        &[dg],
        p(move |c| {
            let code = if c.event(s.slice(0, 1))? {
                one
            } else {
                StdULogic::Zero as u8
            };
            c.assign_code_now(dg, Range::new(0, 1), code);
            c.wait_on(s.slice(0, 1))
        }),
    );
    b.guard_process(pg, &[(s, e0)]);
    b.guard_init(pg);
    let cfg = Config {
        skip_unchanged,
        ..Config::default()
    };
    let (mut sim, trace) = build(b, cfg);
    sim.run_until(ns(5)).unwrap();
    let g: Vec<String> = trace
        .take()
        .into_iter()
        .filter(|l| l.contains("g["))
        .collect();
    assert_eq!(
        g,
        ["1 ns +1 g[0..1]=\"1\"", "2 ns +1 g[0..1]=\"0\""],
        "skip_unchanged {skip_unchanged}"
    );
}
