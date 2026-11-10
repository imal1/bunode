# Fixtures

Small Node.js and Bun programs, and one plain executable, that the Rust
integration tests start for real (requirements §8.2). Each folder can be
registered as a Project.

| Folder | What it does |
|---|---|
| `node-http/` | `server.js`: Node HTTP server on `$PORT` (0 = any free port); prints `listening on <port>` |
| `bun-http/` | `server.ts`: the same with `Bun.serve` |
| `process-tree/` | `tree.js N FILE`: spawns a chain of N descendants and appends every PID to FILE (or `$TREE_PIDS`); the `tree` script runs it through a package manager |
| `crash/` | `immediately.js` exits with code 1 at once; `later.js` runs for half a second, then throws |
| `flood/` | `flood.js [N]` prints N lines (default: forever) as fast as it can, every 10th to stderr, every 7th colored |
| `monorepo-npm/`, `monorepo-yarn/`, `monorepo-pnpm/`, `monorepo-bun/` | The same monorepo declared each package manager's way: `apps/web` (script `where` prints its folder) and `packages/ui` |
| `multi-port/` | `server.js` listens on `$PORT` and on a second free port, and prints both |
| `plain-tool/` | No `package.json`. `my tools/serve` is a shell script, not Node or Bun: prints `plain tool started` and its arguments, then serves HTTP on a free port with `python3` |
