# remacs

A GNU Emacs 31.1-compatible editor implemented in Rust. The Lisp
runtime — reader, evaluator, byte-code VM, printer — the buffer/editor
core, and the front-ends are all native Rust, and it runs the actual
GNU Emacs Lisp library (1,600+ files under `lisp/`, ported
byte-identically from GNU Emacs 31.1).

> **Note:** despite the name, this is not the original remacs project
> (which translated Emacs's C core). Here the whole runtime is Rust;
> what is shared with GNU Emacs is the Lisp library and external
> behavior.

## Features

- **Emacs Lisp runtime** — full evaluator: special forms, macros,
  lexical + dynamic binding, closures, `catch`/`throw`, `unwind-protect`,
  `symbols-with-pos` (positioned symbols) with GNU's transparency
  semantics, `defvar`/`defcustom`, advice, `cl-generic`, and the
  dumped-library feature model.
- **Byte-code execution** — a real GNU `#[...]` executor
  (`src/lisp/bytecode.rs`): loads and runs `.elc` files, including
  `byte-switch` jump tables, handler frames, and the interpreter
  `catch_tags` contract. `byte-compile-file` produces real bytecode.
- **Native compilation** — a generated-C pipeline instead of
  libgccjit: `.el` → C → system `cc` → `.eln` shared object →
  `dlopen`. `(native-comp-available-p)` is `t` when `cc` works;
  `batch-native-compile` is supported via `-f`.
- **Front-ends**
  - Terminal UI via crossterm (`-nw` / TTY default).
  - GPU-rendered GUI via [gpui](https://gpui.rs) — the evaluator lives
    on its own logic thread; the window paints `render_grid` snapshots
    over a latest-wins channel, so the UI never blocks on Lisp.
  - macOS NeXTstep layer (`src/lisp/builtins/nsgui.rs`): real AppKit /
    Foundation calls through dlopen'd `libobjc.A.dylib` +
    `objc_msgSend` — `ns-list-colors`, `x-create-frame`,
    `x-file-dialog`, `ns-badge`, `ns-begin-drag`, services, etc.
- **Library coverage** — all of GNU Emacs 31.1's Lisp tree lives flat
  under `lisp/` and is embedded via `EMBEDDED_LISP`
  (`src/lisp/load.rs`); `require`/`load` resolve against it. Startup
  is driven by `src/lisp/prelude.el`.
- **Data types & primitives** — buffers (gap buffer), markers,
  windows/frames, hash tables, char-tables, bool-vectors, records,
  regexps, charsets incl. CJK tables, processes, timers, FFI-free
  sqlite3, JSON, XML/HTML (html5ever), tree-sitter grammars.
- **External libraries** — optional dlopen'd libs: GnuTLS (crypto +
  TLS sessions), Little CMS 2, D-Bus (sync/async calls, signals,
  service registration), kqueue file-notify.
- **Tree-sitter** — `treesit-*` primitives, parser/node/query records,
  predicates, notifiers, included ranges; load grammars from
  `~/.emacs.d/tree-sitter/` like GNU.

## Building

```sh
cargo build            # debug
cargo build --release  # optimized
```

Rust 2024 edition. No external build-time dependencies beyond crates;
optional libraries (`libgnutls`, `liblcms2`, `libdbus-1`) are dlopen'd
at runtime — remacs works without them and just doesn't provide the
corresponding features. Native compilation needs a working `cc`.

## Running

```sh
remacs                     # GUI (gpui) / TUI depending on platform
remacs -nw                 # terminal UI
remacs FILE...             # open files
remacs --batch --eval '(message "hi %s" 42)'
remacs -Q --batch -l myfile.el
remacs --script script.el
remacs --batch -f batch-native-compile lisp/foo.el
remacs --version
```

Batch mode also honors `--funcall/-f`, `--load/-l`, `--eval`/`--execute`,
and consumes `command-line-args-left` like GNU.

## Development

Diagnostics (all env-gated, inert unless set):

| Variable | Effect |
| --- | --- |
| `PRELUDE_MAX` | truncate prelude evaluation (bisect init) |
| `--ieval` | interactively bisect a top-level form |
| `WHILE_WATCH` | trace eval-level `while` loops |
| `REMACS_TRACE_BC` | trace bytecode PC/opcode/stack |
| `REMACS_BT_ERR` | dump the Lisp stack on `signal` (optional content filter) |
| `DBG_FUNCALL` | trace `funcall`/call dispatch |

Regenerate `EMBEDDED_LISP` entries after adding/removing `lisp/*.el`:

```sh
python3 tools/gen_embedded.py
```

### Tests

```sh
cargo test --test lisp_core     # core evaluator/reader/print/equality
cargo test --test lisp_libs     # Lisp library probes
cargo test --test nativecomp    # .eln pipeline (needs cc)
cargo test --test nsgui         # macOS-gated NS primitives
cargo test --test extlib        # gnutls/lcms2/dbus/filenotify (slow)
cargo test --test compat        # GNU-parity command checks
```

Note: most `ev()` calls boot the full prelude (~30 s each), so the
suites are heavy; prefer running a single test by name.

Byte-compare a ported file against GNU's source:

```sh
GNU=/nix/store/...-emacs-unstable-31.1/share/emacs/31.1/lisp
gzip -dc $GNU/emacs-lisp/byte-opt.el.gz | cmp - lisp/byte-opt.el
```

## Known limitations

- `xwidget-internal` is unimplemented.
- 13 files that used GNU's internal emacs-mule encoding are UTF-8
  transcodes — `U+FFFD` where GNU stored private-plane chars.
- Native compilation: max 256 native functions per unit (trampoline
  pool); `comp.el`'s LIMPLE/optimizing passes are not run;
  `comp-libgccjit-version` → nil.
- Startup still signals some benign `void-variable`/`bool-vector-p`
  noise in batch; `byte-opt` emits "lambda used as function name"
  warnings.
- Platform-gated files (`*-win`, android) fail to load on macOS, as in
  GNU.

See `AGENTS.md` for detailed port notes (layout, EMBEDDED_LISP rules,
NS/native-comp architecture, verification recipes).

## License

GNU General Public License v3 (see `LICENSE`). The Lisp library files
under `lisp/` and `etc/` are taken from GNU Emacs 31.1 and carry their
own copyright notices.
