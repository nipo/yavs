//! Past values from a waveform file, for the console's `examine NAME @ TIME` and `trace x`.
//! The file is read for each question: the questions are rare and the recorder syncs it at
//! every stop. ponytail: no index kept; keep the reader open if a session asks many.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;

use fst_reader::{
    FstFilter, FstHierarchyEntry, FstReader, FstSignalHandle, FstSignalValue, FstVarType,
    FstVhdlDataType,
};

/// How a recorded variable's values are written as VHDL.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Logic,
    Boolean,
    Integer,
    Time,
}

#[derive(Clone)]
struct Var {
    /// Its `FstSignalHandle`.
    index: usize,
    kind: Kind,
    /// Its enumeration table, code to literal.
    table: Option<u64>,
    vector: bool,
}

/// One recorded file.
pub struct History {
    reader: FstReader<BufReader<File>>,
    /// Lower-case dotted name (`top.u1.s`) to variable.
    vars: BTreeMap<String, Var>,
    tables: BTreeMap<u64, Vec<(String, String)>>,
}

impl History {
    /// Read `path`; the message says why it cannot be read.
    pub fn open(path: &str) -> Result<History, String> {
        let bad = |e: &dyn std::fmt::Display| format!("cannot read the wave file `{path}`: {e}");
        let f = File::open(path).map_err(|e| bad(&e))?;
        let mut reader = FstReader::open(BufReader::new(f)).map_err(|e| bad(&e))?;
        let mut vars = BTreeMap::new();
        let mut tables = BTreeMap::new();
        let mut scope: Vec<String> = Vec::new();
        let (mut table, mut info) = (None, None);
        reader
            .read_hierarchy(|e| match e {
                FstHierarchyEntry::Scope { name, .. } => scope.push(name),
                FstHierarchyEntry::UpScope => {
                    scope.pop();
                }
                FstHierarchyEntry::EnumTable {
                    handle, mapping, ..
                } => {
                    tables.insert(handle, mapping);
                }
                FstHierarchyEntry::EnumTableRef { handle } => table = Some(handle),
                FstHierarchyEntry::VhdlVarInfo {
                    type_name,
                    data_type,
                    ..
                } => info = Some((type_name, data_type)),
                FstHierarchyEntry::Var {
                    tpe,
                    name,
                    length,
                    handle,
                    ..
                } => {
                    let short = name.split('[').next().unwrap_or(&name).trim();
                    let mut full = scope.join(".");
                    full.push('.');
                    full.push_str(short);
                    let info = info.take();
                    let kind = match (&info, tpe) {
                        (Some((n, _)), _) if n.eq_ignore_ascii_case("time") => Kind::Time,
                        (_, FstVarType::Integer) => Kind::Integer,
                        (Some((_, FstVhdlDataType::Integer)), _) => Kind::Integer,
                        (Some((_, FstVhdlDataType::Boolean)), _) => Kind::Boolean,
                        _ => Kind::Logic,
                    };
                    vars.insert(
                        full.to_ascii_lowercase(),
                        Var {
                            index: handle.get_index(),
                            kind,
                            table: table.take(),
                            vector: length > 1 || name.contains('['),
                        },
                    );
                }
                _ => {}
            })
            .map_err(|e| bad(&e))?;
        Ok(History {
            reader,
            vars,
            tables,
        })
    }

    /// Every recorded name, lower case.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.vars.keys().map(String::as_str)
    }

    /// The changes of `name` up to and including `upto` (fs), as VHDL text, oldest first; `None`
    /// when it is not recorded.
    pub fn changes(&mut self, name: &str, upto: u64) -> Option<Vec<(u64, String)>> {
        let v = self.vars.get(&name.to_ascii_lowercase())?.clone();
        let table = v.table.and_then(|t| self.tables.get(&t));
        let mut out = Vec::new();
        let filter = FstFilter::new(0, upto, vec![FstSignalHandle::from_index(v.index)]);
        self.reader
            .read_signals(&filter, |t, _, x| {
                if t <= upto {
                    out.push((t, image(x, &v, table)));
                }
                Ok::<(), ()>(())
            })
            .ok()?;
        Some(out)
    }

    /// The value of `name` at `time` (fs): `None` if it is not recorded, `Some(None)` if it has
    /// no value yet.
    pub fn value_at(&mut self, name: &str, time: u64) -> Option<Option<String>> {
        let c = self.changes(name, time)?;
        Some(c.into_iter().next_back().map(|x| x.1))
    }
}

/// One value as VHDL writes it.
fn image(x: FstSignalValue<'_>, v: &Var, table: Option<&Vec<(String, String)>>) -> String {
    let bits = match x {
        FstSignalValue::Real(r) => return format!("{r}"),
        FstSignalValue::String(s) => String::from_utf8_lossy(s).into_owned(),
    };
    if let Some((_, name)) = table.and_then(|t| t.iter().find(|(code, _)| *code == bits)) {
        return name.clone();
    }
    match v.kind {
        Kind::Boolean if bits == "0" => "false".into(),
        Kind::Boolean if bits == "1" => "true".into(),
        Kind::Integer => signed(&bits).map_or(bits, |x| x.to_string()),
        Kind::Time => signed(&bits).map_or(bits, |x| format!("{x} fs")),
        _ if v.vector => format!("\"{}\"", bits.to_ascii_uppercase()),
        _ => format!("'{}'", bits.to_ascii_uppercase()),
    }
}

/// A two's complement bit string of at most 64 bits.
fn signed(bits: &str) -> Option<i64> {
    let n = bits.len();
    if n == 0 || n > 64 {
        return None;
    }
    let u = u64::from_str_radix(bits, 2).ok()?;
    Some(((u << (64 - n)) as i64) >> (64 - n))
}

/// Whether a value as [`History::changes`] writes it has an element that is not a known 0 or 1.
pub fn unknown(text: &str) -> bool {
    let inner = text.trim_matches(['"', '\'']);
    (text.starts_with('"') || text.starts_with('\''))
        && inner
            .chars()
            .any(|c| matches!(c, 'U' | 'X' | 'W' | 'Z' | '-'))
}
