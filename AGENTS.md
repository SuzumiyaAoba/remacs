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

`src/lisp/load.rs` `EMBEDDED_LISP` covers all ported files except the 13
non-UTF-8 ones (`include_str!` cannot embed them; they resolve via the
filesystem fallback in `builtin_dirs()` and are rejected by
`insert-file-contents` anyway — see the encoding gap below). Run
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
- `insert-file-contents` rejects Emacs-internal-encoding files
  (surrogate-encoded unibyte bytes): ARRAY30, ECDICT, ETZY, QJ, QJ-b5,
  ZOZY, pinyin, sisheng, tsang-b5, ethio-util, ethiopic, ind-util,
  leim-list, uni-confusable, uni-name, titdic-cnv, tibetan, tibet-util,
  Punct-b5, japanese — all byte-identical to GNU.
- `transient.el`, `eieio.el` eager macroexpansion `(invalid-function nil)`;
  `byte-opt` "lambda used as function name" warnings.
- `xwidget-internal` and other GUI primitives unimplemented.
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

## Verification recipe

```sh
# byte-compare against GNU 31.1 source
GNU=/nix/store/7l70cwk87fk30viyjpj9iwha6a2wcs6v-emacs-unstable-31.1/share/emacs/31.1/lisp
gzip -dc $GNU/<path>.el.gz | cmp - lisp/<file>.el

# parse-check all files without evaluating (see git history for script)
./target/debug/remacs --batch --eval '(load "/tmp/parsecheck6.el")'
```
