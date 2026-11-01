//! Clock-edge idioms (#632): every one lowers to one `Inst::Edge`, which the interpreter, the
//! LLVM tier and any later backend implement. The shapes are `Db::edge_shape` (the same ones
//! the elaborator makes edge triggers of), and the level of a `wait until` (below).

use yavs_ir::{EdgeKind, Inst, Reg, RegTy};
use yavs_sem::ExprId;
use yavs_sem::hir::{Actual, ExprKind, Lsp, ObjRoot};

use crate::func::Fx;
use crate::{R, ty};

impl Fx<'_, '_> {
    /// The edge test `e` is, as an `Inst::Edge` result; `None` when `e` is not an edge idiom
    /// (or the signal is not a scalar `std_ulogic`, BIT or BOOLEAN name), and is lowered as the
    /// expression it is.
    pub fn edge_test(&mut self, e: ExprId) -> R<Option<Reg>> {
        let Some((sig, kind)) = self.edge_idiom(e) else {
            return Ok(None);
        };
        self.edge_of_name(sig, kind)
    }

    /// The signal name and kind of the edge idiom `e` (`None`: lowered as the expression it is).
    fn edge_idiom(&self, e: ExprId) -> Option<(ExprId, EdgeKind)> {
        let db = self.db;
        let sh = db.edge_shape(e)?;
        // The IEEE function runs from its VHDL body under `YAVS_NO_NATIVE_IEEE` (the check that
        // the natives change nothing).
        if !sh.legacy && self.cx.opts.no_native_ieee && !self.is_predefined_edge(e) {
            return None;
        }
        // `x'event and x = '1'` names the signal twice; the one test needs it to be the same.
        if sh.other.is_some_and(|o| !db.same_name(sh.sig, o)) {
            return None;
        }
        let kind = match (sh.legacy, sh.rising) {
            (false, true) => EdgeKind::Rising,
            (false, false) => EdgeKind::Falling,
            (true, true) => EdgeKind::To1,
            (true, false) => EdgeKind::To0,
        };
        Some((sh.sig, kind))
    }

    /// The edge test an `and` conjunct of the `wait until` condition `until` is, if any: the
    /// condition is false in every cycle without that edge, so the wait may resume its process
    /// only in cycles with it (`Inst::WaitEdge`). Emitted before the `WaitOn`s. Not in a
    /// postponed process, which resumes in a later cycle than the event, and not where
    /// coverage counters count the evaluations of the condition.
    pub fn wait_edge(&mut self, until: ExprId) -> R<()> {
        if self.postponed || self.cx.opts.any_coverage() {
            return Ok(());
        }
        let Some((x, kind)) = self.edge_conjunct(until).and_then(|e| self.edge_idiom(e)) else {
            return Ok(());
        };
        if let Some(sig) = self.edge_sig(x)? {
            self.emit(Inst::WaitEdge { sig, kind });
        }
        Ok(())
    }

    /// `e`, or the first operand of the predefined `and`s it is made of that is an edge idiom,
    /// when nothing to its left can do anything but read: the wait skips the evaluations without
    /// an edge, in which a false edge test stops at its short-circuit but what stands left of it
    /// still runs (#1022).
    fn edge_conjunct(&self, e: ExprId) -> Option<ExprId> {
        let db = self.db;
        if db.edge_shape(e).is_some() {
            return Some(e);
        }
        let ExprKind::Call {
            callee,
            args,
            method_of: None,
            ..
        } = &db.exprs[e].kind
        else {
            return None;
        };
        if !db.decls[*callee].implicit || db.decls[*callee].name.key().as_str() != "\"and\"" {
            return None;
        }
        for a in args {
            let Actual::Expr(x) = a.actual else {
                return None;
            };
            if let Some(found) = self.edge_conjunct(x) {
                return Some(found);
            }
            if !self.only_reads(x) {
                return None;
            }
        }
        None
    }

    /// Whether evaluating `e` has no effect and cannot fail: names, constants and predefined
    /// relations and logic on them. A user function may report or keep state, `ieee` ones warn on
    /// metavalues, an index or arithmetic may fail.
    fn only_reads(&self, e: ExprId) -> bool {
        use yavs_sem::decl::Body;
        use yavs_sem::implicit::Family as F;
        let db = self.db;
        let mut ok = true;
        db.walk_expr(e, &mut |x| {
            ok &= db.exprs[x].value.is_some()
                || match &db.exprs[x].kind {
                    ExprKind::Obj(_) | ExprKind::Const(_) => true,
                    ExprKind::Call { callee, .. } => {
                        db.decls[*callee].kind.subprogram().is_some_and(|sp| {
                            matches!(&sp.body, Body::Implicit(op) if matches!(
                                op.family,
                                F::Eq | F::Neq | F::Lt | F::Le | F::Gt | F::Ge | F::And | F::Or
                                    | F::Nand | F::Nor | F::Xor | F::Xnor | F::Not
                            ))
                        })
                    }
                    _ => false,
                };
        });
        ok
    }

    /// The level `x = '1'` / `'0'` of a `wait until` that waits on `x` alone and is resumed by
    /// its events: an event ending on that level. Not in a postponed process (it may resume in
    /// a later cycle than the event) or a procedure (shared by both), and not with a timeout.
    pub fn wait_level(&mut self, until: ExprId, sens: &[Lsp], timeout: bool) -> R<Option<Reg>> {
        let db = self.db;
        if timeout || self.postponed || self.kind != yavs_ir::FuncKind::Process {
            return Ok(None);
        }
        let Some((x, pos @ (2 | 3))) = db.level_of(until) else {
            return Ok(None);
        };
        let ExprKind::Obj(d) = db.exprs[db.unaliased(x)].kind else {
            return Ok(None);
        };
        match sens {
            [l] if l.root == ObjRoot::Decl(d) && l.path.is_empty() => {}
            _ => return Ok(None),
        }
        self.edge_of_name(
            x,
            if pos == 3 {
                EdgeKind::To1
            } else {
                EdgeKind::To0
            },
        )
    }

    /// Whether `e` is the predefined `rising_edge` / `falling_edge` of BIT or BOOLEAN.
    fn is_predefined_edge(&self, e: ExprId) -> bool {
        matches!(&self.db.exprs[e].kind,
            ExprKind::Call { callee, .. } if self.db.decls[*callee].implicit)
    }

    /// `Inst::Edge` of the signal name `x`, when it is a name of one `std_ulogic`, BIT or
    /// BOOLEAN element.
    fn edge_of_name(&mut self, x: ExprId, kind: EdgeKind) -> R<Option<Reg>> {
        let Some(sig) = self.edge_sig(x)? else {
            return Ok(None);
        };
        let dst = self.reg(RegTy::I64);
        self.emit(Inst::Edge { dst, kind, sig });
        Ok(Some(dst))
    }

    /// The signal reference of the name `x`, when it is a name of one `std_ulogic`, BIT or
    /// BOOLEAN element.
    fn edge_sig(&mut self, x: ExprId) -> R<Option<Reg>> {
        let db = self.db;
        let span = db.exprs[x].span;
        let mut n = x;
        loop {
            match &db.exprs[n].kind {
                ExprKind::Obj(_) => break,
                ExprKind::Index { prefix, .. } | ExprKind::Field { prefix, .. } => n = *prefix,
                _ => return Ok(None),
            }
        }
        let base = db.base(db.exprs[x].subtype);
        if ty::is_array(db, base) {
            return Ok(None);
        }
        let tid = self.ty(base, span)?;
        if !self.is_logic(tid) {
            return Ok(None);
        }
        Ok(Some(self.sig_expr(x)?))
    }
}
