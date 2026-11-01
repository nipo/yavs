//! Compiled code kept across runs (#481), on the design of the LLVM tier's `objcache.rs`: the
//! machine code of each Cranelift function is stored in the `yavs-cache` objects store of the work
//! library ([`set_cache`]) and the next function with the same body links it instead of being
//! compiled (`Jit::define_code`).
//!
//! The key is Cranelift's own (`incremental_cache::compute_cache_key`: the function body and the
//! target, not the names of the functions and data it refers to, which are relocations the module
//! resolves when it links the code) with the yavs build and the optimiser's switches
//! (`yavs_ir::opt::fingerprint`). The body is what the lowering and the inliner made, so a hit is
//! the code a compile would make. A missing, damaged or foreign entry is a miss, a write that fails
//! is silent.
//!
//! The body also says which native callees it calls directly (and may inline) and which through
//! `Mem::ntab`. Under `--tier=clift-auto` that follows what the policy compiles with the function,
//! a function of the samples, so [`Routes`] keeps the first answer: an unchanged design then makes
//! the same bodies every run (#1035).

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use cranelift_codegen::incremental_cache::{CacheKvStore, compute_cache_key};
use cranelift_codegen::ir::Function;
use cranelift_codegen::isa::TargetIsa;
use yavs_cache::{Key, KeyBuilder, Store};

const KIND: &str = "clif";

// ponytail: one store per process, as the LLVM tier's (a run simulates one design).
static STORE: Mutex<Option<Store>> = Mutex::new(None);
static HITS: AtomicUsize = AtomicUsize::new(0);
static STORED: AtomicUsize = AtomicUsize::new(0);

/// Keep the compiled code in the cache of library directory `lib` from now on (`None`: compile
/// every function).
pub fn set_cache(lib: Option<&Path>) {
    *STORE.lock().unwrap_or_else(PoisonError::into_inner) = lib.map(Store::objects_of_library);
}

/// Functions linked from the cache, and functions compiled and stored, so far in this process.
pub fn cache_stats() -> (usize, usize) {
    (HITS.load(Ordering::Relaxed), STORED.load(Ordering::Relaxed))
}

pub(crate) fn store() -> Option<Store> {
    STORE.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

pub(crate) fn note_hit() {
    HITS.fetch_add(1, Ordering::Relaxed);
}

/// The store as Cranelift's cache of compiled functions.
pub(crate) struct Kv(pub Store);

impl Kv {
    fn key(k: &[u8]) -> Key {
        let mut b = KeyBuilder::new(KIND);
        b.text(&yavs_ir::opt::fingerprint()).part(k);
        b.finish()
    }

    /// Whether the store has the compiled code of `func`.
    pub(crate) fn has(&self, isa: &dyn TargetIsa, func: &Function) -> bool {
        // (The hash has no accessor, only its `Display`: `CacheKeyHash:[b0, b1, ...]`.)
        let text = compute_cache_key(isa, func).to_string();
        let bytes: Option<Vec<u8>> = text
            .trim_start_matches("CacheKeyHash:[")
            .trim_end_matches(']')
            .split(", ")
            .map(|b| b.parse().ok())
            .collect();
        bytes.is_some_and(|k| {
            k.len() == 32
                && self
                    .0
                    .payload_len(KIND, &Self::key(&k))
                    .is_some_and(|n| n > 0)
        })
    }
}

impl CacheKvStore for Kv {
    fn get(&self, key: &[u8]) -> Option<Cow<'_, [u8]>> {
        self.0
            .get(KIND, &Self::key(key))
            .map(|e| Cow::Owned(e.to_vec()))
    }

    fn insert(&mut self, key: &[u8], val: Vec<u8>) {
        self.0.put(KIND, &Self::key(key), &val);
        STORED.fetch_add(1, Ordering::Relaxed);
    }
}

/// The kind of the list of compiled functions (`compiled_list`), shared with the LLVM tier's list
/// under its own key.
const LIST: &str = "list";

/// The most functions the list names (the most recent ones).
const LIST_MAX: usize = 1024;

fn list_key() -> Key {
    let mut k = KeyBuilder::new(LIST);
    k.text("clift functions");
    k.finish()
}

/// The functions `--tier=clift-auto` stored compiled code of, as name and number of IR
/// instructions, most recent first: the policy looks up only these (`Worker::probe`).
pub(crate) fn compiled_list() -> std::collections::HashSet<(String, u32)> {
    let Some(e) = store().and_then(|s| s.get(LIST, &list_key())) else {
        return Default::default();
    };
    String::from_utf8_lossy(&e)
        .lines()
        .filter_map(|l| {
            let (len, name) = l.split_once('\t')?;
            Some((name.to_string(), len.parse().ok()?))
        })
        .collect()
}

/// Put `name` (of `len` IR instructions) in front of the list of `compiled_list`.
pub(crate) fn add_compiled(name: &str, len: u32) {
    let Some(store) = store() else {
        return;
    };
    let line = format!("{len}\t{name}\n");
    let mut text = line.clone();
    if let Some(old) = store.get(LIST, &list_key()) {
        let old = String::from_utf8_lossy(&old);
        for l in old.split_inclusive('\n').take(LIST_MAX - 1) {
            if l != line {
                text += l;
            }
        }
    }
    store.put(LIST, &list_key(), text.as_bytes());
}

/// The kind of the entry of [`Routes`].
const ROUTES: &str = "route";

/// How a function reaches its native callees: per callee id, directly (`true`) or through
/// `Mem::ntab`.
pub(crate) type Route = Rc<HashMap<u32, bool>>;

/// The routes by function and whether it is the native-frame variant.
type Known = HashMap<(u64, bool), Route>;

/// The first route of each function compiled in a program (`--tier=clift-auto`), one text entry per
/// program in the objects store, a line `<function> <0|1 native> <callee>+|- ...` each. A function is
/// named by its id, a copy of a process by its `Program::copy_tag`: its id depends on the run. Both
/// routes are the same semantics, so a line applied to another program only mis-steers a call.
#[derive(Default)]
pub(crate) struct Routes {
    /// The entry's key and what it held at the first look; `None`: not looked yet.
    loaded: Option<(Key, Known)>,
    /// The lines not stored yet.
    fresh: String,
}

impl Routes {
    fn parse(text: &str) -> Known {
        let line = |l: &str| {
            let mut w = l.split(' ');
            let f = w.next()?.parse().ok()?;
            let native = w.next()? == "1";
            let calls = w
                .map(|c| {
                    let (g, how) = c.split_at_checked(c.len().checked_sub(1)?)?;
                    Some((g.parse().ok()?, how == "+"))
                })
                .collect::<Option<HashMap<u32, bool>>>()?;
            Some(((f, native), Rc::new(calls)))
        };
        text.lines().filter_map(line).collect()
    }

    /// The route stored for a function of `prog`.
    pub(crate) fn get(&mut self, prog: &yavs_ir::Program, f: u64, native: bool) -> Option<Route> {
        let loaded = self.loaded.get_or_insert_with(|| {
            let mut k = KeyBuilder::new(ROUTES);
            k.text(&yavs_ir::opt::fingerprint())
                .part(&yavs_llvm::program_digest(prog).to_le_bytes());
            let k = k.finish();
            let known = store()
                .and_then(|s| s.get(ROUTES, &k))
                .map(|e| Self::parse(&String::from_utf8_lossy(&e)))
                .unwrap_or_default();
            (k, known)
        });
        loaded.1.get(&(f, native)).cloned()
    }

    /// Remember the route a function of no stored route was just made with.
    pub(crate) fn add(&mut self, f: u64, native: bool, calls: impl Iterator<Item = (u32, bool)>) {
        let mut calls: Vec<_> = calls.collect();
        calls.sort_unstable();
        let mut line = format!("{f} {}", u8::from(native));
        for (g, direct) in &calls {
            line += &format!(" {g}{}", if *direct { '+' } else { '-' });
        }
        self.fresh += &line;
        self.fresh.push('\n');
        if let Some((_, known)) = &mut self.loaded {
            known.insert((f, native), Rc::new(calls.into_iter().collect()));
        }
    }

    /// Store the new lines behind what the entry holds now: another run's line for a function wins,
    /// since it is the first one.
    pub(crate) fn flush(&mut self) {
        let (Some((k, _)), Some(store)) = (&self.loaded, store()) else {
            return;
        };
        if self.fresh.is_empty() {
            return;
        }
        let mut text = store
            .get(ROUTES, k)
            .map(|e| String::from_utf8_lossy(&e).into_owned())
            .unwrap_or_default();
        let there = Self::parse(&text);
        for l in std::mem::take(&mut self.fresh).lines() {
            if Self::parse(l).keys().all(|f| !there.contains_key(f)) {
                text += l;
                text.push('\n');
            }
        }
        store.put(ROUTES, k, text.as_bytes());
    }
}
