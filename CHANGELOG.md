# Changelog

## Unreleased

- Fixed: a Service whose command starts its server in a new process group, like `pnpm dev` with pnpm 12, now shows the server's port, and Stop, Force Stop, a crash of Bunode or a killed runner end the server too (ADR-0009).
