# Workspace boundaries

- `crates/runtime`: Lisp runtime and Emacs editor state; no CLI, GPUI, or crossterm dependencies.
- `crates/libraries`: data-only source/AOT registries for root `lisp/` and `lisp-elc/`.
- `crates/cli`: arguments, application/startup, and optional `terminal` adapter.
- `crates/gui`: GPUI application, logic-thread transport, command keys, and OS text input.
- Root `src/`: compatibility facade and integrated launcher. Keep existing public paths working.
- Common rendering/command loops live in `crates/runtime/src/frontend/`, not in a device adapter.
- Read `docs/architecture.md` and run `rtk python3 tools/check_architecture.py` when changing dependencies.
- Source-tree paths use build-time `REMACS_SOURCE_ROOT`, not a member's `CARGO_MANIFEST_DIR`.

## Development builds

- When building or running an executable during development, use `rtk cargo build --profile fast-dev` or `rtk cargo run --profile fast-dev`. `fast-dev` does not optimize workspace code, omits debug information, and retains incremental compilation.
- For changes that can be checked without a runnable executable, prefer `rtk cargo check --profile fast-dev -p <package>` for the affected package. `cargo check` is faster because it skips code generation and linking, but a regular build is still needed to verify linking or runtime behavior.
- `fast-dev` stores artifacts separately from the regular `dev` profile under `target/fast-dev`, so the first build also compiles dependencies. Later builds reuse artifacts and incremental compilation data from the same profile.
- `fast-dev` executables run more slowly than regular `dev` executables and contain no debug information. Use the regular `dev` profile to check execution speed, and the `debugging` profile when full debug information is needed.

## Choosing tests and validation

- In normal development, choose the smallest regression test that directly checks the changed behavior, along with a check of the affected package.
- Do not include `cargo test --workspace`, builds of every target, or builds of every package in the default validation scope.
- Expand validation as needed when a change affects contracts across multiple crates or startup paths, when targeted tests uncover a broader issue, or when the user requests full validation.
- When changing dependencies, read `docs/architecture.md`, run `rtk python3 tools/check_architecture.py`, and validate the affected packages.
- Report the tests and checks that were run, along with any broad validation that was not run.

## Preventing hangs and resource exhaustion during Emacs/REmacs validation

- Always run Emacs/REmacs validation under an external supervisor with a wall-clock timeout. This applies to direct runs and indirect launches through `cargo test`, Python or shell scripts, and GUI or terminal tests.
- By default, cap a single reproduction or comparison at 30 seconds, a known-hang reproduction at 5–10 seconds, and a focused test group at 120 seconds. If a longer run is needed, decide the reason and a finite limit before starting. Track build time separately from test execution time.
- A tool's `yield_time_ms` or the end of an output wait does not guarantee that the process has exited. Do not rely only on Lisp `with-timeout`, `C-g`, or cooperative interruption inside tests; ensure that an unresponsive process can also be forcefully terminated from outside.
- Run each validation process in a dedicated process group or session (for Python, for example, use `subprocess.Popen(..., start_new_session=True)`). On timeout, send `SIGTERM` to the entire group, including descendants, and send `SIGKILL` if any remain after a grace period of at most 3 seconds. Do not terminate or kill only the parent process.
- On normal exit, timeout, exception, or interrupted validation, clean up the descendants started for that run in a `finally` block or equivalent, wait for the parent to be reaped, and confirm that no processes remain. Also handle cleanup if the supervisor itself is interrupted; do not leave processes in the background. Limit termination to recorded PIDs or process groups, and do not affect the user's existing processes with commands such as `pkill emacs` or `killall remacs`.
- Set CPU-time and memory limits before running, and enforce them with OS resource limits or an external supervisor. Monitor RSS across descendants and terminate the entire group if the memory limit is exceeded, even before the timeout. On macOS and similar systems, verify that memory limits are effective; do not consider an ineffective setting sufficient protection.
- Do not accumulate unlimited output in memory. When redirecting logs to files, impose a size limit and stop validation if it is exceeded.
- Investigate hangs sequentially, using `--test-threads=1` when appropriate. Do not start another process until the previous validation process has exited. Do not retry the same hang indefinitely or extend the timeout without investigating the cause.
- Treat timeouts and resource-limit violations as validation failures. Report the command, limits, termination reason, and cleanup result. For a new or modified validation harness, first use a short-lived sleeping dummy process to confirm descendant cleanup on timeout and supervisor interruption, then run Emacs/REmacs.
