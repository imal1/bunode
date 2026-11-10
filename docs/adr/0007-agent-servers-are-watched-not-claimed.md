# Agent Servers are shown and stoppable, never claimed

Coding agents such as the Claude desktop app start dev servers from their own launch settings (`.claude/launch.json`) inside a Project. Users want to see these servers next to their Services and stop them from Bunode, while the agent keeps owning them. Bunode recognizes an Agent Server only when all three hold: the process is the agent's launcher (for Claude, `Claude.app/Contents/Helpers/disclaimer` under the Claude main executable, excluding `--pgroup` MCP launches); its cwd equals a registered Project root exactly; and its argv matches a configuration in that Project's launch file. Ports are never used for matching. Bunode can stop an Agent Server (SIGTERM to its process group, then SIGKILL after the stop timeout, with no confirmation), but never starts, restarts or logs it, never counts it as a Run, and leaves it running when Bunode quits. Each supported agent is one row in an adapter table: launcher signature, config file location and format, argv rule. Claude is the only row in v1.

## Considered Options

- **Treat them as Discovered Processes**: rejected; they would sit in Other processes away from their Project, and stopping one would need the SIGKILL confirmation meant for processes nobody owns.
- **Import launch configurations as Services**: rejected; a server Bunode starts is not supervised by the agent, so the agent's own view of it would be wrong, and the user loses the agent-side control they wanted to keep.
- **Match by port**: rejected; any process holding the same port would be mistaken for the agent's server.

## Consequences

This is a deliberate, narrow exception to "never claim a running process": Bunode only shows and stops these servers. Stopping one bypasses the SIGKILL confirmation because the user asked for it from a row that belongs to their Project, and the agent simply sees the server exit (Claude records code 143). The detection depends on the agent's process layout, so each adapter needs re-checking when the agent updates. A global toggle in Settings › Projects turns the feature off; the processes then fall back to Other processes if they are Node or Bun.
