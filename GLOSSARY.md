# Bunode

Bunode manages local Node.js and Bun dev processes on macOS: the ones it starts itself, and the ones it finds already running.

## Language

### Configuration

**Project**:
A working directory registered in Bunode, usually one containing a `package.json`. It goes by its folder's name, or by the alias the user gives it in Settings.
_Avoid_: App, workspace, repo

**Service**:
A named run configuration belonging to a Project, either a package script or a custom command, run in the Project's directory or one of its subdirectories (such as a workspace package). It is the unit the user starts and stops, and has at most one live Run at a time.
_Avoid_: Run config, task, job, script (a script is only one possible source of a Service)

**Short-lived Service**:
A Service whose command is a build, test or lint package script, so its Run is expected to end by itself. Every other Service is treated as long-running.
_Avoid_: one-shot task, build job

### Runtime

**Run**:
One execution of a Service, from start until exit. Every restart begins a new Run.
_Avoid_: Instance, session, process (a Run may own many processes)

**Supervisor**:
The part of Bunode that starts, watches and stops Runs, and owns their true state.
_Avoid_: Daemon, manager, backend

**Discovered Process**:
A Node.js or Bun process on the machine that Bunode did not start.
_Avoid_: External service, unmanaged run
