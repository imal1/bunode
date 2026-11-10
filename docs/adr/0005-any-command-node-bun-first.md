# Services run any command; recognition stays Node/Bun only

Users want to start and stop non-JavaScript dev tools from the same list, for example napcat's `'/Applications/QQ.app/Contents/MacOS/QQ' --no-sandbox`. A Service's custom command may now be any executable. Its Run gets the same lifecycle as a Node or Bun command: the per-Run runner and process group (ADR-0003), logs, port detection and auto restart. Everything that has to understand the command stays Node/Bun only: reading `package.json` scripts, the common-script presets, Short-lived Service detection, runtime detection and runtime path overrides, and process discovery for Other processes. Bunode still only manages what it starts itself and never claims a process that is already running.

## Considered Options

- **Keep Services limited to Node/Bun**: rejected; the runner, logs and stop logic never depended on the runtime, so the limit only pushed users back to a terminal for one or two commands.
- **Also discover and adopt any process**: rejected; Bunode would turn into a system-wide process browser, which is a non-goal, and adopting a process it did not start breaks the guarantee that every Run can be stopped and restarted reliably.

## Consequences

The product line becomes "Node/Bun first, any command": principle 2 in the requirements is reworded, and "system-wide process browser" is added to the non-goals. A Project no longer needs a `package.json`; without one its Services are typed in by hand.
