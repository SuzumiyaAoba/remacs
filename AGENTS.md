# Remacs GNU library port notes

## Layout

All ported GNU Emacs 31.1 Lisp files live **flat** in `lisp/`. There are no
subdirectories — name collisions were resolved by renaming:

- `lisp/quail-<name>.el` — files from `leim/quail/` whose basename collided
  with `language/<name>.el` (burmese, cham, cyrillic, ethiopic, georgian,
  hebrew, indian, ipa, japanese, lao, lrt, slovak, tajik, thai, tibetan,
  welsh, emoji, ...). `lisp/leim-list.el` registers them as `"quail-<name>"`.
- `lisp/<dir>-<name>.el` — CEDET files from `cedet/semantic/`, `cedet/ede/`,
  `cedet/srecode/` flattened with their directory prefix. They still
  `(provide 'semantic/ctxt)` etc. with qualified feature names.
- `lisp/subr.el` — 8-line feature stub; subr functionality is folded into
  `src/lisp/prelude.el` but ~100 libraries `(require 'subr)`.
- `lisp/remacs-compat.el` — compatibility fallbacks; guard new shims with
  `(unless (fboundp ...))` and mark edits with `;Remacs:` comments.
- `etc/themes/*.el` — the 25 built-in themes referenced by
  `lisp/theme-loaddefs.el`.

Core GNU files intentionally **not** ported (folded into prelude.el):
bindings, button, custom, faces, files, font-lock, frame, help, indent,
isearch, jit-lock, keymap, menu-bar, minibuffer, mwheel, simple, startup,
subr (stub only), window. Also skipped: `.dir-locals.el`, `ldefs-boot.el`
(loaddefs.el is already generated for remacs).

## EMBEDDED_LISP registration (done — regenerate with tools/gen_embedded.py)

`src/lisp/load.rs` `EMBEDDED_LISP` covers all ported files — every
`lisp/*.el` is now valid UTF-8, so `include_str!` embeds the lot.
(The 13 files that used to carry GNU's emacs-mule internal encoding
— ARRAY30, ECDICT, Punct-b5, ethio-util, ethiopic, ind-util,
quail-ethiopic, quail-tibetan, quick-cns, tibet-util, tibetan,
titdic-cnv, tsang-cns — were transcoded to UTF-8; private-plane
chars Unicode can't name became U+FFFD, so they are no longer
byte-identical to GNU.) Run
`python3 tools/gen_embedded.py` to regenerate the entry lines — it maps
each file to its GNU key by content hash (qualified keys like
`semantic/ctxt` where the flat name came from a subdir) and emits
`// name.el skipped: not valid UTF-8` comments for the embeddable-excluded
files.
Registration rules:

- Flat files: `("name", include_str!("../../lisp/name.el"))`
- CEDET-qualified: key must be the GNU qualified path so
  `(require 'semantic/ctxt)` resolves, e.g.
  `("semantic/ctxt", include_str!("../../lisp/semantic-ctxt.el"))`
- Renamed quail: key `"quail/emoji"` → `lisp/quail-emoji.el`
  (`embedded()` tries exact match, then `dir-base` flat fallback, then
  basename fallback — so a flat `quail-emoji` key also resolves).
- Generated loaddefs with qualified provide (e.g. `semantic/loaddefs`)
  need the qualified key too.

## Known runtime gaps (not port defects)

- Reader stack overflow on deeply nested data: `ja-dic.el`, `ZIRANMA.el`
  (both byte-identical to GNU; recursion depth limit).
- The files that used to be Emacs-internal-encoding (see the
  EMBEDDED_LISP note above) are now UTF-8 transcodes — readable by
  `insert-file-contents`, but with U+FFFD where GNU stored
  private-plane chars; not byte-identical to GNU.
- `transient.el`, `eieio.el` eager macroexpansion `(invalid-function nil)`;
  `byte-opt` "lambda used as function name" warnings.
- `xwidget-internal` unimplemented. NS/macOS GUI primitives are
  covered by `src/lisp/builtins/nsgui.rs` (see below). Native
  compilation is implemented in `src/lisp/builtins/comp.rs` (see
  below).
- Bulk-load test (all lisp/*.el under --batch): only failures besides the
  above are `Lisp nesting exceeds max-lisp-eval-depth` during eager
  macro-expansion (~50 files), platform-gated *-win/android files, and
  unregistered qualified requires (srecode/semantic/*).
- `PRELUDE_MAX` env var truncates prelude evaluation for bisection;
  `--ieval` bisects a form interactively; `WHILE_WATCH` traces eval loops.

## Tree-sitter support (`src/lisp/builtins/treesit.rs`)

Implemented: grammar loading via `libloading` (lookup order:
`treesit-extra-load-path`, `user-emacs-directory/tree-sitter/`, then bare
library names; `~` expanded via `expand-file-name`), all `treesit-*`
primitives (parser/node/query records stored in `Interp::treesit`,
predicates `#eq?`/`#match?`/`#pred?`, searches, sparse trees, notifiers,
included ranges, ABI queries). Grammar .dylib/.so files must be built
separately (e.g. `~/.emacs.d/tree-sitter/libtree-sitter-<lang>.dylib`),
same as GNU.

Design notes:

- Objects are Lisp records `[treesit-parser ID]` / `[treesit-node PID
  NID]` / `[treesit-compiled-query ID]`; `print.rs` renders them as
  `#<treesit-...>'.
- **Never derive nodes from `Tree::clone()`** — it's `ts_tree_copy` (a
  *different* `TSTree`; root id even differs). Nodes must come from the
  parser's *stored* tree — see `stored_root()` which detaches via
  `Node::into_raw`/`from_raw`.
- Node staleness is GNU-style: `check_node` compares `parse_count`;
  `treesit-node-check 'outdated` is passive (nil until a reparse
  happens elsewhere).
- Query capture uses raw `ts_query_cursor_next_match` FFI; a match may
  carry 0 captures (predicate-only patterns).
- `treesit-node-type` and `treesit-node-field-name-for-child` return
  *strings* (not symbols) like GNU.
- `treesit-language-available-p` with DETAIL returns `(t . nil)` /
  `(nil . signal-data)`; `treesit-library-abi-version` returns an int
  (min-compatible when arg non-nil).

Verified: `treesit.el`, `json-ts-mode`, `c-ts-mode`, `rust-ts-mode`
load, parse, and fontify correctly under `--batch`.

## External libraries (`src/lisp/dynlib.rs` + builtins)

Optional GNU libraries are dlopen'd at runtime (`dynlib::Library`):
`libgnutls` (crypto + TLS sessions, `src/lisp/builtins/gnutls.rs` —
TLS I/O routed inside `src/lisp/process.rs`), `liblcms2`
(`lcms.rs`), `libdbus-1` (`dbus.rs`), kqueue file-notify
(`filenotify.rs`). Features (`lcms2`, `gnutls`, `dbusbind`,
`file-notify`) are provided only when the library actually loads;
`register_extlib_features` re-runs installs after the dumped
`features` list is restored.

D-Bus event flow: incoming messages are turned into `(dbus-event
BUS TYPE SERIAL SERVICE DESTINATION PATH INTERFACE MEMBER HANDLER
&rest ARGS)` and dispatched *synchronously* through
`special-event-map` (`filenotify::dispatch_special_event`), with
`last-input-event` set first — the remacs stand-in for GNU's
`kbd_buffer_store_event` + `read_char`. The pump runs inside
`sleep_firing_timers` (`sleep-for`/`sit-for`) and inside the timed
`read-event`/`read-char` path (`SECONDS` argument → pump until the
deadline, nil on timeout, never touches batch stdin — GNU parity).
No libdbus watches are used; `dbus_connection_read_write(0)` +
`pop_message` polling is enough.

Gotchas: libdbus `DBusDispatchStatus` is `DATA_REMAINS=0,
COMPLETE=1` (easy to invert); `gnutls_free` is a data symbol holding
a function pointer; `gnutls_x509_crt_fmt_t` DER=0/PEM=1. A
`(:basic-type x)` Lisp list is an *array* of that type in GNU
semantics, not a scalar — plain fixnums already map to `uint32`.

Verified end-to-end: real session-bus `dbus-call-method` sync/async,
method/signal registration and self round-trips, error replies →
`dbus-error`; TLS 1.3 HTTPS fetch through `open-network-stream`
`:type 'tls`; kqueue file+dir watches; LCMS color math vs GNU.
Regression suite: `tests/extlib.rs` (9 tests, ~6 min — every `ev()`
pays the ~30 s prelude; live-bus D-Bus tests live in `tests/dbus.rs`
and skip when `DBUS_SESSION_BUS_ADDRESS` is unset). kqueue note:
`NOTE_WRITE` only fires on a real write() — `write-region` over an
empty point range is a no-op and produces no event.

## NS/macOS GUI (`src/lisp/builtins/nsgui.rs`)

NeXTstep primitives bridge to AppKit/Foundation via dlopen'd
`libobjc.A.dylib` + `objc_msgSend` (Objc function table kept alive in
a `OnceLock`; the AppKit dylib handle is stored in `Objc::_lib` so
pointers stay valid). `AXIsProcessTrusted` comes from a separately
dlopen'd ApplicationServices (`mem::forget` keeps it mapped —
one-shot, process-lifetime). Headless-capable subrs really call
AppKit: `ns-font-name`, `ns-list-colors` (all NSColorLists, GNU's
<7-char/PANTONE filter + `framep` arg check),
`ns-process-is-accessibility-trusted`, `ns-block/unblock-system-sleep`
(`NSProcessInfo beginActivityWithOptions:` — the activity object is
autoreleased, so **retain it** before the pool pops; passing the raw
token to `endActivity:` used to crash), `ns-badge`,
`ns-request-user-attention`, `ns-progress-indicator`,
`ns-list-services`. GUI-only entry points signal GNU's exact text
`Window system is not in use or not initialized` (macro
`winsys_fn!`) — matching the NS reference build in batch.
`x-create-frame`/`x-select-font` carry the NS-specific messages
`Nextstep windows are not in use or not initialized` /
`Window system frame should be used` (misc.rs).

Startup: `term/ns-win`, `term/common-win` and `fontset` are
pre-registered features, so `(require ...)` would skip them — they
are loaded explicitly with `load_library` in `Interp::new` after the
other dumped libraries (need `cl-generic` first for ns-win's
`cl-defmethod`). This makes the Lisp-layer functions real at -Q:
`ns-handle-nxopen`, `ns-parse-geometry` (Nextstep order
`top left height width`), `x-handle-*`, `x-compose/decompose-font-name`,
`ns-*-working-text`, `ns-define-service`, `x-file-dialog`,
`x-begin-drag` (which consults the `XdndSelection` local binding and
signals `No local value for XdndSelection` like GNU). Full
`mapatoms` diff vs the NS reference build: 128/128 `ns-*`/`x-*`
names bound — GNU marks many as `subr` only because that build has
nativecomp (functions compile to native subrs).

Regression suite: `tests/nsgui.rs` (macOS-gated, 4 tests).

## Native compilation (`src/lisp/builtins/comp.rs`)

Not a libgccjit port — an equivalent pipeline: parse `.el`, generate
C, compile with system `cc` into a shared object named `*.eln`, and
`dlopen` it. `(native-comp-available-p)` is t iff `cc` is usable.

Architecture:

- Generated C treats Lisp values as opaque `em_obj` handles; every
  operation goes through an `em_api` callback table (cons/car/cdr,
  intern, funcall, eval, set, signal, …). `em_env` is the `Interp`
  pointer while loading/calling.
- `EmObj` handles index a thread-local object table (`Value` holds
  `Rc`, so all native-comp state — `OBJTAB`, `NATIVES`, `META`,
  `UNITS`, `PENDING` — is `thread_local!`, never `static Mutex`).
- Native functions register as real `Subr`s via `api_defsubr`;
  since `Subr.func` is a static fn pointer, a fixed trampoline pool
  `tramp::t0..t255` dispatches through `NATIVES`. `native-comp-function-p`
  recognizes subrs by trampoline identity.
- `defmacro` forms are serialized in the unit and evaluated at load
  time (so they stay macros, `macrop` → t); unsupported bodies
  (unwind forms, closures) fall back to interpreter eval via
  `make-interpreted-closure`.
- `.eln` naming matches GNU: `<base>-<md5(truename)[:8]>-<md5(contents)[:8]>.eln`
  under `~/.emacs.d/eln-cache/31.1-remacs/`.
- `native-compile` accepts files, symbols, and lambda objects;
  `native-compile-async`/`native-compile-directory` compile
  synchronously (a documented simplification); `(load "x.eln")`
  works — `load.rs` prefers `.eln` over `.elc`/`.el` like GNU and
  routes through `comp::native_load_file`. `--batch -f
  batch-native-compile FILE...` works via `command-line-args-left`
  (main.rs now implements `-f`/`--funcall`, which GNU had and remacs
  lacked).
- Public `native-*`/`batch-*` entry points are thin `defun` wrappers
  over `comp--remacs-*` subrs installed at startup in `Interp::new`
  — deliberately NOT named `native-compile` so loaddefs autoloads
  can't pull GNU's comp.el LIMPLE pipeline in. `interactive-form`,
  `commandp`, `call-interactively`, `documentation`, and
  `subr-native-lambda-list` consult the native metadata table.
- `comp-libgccjit-version` → nil (no jit), same as a GNU
  `--without-native-compilation` build's surface.
- Known limits: max 256 native functions/units (trampoline pool),
  `comp--*` internals cover the stable entry points only
  (`comp--register-subr`, `comp--init/release-ctxt`,
  `comp--compile-ctxt-to-file0`, `comp--late-register-subr`,
  `comp--install-trampoline`), `comp.el`'s LIMPLE/optimizing passes
  are not run.

Regression suite: `tests/nativecomp.rs` (8 tests).

## Verification recipe

```sh
# byte-compare against GNU 31.1 source
GNU=/nix/store/7l70cwk87fk30viyjpj9iwha6a2wcs6v-emacs-unstable-31.1/share/emacs/31.1/lisp
gzip -dc $GNU/<path>.el.gz | cmp - lisp/<file>.el

# parse-check all files without evaluating (see git history for script)
./target/debug/remacs --batch --eval '(load "/tmp/parsecheck6.el")'
```
