//! "Did you mean" suggestions for declarations (sem.md 8.3); the matching itself lives in
//! `yavs_diag::suggest`, re-exported here.

use yavs_diag::{Diagnostic, Span};

use crate::db::Db;
use crate::decl::{DeclKind, Designator, ObjClass, ObjRole, SubprogramKind};
use crate::ids::DeclId;

impl Db {
    /// What the declaration is, for "a signal with a similar name exists".
    pub(crate) fn decl_noun(&self, d: DeclId) -> &'static str {
        match &self.decls[d].kind {
            DeclKind::Object(o) => match (&o.role, o.class) {
                (ObjRole::Port, _) => "port",
                (ObjRole::Generic, _) => "generic",
                (ObjRole::Param, _) => "parameter",
                (ObjRole::LoopParam | ObjRole::GenerateParam, _) => "loop parameter",
                (_, ObjClass::Constant) => "constant",
                (_, ObjClass::Signal) => "signal",
                (_, ObjClass::Variable) => "variable",
                (_, ObjClass::SharedVariable) => "shared variable",
                (_, ObjClass::File) => "file",
            },
            DeclKind::Subprogram(s) | DeclKind::GenericSubprogram(s) => match s.kind {
                SubprogramKind::Function { .. } => "function",
                SubprogramKind::Procedure => "procedure",
            },
            DeclKind::Type(_) | DeclKind::GenericType(_) => "type",
            DeclKind::Subtype(_) => "subtype",
            DeclKind::EnumLit { .. } => "enumeration literal",
            DeclKind::PhysUnit { .. } => "physical unit",
            DeclKind::Alias(_) => "alias",
            DeclKind::Attribute { .. } => "attribute",
            DeclKind::Component(_) => "component",
            DeclKind::Label(_) => "label",
            DeclKind::Library(_) => "library",
            DeclKind::Unit(_)
            | DeclKind::Package { .. }
            | DeclKind::PackageInst(_)
            | DeclKind::GenericPackage(_) => "design unit",
            _ => "name",
        }
    }

    /// Candidate for [`similar`]: identifier declarations only (not operators or characters).
    pub(crate) fn candidate(&self, d: DeclId) -> Option<(&str, DeclId)> {
        match &self.decls[d].name {
            Designator::Ident(i) => Some((i.key.as_str(), d)),
            _ => None,
        }
    }

    /// Add the suggestion for the declarations `found` to `diag` (primary span `at`): the
    /// help line and, for a single one in the same file, a label at its declaration.
    pub(crate) fn suggest_decls(
        &self,
        mut diag: Diagnostic,
        at: Span,
        found: &[DeclId],
    ) -> Diagnostic {
        if found.is_empty() {
            return diag;
        }
        let shown: Vec<(String, &str)> = found
            .iter()
            .map(|&d| (self.decls[d].name.to_string(), self.decl_noun(d)))
            .collect();
        diag = diag.with_help(help(&shown));
        if let [d] = found {
            let decl = &self.decls[*d];
            let span = decl.name_span();
            if !decl.implicit && span != Span::default() && span.file == at.file {
                diag = diag.with_secondary(span, "similar name declared here");
            }
        }
        diag
    }
}

pub use yavs_diag::suggest::{distance, help, similar, suggest};
