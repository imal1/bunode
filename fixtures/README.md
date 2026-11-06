# Fixtures

Small Node.js and Bun programs that the Rust integration tests start for
real (requirements §8.2). Each folder can be registered as a Project.

| Folder | What it does |
|---|---|
| `node-http/` | `server.js`: Node HTTP server on `$PORT` (0 = any free port); prints `listening on <port>` |
| `bun-http/` | `server.ts`: the same with `Bun.serve` |
| `process-tree/` | `tree.js N FILE`: spawns a chain of N descendants and appends every PID to FILE (or `$TREE_PIDS`); the `tree` script runs it through a package manager |
