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

## Pending: EMBEDDED_LISP registration

`src/lisp/load.rs` `EMBEDDED_LISP` needs entries for the ~600 ported files
(rounds 25–31.9). Registration rules:

- Flat files: `("name", include_str!("../../lisp/name.el"))`
- CEDET-qualified: key must be the GNU qualified path so
  `(require 'semantic/ctxt)` resolves, e.g.
  `("semantic/ctxt", include_str!("../../lisp/semantic-ctxt.el"))`
- Renamed quail: key `"quail/emoji"` → `lisp/quail-emoji.el`
  (`embedded()` tries exact match, then basename fallback).
- Generated loaddefs with qualified provide (e.g. `semantic/loaddefs`)
  need the qualified key too.

## Known runtime gaps (not port defects)

- Reader stack overflow on deeply nested data: `ja-dic.el`, `ZIRANMA.el`
  (both byte-identical to GNU; recursion depth limit).
- `insert-file-contents` rejects Emacs-internal-encoding files
  (surrogate-encoded unibyte bytes): ARRAY30, ECDICT, ETZY, QJ, QJ-b5,
  ZOZY, pinyin, sisheng, tsang-b5, ethio-util, ethiopic, ind-util,
  leim-list, uni-confusable, uni-name — all byte-identical to GNU.
- `transient.el`, `eieio.el` eager macroexpansion `(invalid-function nil)`;
  `byte-opt` "lambda used as function name" warnings.
- `xwidget-internal` and other GUI primitives unimplemented.
- `PRELUDE_MAX` env var truncates prelude evaluation for bisection;
  `--ieval` bisects a form interactively; `WHILE_WATCH` traces eval loops.

## Verification recipe

```sh
# byte-compare against GNU 31.1 source
GNU=/nix/store/7l70cwk87fk30viyjpj9iwha6a2wcs6v-emacs-unstable-31.1/share/emacs/31.1/lisp
gzip -dc $GNU/<path>.el.gz | cmp - lisp/<file>.el

# parse-check all files without evaluating (see git history for script)
./target/debug/remacs --batch --eval '(load "/tmp/parsecheck6.el")'
```
