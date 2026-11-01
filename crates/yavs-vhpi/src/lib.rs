//! IEEE 1076-2019 VHPI for yavs: the C ABI (`vhpi_user.h`), the exported
//! `vhpi_*` symbols and plugin loading.
//!
//! There is no separate VHPI library. The simulator executable exports
//! the symbols listed in `src/exports.txt`: on Linux through a linker
//! version script, on Windows through a `.def` file; no import
//! library is built or shipped. `build.rs` generates
//! those files; see `docs/decisions.md` for the reasoning.
//!
//! The object model behind the ABI: the design hierarchy (`design`, a
//! snapshot of the elaborated design) and the tool. Values follow in M5.
//! Simulation-phase and after-delay callbacks run from a kernel observer
//! (`VhpiObserver`).
//!
//! Unsafe code: a C ABI cannot be written without it, so this crate does
//! not inherit the workspace `forbid(unsafe_code)` (see Cargo.toml). It is
//! confined to `abi` (the exported functions) and `plugin` (dlopen and
//! calling the plugin's entry points); the other modules hold no unsafe.

pub mod sys;

mod abi;
mod callbacks;
mod connect;
mod design;
mod foreign;
mod model;
mod plugin;
mod sim;
mod types;
mod value;

pub use model::Control;
pub use plugin::{
    ForeignLib, LoadError, PluginSpec, bind_foreign, divert_stdin, divert_stdout, load,
    load_foreign,
};
pub use sim::VhpiObserver;
pub use yavs_ffi::ArgLayout;

/// Keeps every exported `vhpi_*` symbol linked into the executable. An
/// executable that exports them calls this once at startup; without a
/// reference the linker drops the symbols before the export list applies.
pub fn keep_exports() {
    std::hint::black_box(abi::exported_symbols());
}

/// Sets the design plugins see: the elaborated hierarchy of `e`, with names from `db`. Call
/// it before loading plugins (their startup routines may already walk the design).
pub fn set_design(db: &yavs_sem::Db, e: &yavs_elab::Elaborated) {
    let d = design::Design::build(db, e);
    abi::with_state(|s| s.design = d);
}

/// Sets the interpreter side of the simulation about to run (`Elaborated::build_full`):
/// plugins read and write shared variables through it (#309).
/// Runs the models of the design's VHPI foreign architectures (#330) after the plugins
/// loaded: their `elabf`, then their `execf`. The processes and drivers they created go into
/// `Elaborated::ext_procs` before the build. An instance whose model no plugin registered
/// is `E0905`.
pub fn elaborate_foreign(
    e: &yavs_elab::Elaborated,
) -> Result<Vec<yavs_elab::ExtProc>, Box<yavs_diag::Diagnostic>> {
    foreign::elaborate(e).map_err(|m| {
        Box::new(yavs_diag::Diagnostic::error("E0905", m).with_note(
            "register it with `vhpi_register_foreignf` (`vhpiArchF`) in a plugin given as \
             `--vhpi=LIB`",
        ))
    })
}

pub fn set_runtime(b: yavs_elab::Built) {
    abi::with_state(|s| s.built = Some(b));
}

/// Sets the tool's arguments that plugins read (`vhpiArgcP`, `vhpiArgvs`).
pub fn set_argv(argv: Vec<String>) {
    abi::with_state(|s| s.argv = argv);
}

/// Sets a design of only a root instance called `name` (a host without an elaborated design).
pub fn set_design_root(name: &str) {
    abi::with_state(|s| s.design = design::Design::named(name));
}

/// Sets the resolution limit (`yavs run --resolution`): VHPI times count in
/// this unit. Femtoseconds until set.
pub fn set_resolution(unit: yavs_kernel::Unit) {
    abi::with_state(|s| s.clock.resolution = unit);
}

/// Runs the enabled callbacks of the phase `reason` (a `vhpiCb*` constant
/// without `Rep`) outside a simulation, and returns how many ran. A
/// simulation uses `VhpiObserver` instead.
pub fn run_callbacks(reason: i32) -> usize {
    abi::fire(&abi::with_state(|s| s.cbs.due(reason)))
}

/// The phases around elaboration (#832). Plugins load after yavs has elaborated, so what a plugin
/// sees as elaboration is the part it takes part in: `vhpiCbStartOfTool` and
/// `vhpiCbStartOfElaboration` when all plugins have loaded, then the foreign models.
pub fn start_of_elaboration() {
    run_callbacks(sys::vhpiCbStartOfTool);
    abi::with_state(|s| s.tool_phase = model::ToolPhase(sys::vhpiElaborationPhase));
    run_callbacks(sys::vhpiCbStartOfElaboration);
}

/// `vhpiCbEndOfElaboration`: the foreign models are bound, the initialization phase follows.
pub fn end_of_elaboration() {
    run_callbacks(sys::vhpiCbEndOfElaboration);
    abi::with_state(|s| s.tool_phase = model::ToolPhase(sys::vhpiInitializationPhase));
}

/// `vhpiCbEndOfTool`, the last thing plugins see of a run that got as far as loading them
/// (once). Reads of signals are errors here: the simulation has no observer call left.
pub fn end_of_tool() {
    abi::with_state(|s| s.tool_phase = model::ToolPhase(sys::vhpiTerminationPhase));
    run_callbacks(sys::vhpiCbEndOfTool);
}

/// `vhpiCbEnterInteractive` or `vhpiCbExitInteractive`: a debugger stops or goes on.
pub fn interactive(enter: bool) {
    run_callbacks(if enter {
        sys::vhpiCbEnterInteractive
    } else {
        sys::vhpiCbExitInteractive
    });
}

/// Ctrl-C ends this run (safe from the signal thread): `vhpiCbSigInterrupt` as it ends.
pub fn note_interrupt() {
    sim::INTERRUPTED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Why this run cannot do `vhpi_control(vhpiReset)`; set before plugins load.
pub fn set_reset_refusal(why: Option<String>) {
    abi::with_state(|s| s.reset_refused = why);
}

/// The seed of `--shuffle` that plugins read (`vhpiRandomSeedP`); set before plugins load.
pub fn set_shuffle_seed(seed: Option<u64>) {
    abi::with_state(|s| s.shuffle_seed = seed);
}

/// A plugin asked for a reset and the run has ended: build the simulation again, subscribe
/// [`VhpiObserver`], then call [`reset_done`].
pub fn reset_pending() -> bool {
    abi::with_state(|s| s.reset == model::Reset::Asked)
}

/// The new simulation is built; what belonged to the old one is forgotten.
pub fn reset_done() {
    abi::with_state(|s| {
        s.reset = model::Reset::Built;
        s.puts.clear();
        s.foreign.pending.clear();
        s.control = None;
        s.clock.now = yavs_kernel::Time::ZERO;
        s.clock.delta = 0;
        s.cbs.reset();
    });
}

/// Takes the pending `vhpi_control` request, if a plugin made one.
pub fn take_control() -> Option<Control> {
    abi::with_state(|s| s.control.take())
}

/// The model a plugin registered with `vhpi_register_foreignf` for the `'FOREIGN "VHPI library
/// model"` subprogram `library model` (a function when `function`), for the elaborator to
/// bind. Runs the model's elaboration callback. `Ok(None)`: nothing registered under that
/// name; `Err`: registered as the other kind of subprogram.
pub fn foreign_model(
    library: &str,
    model: &str,
    function: bool,
) -> Result<Option<std::sync::Arc<dyn yavs_interp::ForeignModel>>, String> {
    foreign::model(library, model, function)
}
