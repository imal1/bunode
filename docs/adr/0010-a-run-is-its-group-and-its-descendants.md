# A Run is its process group and every descendant

ADR-0003 has the runner put the Service's command in a process group of its own and end that group. That was taken to mean "a Run's processes are its group". pnpm 12 broke it: `pnpm dev` runs the script through `sh` in a new process group, so the dev server left the Run's group. Its port went unseen, and a force kill, a killed runner or a SIGTERM the launcher doesn't pass on left the server running. A Run's processes are now its group plus every process whose parent is one of them (`bunode_core::tree`), read from the process table whenever ports are scanned or the Run is signalled. The members outside the group are remembered by PID and start time, so a SIGKILL after the stop timeout still reaches them once their parent is gone, and are only signalled while they are still the same process. One that leads a group of its own is signalled as a group, which also catches its newest children.

## Considered Options

- **Keep only the group, and wait for pnpm to change**: rejected; nothing stops any other launcher doing the same, and ADR-0003 promises that nothing outlives a Run.
- **Have the runner adopt orphans (Linux's child subreaper)**: macOS has no such thing.
- **The runner rescans the process table every second while the Run is up**: would also find descendants whose parent died long before the Run is stopped, but every Run would poll forever, against the idle CPU budget. Rejected for now.

## Consequences

Reading the process table on each signal costs a few milliseconds; the discovery scan already reads it. A descendant that left the group and whose parent exits while the Run is still up (a deliberate daemon) has nothing left leading to it: it is not part of the Run, and shows up as a Discovered Process instead. The next launch's leftover clean-up (ADR-0003) still only knows the group; descendants outside it survive if Bunode and the runner are both killed. Since only the runner remembers members whose parent is gone, Bunode asks for a stop with a `stop` line on the lifeline instead of closing it, so a force kill after a stop still reaches the runner. `bunode-runner` now depends on `bunode-core`.
