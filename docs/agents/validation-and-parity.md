# Validation and parity notes

## Portable time formatting

`crates/runtime/src/lisp/builtins/timefmt.rs` implements GNU flag ordering, field widths,
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

## Cooperative Lisp threads (`crates/runtime/src/lisp/threads.rs`)

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

## Coverage probes (`src/lib.rs` + `tests/probe_all.rs`)

`tests/support/probe.rs` runs each probe in an independent copy of the same
instrumented test binary, with a 64 MiB evaluator stack, private working
directory and EOF stdin. The parent serializes probes that compete for
ptys/ports and enforces a 120-second child deadline; Unix timeouts kill the
owned process group. Logs use files so inherited pipe writers cannot stall
collection, and a completion handshake prevents an empty test selection
from passing. Stderr is restored after probes that redirect debugging output.

Evaluate each file once. Do not replay failed files in their already-mutated
interpreter to locate errors. The side-effect fixture pins this, and watchdog
tests verify timeout cleanup, lock release, subsequent execution and failure
status. `probe_eval14.el` renders primitive bindings descriptions in fresh
output buffers while retaining explicit source-buffer arguments: earlier
`edit-abbrevs` changes the current buffer and enables `jit-lock-after-change`,
making repeated bulk output state-dependent. The
buffer, editor and runtime fixtures likewise restore the seeded current
buffer across bury/unbury, handle the expected EOF from an input-free
yank selector, and restore `car` after testing its void function cell.
The Custom fixture uses an owned temporary `custom-file`. These preserve
the probe calls and negative branches without leaking state into later
calls or depending on developer init files. Probes explicitly require
`cl-lib` for `cl-assert` and `subr-x` for threading macros instead of relying
on a preceding probe to have loaded them (#298).

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
  does not exist in GNU 31.1 either (stale ledger note).  JSON parser
  EOF and malformed UTF-8 error positions match GNU's consumed-byte
  offsets (`qa_json_utf8`, `qa_json_narrow`, `qa_json_number_range`).
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

Total: ~5000 GNU-verified checks. The full ledger is available with
`python3 tools/guarded_exec.py --preset full-test -- cargo test --test qa -- --test-threads=1`
when a full parity sweep is requested or warranted.


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

Run: after `python3 tools/guarded_exec.py --preset build -- cargo build --release`,
`python3 tools/guarded_exec.py --preset full-test -- python3 tools/difftest.py`
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

The 2026-11 release sweep completed 22,916 rows on each side: 351 known
divergence names, zero new divergences. It supplied the current GNU/Remacs
autoskip names through `DIFF_AUTOSKIP` with `--no-resume`, leaving the tracked
autoskip files unchanged.
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

- `.elc` byte-code functions were misclassified as native subrs when
  `Lambda.compiled` was set for compiled-equivalent callsites. Type and
  subr predicates now also inspect the byte-code slots, preserving GNU's
  `byte-code-function` / `subrp=nil` / `byte-code-function-p=t` result.
  Regressions: `qa_bytecode_subr_unintern_identity`, `qa_types2`,
  `qa_subrinfo`, and the native-function checks in `qa_native_comp_load`.
- `describe-register-1` had a prior full-sweep-only `symbolp (null byte-not)`
  report. A fresh standalone differential run (2026-11) matched GNU; keep
  tracking it only if a future full sweep reproduces it.
- `describe-buffer-case-table`, `describe-key--helper`,
  `search-emacs-glossary`: prior timeouts were stale `autoskip-remacs.txt`
  entries masking slow-but-finite targets. They were re-verified standalone
  at 60-120 s (2026-10); the autoskip file was purged, and the harness
  re-records genuine offenders.

