# Stability and performance

How Bunode's stability requirements (requirements §5, §8.3) are checked,
and how to measure the performance targets of §5.2.

## Automated, on every CI run

`make test` runs these on the macOS runner, with real node, bun, npm and
pnpm processes from `fixtures/`:

| Requirement | Test |
|---|---|
| Start and stop one Service 100 times: no duplicate Run, right final state, nothing left behind | `crates/bunode-runner/tests/stability.rs` › `a_hundred_starts_and_stops_leave_nothing_behind` |
| 10 and 30 Services at once; status updates usually within 1 s | `ten_services_at_once`, `thirty_services_at_once` (print median and 90th percentile Start → Running) |
| Rapid Start/Stop clicks never make two Runs | `failures.rs` › `rapid_start_and_stop_never_leaves_two_runs`, `restart.rs` › `repeated_restarts_never_leave_two_runs` |
| A Run that fails or crashes never stays in Starting / Running | `failures.rs` |
| Stopping a parent leaves no Node/Bun children | `runs.rs` › `stop_ends_every_process_of_the_group`, `stop_ends_processes_started_by_package_managers` |
| Bunode crashing or being killed leaves no Run behind | `lifetime.rs` › `killing_the_app_ends_its_runs`; `lifeline.rs` |
| Leftovers are ended on the next launch, reused PIDs never touched | `lifetime.rs` › `the_next_launch_ends_leftovers_and_spares_reused_pids` |
| A flood of logs stays bounded and coalesced | `logs.rs` › `a_flood_of_output_stays_bounded_and_coalesced` |
| A flood of logs doesn't block the main thread | `macos/BunodeTests/LogTests.swift` › `mainThreadWorkStaysShortUnderAFlood` (slowest main-thread tick under `yes`, printed) |
| Final states match what the Supervisor reports | every test above waits on the Supervisor's snapshot and then checks the processes themselves |

## Long soaks, on demand

The long tests are `#[ignore]`d. `BUNODE_SOAK_SECS` sets their length.

```bash
make soak
```

```bash
BUNODE_SOAK_SECS=1800 cargo test --release -p bunode-runner --test stability -- --ignored --nocapture soak_a_flood_of_logs
```

```bash
BUNODE_SOAK_SECS=86400 cargo test --release -p bunode-runner --test stability -- --ignored --nocapture soak_runs_for_a_day
```

- `soak_a_flood_of_logs` (default 30 minutes): `fixtures/flood` writes as
  fast as it can. Every 10 s it prints the test process's resident memory,
  lines so far and lines dropped; at the end lines per second, change
  reports per second and peak memory. It fails if the Run stops, the log
  grows past its bound, reports exceed about ten a second, or memory grows
  more than 64 MB over the baseline.
- `soak_runs_for_a_day` (default 24 hours): ten Services are started,
  restarted and stopped in turn every 200 ms. After every round each
  Service must be Running or Stopped with at most one Run, every ended
  Run's process group must be empty, and every 50 rounds memory must stay
  within 64 MB of the baseline.

The Soak workflow (`.github/workflows/soak.yml`) runs the 30-minute log
soak and one hour of the Run soak every week and on demand from the
Actions tab. GitHub's runners stop jobs after 6 hours, so the full 24-hour
soak runs on a Mac: `make soak` with `BUNODE_SOAK_SECS=86400`.

Results of the last local run (Linux container, 40 s each, release build):
785,000 lines/s through the log path, 9.9 change reports/s, memory 60 MB at
the start and 64 MB at the peak; 100 rounds of the Run soak with memory
flat.

## Measuring the targets of §5.2

These are acceptance targets, measured on a release build
(`make app`, then `make run`).

| Target | How to measure |
|---|---|
| Cold start under 1 s | Instruments › App Launch: `xcrun xctrace record --template 'App Launch' --launch -- build/DerivedData/Build/Products/Release/Bunode.app`. Time to the status item appearing. |
| Idle CPU under 1 % | With a few Services running and the popover closed: `top -pid $(pgrep -x Bunode) -l 30 -stats cpu,mem`, or Activity Monitor › CPU. The discovery scan wakes every 3 s; nothing else polls. |
| Idle memory around 50 MB | Same `top` line (MEM), or Activity Monitor › Memory › Bunode. |
| Feedback within 100 ms | Every click sets the row's state before the Supervisor answers (`AppModel.optimistic`); Instruments › Hangs shows none while clicking Start/Stop quickly. |
| Status updates usually within 1 s | `ten_services_at_once` and `thirty_services_at_once` print the Start → Running latency. Changes are reported at most every 100 ms. |
| At least 30 Services | `thirty_services_at_once`; for the UI, add 30 Services (any command) and scroll Home. |
| Heavy logs don't freeze the GUI | Add `fixtures/flood` as a Project with the Service `node flood.js`, start it, open its Logs, and record Instruments › Hangs and Animation Hitches while scrolling and searching. `mainThreadWorkStaysShortUnderAFlood` checks the main-thread side automatically. |
| The menu bar opens smoothly | Instruments › Animation Hitches while opening and closing the popover repeatedly, with 30 Services. |
