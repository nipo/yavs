# Changelog

## Unreleased

- "Show Drivers" in the context menu of a signal, and "Find Signals or Variables...": a quick pick of matches to add to the wave.
- The Breakpoints view has filters that stop at a report of a severity, with the stack intact.
- A launch that fails early shows why: the diagnostic in the Debug Console, or a notification with the command when yavs refuses its command line.
- The Debug Console completes commands, names, fields and source files, and shows the exit status at the end of a session.
- A Watch takes a field, an element, a slice, `.all` and expressions; the Variables view expands access values and lists the variables of a protected object.
- The problem matcher `$yavs` puts the diagnostics of a yavs task in the Problems view.
- The extension is released with yavs and carries its version (one release with every artifact).

## 0.1.0

First release. The `yavs` debugger type (launch and attach over the Debug Adapter Protocol),
the `vhdl` language id for breakpoints, and the commands for running for or until a time,
stepping a delta or a time step, signal value breakpoints, and force, deposit and release.
Needs yavs 0.11.0 or newer.
