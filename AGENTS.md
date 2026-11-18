# Repository guidance

## Common rules

- Do not switch branches unless explicitly instructed.

## Task-specific guidance

- [`docs/agents/development-workflow.md`](docs/agents/development-workflow.md) — Guidance on crate boundaries, source-tree handling, development builds, test scope, and monitoring validation processes. Consult it when changing builds, tests, or dependencies.
- [`docs/agents/lisp-runtime.md`](docs/agents/lisp-runtime.md) — Compatibility notes on GNU Lisp library registration, startup and loading, the reader, strings, variables, and related topics. Consult it when changing the Lisp runtime or libraries.
- [`docs/agents/runtime-services.md`](docs/agents/runtime-services.md) — Implementation notes on Tree-sitter, processes, external libraries, GUI, native compilation, minibuffers, and related topics. Consult it when changing the relevant subsystem.
- [`docs/agents/validation-and-parity.md`](docs/agents/validation-and-parity.md) — Notes on GNU compatibility validation, QA and differential sweeps, probes, threads, and time handling. Consult it when working on validation or compatibility.
- [`docs/agents/runtime-internals.md`](docs/agents/runtime-internals.md) — Internal design notes on memory measurements, byte-code, backtraces, portable images, gap buffers, and related topics. Consult it when changing the relevant internals.
- [`docs/agents/regression-notes.md`](docs/agents/regression-notes.md) — Additional validation recipes and cross-cutting regression cases. Consult it when changing related behavior.
- [`docs/architecture.md`](docs/architecture.md) — Design of the crate structure and dependencies. Read it before changing dependencies, then run `rtk python3 tools/check_architecture.py`.
