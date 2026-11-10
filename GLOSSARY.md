# Bunode

Bunode manages local Node.js and Bun dev processes on macOS: the ones it starts itself, and the ones it finds already running.

The Chinese UI uses the fixed translations given as _中文_ below. Log contents, commands and paths are never translated.

## Language

### Configuration

**Project**:
A working directory registered in Bunode, usually one containing a `package.json`. It goes by its folder's name, or by the alias the user gives it in Settings. A `package.json` is optional; without one, Services are typed in by hand.
_Avoid_: App, workspace, repo
_中文_: 项目

**Service**:
A named run configuration belonging to a Project, either a package script or a custom command, run in the Project's directory or one of its subdirectories (such as a workspace package). A custom command can be any executable, not only Node or Bun. It is the unit the user starts and stops, and has at most one live Run at a time.
_Avoid_: Run config, task, job, script (a script is only one possible source of a Service)
_中文_: 服务

**Short-lived Service**:
A Service whose command is a build, test or lint package script, so its Run is expected to end by itself. Every other Service is treated as long-running.
_Avoid_: one-shot task, build job
_中文_: 一次性服务

### Runtime

**Run**:
One execution of a Service, from start until exit. Every restart begins a new Run.
_Avoid_: Instance, session, process (a Run may own many processes)
_中文_: 运行

**Supervisor**:
The part of Bunode that starts, watches and stops Runs, and owns their true state.
_Avoid_: Daemon, manager, backend
_中文_: not shown in the UI

**Discovered Process**:
A Node.js or Bun process on the machine that Bunode did not start. An Agent Server is not one.
_Avoid_: External service, unmanaged run
_中文_: 其他进程

**Agent Server**:
A dev server that a coding agent, such as the Claude desktop app, started from its own launch settings in a Project's root folder. Bunode shows it with its Project and can stop it, but never starts it; it is neither a Service nor a Run. The term never appears in the UI.
_Avoid_: Watched Service, Agent Service
