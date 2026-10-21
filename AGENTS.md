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
  provided by `src/lisp/prelude.el` at init, and `(require 'subr)` hits
  the pre-registered feature, so the file only matters for `load`.
- `etc/themes/*.el` — the 25 built-in themes referenced by
  `lisp/theme-loaddefs.el`.

All core GNU files are now ported byte-identically — including the
ones that used to be folded into `src/lisp/prelude.el` (bindings,
button, custom, faces, files, font-lock, frame, help, indent, isearch,
jit-lock, keymap, menu-bar, minibuffer, mwheel, simple, startup, subr,
window).  The prelude's own folded definitions still drive init; the
real files resolve `(require ...)`/`load` through EMBEDDED_LISP.
Skipped: `.dir-locals.el`, `ldefs-boot.el`
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

`locate-library` searches the caller's load-path before the embedded
table. An explicit PATH is authoritative. Default embedded hits return
absolute, readable `.el` paths: use an existing flat bundled file, or
publish its source into a private process-local temporary directory
(`src/lisp/embedded_cache.rs`). Publication is atomic; repeated lookups
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

`src/lisp/dump_history.json` records GNU 31.1's 136 dumped libraries and
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
(`src/lisp/forwarded_variables.txt`, regenerate with
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
(`src/lisp/builtins/backquote.rs`) — GNU byte-compiles backquote.el, so
an interpreted copy both cost ~700 ms of macroexpansion per boot and
broke on `(fmakunbound 'car)' (its `car' calls are `Bcar' opcodes in
GNU).  `backquote'/`\`'/`backquote-list*' are bound as
`(macro . #<subr>)' cells after all library loads (`install_macros' in
`Interp::new'), and the `backquote-*-symbol' variables are honored.
Similarly, `subr.el''s Lisp defuns are loaded for cell-shape parity but
then any symbol that no longer names a Rust subr is re-fset to the
subr (right after `boot_lib "subr"') — several subrs carry parity
tweaks (gensym's interned-name skip, oddp's marker rejection,
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

## AOT .elc twins (`src/lisp/embedded_elc.rs` + `lisp-elc/`)

`tools/gen_boot_elc.py` runs GNU Emacs's `byte-compile-file' over the
boot libraries and emits `lisp-elc/<stem>.elc' blobs plus a Rust table
`src/lisp/embedded_elc.rs'.  `load_library_opts_msg' prefers a blob
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



## Known runtime gaps (not port defects)

- The files that used to be Emacs-internal-encoding (see the
  EMBEDDED_LISP note above) are now UTF-8 transcodes — readable by
  `insert-file-contents`, but with U+FFFD where GNU stored
  private-plane chars; not byte-identical to GNU.
- `byte-opt` "lambda used as function name" warnings (stale — no
  longer emitted since the `equal`-on-`#[...]` fix; keep the note as
  a canary: if the warning returns, suspect the constant-dedup path).
- `xwidget-internal` unimplemented. NS/macOS GUI primitives are
  covered by `src/lisp/builtins/nsgui.rs` (see below). Native
  compilation is implemented in `src/lisp/builtins/comp.rs` (see
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
  NID GENERATION]` / `[treesit-compiled-query ID]`; `print.rs` renders them as
  `#<treesit-...>'.
- **Never derive nodes from `Tree::clone()`** — it's `ts_tree_copy` (a
  *different* `TSTree`; root id even differs). Nodes must come from the
  parser's *stored* tree — see `stored_root()` which detaches via
  `Node::into_raw`/`from_raw`.
- `SharedText` publishes physical insert/delete/character/reset edits to
  weak Rust observers, independently of Lisp modification hooks. Each
  parser owns an edited copy of its last tree plus the visible UTF-8
  input/bounds. Apply every edit in order: combining distant edits into
  one replacement gives correct syntax but incorrect notifier ranges.
  The original tree remains stored for existing Lisp nodes until parsing
  succeeds. Nodes never come from the edited copy. Parser deletion drops
  its subscription; text-storage changes reattach it, and independent
  buffer copies start without observers. Narrowing/context changes sync
  the visible input separately. Regressions: `qa_treesit_edit` and the
  UTF-8 point/edit-observer lifetime unit checks.
- Parsers belong to the requested buffer, including indirect buffers.
  Resolve/reuse/list them by that buffer's identity, not its base buffer.
  Shared text edits reach each parser through the physical edit observers;
  narrowing, parse generations, tags, included ranges and buffer lifetime
  remain independent. Regression: `tests/qa_treesit_indirect.el`.
- Node staleness is GNU-style: `check_node` compares `parse_count`;
  `treesit-node-check 'outdated` is passive (nil until a reparse
  happens elsewhere). Node records and cache keys include that generation:
  native node-id reuse cannot refresh an old Lisp node. Outdated printing
  returns `#<treesit-node-outdated>` before accessing the native node,
  whose original tree may already be freed. Same-generation wrappers
  remain `equal` and `treesit-node-eq` while distinct under `eq`.
  Regression: `tests/qa_treesit_node_generation.el`.
- Query capture uses raw `ts_query_cursor_next_match` FFI; a match may
  carry 0 captures (predicate-only patterns).
  Query `#match?` regexps are always case-sensitive, independent of
  `case-fold-search`; leave that variable's value unchanged. Regression:
  `tests/qa_treesit_query_case.el`.
  Predicate capture-name lookup selects the last same-name capture in
  each match, matching GNU's reversed internal capture list. Public
  capture/group/node-only results retain query order. This applies to
  `#eq?`, `#match?` and `#pred?`; it is not an all-captures test.
  Regression: `tests/qa_treesit_repeat_capture.el`.
- Included-range validation runs in list order with fixnum endpoints.
  Bounds/ordering/overlap errors carry GNU's explanation and the original
  whole range list; improper-list errors also retain that original list.
  Validate each prefix before inspecting its tail and retain cycle
  detection. Regression: `tests/qa_treesit_ranges.el`.
- Notifier add/remove validate the parser before checking `symbolp`;
  symbols need not be bound to functions. Registration is unique by `eq`,
  public lists are fresh copies in registration order, and dispatch runs
  the latest registration first. Regression: `tests/qa_treesit_notifiers.el`.
- `treesit-node-type` and `treesit-node-field-name-for-child` return
  *strings* (not symbols) like GNU.
- `treesit-language-available-p` with DETAIL returns `(t . nil)` /
  `(nil . signal-data)`; `treesit-library-abi-version` returns an int
  (min-compatible when arg non-nil).

Verified: `treesit.el`, `json-ts-mode`, `c-ts-mode`, `rust-ts-mode`
load, parse, and fontify correctly under `--batch`.

## Subprocess pipe and PTY transports (`src/lisp/process/child.rs`)

`make-process` selects stdin and stdout independently: `pipe`, `pty`, or
the `(INPUT . OUTPUT)` pair; nil on either side inherits the dynamic
`process-connection-type`. Stderr shares stdout unless explicitly separated.
`process-tty-name` accepts `stdin`, `stdout`, or `stderr` to inspect a stream;
its nil default reports a terminal used by either side.

`ProcIo::Child` owns separate stdin/stdout descriptors plus an optional PTY
control descriptor. Every descriptor is close-on-exec. Pipes preserve raw
control bytes; `process-send-eof` drains pending input and closes only the
stdin writer, leaving stdout readable. Later writes to a still-running
pipe child are discarded, matching GNU's `/dev/null` redirection. PTY input
keeps GNU's canonical discipline and VEOF behavior. Normal exit, deletion
and failed spawn release all owned descriptors, including separate stderr.
Observe child exit before draining output so final bytes cannot arrive
between an empty read and a decision to close the channel.

Regressions: `qa_process_connection.el` checks GNU's transport matrix and
stream selectors; `tests/process_pipe.rs` verifies kernel FIFOs, raw bytes,
EOF, descriptor closure and failed-spawn cleanup.

`process-send-eof` half-closes TCP and Unix streams with `shutdown(Write)`
while retaining their read descriptor and live status. `Proc.write_eof`
models GNU's subsequent `/dev/null` writes: validate/encode normally, then
discard output, including TLS output, and make repeated EOF harmless.
Pending TCP connections finish through the existing event-loop wait;
listeners signal a not-running error and datagram EOF remains a no-op.
Return the original argument, including names, buffers and nil. EOF does
not add an ISO-2022 reset or an unused UTF-16 signature. Regressions:
`qa_process_network_eof` and `tests/network_eof.rs` (owned IPv4/IPv6/Unix
sockets, actual kernel EOF, receive continuity and descriptor closure).

Deleting a TCP or Unix listener drops its backend before the deletion
sentinel, even when Lisp keeps references. Repeated deletion must not close
a recycled descriptor or affect a replacement listener. GNU leaves a Unix
socket's filesystem pathname on explicit deletion: clear the backend's
cleanup path before dropping it; callers unlink it before rebinding.
Regressions: `qa_network_listener_delete` and the listener-lifetime test in
`tests/network_eof.rs` (serialized descriptor-reuse checks).

Accepted TCP/Unix connections are registered before callbacks. Invoke
`:log` with the listener, accepted process and `accept from HOST\n`, then
read the accepted process's current sentinel and send `open from HOST\n`.
Callbacks run without process borrows; log may replace functions, read
input recursively, or delete either process. Deletion does not suppress
the subsequent open notification; a log error propagates and stops it.
Unix uses `-` for HOST; IPv6 preserves GNU's uncompressed native-word
spelling. Regression: `qa_network_accept_callbacks`.
Keep the constructor's log function in `Proc.log` and include it in
`Proc::gc_values`; changing a returned contact list must not replace it.

`Proc.buffer_object` is the canonical Lisp buffer reference and is included
in `Proc::gc_values`. It survives `BufferSet` unlinking the killed buffer;
`Proc.buffer` remains the operational id used by live-buffer I/O lookups.
Constructor attachment and `set-process-buffer` update both, while nil
explicitly releases the reference. Network/pipe/serial setters also update
the contact plist's `:buffer`, independently of the canonical slot. Marker
detachment still follows buffer death. Regressions: `qa_process_buffer_identity`
(GC, dead attachment, name reuse, deletion, contact mutation) and the promoted
check in `qa_process_defaults`.

`set-process-window-size` checks PROCESS first, then HEIGHT/WIDTH as unsigned
16-bit integers (including exact bignum range errors). It applies TIOCSWINSZ
to the process input descriptor, which is a child's output-side fd; using
the separate tty fd would incorrectly succeed on input-only PTY connections.
Return t on ioctl success and nil for non-terminals, network processes or
closed descriptors. Pixel fields are zero, matching GNU sysdep.c.
`qa_process_window_size` reads real child terminal geometry for ordinary and
mixed pipe/PTY connections, repeated resizes and both dimension boundaries.

IP contact addresses come from the socket's real local/peer endpoints,
including after DNS resolution. Synchronous and asynchronous clients both
honor the requested address family. Accepted connections copy the listener's
creation plist and override server/host/service/local/remote; the reported
service is the peer port. Preserve the original hostname for outgoing clients.
`process-contact` with t returns the stored plist, matching GNU's shared-data
identity. Unix addresses are unibyte strings from a zero-initialized sockaddr
and its bounded reported length; retain anonymous/abstract embedded NUL bytes.
Retain Unix accept's peer address before callbacks or a fast disconnect;
accepted descriptors remain close-on-exec. Outgoing Unix remote information
keeps the actual connect pathname spelling, including symbolic links.
Regressions: `qa_network_contacts` and kernel endpoint checks in
`tests/network_eof.rs`, including simultaneous IPv4/IPv6 listeners for DNS.

## Process coding (`process/coding.rs` + `builtins/coding_stream.rs`)

Asynchronous child, pipe, TCP, Unix and datagram I/O uses the shared
string/file codecs. Constructor coding names are validated before creating
OS I/O; `set-process-coding-system` validates both directions before changing
either. Accepted network processes inherit the listener's coding names with
fresh decoder/encoder state. Separately buffered stderr uses its own default
coding, matching GNU's separate stderr process.

The stream layer retains only incomplete character/EOL bytes and finite
ISO-2022 designation state. UTF-16 signatures are emitted once per encoder
run. Unibyte sends bypass character conversion while preserving EOL
conversion; they reset encoder state even when empty. UTF-7 and HZ keep GNU's
Lisp converter behavior per read, with only their underlying UTF-8 framing
carried between reads.

Conversion hooks run outside the process's RefCell borrow. Codec state and
coding names are stored before post-read hooks or filters can reenter I/O.
Filter strings retain byte kind and charset properties; default insertion
maps raw bytes into multibyte buffers' eight-bit characters. Flush pending
input at natural EOF before the terminal sentinel, including the incomplete
character rule for empty datagrams.

Regressions: `qa_process_coding.el`, existing coding/network QA, and the
`coding_stream` boundary/state unit tests.

`process-send-region` resolves PROCESS before validating START/END.
Both positions accept integers and live markers (including markers from
another buffer); bounds use the current buffer's accessible BEGV..ZV.
Range errors preserve the source buffer and original endpoint order and
marker identities. Wide bignums are range errors after both argument
types have been checked. Valid regions retain byte kind and text
properties through `buffer-substring`; endpoints are never clamped.
Regression: `qa_process_send_region` (marker, narrowing and wire checks).

## Process input pause (`src/lisp/process.rs`)

`stop-process` inhibits reads/accepts for every network backend, including
Unix streams, TCP/Unix listeners and TLS. Connect completion still runs;
stopping input preserves writes, queued kernel bytes and decoder state.
Repeated stop must retain the first pre-stop status so one continue restores
the connection/listener. Keep this separate from `filter=t`, whose listener
exception still accepts peers; do not suppress buffered output from real
OS-stopped child processes. Regressions: `qa_network_stop` and the stopped
stream kernel peek test in `tests/network_eof.rs`.

The canonical `filter=t` stops descriptor reads, including pipe, local,
TCP/TLS and serial input. Listening servers still accept connections;
their accepted peers inherit the filter. Child exit notification and
separate stderr keep progressing while stdout is paused. A targeted
`accept-process-output` waits for stdout; a global wait also counts stderr.

`poll_proc` dispatches a single read before the next read, so callbacks
can pause, delete or recursively wait without consuming later chunks in
advance. Each turn is bounded, stdout/stderr priority alternates, and the
datagram path retains its own packet bound. Exited paused children discard
unread stdout and decoder tails; reported terminal processes cannot flush
the same tail again after the filter changes. Regression:
`qa_process_filter_pause` (24 GNU checks, including 50 KB pause/resume).

`accept-process-output` waits indefinitely only when PROCESS is specified
and both SECONDS and MILLISEC are nil/omitted. PROCESS=nil without a timeout
polls immediately; an explicit zero in either time argument is finite.
Dead targets return immediately. The wait keeps timers and cooperative
threads progressing, respecting JUST-THIS-ONE's fixnum timer inhibition.
Polling other processes excludes the already-polled target so output
cannot be consumed there with its return value discarded. Regression:
`qa_accept_process_wait` (timers, threads and a queued-output race).

Large `accept-process-output` timeouts use elapsed floating-point
accounting, never unchecked Duration/Instant construction. The effective
wait is bounded by the next active ordinary timer (unless inhibited);
child-exit notifications also force one nonblocking turn. Large effective
waits are validated with the host's `pselect` on an internal ready,
close-on-exec socket, so validation never sleeps or consumes process data.
OS failures signal `file-error` with `Failed select` and the actual errno.
This preserves timer-before-error ordering and platform-specific limits.
Listener targets return without selecting; stopped connections retain
their select-timeout validation. Exit notifications bound only one turn.
Regression: `qa_process_timeout`, plus CLI catch/exit/notification checks.
The headless entry point propagates unexpected interpreter-worker panics
after worker cleanup, instead of silently reporting exit status zero.

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
The backend feature is `native-compile`; do not also provide `native-comp`.
Feature presence describes the included backend while the availability
predicate probes the runtime compiler. Optional `dbusbind`/`lcms2`
features remain gated by successful runtime library loading.
Regressions: `qa_feature_capabilities` and `tests/nativecomp.rs`.

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
- File units retain the macroexpanded top-level program, interleaving
  ordinary forms with native function registration in source order.
  Ordinary initialization runs on every load, not during compilation;
  `eval-when-compile` and `eval-and-compile` retain their phase semantics.
  A failed load keeps earlier definitions callable and skips later forms.
  Existing `.eln` files must be recompiled to gain corrected generated
  code or newly retained top-level forms; old units remain loadable.
- File compilation propagates reader errors, including malformed tails after
  valid definitions. Only clean EOF ends the source scan; failure restores
  compiler-local macros and bindings without creating or replacing output.
- Native `and` starts with the identity `t`, so `(and)` returns `t`;
  nonempty `and` and `or` retain left-to-right short-circuit evaluation.
- Native literal objects (including quote and fallback body constants) are
  read once per registered function and retained as Values, never temporary
  object-table handles. Nested native calls restore the caller's constant
  cache. Runtime allocation and ordinary file initialization remain fresh.
  Existing `.eln` files remain loadable but need recompilation for this fix.
- Direct native string constants retain their source byte-kind flags as well
  as their raw characters, including unibyte bytes and explicitly multibyte
  ASCII. The constant cache distinguishes identical bytes of different kinds.
  The kind-aware callback is appended to preserve existing unit ABI; old
  `.eln` files remain loadable and need recompilation for this correction.
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

Regression suite: `tests/nativecomp.rs` and the `qa_native_*` fixtures.

## Subprocess environments (`src/lisp/process/environment.rs`)

Subprocess environments are built by `src/lisp/process/environment.rs`
from the current Lisp `process-environment` binding. Async children,
`call-process`, `call-process-region`, `process-lines` and shell command
helpers clear inherited OS variables and install this child-local copy;
they never mutate the editor's OS environment. Like GNU, the first entry
wins (bare names suppress later assignments), only the initial string
prefix of the list is consumed, and entries end at the first NUL.
Unibyte values retain their bytes. Program lookup uses Lisp `exec-path`
before applying the child's PATH. Regression: `qa_process_environment`.

`editor::process_directory` validates the launching buffer's
`default-directory`, using `unhandled-file-name-directory` (nil means
HOME), and sets each child's `Command::current_dir` without changing the
editor's OS directory. Relative program/exec-path names and synchronous
input files resolve against this directory. Existing PWD entries are
corrected to the logical child directory, including symlinks; an unset
PWD stays unset. Invalid directories signal `Setting current directory`
with the original Lisp name. Regression: `qa_process_directory`.

Executable lookup preserves `exec-path` element order, including nil
entries for the child directory and dotted list prefixes. A nil path
uses that directory; a nonempty path has no implicit cwd fallback.
Relative program names containing slashes still search each entry.
`faccessat(..., X_OK, AT_EACCESS)` checks effective execution permission;
later missing candidates preserve earlier specific errors (permission,
directory, symlink loop). Regression: `qa_process_exec_path`.

## Minibuffer reads (`src/lisp/eval.rs` + `src/editor/mod.rs` + `src/term`)

GNU structure kept: `read-file-name`/`completing-read`/`read-buffer`
are subrs that delegate to the user variables
(`read-file-name-function`, `completing-read-function`,
`read-buffer-function`) or their -default counterparts, exactly like
GNU's Lisp wrappers.  `f_completing_read_default` does
`completing-read-default`'s work in Rust: picks
`minibuffer-local-must-match-map`/`minibuffer-local-completion-map` by
REQUIRE-MATCH, composes `minibuffer-local-filename-completion-map` on
top while `minibuffer-completing-file-name' is non-nil (so its nil SPC
binding un-binds `minibuffer-complete-word`), specbinds
`minibuffer-completion-table'/`-predicate'/`-confirm'/`--require-match',
then `minibuf_read`.  `minibuf_read` is `read_minibuf`: installs the
prompt (field props), KEYMAP or `minibuffer-local-map' as the buffer's
`local-keymap', runs `minibuffer-setup-hook' (plain, not safe), pushes
the `exit' catch tag, and the front-end `minibuf_loop' dispatches keys
through the minibuffer's real keymaps until `exit-minibuffer's throw
(`minibuf_exited') or `minibuffer-quit-recursive-edit'
(`minibuf_exit_fn' — called after unwinding).  `execute-extended-command'
goes through Lisp `read-extended-command' (must-match completion).

Gotchas: `subr_interactive`'s sentinel for `(interactive)' is
`"\u{1}nil"` — a literal backslash typo in the table silently parses
as 8 unknown letter codes and breaks every affected command with
`wrong-number-of-arguments` on RET (regression: `exit-minibuffer` in
the minibuffer).  `minibuf_loop` must propagate `poll_key` errors —
swallowing them spins forever on dead input.

Regression suite: `tests/tui_session.rs` (scripted `KeyIo` drives
C-x C-f/TAB/C-x b/M-x/C-x C-s/C-g/C-x C-c through the real loop, and
captures `render_grid` output for the point-visibility scroll test).

`term::render_grid` is the shared display engine (crossterm + gpui):
like GNU's `redisplay_window' it keeps each window's point on screen
by scrolling `window.start' — the minimal amount when point sits just
outside, recentered (`point - text_rows/2`) when it is more than a
window-height away — and `window.hscroll' likewise for the cursor
column (lines are truncated, no continuation rows).  Without it
cursor motion past a window edge left the buffer text frozen and hid
the cursor.

Scrolling primitives live in `src/editor/mod.rs` (`window_scroll'):
like GNU's `window_scroll' they move the window's `start' AND, when
the scroll would leave point outside the window, move point to the
edge GNU picks — first visible line on scroll-up, last on scroll-down
— so render_grid's snap-back cannot cancel the scroll.  C-v/M-v are
the Lisp `scroll-*-command's in `lisp/window.el' (loaded via
EMBEDDED_LISP), which call the `scroll-up'/`scroll-down' subrs;
defaults are `text_rows - next-screen-context-lines' and raw `P'
prefixes like `(4)' are decoded via `scroll_arg'.  `select-window'
saves the previously selected window's buffer point into
`window-point' (only when that window shows the current buffer —
`with-selected-window'/`scroll-other-window' depend on it).
Horizontal scrolling follows GNU's `suspend_auto_hscroll' model:
`scroll-left'/`scroll-right'/`set-window-hscroll' set
`Window.suspend_hscroll' (plus `min_hscroll' when given the
interactive SET-MINIMUM arg) instead of moving point, and
`render_grid' skips its cursor-column hscroll snap while suspended —
clearing it as soon as `window.point != window.old_point' (the
old_pointm check in GNU xdisp.c) and clamping the displayed cursor
to the window edge meanwhile.  `auto-hscroll-mode' is read
buffer-locally per window (t default).  `scroll-left'/`-right' are
`disabled' commands under GNU — `C-x <'/`C-x >' prompt first.
Regression: `tui_hscroll_suspends_until_point_moves' in
tests/tui_session.rs.

Faces: `render_grid' also emits `Grid.attrs[y][x]` (a `CellStyle' per
cell — fg/bg RGB, bold/italic/underline/overline/strike/inverse/
extend).  `style_at_pos' merges GNU's `compute_char_face' order:
`default' seeds, the active `region' face, `face'/'font-lock-face'
text props (later runs win), then overlays sorted by ascending
`priority' (a `window' property restricts an overlay to its window;
nil = all).  `FaceCtx::named' merges `face_t_table' (new-frame
defaults) under `face_table' (defface/spec layer) over
`builtin_face_defaults' — the `(t ...)'/`((type tty) (class mono)
...)' fallbacks of GNU's defface specs for faces whose files aren't
booted (`isearch'→inverse, `lazy-highlight'→underline, `region'→
gray bg + extend, `highlight'/`match'→inverse, mode-line faces→
inverse/bold).  Colors come from `#rgb`/`#rrggbb` or
`color-name-rgb-alist' + the eight tty-standard names;
`:distant-foreground' fills fg only when no real :foreground (GNU
drops it when it blends in); `:inherit' chains through
`face-resolve' (face-alias aware).  The crossterm front-end emits
true-color SGR escapes (fg default → SGR 39); gpui maps styles to
highlights incl. a `defhighlight' cache for font-lock runs.
`isearch_loop' itself installs real overlays after each key —
`isearch' face + priority 1001 on the current match,
`lazy-highlight' + priority 1000 + `window' on other visible matches
(isearch.el isn't booted, so the step logic lives in Rust;
`search-highlight'/`isearch-lazy-highlight' gate it, unbound→t).
Regression: `isearch_extends_match_in_place_and_overlays_paint',
`tui_isearch_highlights_matches', `tui_face_text_props_and_region_paint'.


## Portable time formatting

`src/lisp/builtins/timefmt.rs` implements GNU flag ordering, field widths,
numeric fields, composite dates, fractional precision, and zone formatting.
Never forward GNU flags or widths to libc: macOS strftime does not support
those extensions. `misc::strftime_spec` supplies only unadorned locale text
(names, locale composites, and eras). Keep the resolved zone name owned while
native locale conversion runs, since restoring TZ can invalidate `tm_zone`.
Width is measured in bytes, numeric zero-padding follows the sign, text
normally space-pads, and the last padding flag wins. `%N` uses nanoseconds
and right-padding; `%F` applies its width to the year field. Regressions:
`qa_time_modifiers`, `qa_time_zero_zone`, `qa_timefmt`, and the portable
formatter unit test that rejects nonportable calls to its native callback.

## Cooperative Lisp threads (`src/lisp/threads.rs`)

`make-thread` queues a `corosensei` coroutine; it returns before the worker
runs. `thread-yield`, joins, sleeps, mutex/condition waits, timed event reads,
process polling and terminal input loops provide scheduling points. Workers
share the interpreter's OS thread, so `Rc` values and native-comp TLS handles
never cross OS threads. Each coroutine reserves a 64 MiB virtual stack for
the Rust evaluator's larger frames.

Do not pass a raw shared `&mut Interp` between suspended stacks. Every stack
has its own interpreter storage: owned runtime boxes move through coroutine
inputs/yields, leaving a vacant, unevaluated interpreter in the suspended
storage. `ThreadContext` parks stack-specific fields (lexical environment,
dynamic bindings, current buffer, handlers, search registers, load/eval
flags, backtrace) on its own coroutine stack. Dynamic cells are exchanged in
reverse/forward binding order without firing watchers, exposing global
values to other threads and preserving each thread's nested bindings.
The scoped yielder pointer only refers to its own live coroutine stack;
there is no `Send` implementation for the interpreter or Lisp values.
Native call-local TLS (`OBJTAB`, `PENDING`, `ACTIVE_NATIVE`, compiler context)
is parked with that stack too; native code/metadata registries stay shared.
Native callbacks and entries use `C-unwind`, and generated C includes unwind
tables, so destroying a worker suspended inside native code releases its
Rust frames. Never take TLS from a vacant runtime during forced unwinding:
that TLS currently belongs to the resumer.

Mutexes track an owner and recursive count. Condition waits release all
levels and restore them after notification or signaling; `thread--blocker`
reports the actual wait object. Portable images preserve mutex depth, with
old owned-mutex images interpreted as one level. Scheduling is cooperative:
ordinary computation and blocking foreign calls do not preempt themselves.

Regressions: `qa_thread_scheduling.el`, `qa_thread_bookkeeping.el`, the
`eval::threads` unit tests, and CLI/TUI/portable-image thread tests.

## GNU-compat QA suite (`tests/qa.rs` + `tests/qa_*.el`)

Each `qa_*.el` file is a flat list of `(qa FORM EXPECTED)` checks driven
by `tests/qa_framework.el`.  Every expected value was verified against
GNU Emacs 31.1 (`emacs -Q --batch -l tests/qa_framework.el -l
tests/qa_<dom>.el` must print "N checks passed").  A file doubles as the
GNU-side ground truth and the remacs regression runner: `cargo test
--test qa` evals framework+file per `#[test]` (one `Interp` each).

Known GNU↔remacs divergences are pinned in-place:

- `qa-xf` / `qa-xf-err` / `qa-xf-err-data` / `qa-xf-prin1` /
  `qa-xf-same` — check still enforced under GNU; under remacs the result
  is recorded as XFAIL (still wrong) or XPASS (now matching GNU).
- `qa-skip` / `qa-skip-err` — for forms that CRASH or HANG remacs
  (uncatchable); still run under GNU.
- remacs is detected via `(string-match-p "remacs" (emacs-version))`.
- `qa--done` fails only on unexpected FAILs; XFAIL/XPASS/SKIP entries
  are printed for the ledger but the summary still says
  "N checks passed (K xfail) [M xpass]".

Known remacs bugs the ledger currently documents (fix → flip the
qa-xf back to qa and watch it go XPASS→pass):

- `(require 'eieio)` / `(require 'cl-lib)` work now; qa_eieio passes
  fully.  Two causes fixed: a mis-balanced `cl--loop-expand' in
  prelude.el (a `cond' closed early, splicing later clauses into the
  `let' body so clause forms ran as calls and loop expansion never
  finished — this same bug hung `require'), and `equal' missing GNU's
  EQ fast-path, which overflowed the stack on eieio's cyclic class
  records.
- `copy-marker` on a non-marker signals `wrong-type-argument' (the
  `end-of-file' report was fallout of the same prelude misparse);
  `(copy-marker)' with no args makes a detached marker (qa_marker).
- No unibyte buffers: `enable-multibyte-characters' tracks
  `set-buffer-multibyte' (qa_misc3).  Unibyte *strings* are now
  modeled: literals with only ≤0xFF octal/hex escapes stay unibyte,
  `concat' follows GNU's promotion rules, `string-make-unibyte'/
  `string-as-multibyte'/`string-as-unibyte'/`encode-coding-string'
  (incl. `:default-char' → "?" for us-ascii) and `string-bytes'
  (per-char storage count) match GNU.  `string-to-list` on unibyte
  data and `position-bytes`/`byte-to-position` base-1 conversions
  are under GNU-parity rework (verify before trusting).
- charset layer: rewired to GNU char_charset() semantics — ordered
  charset list model, `char-charset`/`charset-after`/`make-char`/
  `split-char`/`decode-char`/`encode-char` (incl. integral-float and
  cons args), `char-width`, `charset-priority-list`,
  `set-charset-priority`, `sort-charsets`, `find-charset-*`,
  `coding-system-charset-list`, synthesized builtin attribute plists.
  `char-width` and the string width/truncation helpers consult the
  live `char-width-table` using original Lisp character codes, including
  surrogate/high-Mule/eight-bit proxies. ASCII uses GNU's direct width
  rules; sparse bootstrap tables fall back to the static width ranges.
  Zero-width overrides, parent inheritance, mutation, buffer-local
  tables, and truncation at an exact width are covered by
  `qa_character_width_table`.
- Indirect buffers: fixed — text, modification/save ticks, text properties,
  and the undo list are shared via SharedText (per GNU's buffer_text).
  BEGV/ZV/point/markers
  stay per-buffer, `buffer-base-buffer` returns the buffer object. Lisp
  references can retain killed objects after `BufferSet` unlinks them.
  `buffer-swap-text`
  errors on indirect buffers, `set-buffer-multibyte` errors on
  indirect buffers, buffer-list-update-hook runs on create/kill/bury,
  and `set-buffer` syncs the undo list with the base. Auto-save history
  (`Buffer.auto_save_tick`) is per-buffer; setting the `autosaved` flag
  through an indirect view updates its base buffer's history.
- `cl-loop`: fully works now — the ledger's hang was the same
  `cl--loop-expand' misparse; qa_clloop{,2..7} all pass.
- `cl-destructuring-bind`/`cl-struct`/`cl-typep`/`cl-the` fixed too —
  the same misparse was starving their expansions.  The old
  `cl-locally (declare (special …))' pin was itself mis-specified
  (it asserted void-variable under DYNAMIC eval; GNU's `(eval F nil)'
  is dynamic regardless of `lexical-binding'); the checks now run
  under `qa-lex' and pass on both.
- `and-let*' fixed: `lisp/subr-x.el''s Emacs-28 recursive copy shadowed
  the prelude's GNU subr.el defmacro (dropped); empty BODY now returns
  the last binding value (qa_subrx).  `progv' is gone from 31.1 core —
  it lives in obsolete cl.el (`(require 'cl)' first; tests updated).
  `pushnew` follows the same obsolete-package boundary: `cl-lib` alone
  leaves it void. `qa_obsolete_pushnew` pins `cl`'s keyword comparisons,
  key functions, generalized places, and single evaluation of subforms.
  `macrop`/`macroexpand` follow public function aliases, including the
  obsolete name's alias to `cl-pushnew`; predicate inspection preserves
  macro autoloads without loading them.
- Batch frame model differs from GNU -Q tty: `frame-parameter` values,
  window edges, `tty-*`/`pos-visible-in-window-p`, etc. (qa_window).
- `define-key` on prefix key, `keymap-set` return value, `help-function`
  docstrings, `defalias`/`defvaralias` cycle detection
  (`cyclic-*-indirection`), `subr-arity` on specials, `func-arity`,
  `documentation` for unbound/`when`, `interactive-form`,
  `funcall-interactively` — all fixed; qa_keymap/qa_symbol/qa_advice
  pass with no pins left.
- `format-time-string` %-zones/locale names, `format-spec`,
  `float-time` rounding, `time-convert`, text-properties
  (`add-face-text-property` defaults, `put-text-property` nil-end
  clamp, `text-properties-at` boundary, `char-property-alias-p`),
  `completions`/`completion-table-*`/`all-completions` boundary
  sorting, charset-priority order — all fixed; qa_timefmt,
  qa_textprops, qa_completion, qa_charset, qa_window, qa_case,
  qa_readprint, qa_regex all pass clean.
- `(describe-function)`/docstrings wired fully; `alist-get' default
  test fixed to `eq'/`assq' (qa_subrx passes clean).
- `process`/`file-attributes`/`locate-file`/`format-network-address`
  tails fixed; `substitute-in-file-name' "/a/~/b" → "~/b" now drops
  the slash like GNU (qa_filesys clean).  Process status follows
  GNU's status_notify (exit/signal removed from the list, EOF via
  GNU child_setup_tty termios: ICANON+VEOF=^D, -ECHO, +OPOST -ONLCR
  -TAB3 -*DLY); `file-locked-p' ignores stale same-host locks whose
  recorded boot differs (GNU within_one_second rule); `write-file'
  runs the GNU files.el tail (mode/hack-local-variables) so
  require-final-newline applies (qa_bufferfile clean).
- `numbers`: arbitrary-precision integers use immutable `Rc<BigInt>`
  objects (`num-bigint`); fixnums retain the fast inline integer path.
  Construct numeric results with `Value::integer` / `from_bigint` to
  canonicalize the fixnum boundary. `eq` preserves bignum identity;
  `eql`/`equal` compare their values. Exact integer/float comparisons,
  integer rounding of large floats, shifts, bitwise operations, printing,
  readers, and seeded BSD TYPE_3 random (including GMP-style 64-bit limb
  generation for large limits) share the unbounded numeric model.
  Hash dispatch uses the inclusive signed fixnum range: the minimum
  fixnum must not enter the bignum hash path via an absolute-value
  comparison. `qa_sxhash_boundaries` pins numeric/content hashes and
  identity-hash invariants; address-derived hashes can vary across GNU
  sessions and backends and must not be pinned to one process's values.
- `json`: `json-serialize' returns unibyte UTF-8, plist input forms
  objects, lone-surrogate `concat` crash skipped.  `json-value-p'
  does not exist in GNU 31.1 either (stale ledger note).  Remaining:
  `json-end-of-file' byte-offset in error data.
- `search`/`replace`: `search-forward'-COUNT all-or-nothing,
  `match-data 'integers' appends the buffer, `search-*-regexp' are
  same-cell aliases, `inhibit-changing-match-data' honored
  (qa_search); `forward-paragraph' reports subrp like GNU's
  nativecomp build; `replace-match' in-buffer paths (qa_replace);
  `re-search-backward' honors the GNU re_match STOP limit (matches
  can't run past pos), fixing `forward-symbol'/`beginning-of-defun'
  boundary behavior; case-fold items pass (qa_case clean).
  `replace-match` range-error data reports the selected match registers,
  independently of replacement length or escape parsing. Tests must set
  match data explicitly rather than depend on the last startup regexp;
  `qa_replace_bounds` covers this error precedence and preservation of
  buffer contents/match data after a range error.
- `seq`/`map`/`ring`: `seq-let' vectors, `make-ring' arity errors, and
  bool-vector `map-pairs'/`seq-do-indexed' all fixed (qa_seq/qa_mapring
  clean; map_seq gained a bool-vector arm).
- `syntax`: `modify-syntax-entry' on copied tables fixed; temp-buffer
  `syntax-table' scope pins resolved (qa_syntax clean).
- Batch `message' echo fixed: `load'-time "Loading FILE (source)..."
  now goes to stderr/*Messages* like GNU --batch, not stdout.
- `errors`: `arith-error' data is bare `(arith-error)' like GNU;
  `error-message-string' validates like print.c (listp arg, symbolp
  car — non-error data signals wrong-type-argument);
  `signal'/`user-error'/`file-error' hierarchies match; Fsignal
  rejects symbols without `error-conditions' ("Invalid error
  symbol") like GNU.

- Buffer-arg strictness split (GNU `decode_buffer' vs `get_buffer'):
  `buffer-size'/`buffer-modified-p'/`buffer-file-name'/
  `buffer-base-buffer'/`buffer-local-variables'/`buffer-local-value'/
  `buffer-local-boundp'/`local-variable-p'/`local-variable-if-set-p'/
  `verify-visited-file-modtime'/`buffer-swap-text'/
  `buffer-modified-tick' CHECK_BUFFER (a name string signals
  `wrong-type-argument bufferp'); only
  `insert-buffer-substring'/`compare-buffer-substrings'/
  `buffer-disable-undo'/`buffer-enable-undo'/`set-buffer'/
  `kill-buffer'/`bury-buffer'/`other-buffer' resolve names
  (`buf_or_name').  `args-out-of-range' from `buffer-substring'
  carries the buffer object like GNU's validate_interval_range and
  validates against BEGV..ZV (was whole-text len).
- `replace-regexp-in-string' SUBEXP past the match registers now
  signals `(error "replace-match subexpression does not exist" N)'
  like GNU's Freplace_match (was args-out-of-range).
- `error-message-string' accepts nil (→ "peculiar error" like GNU);
  still signals listp on non-list non-nil.
- `locate-library "subr-x"' returns the .el source — correct in this
  tree (no .elc exists); pinned as an artifact-layout divergence in
  qa_load.
- `internal--syntax-propertize' held the buffer borrow across an
  eval of `syntax-propertize-function' — now released before the
  call (and likewise in `f_syntax_after').

Total: ~5000 GNU-verified checks; run all: `cargo test --test qa`.


## Differential sweep (`tools/difftest.py` + `tests/diff/`)

GNU Emacs 31.1 was used as the oracle: every `fboundp` symbol
(~5.6k targets incl. Lisp fns, `targets.el`/`targets-sf.el` generated
by `dump-subrs.el` against GNU) is called via `apply` with 4–6
deterministic arities — min args, min+seeded corpus picks, and the
min-args buf-context variant (`dif--call-buf` runs inside a fresh
fixture buffer holding 3 lines of text, point at 9).  Each call is
`condition-case`-wrapped; the row payload is `dif--norm`'d
(objects → `(marker pos hasbuf)`/`(buf name)`/`(hash test count)`
shapes; wna payloads → `(wna ARGC)` since GNU's datum carries the
function object while remacs carries `(MIN . NONREST)` conses for
byte-code expanders).  A `ZZstate` row after every target reports
point-max/point/mark/current-buffer/local-leaks so buffer-state
drift attributes to the causing target instead of poisoning later
payloads.

Run: after `cargo build --release`, `python3 tools/difftest.py`
(`--only a,b` or `DIFF_ONLY`, `--side gnu|remacs`, `--timeout SEC`
(900), `--no-regen`, `--no-resume`, `--gnu`/`--remacs PATH`).  Unless
`--no-regen', `tests/diff/targets.el' is regenerated from GNU (every
non-special-form `subrp' symbol plus its `subr-arity').  Each side runs
`driver.el' with cwd `tests/diff/sandbox/' (recreated per attempt;
TZ=UTC, LC_ALL=C, empty EMACSLOADPATH) and `message's
`DIF|NAME|SEQ|ok/err/unbound|PAYLOAD' rows into
`tests/diff/{gnu,remacs}.err', with a `DMARK NAME SEQ' line before
each call.  A hang/crash autoskips the in-flight name (appended to
`tests/diff/autoskip-<side>.txt', reused next run unless
`--no-resume') and resumes after it via DIFF_FROM.
`tests/diff/report.txt' lists `DIVERG'/`KNOWN' names (remacs-only
autoskips count as DIVERG); exit 1 iff a non-known divergence remains.
`known.txt' lines are a whole NAME or `NAME|SUBSTRING' (matched
against `TAG|PAYLOAD' on both sides) — no `*' wildcards, so entries
like `registerv-*' are inert.  Never-called names (exit/hang/
destructive, `signal-process' included) live in driver.el's
`dif--skip'/`dif--skip-prefix'/`dif--skip-noarg'.

Fixed divergences so far (all regression-covered in qa_*.el):

- `set-buffer-modified-p`/`restore-buffer-modified-p`/`not-modified`
  restore GNU counter state: making clean text modified keeps the old
  MODIFF in SAVE_MODIFF and advances MODIFF without changing CHARS_MODIFF.
  These counters live in SharedText. The per-buffer auto-save counter
  makes `buffer-modified-p` return `autosaved` until the next modification;
  ordinary truthy flags retain an existing auto-save state. Portable
  images preserve that counter and accept older images without it.
- `kill-buffer`/bury/`replace-buffer-in-windows`/`quit-window` picked
  `*scratch*` (visible) instead of a non-displayed buffer: GNU
  `other_buffer` prefers not-shown buffers unless VISIBLE-OK — the
  `buffers.other` helper gained the two-pass rule
  (`other_visible_pref`), and `other-buffer` arg2/arg3 got the right
  order (VISIBLE-OK, FRAME).
- `select-window` recorded every selection: GNU records only when
  NORECORD is nil (`switch-to-buffer` passes norecord through).
- `wrong-number-of-arguments` datums: byte-code macro expanders
  (`.elc`-twin `Lambda`s) now emit GNU's `(MIN . NONREST)` cons —
  `macroexp_wna` translates at `macro_expand_call` sites and
  `call_lambda_inner` picks the argdesc pair for `is_bytecode`
  lambdas, the name for other `compiled` twins, the object for
  interpreted lambdas.
- `subr-arity`/`byte-code-function-p` accepted compiled/Subr values
  GNU rejects — subrs are not byte-code (`apply-partially` results
  now signal `wrong-type-argument` like GNU).
- `display-message-or-buffer` only accepted strings: a buffer arg is
  legal in GNU (returns t) and is the `shell-command` output path.
- `args-out-of-range` payloads were uniformly `(POS POS)`; GNU's
  shape varies per entry point — `get-pos-property` `(POS-1 POS-1)`,
  `get-char-property*`/`field-*`/`next-single-char-property-change`
  bare `(POS)`, `previous-single-char-property-change` `(POS-1)`,
  `get-text-property`/`text-properties-at`/`next-property-change`
  `(POS POS)`.  `want_pos_in_buf_dup`'s `dup` arg carries the shape.
- `cl-struct--pcase-macroexpander` (subr-x vendored) only consulted
  the `cl-struct--pcase-info` symbol property; `(cl-struct X ...)` in
  a `pcase` failed "X is not a known cl-struct type" for any struct
  registered class-only (xref structs inside `(require 'xref)` loads,
  `lisp-indent-state`, ...).  It now falls back to the
  `cl-structure-class` object via `cl--find-class`, deriving the
  tag-list predicate and slot offsets from the class record.  The
  manually-defined `lisp-indent-state` is also class-registered
  (`cl--struct-register-class`) like GNU's `cl-defstruct` expansion.
- `unrecord-window-buffer` was a no-op stub with the wrong signature
  `(window buffer &optional all-frames)`; replaced with the GNU
  window.el body (`&optional window buffer all` →
  `window-normalize-window` + `window-discard-buffer-from-window`).
- `japanese-katakana/hiragana/hankaku/zenkaku-region` vendored copies
  used `(goto-char from)` + a `(while (< (point) to))` scan instead
  of GNU's `(save-restriction (narrow-to-region from to))` +
  `re-search-forward` — floats silently passed, out-of-range region
  bounds gave the wrong error.  Replaced with verbatim GNU bodies.
- `register--get-method-type` vendored stub never consulted the
  generic function (GENFUN arg unchecked); deleted so register.el's
  real definition (per-type specializer walk via `cl-find-method`)
  survives — a non-symbol genfun now signals `symbolp` like GNU.
- `make-ppss` keyword-error format used `%s` for the bad key —
  GNU prints `%S`, so buffer/symbol keys render differently.
- vendored `subr-x.el` `--inliner` defuns quoted the *predicate*
  name (`''cl-slot-descriptor-p` etc.) as the
  `wrong-type-argument` datum; GNU emits the struct name
  (`''cl-slot-descriptor`).  Fixed for cl-structure-class /
  cl-slot-descriptor / cl--class / cl--generic{,-generalizer,-method}
  / oclosure--class accessors — observable via `setf`/`cl-callf`
  expansion (e.g. `cl--copy-slot-descriptor`'s
  `(cl-callf copy-alist ...)').

Open divergence classes under triage:

- `.elc`-twin objects print `#[...]` where GNU prints `#<subr>` and
  `(macro . #[...])` where GNU's `when` cell is `(macro . byte-code)`.
  `subrp`/`type-of`/`byte-code-function-p` on `when`'s cdr can't be
  t AND nil — one cell can't split into two kinds; wna datum was
  still normalized to the argdesc cons since that's the observable
  datum GNU callers see.
- `describe-register-1` sweep rows signal `symbolp (null byte-not)`
  inside remacs only — some `.elc`-consts path evaluates the
  `(null byte-not)` datum; unreproducible in isolation, parked.
- `describe-buffer-case-table`, `describe-key--helper`,
  `search-emacs-glossary`: *previously* listed as remacs hangs inside
  the full sweep — re-verified clean standalone at 60-120 s (2026-10);
  the apparent hangs were the stale `autoskip-remacs.txt` masking
  slow-but-finite targets.  The autoskip file was purged (it had
  collected ~840 entries from before the `cl--loop-expand`/`equal`
  fixes); the harness re-records genuine offenders.

## Managed memory statistics (`src/lisp/memory.rs`)

`memory-use-counts` records cumulative cons, float, vector-cell, symbol,
string-byte, interval, and string allocations. Route new managed objects
through `Cons::new`, `Value::float`, `Value::string[_ref]`,
`Value::vector[_ref]`, `Value::record`, `memory::allocate`, or `.tracked()`.
Aliases are deduplicated by allocation identity. Buffer text and internal
Rust work arrays are not Lisp vector allocations.
`Rc::get_mut` also tests weak observers: closure metadata updates must use
`memory::update_unique_lambda` to release the accounting observer for the
update while still respecting actual shared or weak references.

`garbage-collect` and `garbage-collect-heapsize` report the actual Rust
object sizes and live allocation counts, with GNU's nine row shapes.
The allocation registry's weak references observe object lifetimes without
keeping objects alive;
periodic reaping bounds stale accounting entries. Explicit collection also
prunes dead string metadata, updates `gcs-done`/`gc-elapsed`, and runs
`post-gc-hook`. Ordinary payloads are dropped by reference counting.
`FREE` describes reusable capacity, not cumulative drops: conses, floats,
and string objects have no free pool; symbol/interval/vector storage can
have spare capacity. Vector-slot and pseudovector rows overlap, as in GNU;
these statistics are not a process RSS measurement.

Allocation accounting observes the existing Rc allocator. Cycles not broken
by weak-entry or side-table removal, and objects retained by runtime tables,
remain allocated and therefore remain in `USED`. Interval ownership is tracked
once per shared
buffer text or string property tree, not again for its legacy flat view.
Regression coverage: `tests/qa_memory_statistics.el` and `memory::tests`.

Weak hash entries are swept by `src/lisp/memory/weak.rs`: an ownership graph
counts internal Rc references, marks external owners, and follows conditional
key/value edges to a fixed point. `key-and-value` requires both endpoints to
be live; `key-or-value` requires either. Shared lexical environments and text
property storage are visited once, while borrowed payloads are retained for
that collection. Scalar symbol IDs keep their existing obarray lifetime, and
obarray-owned values remain roots; this does not introduce general symbol or
cycle reclamation. Hash tables cache their GC policy independently of symbol
IDs, rebuilding it when reading literals or restoring portable images.
Regression coverage: `qa_weak_hash`, the borrowed-table memory unit test, and
the portable-image weak-table test.

## Profiler logs (`src/lisp/profiler.rs`)

CPU and memory profilers keep independent pending logs across stop/start.
Export returns an `equal` hash table, clears the pending log, and creates
a fresh one when the profiler is still running. A started profiler has a
log even before its first sample; nil means no pending run. CPU intervals
are positive fixnums in nanoseconds, with the running-state error checked
before interval validation. `lisp/profiler.el` remains GNU's original UI.

The sampler uses thread CPU time (`clock_gettime`, process CPU fallback)
at interpreter safe points rather than accessing Rc objects in SIGPROF.
Calls flush before stack pushes/pops; interpreted and byte-code loops poll
periodically. A long primitive is charged at its return boundary. Memory
weights come from actual allocation bytes tracked by `memory.rs`; paused
allocations and export's own allocations are excluded from that channel.
Backtraces put the innermost function first, truncate/pad to the configured
depth, and retain their actual callable objects. Logs honor the configured
entry limit, evict low-weight traces, and preserve evicted weight under
`[Discarded\ Samples nil]`. Export also accumulates equal rows without
losing weight. The inactive path only checks the running flags.

Regression coverage: `tests/qa_profiler.el` (lifecycle, samples, bounded
logs, all three report modes) and the profiler log weight-conservation test.

## Byte-code diagnostics (`src/lisp/bytecode.rs`)

`byte-code` validates a string, a constants vector, and a non-negative
fixnum stack depth before execution. This legacy entry converts multibyte
instruction strings to their storage bytes. `make-byte-code` requires an
unibyte instruction string; the `#[...]` reader normalizes old multibyte
streams and validates fixnum argument descriptors/depths. Invalid opcodes
report their numeric value and byte offset. Storage conversion uses GNU's
internal encoding, including surrogate and non-Unicode characters.

Out-of-range constant reads remain an unresolved part of #121. GNU 31.1's
release interpreter reads past the vector without checking. With the same
`\305\207` program and `[1 2]` constants, the reference build returned the
constants vector itself; allocating `(make-vector 32 12345)` after the
constants vector changed that result to `12345`. Such heap-layout-dependent
observations are not portable QA expected values. Remacs's existing nil
fallback is unchanged pending a decision on these malformed inputs.

Regression: `tests/qa_bytecode_errors.el` covers defined validation,
invalid-opcode offsets, valid constant loads, and legacy string conversion.

Byte-code caches validate the instruction string's contents as well as its
slot identity. Lisp can edit future instructions during a running call,
including through variable watchers or recursive calls. Destructive string
writers (`aset`, `fillarray`, `clear-string`, `store-substring`, and in-place
`subst-char-in-string`) must call `note_string_data_change`. Dispatch checks
that tick between instructions and decodes changed source into a local copy;
unchanged code keeps the cached bytes. The source Value stays on the active
call, avoiding an untraced persistent GC edge in the cache. Regression:
`qa_bytecode_mutation` covers shared code, operands, raw `byte-code`, watchers,
callbacks, recursion, unrelated string edits and GC.

`aset` rejects byte-code function objects with `wrong-type-argument arrayp`,
even for a negative or out-of-range fixnum index. Its index type check still
comes first (`fixnump`, including marker and bignum rejection). `aref` can
read function slots; returned instruction strings and constants vectors stay
mutable, but replacing the slots is forbidden. `qa_bytecode_slots` covers
four/six-slot constructors, reader literals, error data identity, unchanged
slots/results, nested content updates and ordinary mutable-array controls.

`string-to-list` delegates to `(append INPUT nil)`, including non-string
inputs. `append` copies the first list element before traversing its CDR;
keep that GNU detector start so `circular-list` carries the same tail.
It also copies interpreted/compiled closure slots, like GNU's CLOSUREP
branch, while other sequences share the raw-byte-aware conversion helper.
The list spine is fresh but elements stay shared. `qa_string_to_list` covers
aliasing, dotted/cyclic errors, closure slots, byte kinds, boolean vectors,
and direct multi-argument append/tail behavior.

`string-to-vector` delegates to `vconcat`, including lists, vectors,
boolean vectors and closure slots. `vconcat` keeps its own list traversal
(GNU checks the full list's length, unlike append's initial CAR copy),
and returns the shared `zero_vector` when empty. Nonempty results have a
fresh vector spine with shared elements. `qa_string_to_vector` pins these
rules, all 256 raw bytes, proxy characters, circular error tails, type/error
data identity and direct vconcat behavior.

`string-lessp` compares unibyte bytes as 0..255 and multibyte characters by
their original Lisp codes, including byte8, surrogate and Mule proxies.
Use the actual Lisp-visible symbol-name string so its byte kind and mutable
contents survive normalization; the obarray's internment key is insufficient.
The NS `string-collate-lessp` fallback shares that ordering and ignores its
locale/ignore-case arguments, as the GNU reference does. Equality and version
comparison have different rules. `qa_string_order` covers raw bytes, proxy
codes, live symbol names, prefixes, errors, aliases and byte-code opcode 153.

Case folding uses distinct installed case maps: `char-equal` reads DOWNCASE,
`compare-strings` upcases differing characters through UPCASE, and literal/
regexp searches use CANON and EQUIV. Numeric upcasing follows GNU's unibyte
buffer promotion/flattening rules, without full string expansions. It repairs
invalidated extras only when upcasing is needed; equal characters skip that
work. Search itself does not eagerly rebuild nil canonical extras.
The regexp `Syntax` context supplies equality and class-equivalence callbacks
for literals, backreferences and classes; plain engine clients retain their
Unicode fallback. `RegexSyntax` supports surrogate/Mule mappings and walks
equivalence cycles without per-candidate allocation. `qa_case_table_fold`
covers custom/buffer-local maps, table reinstallations, bounds, classes,
backreferences, POSIX searches, unibyte buffers and distinct non-Unicode codes.
The same suite separately pins #260's Mule, surrogate and byte8 pairs under
both fold modes, equal-code controls, valid custom non-Unicode case pairs,
and the unibyte-buffer raw-byte alias rule. Unsupported Rust `char` values
must retain their Lisp integer identity rather than comparing two `None`s.

Functional `read` sources share the normal parser through lazy `Reader`
input: zero-argument calls supply characters, one-argument calls return
lookahead, and any non-character return means EOF. Do not collect the source
to EOF or repeatedly parse prefixes. Input/peek errors propagate immediately;
callback EOF data is preserved. Token and escape boundaries return lookahead
before symbol interning, including invalid character-literal delimiters.
`#@00` explicitly drains its source as GNU does. Reader whitespace is ASCII
controls through space plus NBSP, not Unicode White_Space; preserve original
Lisp codes in direct and hex-escaped Mule characters. `qa_reader_function_stream`
pins one-form consumption, callbacks, errors, GC/reentry, labels and byte kinds.
`qa_reader_error_context` separately pins #262's error-position contract:
buffer and marker cursors advance before parse errors propagate, marker reads
leave the buffer's own point unchanged, and EOF data identifies the proper
input kind. This behavior was already provided by the #93 reader fix.

`prin1` and `prin1-to-string` apply GNU's OVERRIDES mapping with temporary
specbindings. Keep all 17 entries, including `unreadeable-function`'s historic
alias and duplicate default binding. Settings apply left to right; a `t`
element resets all mapping defaults. Unknown keys signal `(symbolp nil)`,
and malformed tails report the remaining cons. Validate before destination
lookup, while capturing nil destinations' `standard-output` value first.
Bindings survive formatting/output callbacks and unwind on errors. Unreadable
callbacks temporarily bind their printer to nil; top-level print resets the
number table without watcher notification when numbering is not continuous.
Watcher binding notifications precede stores; global values are saved before
`let` watchers, and forwarded global restoration emits `set` then `unlet`.
`qa_print_overrides` covers defaults, ordering, settings, sinks, errors,
restoration, callbacks and watcher state.

`qa_eval_narrowing` also pins #265 PRINTFLAG handling for both eval entry
points: nil discards source output and results; buffer, marker and function
streams receive source output plus printed evaluation results. The caller
stream is restored, and non-nil PRINTFLAG records results in `values`.
This behavior was already provided by the #264 live-buffer evaluation fix.

`eval-buffer` selects lexical mode from the first accessible comment line
(after an optional shebang), unless the source buffer already has a local
`lexical-binding` slot. A missing cookie calls
`internal--get-default-lexical-binding-function` with the source buffer, or
uses `default-toplevel-value` when the hook is nil. Only the exact cookie
value `nil` disables lexical mode. Bind output/eval-buffer-list before the
hook, reset source point afterward, and restore bindings, lexical environment
and caller point on errors. `eval-region` keeps the caller mode and ignores
header cookies. `qa_eval_lexical` pins these #266 rules.

`qa_eval_narrowing` also pins #267 READ-FUNCTION: pass the source buffer,
read each form through the callback, prefer it over `load-read-function`,
skip comment-only input, evaluate substituted forms, and unwind reader errors.
The #264 shared evaluation loop already provided this behavior.

The same suite pins #268 nil returns from both evaluation entry points for
nil-valued, numeric, sequential and quoted-list source forms. Returned
values stay independent from PRINTFLAG output; #264 already fixed the loop.

Backward buffer search must continue counting after a nonempty match at
BEGV. Only a zero-width match there can satisfy remaining repetitions at
the same position. `qa_search` pins #272 across literal, regexp and POSIX
search, negative forward counts, NOERROR modes, narrowing and zero-width
controls. An incomplete count keeps point for nil/t NOERROR and moves to
the bound for other non-nil NOERROR values.

Replacement back-references use `MatchData.base`, the search-time buffer
origin, rather than the current BEGV. Widening or re-narrowing after a search
must not retarget captures. `qa_replace` pins #273 with ASCII/multibyte
text and subgroup replacement.

`replace-match` string results retain source/replacement properties and
unibyte/multibyte representation through real Lisp substring/concat values.
Back-references copy source intervals; escaped slashes keep the first
slash interval. Buffer replacements retain replacement properties only
when literal or processing no backslash, matching GNU's `really_changed`
branch. Install intervals inside the single replacement change, before
after-change hooks. Apply buffer case transfer afterward with region
casing; string case transfer uses the ordinary string casing rules.
`qa_replace_metadata` pins #274 properties, representation, hooks and undo.

`match-data` destructively fills REUSE cons cells, pads unused cars with
nil, and extends short lists. Non-cons reuse is ignored; a sufficient
dotted list keeps its tail. RESEAT first detaches all old markers and
replaces those cars with nil, even with no match data. New buffer-match
markers are freshly allocated rather than reusing old marker objects.
`qa_match` pins these #275 identity, padding, extension and marker rules.

Regexp syntax escapes and word/symbol boundaries use position-aware
`Syntax::code_at`. Source-specific snapshots apply `syntax-table` text
properties, including table-valued entries and category-derived values,
when `parse-sexp-lookup-properties` is non-nil. Buffer offsets are relative
to the searched source origin; string searches use the string's own
intervals. POSIX classes keep their static syntax classification. Ordinary
`re_syntax` remains property-free for plain-text engine clients.
`qa_regex_properties` pins these #276 source and boundary distinctions.

Window configurations save point/start/old-point with real markers, so
subsequent edits move saved positions. Window point markers honor the
buffer's `window-point-insertion-type`; start markers stay before inserted
text. Restore point, start, hscroll/min-hscroll/suspension, old-point and
margins for each live saved buffer. Preserve the saved current buffer's
present point, choosing the final selected window's own pre-restore point
when selection changed between windows showing that buffer. Before
restoring, unshow outgoing windows under GNU's point-ownership rules;
restored nonselected window points leave those buffer points alone.
`qa_window_configuration` pins these #277 view restoration rules.

Configuration record slot 5 snapshots the window tree independently of
mutable live child lists. Restore original horizontal/vertical internal
windows, parent links and combination limits; revive saved nodes and
mark unsaved internal nodes dead. Derive restored internal rectangles
from their restored children. Validate topology and leaf membership
before changing live state. The same QA suite pins #278 nesting, root
identity, repeated switching, node liveness and `window-tree` equality.

Configuration comparison uses captured frame dimensions/menu rows, frame
and current-buffer identity, selected-window correspondence, full leaf
geometry, margins and tree shape. Point/start/hscroll and window identity
are ignored, as are dedication and combination-limit flags. Record slot
6 captures frame dimensions instead of reading the subsequently mutable
frame object. The pure helper implements the explicit layout
comparison. The same QA suite pins these #279 distinctions.

Genuine window configurations are registered by Rc identity in a
thread-local weak registry. Type checks and decoders require this factory
identity, so an ordinary record with the same tag remains an ordinary
record. Genuine configurations print opaquely, reject record/array access
and sequence copying, and compare by object identity for `equal`, property-aware
equality and `cl-equalp`, including nested occurrences. Weak registration
does not retain snapshots or confuse reused allocation addresses. The
same QA suite pins these #280 factory/forgery and opaque-type rules.

`primitive-undo` binds `inhibit-read-only` to t for the complete undo
operation, including change hooks and apply callbacks. Restore the caller
binding on success, errors and throws; do not suppress modification hooks.
`qa_undo` pins these #281 protection and binding-lifetime rules.

The undo dispatcher reports successful apply entries separately from
position/text/property/modtime entries. Add the compensating `(apply cdr
nil)` history only when an apply entry ran and history identity did not
change. Point-only and undo-disabled ordinary edits do not create it.
`qa_undo` pins these #282 history and explicit-apply distinctions.

`run-window-scroll-functions` switches to the window's buffer for hook
lookup/calls and restores a live caller buffer on normal return, errors
and throws. Pass the original WINDOW argument (nil stays nil) and the
1-based saved window start. Splitting first notifies through the internal
buffer-installation primitive, then invokes the public runner once, as GNU's
public Lisp splitter does. Release frame borrows before either callback.
Local hook t sentinels expand default hooks in place.
`qa_window_scroll_hooks` pins these #283 context and notification rules.

## Live backtraces (`src/lisp/backtrace.rs`)

`lisp_stack` holds evaluated calls for the profiler and evaluator. Source
expressions add scoped `pending_frames` at the current call-stack depth;
`backtrace_snapshot` merges both in dynamic order, preserving raw argument
forms with `evald=nil` until the call enters its function. Macro invocation
frames remain around expansion, as in GNU `eval_sub`. Compiled-equivalent
bodies suppress source control-flow frames (`current_body_compiled`);
explicit `eval`/source loads/raw lambdas reset that provenance.

`mapbacktrace`, `backtrace-frame--internal`, and signal-time snapshots share
this representation. Offer fresh signals before their live call/expression
scope unwinds so handlers can inspect the signaling frame. Host CLI actions
record their actual `normal-top-level`/`command-line`/`command-line-1` scopes;
source loads record `load-with-code-conversion`, `--eval` records each `eval`,
and `-f` directly invokes its function. `--script` runs the real GNU
`command-line--load-script` callback path. Batch error output prints the
captured frames, without appending a second fabricated CLI tail. Frame counts
depend on evaluation context and native/interpreted implementation details;
never satisfy a count mismatch by padding to a constant.

Regression suites: `tests/qa_backtrace_frames.el`, `tests/backtrace_cli.rs`.

CLI `--eval`/`--execute` use `eval_first_form`: read and evaluate one object,
ignoring the remaining characters (including another form or invalid trailing
syntax), as verified against the reference GNU 31.1 executable. Empty,
comment-only, or malformed first input signals its read error and aborts later
options. Preserve lexical evaluation and the real `eval` backtrace frame;
`eval_forms` remains the separate all-forms API. Regressions: the
`eval_first_*` cases in `tests/main_cli.rs`.

CLI file arguments are visited inline, including in batch mode, so later
actions and exit hooks see their buffers. Resolve every relative name
against the captured `command-line-default-directory`, not the previously
visited buffer's directory. Pass filename values to the Lisp functions
directly instead of interpolating them into source. Regressions:
`batch_file_arguments_*` in `tests/main_cli.rs` (normal/source startup).

The headless CLI consumes the live Lisp `command-line-args-left` list one
string at a time. Lisp actions can consume a prefix, remove middle entries,
replace/prepend arguments, or mutate operand strings; do not reconstruct
the queue from the original argv or require a retained suffix. Validate
each element/tail only when reached, preserving earlier output before
`stringp`/`listp` errors. Shared option arity still distinguishes operands
from the standalone `--` separator. Regressions:
`tests/cli_argument_queue.rs`, `tests/cli_separator.rs`, nativecomp CLI tests.

CLI `-L`/`--directory` (also `--directory=DIR`) update `load-path` after
bootstrap, in argument order. `load::CommandLineLoadPath` implements GNU's
cons splice cursor, including the head reset after a colon-prefixed append;
retain duplicates and resolve relative directories against the current
buffer. Call the real filename normalizer/expander, preserving empty-input
errors. GUI startup shares the same helper and interleaves directory options
with file visits. Regressions: `tests/cli_load_path.rs` and the headless GUI
logic-thread directory test.

`cli::normalize_option` provides canonical names and inline values for
supported GNU spellings (single-hyphen long aliases and double-hyphen
`=VALUE`). Keep raw argv tokens and indices intact; normalize only when
scanning or executing options, never operands or filenames after `--`.
The startup scans, live Lisp queue, GUI startup, and dump parsing share
this view. Empty inline values are present operands. Regressions:
`tests/cli_option_forms.rs` and parser unit cases.

Startup `--help`/`-help` prints Remacs usage without Lisp boot when no image
is selected, and without GUI creation. Selected portable images retain
their existing validation/restore ordering before help; `--version` retains
GNU's precedence over help. Keep help
tokens literal when they are operands or follow `--`. GNU does not accept
`-h` as help, and help inserted into the live Lisp queue after startup is
still an unknown runtime option. Regressions: `tests/cli_help.rs`.

## Portable images (`src/lisp/dump/`)

`dump-emacs-portable` writes a real Remacs image; `--dump-file FILE`
or `--dump-file=FILE` restores it before command-line actions. Without an
explicit option, the front ends load an existing executable-side `.pdmp`
twin. The format is Remacs-specific, with a version, executable SHA-256
fingerprint, compressed JSON object graph, payload length/checksum, and an
incomplete-file marker committed only after writing succeeds. GNU's binary
`.pdmp` files are not interchangeable with these images.

The flat graph stores object ids rather than process pointers. It preserves
cycles, cons/string/vector/record/float identities, shared lexical frames,
closure/byte-code metadata, symbol cells and obarray membership, hash-table
keys, string representations and properties. Static primitives use a
fingerprinted ordinal/name registry, including private macro subrs. Hashes
are rebuilt after referents and char-table side tables have been restored.
Core symbol ids remain fixed; ordinary, uninterned, and custom-obarray
symbols are rebased separately. String data stores Emacs characters rather
than thread-local proxies for non-Unicode characters.

Session data includes shared buffer text, gap state, ticks, narrowing,
locals, markers, indirect buffers, detached overlays, frame/window trees,
history, parameters, and persistent Rust-side metadata (faces, charset and
coding definitions, advice, watchers, char-table relations, etc.). Execution
stacks and derived caches are reset. Process-specific startup variables,
environment, command line and standard I/O come from the new process;
embedded source-cache paths are relocated. `after-pdump-load-hook` runs
after restoration and can inspect the new load statistics.

An undumped session's `pdumper-stats` is nil, as GNU specifies. Only a
successful image load sets the real absolute filename and measured restore
time; saving an image does not change this session's provenance. The initial
loader boots a fresh interpreter before restoring the graph, so its time
includes that reconstruction rather than claiming a memory-mapping speedup.
Rc cycle retention follows the allocator's existing limitations.

Dumping rejects attached overlays and user-defined hash tests like GNU.
Active processes, non-main thread objects, tree-sitter/SQLite handles,
file-notify/D-Bus resources and dynamic native compilation units cannot be
ported across processes by this backend and are rejected. File locks and
other OS-owned state are not transferred. Add newly introduced persistent
fields to the runtime/session codec or explicitly reset/reject them;
silently omitting state is not an acceptable image format change.

Regression coverage: dump graph/session unit tests, `tests/portable_dump.rs`
(fresh copied executables, separate caches, corruption and startup metadata),
and `tests/qa_pdumper.el` (GNU-verified errors and dump output).

## Gap allocation (`src/buffer/gapbuf.rs`)

The backing vector holds decoded characters but budgets Emacs encoded-byte
capacity. `text_bytes` counts live storage (including two-byte eight-bit
characters and five-byte extended Mule codes), so `gap-size` is byte
headroom rather than the number of unused char slots. On exhaustion, add
`max(missing_bytes, existing_characters / 64) + 2000`, then consume the
inserted bytes. Deletion returns its encoded bytes to the gap.

Read-only scan/syntax access uses a cached contiguous projection for a
middle gap, and direct slices for end gaps; it must not move `gap-position`.
Mutations invalidate that projection. Whole-text replacement reuses the
allocation through delete/insert. Multibyte mode is shared with the text;
portable images retain it along with physical gap geometry and ticks.
This accounts for the existing decoded buffer representation; the separate
known buffer encoding-conversion gaps are not hidden by the allocator.

Regression coverage: `tests/qa_gap.el`, direct gap-buffer mutation/projection
tests, and portable image round trips.

## Verification recipe

```sh
# byte-compare against GNU 31.1 source
GNU=/nix/store/7l70cwk87fk30viyjpj9iwha6a2wcs6v-emacs-unstable-31.1/share/emacs/31.1/lisp
gzip -dc $GNU/<path>.el.gz | cmp - lisp/<file>.el

# parse-check all files without evaluating (see git history for script)
./target/debug/remacs --batch --eval '(load "/tmp/parsecheck6.el")'
```

`subst-char-in-region` validates equal internal byte lengths in multibyte
buffers before scanning, including empty and no-match regions. Unibyte
buffers truncate both characters to eight bits. It restarts the scan after
before-change hooks, changes characters without moving point or markers,
and records each replacement with `record_change`, preserving properties.
`prepare_inplace_change` adds the first-change record and composite tick
once. NOUNDO temporarily disables undo and hides the visited file name;
its SAVE_MODIFF adjustment follows GNU even when a multi-character range
still becomes modified. `tests/qa_substitute_chars.el` pins these details.

Variable watchers (`data.rs`, `Interp::var_watchers`) register on the
ultimate variable alias target. New watchers run first; add/remove use
`equal` for GNU's `member`/`delete` semantics. `defvaralias` notifies the
old cell's watchers before installing the new alias, with BASE as NEWVAL
and `defvaralias` as OPERATION; an error aborts aliasing. Per-buffer `setq`
and `set` both notify for `fill-column` in GNU 31.1, despite the original
issue report's claim. `tests/qa_variable_watchers.el` pins these behaviors.

Bool-vector binary operations validate a non-nil destination's type and
length; `wrong-length-argument` carries the two source lengths plus the
destination length when supplied. A supplied destination is returned only
when changed, otherwise nil (including an empty vector). `bool-vector-not`
always returns its destination. `bool-vector-count-consecutive` accepts
only nonnegative fixnum indices through the vector length, inclusive.
`tests/qa_array.el` covers validation, aliases, and unchanged destinations.

Interactive dispatch shares `Interp::call_interactively`: `command_execute`
handles disabled commands and keyboard macros, then delegates; direct
`call-interactively` validates KEYS first and bypasses disabled handling.
`interactive-form` validates alias cycles, consults symbol overrides along
the alias chain, and resolves autoloads before inspecting the real spec.
`commandp` stays passive. Autoload TYPE/FILE reads use bounded `nth`, so
malformed prefixes retain GNU's listp error data without rejecting unused
dotted tails. `tests/qa_autoload_dispatch.el` covers loading and ordering.

`previous-single-char-property-change` clamps POSITION to LIMIT (floored at
point-min) before reading any character. Otherwise it first reads POSITION
minus one (negative positions stay unchanged), then validates POSITION for
the boundary search. This ordering gives a bare read offset on an invalid
initial read, but `(POSITION POSITION)` when the initial read succeeds and
the boundary search is out of range. An empty buffer at 500 still reports
499; a five-character buffer at 7 reports `(7 7)`. `qa_textprops.el` pins
these distinctions, narrowing, overlays, and next-property-change's LIMIT.

Event conversion (`misc.rs`, `editor/mod.rs`) follows GNU's separate
representations: `listify-key-sequence` preserves vector/multibyte-string
elements and applies the `listify-key-sequence-1` XOR only to values above
127 in other sequences. `event-convert-list` retains its original base,
recognizes solitary modifier aliases only before another cons, and uses
GNU's character/control folding and symbol modifier cache. Click does not
appear in symbol names. `single-key-description` masks numeric events to
28 bits and makes multibyte descriptions (including raw-byte characters),
handles nil as a symbol, copies string inputs, and shares symbol-name with
NO-ANGLES. The key parser consumes a modifier prefix only when another
character follows it, so a lone `C-` remains literal. Regression coverage:
`tests/qa_event_conversion.el`; text-char-description 128 already matched.

`event-convert-list` also preserves `FOR_EACH_TAIL_SAFE` cycle behavior:
Brent's walk stops without signalling, while a second base can fail before
the cycle is detected. Its u16 step counter mirrors GNU's unsigned short
counter; using a visited set changes these observable results.

Arbitrary-precision integration (`qa_arbitrary_precision.el`) also covers
bytecode/native arithmetic and immutable integer sharing. Portable images
encode bignums as heap nodes rather than inline decimal references, so
aliases and eq-hash keys survive restoration. Bounded C/Rust interfaces
use checked integer projections; integer predicates must include `BigInt`
rather than matching only `Value::Int`. JSON uses full decimal integers,
SQLite binds checked signed 64-bit values, and explicitly typed D-Bus
integers validate their native ranges. Time arithmetic/conversion retains
BigInt tick/frequency pairs and GNU's resolution normalization; conversion
to floating time uses exact rational values. Internal position helpers may
still use i128 after the relevant API validates the requested bounds.

`make-string` follows GNU alloc.c: only an ASCII INIT with nil MULTIBYTE
produces a unibyte string; non-ASCII INIT or a non-nil MULTIBYTE argument
produces a multibyte string, including length zero. Explicitly record the
kind even for ASCII/empty results. INIT uses the same logical-character
conversion as `char-to-string`, preserving surrogate, byte8, and Mule
characters. Validate LENGTH then INIT before allocation checks. The ASCII
zero default remains unibyte for byte-oriented code such as md4.el.
`tests/qa_strings.el` covers these cases.

String `fillarray` uses the effective stored byte kind, including explicit
multibyte ASCII and raw-byte unibyte strings. Unibyte fills truncate ITEM to
one byte; multibyte fills require character count times ITEM's Emacs byte
width to equal the original total byte count. Original characters need not
share that width. Count logical surrogate/Mule/eight-bit codes, not the UTF-8
width of Rust proxy characters. Validate ITEM even for empty strings, retain
properties/identity/kind, and leave failed fills unchanged. Regression:
`qa_fillarray_strings` covers the representation and width matrix.

`clear-string` shares `string-bytes`' logical Emacs storage-byte counter,
clears the whole property tree, and replaces nonempty data with that many
unibyte NUL characters. Rust UTF-8 proxy storage is not the Lisp byte count.
The GNU 31.1 oracle preserves the byte kind of empty strings, including
explicit multibyte empties. Keep the original object and aliases, with
independent copies unchanged. Regression: `qa_clear_string` covers raw-byte,
surrogate/Mule, mixed-width, properties, repeated calls, GC and empty cases.

String `nreverse` delegates to `reverse`, as GNU fns.c does: leave the input
unchanged and omit output properties. Explicitly record the output byte kind.
GNU chooses unibyte storage when character count equals byte count, including
multibyte ASCII/empty input; other multibyte text keeps its encoded characters.
An empty unibyte input is returned unchanged. `qa_reverse_strings` pins raw
bytes 0..255, aliases, properties, GC, repeated reversals and proxy characters.
The same suite separately pins concat-origin property removal for both
functions, preserving source intervals and property values when output
properties are subsequently added (`nreverse`'s former separate path differed).

TAB display widths share GNU's `sanitize_tab_width` rule: read the current
buffer's `tab-width`, accept integers in 1..1000, and use 8 for other values.
`string-width` adds that width for every TAB without tracking tab stops;
`char-width`, format padding and truncation must use the same rule.
Regression: `qa_string_width_tabs` covers valid/invalid settings, substring
bounds, buffer-local and dynamic bindings, padding and ellipsis. Non-TAB
widths are explicitly set where startup character-width tables differ.

`string-glyph-split` groups explicit non-nil `composition` property ranges,
joining adjacent EQ values across unrelated property changes (including
category-derived values). Static string ranges are found by
`find-composition-internal`, so GNU's autoloaded Lisp wrapper receives them;
the wrapper uses the range even when the composition is invalid. Both it
and the Rust fallback use substring's byte-kind/property copying. Regression:
`qa_string_glyph_split` covers range boundaries,
identity, properties, raw bytes and surrogate/Mule characters. Its temporary
buffer is longer than each input because GNU clips the string search LIMIT
to the current buffer's ZV; short buffers can make GNU's wrapper loop backward.
