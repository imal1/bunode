# Runs never outlive Bunode, enforced by a per-Run runner

When Bunode quits, crashes or is killed, every Run must end. macOS has no parent-death signal, so Bunode never spawns a Service's command directly: it spawns a tiny `bunode-runner` binary that creates the process group, starts the command inside it, and holds a lifeline pipe to the app. When that pipe closes for any reason, the runner sends SIGTERM to the group, escalates to SIGKILL after the timeout, and exits. The command's stdout/stderr go straight to the app, not through the runner. As a fallback, the Supervisor records each Run's PID, process group and start time; on the next launch, any survivor whose identity still matches is killed without asking.

## Considered Options

- **Re-attach or offer to keep leftover Runs after a crash**: rejected; their log pipes are gone and the state is murky. A clean slate is easier to trust.
- **One global reaper process**: rejected; a Run started just before a crash, but not yet registered with the reaper, would leak. The per-Run runner owns its group from the first instant, so there is no such window.

## Consequences

Every Run costs one extra small process, which shows up in process listings. A future standalone Supervisor daemon (see ADR-0002) would hold the lifeline instead of the app, so Runs could then outlive the GUI by design.
