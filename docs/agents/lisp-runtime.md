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
- `lisp/remacs-compat.el` — compatibility fallbacks; guard new shims with
  `(unless (fboundp ...))` and mark edits with `;Remacs:` comments.
- `lisp/subr.el` — real GNU file now; subr's functionality is still
  provided by `crates/runtime/src/lisp/prelude.el` at init, and `(require 'subr)` hits
  the pre-registered feature, so the file only matters for `load`.
- `etc/themes/*.el` — the 25 built-in themes referenced by
  `lisp/theme-loaddefs.el`.

All core GNU files are now ported byte-identically — including the
ones that used to be folded into `crates/runtime/src/lisp/prelude.el` (bindings,
button, custom, faces, files, font-lock, frame, help, indent, isearch,
jit-lock, keymap, menu-bar, minibuffer, mwheel, simple, startup, subr,
window).  The prelude's own folded definitions still drive init; the
real files resolve `(require ...)`/`load` through EMBEDDED_LISP.
Skipped: `.dir-locals.el`, `ldefs-boot.el`
(loaddefs.el is already generated for remacs).

## EMBEDDED_LISP registration (done — regenerate with tools/gen_embedded.py)

`crates/libraries/src/lib.rs` `EMBEDDED_LISP` covers all ported files — every
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

- Flat files: `("name", include_str!("../../../lisp/name.el"))`
- CEDET-qualified: key must be the GNU qualified path so
  `(require 'semantic/ctxt)` resolves, e.g.
  `("semantic/ctxt", include_str!("../../../lisp/semantic-ctxt.el"))`
- Renamed quail: key `"quail/emoji"` → `lisp/quail-emoji.el`
  (`embedded()` tries exact match, then `dir-base` flat fallback, then
  basename fallback — so a flat `quail-emoji` key also resolves).
- Generated loaddefs with qualified provide (e.g. `semantic/loaddefs`)
  need the qualified key too.

`locate-library` searches the caller's load-path before the embedded
table. An explicit PATH is authoritative. Default embedded hits return
absolute, readable `.el` paths: use an existing flat bundled file, or
publish its source into a private process-local temporary directory
(`crates/runtime/src/lisp/embedded_cache.rs`). Publication is atomic; repeated lookups
preserve edits to the materialized file. Never return `builtin:` load
identities as filenames. Respect NOSUFFIX and configured suffixes rather
than inventing `.elc` files for embedded bytecode twins.

Flat returned filenames resolve back to their canonical registry keys
(`semantic-ctxt.el` → `semantic/ctxt`) for recursive-load detection.
Post-boot `subr-x` returns GNU's `emacs-lisp-subr-x.el`; loading that
absolute path retains the same live-definition protection as a normal
`load "subr-x"`. A user's cwd `lisp/` directory is searchable but is
not classified as shipped source. The checkout, installed Lisp tree,
and private materialized source directory are shipped-source roots.
Regressions: `qa_locate_embedded`, `tests/embedded_cache.rs`, and the
standalone copied-binary tests in `tests/embedded_libraries.rs` (also
run with `REMACS_NO_ELC`).

`crates/runtime/src/lisp/dump_history.json` records GNU 31.1's 136 dumped libraries and
their definition/dependency ownership. Regenerate with
`python3 tools/gen_dump_history.py --emacs /path/to/emacs-31.1` and use
`--check` to verify it. At the end of bootstrap, `dump_history::restore`
resolves these records to readable ported `.el` files; this does not
evaluate definitions or change features. External loads and live
Remacs-only helpers retain their own records. `builtin:` remains an
internal bootstrap identity, never a public provenance filename.

Runtime file/buffer evaluation initializes `current-load-list` with its
filename marker and shares `load::build_history`; the marker list is
reversed in place into the history entry. `defalias` tracks runtime
`function-history` so unloading a redefinition restores the prior cell;
artificial bootstrap cell replacements are excluded. Portable images
record unchanged shipped source identities and filename object IDs.
Restoration republishes those sources and relocates metadata before
graph decoding, preserving shared filename strings and hash keys.
Unrelated equal strings and external/edited sources retain their values.
Regressions: `qa_dump_history` and `tests/dump_history_startup.rs`.

On-disk `load` classifies compiled input with GNU's
`safe_to_load_version` rule: inspect the first 512 raw bytes, search
after the first newline using `bytecomp-version-regexp` with case
folding, and preserve match data. An actual `.elc` suffix requires a
valid compiler header (empty regular files are accepted); the ELC
marker alone does not suffice. Initialize the regexp before boot source
loads. Recognized headers select the direct compiled reader even for
`.el` names. Compressed `.elc.gz` files follow the decompression/source
path and may contain ordinary source, as in GNU. `--script` uses buffer
evaluation and bypasses this classification. Regression:
`qa_load_elc_header`.

Source `load` and `eval-buffer` share lread.c's lexical-cookie scanner:
the first declaration wins, only the exact value `nil` means dynamic,
and a missing declaration differs from an explicit nil declaration.
Scan only an initial comment or the comment after a shebang, using GNU's
marker state machine and field limits. With no cookie, call the real
`internal--get-default-lexical-binding-function`; preserve its returned
object. The source callback sees its consumed buffer position and the
caller's standard-input, while standard-output is already nil. Restore
the read position before evaluation and unwind callback failures normally.
Direct compiled loads bypass that callback and use the toplevel default.
Reader signals are offered before load bindings or the source buffer are
removed. Regression: `qa_load_lexical_cookie` (38 GNU checks). Ordinary
post-boot loads retain the caller's byte-optimize value; only bootstrap and
the source optimizer's own recursive loading inhibit it.
Native compilation follows bytecomp.el's separate normal-mode/safe-local-
variable protocol, where non-boolean values are ignored and duplicate
directives can select the last value. `qa_native_comp_load` covers both
rules with actual native functions as well as compiler-library reloads.

Both `read` and `read-positioning-symbols` resolve nil/omitted streams
through `standard-input`. Source loads expose the existing input buffer;
direct/compiled loads expose the private uninterned `get-file-char`
symbol and share a `LoadStream` character cursor with the loader and
custom `load-read-function`. Direct streams create no scratch buffer.
Nested loads restore both the cursor and input binding. Positioned file
reads are relative to each read; buffer positions are absolute. File EOF
data uses `load-true-file-name`. `qa_load_read_stream` covers consumption,
reader switching, Unicode positions, nesting, and restoration on errors.
EOF data identifies buffer/file inputs; string and marker reads carry no
stream datum. Consuming reads advance buffer/marker/file cursors even
when parsing signals, while markers past a narrowed end are not rewound.
Numbered-object syntax honors `read-circle`, accepts fixnum-sized label
IDs with checked digit accumulation, and preserves original spellings in
errors. Positioned reads annotate literal dots before a closing delimiter.
Regressions: `qa_reader_error_context`.
Numbered objects register a fresh cons placeholder before reading their
body, including when redefining a label. Cons results copy their car/cdr
into that placeholder without overwriting a nested definition's mapping;
other results substitute placeholder references and update the mapping.
Nested cons labels therefore need not alias, while vector labels can.
Regressions: `qa_reader_circles`.

`write-char` follows GNU's CHECK_FIXNUM and unsigned 32-bit printchar
conversion, not CHECK_CHARACTER. Function streams receive the unencoded
32-bit value; text streams resolve ASCII Shift/Control, discard the six
event modifier bits, and reject remaining invalid bits. High Mule codes
use the existing character proxies rather than U+FFFD. Return the original
fixnum, including when its encoded character differs. `qa_write_char`
checks string/buffer/marker/function destinations and validation ordering.

Variable protection uses a committed GNU forwarding snapshot
(`crates/runtime/src/lisp/forwarded_variables.txt`, regenerate with
`tools/gen_forwarded_variables.py --emacs GNU31_BINARY`). `G` entries
use non-buffer forwarding; `B` entries are C buffer slots. Apply flags only
to existing bound symbols after boot, without relying on DOC availability.
C globals become localized when made buffer-local; C buffer slots stay
forwarded. Both remain C-backed and cannot be unbound or made aliases.
Constants and aliases to constants reject buffer-local creation before
changing state. `qa_variable_protection` covers protection, alias-local
access, validation order, and failure atomicity.

`file-name-directory` dispatches through `file-name-handler-alist` before
the local slash split. Tramp supplies remote prefix and multihop behavior;
do not approximate that parsing with colon heuristics. Pass the original
filename object to the handler and honor operation/inhibition properties.
Non-string handler results become nil, as in GNU's fileio.c.
Regressions: `qa_file_directory_handlers` (name parsing only, no connection).

`file-truename` expands home/default directory without collapsing path
components before resolving links. The shared buffer truename helper
preserves exactly two leading slashes and walks components in physical
order, including canceled missing directories. Relative links retain the
root; absolute targets replace it. Nil/symbol arguments report
`arrayp` like GNU files.el. Regressions: `qa_file_truename_roots`.

`copy-file` copies data through an open descriptor rather than macOS's
metadata-cloning `fs::copy`. Without KEEP-TIME, new files get current
times and existing destinations retain their atime. KEEP-TIME restores
both source times through that descriptor, including for read-only modes.
Like GNU, open/truncate the destination before comparing the open file
descriptors' device/inode identities; self-copies signal `file-error`
after truncating the input. Overwrite refusal and output-open errors take
precedence. Directory targets require a trailing slash. Preserve uid/gid
and permissions independently; macOS ACL copying is explicit and
does not clone times. Existing hard-link targets still
require overwrite permission, like GNU. Regressions: `qa_copy_file_times`,
`qa_copy_file_self`.

`rename-file` checks source/destination entries with `symlink_metadata`,
so dangling and looping symlinks can be renamed and count as existing
destinations. `copy-file` first opens with exclusive creation; after an
existing-entry error it enforces overwrite permission, then opens with
truncate but without create. Thus a dangling destination never creates
its missing referent. Source errors take precedence over collisions.
Regressions: `qa_file_broken_symlinks`.

`file-exists-p`, `file-readable-p`, `file-writable-p`, modes/times setters,
copy/rename/delete, and insert/write dispatch through the shared
operation-aware filename-handler finder.
Keep unchanged absolute filename objects (properties and identity), pass
all optional arguments, and search input before output (write: filename
before raw VISIT). Insert handlers bypass offset checks and REPLACE's
physical-file point restoration, but still run the common decoding/hook
tail; nil results synthesize `(filename count)`. Literal reads bind GNU's
conversion/hook/handler inhibitors. Write handlers get normalized regions
and the default LOCKNAME object; VISIT updates only filename/save state,
leaving truename, modtime, and default-directory to the handler.
Assign save ticks directly in these handler branches: unlike
`set-buffer-modified-p`, GNU does not release the live file lock there.
Regressions: `qa_file_primitive_handlers`, `qa_file_handler_io`.

Frame parameter updates validate the full alist and every element before
applying it in reverse, so the first duplicate wins. Both setter APIs use
the same store logic. TTY name/font reports come from frame state; keep
the shared Lisp name object for identity/properties and later mutations,
and allocate automatic F names per interpreter. Existing minibuffer
ownership cannot be changed to a different kind or window. Quote errors
through `error_q`. Regressions: `qa_frame_parameters`.
Frame/buffer change snapshots retain the frame's name object, rather
than a fresh string that would compare non-EQ on every query. Track
read-only/modified flags, not modification ticks, and ignore space-prefixed
buffers. Sole-frame deletion has distinct default/FORCE errors.
Regressions: `qa_frame_buffer_changes`.

Finalize public `load-path` after internal bootstrap. Retain real library
directories; a source-less executable publishes a core source in the
private process cache and exposes that directory. Other sources are
materialized on demand at the cache entry's position in the search path.
EMACSLOADPATH directories preserve order/duplicates and relative spelling;
empty elements insert defaults. Environment paths take effect after boot
so they cannot replace internal compatibility libraries mid-initialization.
Regressions: startup checks in `embedded_libraries` and `load_path_startup`.

## Startup profiling (`REMACS_BOOT_PROF=1`)

Before full eager source expansion, warm release `--batch` startup
measured ~0.85 s: prelude eval ~250 ms + ~350 ms of
`boot_lib` loads.  Boot libraries are AOT byte-compiled by GNU Emacs
into `lisp-elc/*.elc` (see below); `load_library` prefers an embedded
`.elc' twin over the `.el' source while `boot_loading', and the
`REMACS_NO_ELC' env var drops back to interpreted source for parity
debugging.  The backquote expander is a Rust port
(`crates/runtime/src/lisp/builtins/backquote.rs`) — GNU byte-compiles backquote.el, so
an interpreted copy both cost ~700 ms of macroexpansion per boot and
broke on `(fmakunbound 'car)' (its `car' calls are `Bcar' opcodes in
GNU).  `backquote'/`\`'/`backquote-list*' are bound as
`(macro . #<subr>)' cells after all library loads (`install_macros' in
`Interp::new'), and the `backquote-*-symbol' variables are honored.
Similarly, `subr.el''s Lisp defuns are loaded for cell-shape parity but
then any symbol that no longer names a Rust subr is re-fset to the
subr (right after `boot_lib "subr"') — several subrs carry parity
tweaks (gensym's native call/update semantics, oddp's marker rejection,
apply-partially's `#[128]' shape, locate-library's suffix rules); GNU's
nativecomp build reports `subrp' t for all of these anyway.
Macro cells (`(macro . _)', `is_macro' lambdas) are skipped: the
prelude's `with-output-to-string' defmacro is the faithful GNU shape
(it binds `standard-output' to a real buffer, which `vc-git''s
`with-current-buffer standard-output' requires), and shadowing it with
the `f_with_output_to_string' subr broke `M-x dired' file selection
with `wrong-type-argument stringp t'.

`command_execute' resolves `(autoload ...)' function cells through
`autoload_do_load' before interactive-spec lookup — GNU's
call-interactively does the same via `indirect_function'; without it,

## Build identity

`build.rs` records Cargo's target in `REMACS_SYSTEM_CONFIGURATION`, adding
the build host's Darwin release for Darwin targets.  Keep this a build-time
constant; the runtime environment must not replace the configured identity.
After booting GNU `version.el`, startup clears `emacs-build-system` and
`emacs-build-time` to match the reproducible reference build.  The Lisp
version formatter remains real and honors callers' explicit variable binds.
Regressions: `qa_build_identity.el`, `tests/build_metadata.rs`, and the CLI
build-identity test.

## Menu constructors and interpreted closures

`menu-bar.el` is loaded explicitly after `subr-x` and before `tool-bar`.
The prelude no longer seeds its Edit menu from a serialized keymap: that
snapshot replaced the function-valued `:keys` and paste `:enable` fields
with nil.  The GNU constructor now creates the actual functions in both
AOT and source boot modes.  Regenerate its twin with
`python3 tools/gen_boot_elc.py --only menu-bar`.

`Ffunction` and runtime `lambda`/`defun`/`defmacro` construction honor
`internal-make-interpreted-closure-function`'s five-argument protocol.
The variable is nil during bootstrap and becomes
`cconv-make-interpreted-closure` after the dumped libraries are ready.
Lexical environments have persistent alist heads and shared mutable
binding conses: optimizer trimming must retain those cells, and captures
must not acquire later `let*` bindings.  The printer exposes the actual
environment instead of sorting or reconstructing its contents.  Keep
nativecomp metadata on new closures built by dumped code; a custom
constructor can return a shared object or even a non-function value.

Regressions: `qa_menu_keys.el`, `qa_closure_constructor.el`, and the fresh
CLI menu/closure test in `tests/main_cli.rs`.
The constructor suite also pins argument-bearing closures' empty `(t)`
environment, argument/inner-let shadowing, and quoted names (#70).

Voided macro cells do not expose `remacs--dump-fn` definitions to user
code. `current_lambda_dumped` tracks the immediate function rather than
compiled ancestors; `compiled_macro_expansion` tracks the input forms
separately from the dumped macroexpander implementation. `with_eval_context`
scopes eval, buffer/string evaluation, loads, and user callbacks so
unloaded CL macros signal `void-function` until `cl-lib` is required.
Regular folded-function fallbacks remain for incomplete source/autoload
paths (face-remap, rx, etc.). Regressions: `qa_cl_boot_surface` and the
copied-binary library tests, whose watchdog also guards against loop hangs.

## AOT .elc twins (`crates/libraries/src/embedded_elc.rs` + `lisp-elc/`)

`tools/gen_boot_elc.py` runs GNU Emacs's `byte-compile-file' over the
boot libraries and emits `lisp-elc/<stem>.elc' blobs plus a Rust table
`crates/libraries/src/embedded_elc.rs'.  `load_library_opts_msg' prefers a blob
over the `.el' source while `boot_loading' (Interp::new's boot-lib
window) is set — post-boot `(require '…)'/`load' of the same names
reads the real `.el' so `symbol-file'/`load-history' stay right.
`eval_src_opts' detects the `;ELC\x1f\0\0\0' magic and sets
`Interp::reading_elc' for that eval; `make_interpreted_closure' then
marks `#[...]' objects `compiled' so `subrp'/`type-of'/`func-arity'
report GNU's nativecomp surface.

Default shipped-library loads also scope `loading_compiled_library`:
requires performed during eager macro expansion are compiler helpers,
not runtime `load-history` dependencies. Explicit `.el` loads and user
load-path overrides keep GNU source-loading dependency records,
including `eval-when-compile`. At the end of bootstrap, discard require
records for features rolled out of the dump while retaining definitions,
provides, and dependencies on live dumped features. This lets `cl-lib`
unload normally and preserves dependency errors for user libraries.
Regressions: `qa_unload_dependencies` and the fresh-process unload test
in `tests/main_cli.rs` (checkout/embedded paths, normal/source boot).

Gotcha: `Lambda.bc_closure' distinguishes `make-closure' products
(env spliced into the constants vector) from bare `#[...]' literals.
`unidata_bc_dispatch' must skip `bc_closure' objects — a mutated
capture box can turn consts[0] into a proper alist of conses, which
the `unidata-describe-*' alist arm then short-circuits to nil,
silently skipping the real bytecode (this broke every `cl-defgeneric'
method that closes over a box — e.g. `seq-reduce').

`REMACS_NO_ELC' bypasses twins entirely (parity debugging); `SKIP' in
tools/gen_boot_elc.py lists files GNU marks `no-byte-compile'
(charprop) plus remacs-only files without a GNU counterpart.  The
`.elc' files are committed; regenerate when a booted `lisp/*.el'
changes — stale twins silently diverge from the source.

`lisp/subr-x.el' is NOT GNU's subr-x: it amalgamates the dump-time
compatibility definitions (~2300, loaded once at boot) with GNU's
subr-x.  Post-boot `subr-x' loads — `(require 'subr-x)', the
`string-pad'/`string-fill'/... autoloads, any `.el' source whose
`eval-when-compile' requires it (xref, project, ...) — evaluate GNU's
verbatim copy `lisp/emacs-lisp-subr-x.el' instead (`load_library_opts_msg'
→ `eval_gnu_subr_x'), then `keep_live_definitions' restores any name
it redefined that already had a live (non-void, non-autoload) cell.
Re-reading the amalgam used to replace ~2300 live definitions (Rust
subr shadows, replace.el's `replace-highlight', ...) and re-run its
top-level `setq'/`defconst'/`add-hook' forms (reset
`file-name-handler-alist', `electric-quote-string').  So edits to the
amalgam only take effect through the boot load; `tests/qa_subrx3.el'
pins the post-boot behavior.

`japan-util` is lazy in GNU -Q: its feature, private helpers, tables and
conversion properties are absent until an autoload or require loads the file.
Do not pre-provide it or fold its functions/data into the `subr-x` amalgam:
live copies prevent `loaddefs` from installing autoloads and separate the
converters from their data. The real `lisp/japan-util.el` initializes both
together. `qa_japanese` checks the initial autoload surface, first-use loading,
kana/width conversion, composition, region operations and explicit reload
(#291).

## Source loading and eager macro expansion

`load.rs` follows GNU `readevalloop`: call
`internal-macroexpand-for-load` with nil, splice a resulting top-level
`progn` in evaluation order, then call the hook with t for other forms.
The hook remains observable to advice and redefinition.  Compiled streams
skip both passes.  `macroexpand--all-toplevel` uses the Rust deep walker
after the boot amalgam is loaded, avoiding the interpreted GNU walker's
extra Lisp depth during initialization.

Eager expansion does not memoize cons identities: separate occurrences
can share reader objects while macro/compiler-macro calls still have
observable effects.  Compiler macros can expand to nil and are retried
after their arguments change; the GNU wrapper handles inhibition and
optimization warnings.  `macroexp--dynvars` is scoped per load and per
lexical binding/function body.  The internal `eieio--class` structure
gets its GNU safety-0 accessor and inliner definitions when registration
completes; a feature's after-load hook is too late for expanded consumers.
Regressions: `qa_load_macroexpand`, `qa_eieio`, and the existing macro,
pcase, cl-loop, load and CLI suites.

`macroexpand` validates ENVIRONMENT only for conses with a symbol head;
`macroexpand-1` and `macroexpand-all` validate it only for conses. Atom
inputs preserve their identity even with an invalid unused environment.
Regression: `qa_macro_environment` (54 GNU-verified checks, #293).

`key-parse` uses the GNU keymap.el definition folded into the prelude.
Do not register a Rust `key-parse` subr: the post-subr boot restoration
would overwrite that definition with the bootstrap token parser, losing
repeat counts, menus, comments and whitespace handling. Regression:
`qa_key_parse` (35 GNU-verified checks, #296).

Named key codes hash the base event name and carry A/C/H/M/S/s in modifier
bits, so Lisp symbols such as `C-home` match front-end base-plus-modifier
events. Reverse lookup reconstructs canonical prefixes. Only character
Meta events use the ESC submap; `M-left` remains a single symbol even if
ESC has a non-prefix binding. Regression: `qa_modified_named_keys` and
the GUI/terminal `modified_named_keys` tests (#297).

`display-buffer` falls back through the existing GNU
`display-buffer-pop-up-window` action and then `display-buffer-use-some-window`.
The split policy honors `pop-up-windows`, tries largest/LRU candidates, and
catches split errors rather than repeatedly splitting the selected window.
Regression: `qa_display_fallback` (8 GNU-verified checks) and `qa_buffmenu` (#294).



## Known runtime gaps (not port defects)

- The files that used to be Emacs-internal-encoding (see the
  EMBEDDED_LISP note above) are now UTF-8 transcodes — readable by
  `insert-file-contents`, but with U+FFFD where GNU stored
  private-plane chars; not byte-identical to GNU.
- `byte-opt` "lambda used as function name" warnings (stale — no
  longer emitted since the `equal`-on-`#[...]` fix; keep the note as
  a canary: if the warning returns, suspect the constant-dedup path).
- `xwidget-internal` unimplemented. NS/macOS GUI primitives are
  covered by `crates/runtime/src/lisp/builtins/nsgui.rs` (see below). Native
  compilation is implemented in `crates/runtime/src/lisp/builtins/comp.rs` (see
  below).
- Bulk-load test (all lisp/*.el under --batch, `blessmail.el` skipped —
  it writes the `blessmail` script to CWD and calls `kill-emacs` by
  design): platform-gated *-win/android files fail as on any macOS
  build.
- Startup ordering parity: the prelude no longer puts `*scratch*' in
  `lisp-interaction-mode' — `Interp::new' does it after all library
  loads (GNU startup.el ordering).  Otherwise `characters.el''s
  unqualified `modify-syntax-entry' calls edit scratch's
  lisp-interaction syntax table instead of `standard-syntax-table'
  (GNU buffers' syntax_table field defaults to the standard object).
  Case tables follow casetab.c: `set-case-table'/`set-standard-case-table'
  recompute nil UPCASE/CANON/EQV extras (`set_case_table'), and buffer
  creation re-derives them when the standard table's extras aren't all
  char-tables (GNU `init_buffer', buffer.c:1150) — `characters.el'
  clears extras 1–2 mid-loadup, so without the heal `-Q' reports nil.
  `(require 'pcase)' sits after the vendored cl machinery
  (cl-defstruct/cl--define-built-in-type), matching GNU loadup order
  cl-preloaded.el → pcase.el: pcase's `with-eval-after-load
  'cl-preloaded' upgrade of `pcase--subtype-bitsets' calls
  `built-in-class-p', which must already exist when the deferred form
  runs (aborted the whole prelude otherwise).
- `(setf cl--find-class)`: GNU's `cl--find-class' is a `cl-macs'
  defmacro, so `setf' on its call site compiles to `put' — but remacs
  runs `cl--define-derived-type' interpreted, and after the macro's
  cell is voided (GNU_VOID_FNS_LATE, matching GNU -Q) gv's lazy
  expansion falls back to a `(setf cl--find-class)' function.
  prelude.el defines that function and eval.rs voids it into
  `remacs--dump-fn', keeping `symbol-function'/`fboundp' nil like GNU.
- `lisp/loaddefs.el` is a *hand-maintained* reduced port of GNU's
  generated loaddefs.el.  Its non-`(autoload ...)' side-effect forms
  were grafted at the tail (`;Remacs: GNU loaddefs.el side-effect
  forms...' block): `define-symbol-prop 'rx 'pcase-macroexpander`
  (without it `pcase' can't expand `(rx ...)' patterns), `put'/
  `function-put'/`make-obsolete'/`define-obsolete-function-alias'.
  Do not regenerate it with a pure-autoload extractor.
- `PRELUDE_MAX` env var truncates prelude evaluation for bisection;
  `--ieval` bisects a form interactively; `WHILE_WATCH` traces eval loops.

## Kill-ring selection and paste

`current-kill` follows GNU simple.el: compute the yank index from the
length of `kill-ring-yank-pointer`, not by finding an identical tail.
Use the ordinary numeric/sequence primitives so malformed pointers and
float arguments retain GNU's validation order. Return the selected car
itself, preserving string identity, representation, and properties.

For N=0, consult `interprogram-paste-function` before checking for an
empty ring. Import a reversed copy of list results through `kill-new`,
with paste/cut callbacks dynamically disabled during import. Positive
rotation notifies the cut function only when selection changes are
enabled and DO-NOT-MOVE is nil. `push_kill_ring_value` keeps supplied
string objects and the old ring tail. `yank` delegates selection to
`current-kill` and insertion to `insert-for-yank`; cons prefixes swap
point/mark without activating the region. Regressions: `qa_current_kill`
and `tui_yank_imports_external_text_and_preserves_properties`.

`kill-do-not-save-duplicates` compares only the newest kill using
`equal-including-properties`; matching text with different properties
still adds an entry. Skipping a duplicate retains the old head object,
resets the yank pointer, and still notifies the cut function with the
new argument. It does not remove matching older entries. REPLACE uses
`setcar` semantics on an existing ring, retaining the head cons and tail.
Regression: `qa_kill_new`.

## Case conversion and symbol names

GNU casefiddle has distinct string paths: nonempty unibyte strings use
single-character mappings on a copy, retaining properties and byte
representation; multibyte strings allocate a fresh multibyte result
without properties. Empty strings return the original object.
`propertize` must preserve explicit representation flags when copying.
`qa_propertize_bytes` pins all 256 unibyte values through `aref`, list/vector
conversion, repeated copies and GC, as well as explicit multibyte ASCII and
raw-eight-bit multibyte strings; text and intervals are copied independently,
while property values retain identity.

Region and word casing share `apply_case_text`. Character replacements
preserve intervals; expansion inserts extra prefix characters with only
the previous character's rear-sticky properties. Like GNU's
`replace_range_2` with MARKERS=nil, marker/overlay character positions
stay fixed while point and the selected buffer's accessible end grow.
`record_change` saves an aggregate undo pair with the original string's
properties. Undo insertion restores those properties before change hooks.
`undo-boundary` adds a nil marker only to a nonempty history without
an existing boundary. An empty history stays nil, but still remembers
the pre-command point for the next edit (`qa_undo`).

Deleted undo entries must contain the original property's intervals and
byte kind, not only text. `raw_delete_region_with_undo` returns the recorded
string; interpreter deletion paths use `delete_preserving_undo` to copy
the buffer range after before-change hooks and attach metadata before
after-change hooks. Rectangle/indent/message/minibuffer and coding/replace
paths share this bridge. Property-free ranges keep no string interval
tree; explicit nil-valued properties remain present. Undo already inserts
the saved string's properties. Regressions: `qa_undo_properties` (including
redo, indirect views, narrowing, hooks, GC, and multibyte/unibyte history).

`replace-region-contents` compares characters with GNU's bidirectional
Myers algorithm (`buffer/replace.rs`), including the cost heuristics, then
applies edit runs backwards. Equal islands retain their properties and
markers; even an unchanged nonempty region gets one before/after hook
pair, but no new modification tick or undo entry. `raw_replace_region_with_undo`
records insertion before deletion and relocates both endpoints as a
replacement, including sibling buffers and overlays. Sources honor buffer
narrowing and `[BUFFER BEG END]`; deprecated callbacks temporarily narrow
the target and restore excursion/restriction on errors. An integer zero
comparison limit requests plain replacement (returns t); expiration falls
back and returns nil. Match the actual GNU 31.1 sixth-argument behavior:
INHERIT is passed as `replace_range`'s ADJUST_MATCH_DATA flag while its
property-inheritance flag stays false. Regression: `qa_replace_region_contents`.
`qa_replace_region_properties` separately pins source properties on inserted
runs: strings, narrowed buffers, source vectors and callback results preserve
their clipped intervals and property-value identity, while equal characters
keep the target's old properties. Plain replacement on zero limits or timeout
copies the entire source's properties, including explicit nil values.
`qa_replace_region_narrowing` pins accessible SOURCE bounds in normal,
zero-limit and timeout paths, including empty ranges and Unicode offsets.
Reading a source preserves its point/restriction and restores the caller's
current buffer even when a source vector fails range validation.
Source vectors also accept marker endpoints and reversed bounds on the diff
path; unused vector slots are ignored (`qa_replace_region_narrowing`).
The same suite checks callback-visible target bounds, point clamping and
restoration after normal returns, errors, throws and callback buffer switches.

`transpose-regions` uses one in-place overwrite and one before/after hook
pair. Point and markers follow GNU's three-segment permutation (start
inclusive, final end exclusive), regardless of marker insertion type;
LEAVE-MARKERS preserves character coordinates. Overlays remain fixed.
Indirect viewers' saved point/restriction markers move with the shared
text. Undo records equal separated regions individually, otherwise the
whole span; source properties and byte kind survive. Attribute clearing
records property undo without extra hooks or ticks. An interfering gap
moves to the nearer span boundary, choosing the end on a tie. One Replace
observer edit updates tree-sitter without transient delete/insert states.
Regressions: `qa_transpose_regions`, `qa_treesit_edit`, and the shared-text
observer unit test.
Like GNU's modification preparation, transposition sets `deactivate-mark`
after successful before-change hooks. Inhibited modification hooks and the
adjacent-empty no-op leave it unchanged.
`qa_transpose_properties` separately pins clipped and overlapping plists,
explicit nil property presence, and shared property-value identity after GC
for equal, unequal, adjacent and reversed regions with either marker flag.

`append-to-buffer` follows simple.el's source/destination/window lookup and
save-excursion ordering, then delegates to `insert-buffer-substring` instead
of copying a plain Rust string. The transfer primitive snapshots properties
after before-change hooks and grafts them before after-change hooks, with
one text tick and no extra property tick. Even an erroring after hook sees
the copied attributes. Empty transfers report only the zero-length after
change; no modification or mark deactivation occurs. Regressions:
`qa_append_to_buffer` and `qa_buffer_transfer` (byte-mode conversion,
restrictions, reversed/marker bounds, self-copy, read-only/error ordering,
hook context and window points).
The append suite also pins lower/upper/outside source restriction errors,
unchanged destination/current-buffer/point on errors, and nil/reversed valid
bounds (`append-to-buffer` uses the transfer primitive's validation).

`save-mark-and-excursion`'s direct special-form path uses the same mark
save/restore helpers as its macro expansion. Restore point/current buffer
before mark cleanup, and propagate cleanup errors ahead of body errors.
An initially unset mark must detach again; activation values are copied
verbatim through normal variable binding/watch semantics. Saved markers
track their actual buffer, including a foreign mark marker. The prelude
macro uses a fresh uninterned saved-variable symbol to avoid capture.
Regression: `qa_mark_excursion` covers direct, expanded and byte-compiled
paths, hook state/order, binding values, GC, nested scopes and nonlocal exits.
Its hook-count fixtures bind transient-mark-mode explicitly because the
library harness and CLI startup have different preexisting mode defaults.

`Symbol.name_string` retains the Lisp name string by identity for new
`intern`/`make-symbol` calls; internal names acquire a stable string
lazily. Existing interned names keep their original string. Portable
images encode the name as a graph reference so aliases, representation,
and live properties survive restoration. Regressions:
`qa_case_name_properties` and `tests/portable_dump.rs`.

## Character strings and output

`char-to-string` retains every accepted Emacs character code through
`lisp_char` or the Mule proxy registry. Non-ASCII results are explicitly
multibyte, including eight-bit character codes. Surrogates remain logical
characters internally; readable printing must retain them (or escape the
logical code), and batch stdout must encode them with `internal_encode`.
Encoding the Rust proxy scalar directly emits the wrong bytes. Regressions:
`qa_char_to_string` and the byte comparisons in `tests/char_output_cli.rs`.

## Overlay equality

Registered `[overlay BID IDX]` handles are pseudovectors, not ordinary
vectors for equality. `overlay_state` reads their current editor data.
When a buffer is killed, its canonical handles append their plist and
insertion flags, so detached state lives with the Lisp object rather than
the dead buffer. Moved-away stubs must never overwrite that state. Revival
registers a fresh live slot and drops the inline state; image restoration
must prefer inline state over potentially reused numeric buffer ids.
Overlay copies and `overlay-properties` copy plist cells, retaining shared
property values. Regressions: `qa_overlay_lifetime` and killed-overlay
state in `tests/portable_dump.rs`.
GNU compares buffer and live bounds, then the ordered overlay plist at
depth + 1; insertion flags do not participate, and detached bounds are
ignored. A look-alike ordinary vector must remain a different type.

The shared comparison carries the `equal-including-properties` mode into
containers and overlay plists. String interval property values still use
ordinary `equal` at depth zero, as GNU's `intervals_equal_1` does.
Equal-table normalization sends overlays (including nested ones) through
the existing bounded semantic sxhash and collision comparison, rather
than hashing the handle's identity slots. Regressions:
`qa_overlay_equality` and deleted-overlay keys in `tests/portable_dump.rs`.

Overlay precedence shares GNU's primary priority, containment and secondary
priority comparison between sorted queries, effective character properties
and rendered faces. Priority components are fixnums; a cons supplies both
components, including `(nil . N)`. Category defaults apply unless explicitly
shadowed. The comparison can be non-transitive for crossing intervals, so the
shared sorter remains bounded without assuming a total order. Face candidates
are ordered per position. Regressions: `qa_overlay_priority` and
`tui_overlay_priority_matches_effective_face`.

## Custom hash-table tests

`define-hash-table-test` tables capture their comparison/hash functions at
creation. `gethash`/`puthash`/`remhash` call the hash function and compare
colliding entries with the supplied predicate, preserving the original key
on replacement. Copies retain the captured functions and test name; printed
custom tables read through the same constructor/insertion path. Lisp
callbacks may read the table but cannot mutate it, and errors/throws restore
mutability. Built-in eq/eql and compact acyclic equal keys keep their
normalized-key path. Cyclic or large equal keys use a bounded `sxhash`
bucket plus `equal` on the original keys: different cycle periods can
compare equal, while truncated-hash collisions must remain distinct.
Equal/custom tables retain collision-chain order through slot reuse and
resize; the reader uses the same lookup path. Normalization is bounded
so long tails/shared DAGs cannot overflow before collision checking.
Regressions: `tests/qa_hash_custom.el`, `tests/qa_hash_cycles.el`
(GNU Emacs 31.1).

