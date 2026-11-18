# Runtime internals notes

## Managed memory statistics (`crates/runtime/src/lisp/memory.rs`)

`memory-use-counts` records cumulative cons, float, vector-cell, symbol,
string-byte, interval, and string allocations. Route new managed objects
through `Cons::new`, `Value::float`, `Value::string[_ref]`,
`Value::vector[_ref]`, `Value::record`, `memory::allocate`, or `.tracked()`.
Aliases are deduplicated by allocation identity. Buffer text and internal
Rust work arrays are not Lisp vector allocations.
`Rc::get_mut` also tests weak observers: closure metadata updates must use
`memory::update_unique_lambda` to release the accounting observer for the
update while still respecting actual shared or weak references.

The cumulative string-byte counter uses the Lisp string's allocation byte
kind: unibyte strings count one byte per character, while multibyte strings
sum each `EmacsChar`'s internal storage width. Do not substitute Unicode
character count or Rust UTF-8/`Vec` capacity. Live heap-size rows still report
the current Rust representation. Regression coverage includes Japanese text
and byte8 strings in both byte kinds.

`garbage-collect` and `garbage-collect-heapsize` report the actual Rust
object sizes and live allocation counts, with GNU's nine row shapes.
The allocation registry's weak references observe object lifetimes without
keeping objects alive;
periodic reaping bounds stale accounting entries. Explicit collection also
prunes dead string metadata, updates `gcs-done`/`gc-elapsed`, and runs
`post-gc-hook`. Ordinary payloads are dropped by reference counting.
Cons children are drained through a per-thread worklist when a cons reaches
zero references, so dropping long or deeply nested cons graphs does not recurse
on the Rust stack. Cycles still follow the Rc allocator's documented retention
behavior. `value::tests::dropping_deep_cons_nesting_does_not_recurse_on_the_rust_stack`
and `qa_assoc_default_long_cycle` cover the large-spine case.
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

Weak hash entries are swept by `crates/runtime/src/lisp/memory/weak.rs`: an ownership graph
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

## Profiler logs (`crates/runtime/src/lisp/profiler.rs`)

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

## Byte-code diagnostics (`crates/runtime/src/lisp/bytecode.rs`)

`byte-code` validates a string, a constants vector, and a non-negative
fixnum stack depth before execution. This legacy entry converts multibyte
instruction strings to their storage bytes. `make-byte-code` requires an
unibyte instruction string; the `#[...]` reader normalizes old multibyte
streams and validates fixnum argument descriptors/depths. Invalid opcodes
report their numeric value and byte offset. Storage conversion uses GNU's
internal encoding, including surrogate and non-Unicode characters.

Out-of-range constant reads are rejected with `error` / `"Invalid byte-code"`
for immediate constants, `constant2`, and variable-constant operands. GNU
31.1's release interpreter reads past the vector without checking. With the
same `\305\207` program and `[1 2]` constants, the reference build returned the
constants vector itself; allocating `(make-vector 32 12345)` after the
constants vector changed that result to `12345`. Those heap-layout-dependent
values are undefined and not a compatibility target. A Remacs-only regression
in `tests/qa_bytecode_errors.el` pins the bounds checks without executing the
undefined program in GNU.

Regression: `tests/qa_bytecode_errors.el` covers defined validation,
invalid-opcode offsets, valid constant loads, legacy string conversion, and
Remacs-only out-of-range constant rejection.

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
`read` and `read-positioning-symbols` share stream dispatch, including
callable sources. The exact t/read-char stream identities call the public
`read-minibuffer` with "Lisp expression: "; this path returns ordinary
objects even for positioned reads. Nil resolves `standard-input` once,
so a nil-valued standard input is called as a function and signals
`void-function`. Regressions: `qa_reader_source_dispatch` and the physical
stdin cases in `tests/batch_input.rs`.

Reader source kinds also apply to strings, buffers and markers: preserve
the source's multibyte flag when constructing new symbol-name strings,
including uninterned symbols and ASCII names. Existing interned names keep
their original objects and kinds. Literal strings choose their own kind:
byte8 characters alone produce unibyte storage, while a real non-ASCII
character makes the whole literal multibyte. The shared character-escape
parser supplies Unicode/surrogate, Shift and Meta characters; string-specific
handling removes escaped spaces/newlines, folds ASCII Shift, translates
Meta to byte8 and rejects remaining modifiers. ASCII Unicode/long hex
escapes do not force multibyte storage. `string_to_object` retains the input
or default string's kind before parsing. Regressions:
`qa_reader_string_kind` and the raw object-read cases in `tests/batch_input.rs`.
The `#_` and `#:` token paths use that same symbol-name construction and
annotate nonempty names starting after the two-character prefix. Empty
names and the interned nil remain bare. `bare-symbol-p` accepts nil, and
`bare-symbol` returns nil both directly and when unwrapping a positioned
nil. Regression: `qa_reader_symbol_source`.

Public interning APIs and reader symbol tokens honor the dynamic `obarray`
value. Validate it before string checks or shorthand lookup; numeric reader
tokens and uninterned `#:` names bypass that validation. `intern-soft` with
a symbol requires that exact member, including positioned symbols; a
same-name uninterned twin is not a hit. Removing a symbol updates membership
and enumeration without discarding its cells. Registration keys contain
Emacs storage bytes and character counts, so an unibyte byte and a multibyte
byte8 character can name distinct symbols. Preserve the supplied name object
on first registration. Portable images store those keys and allocate separate
nodes for distinct symbols sharing an original key after name mutation.
The native interner's ASCII index is derived from that membership, not a
separate namespace. Synchronize it on registration/removal/boot cleanup and
rebuild it after image restoration; evaluator polls must not allocate byte
keys for their usual ASCII variable names.

Native literal symbol references use `Interp::q`/`q_soft`, backed by the
bootstrap symbol objects, so uninterning or clearing a namespace does not
retarget C-style Q references. Public interning and reader lookup stay in the
mutable obarray. Fixed references are GC roots and serialized as graph symbol
references, including uninterned originals. The current static-call migration
has compile, focused QA, image and isolated GNU clear-table verification;
the full regression suite still needs to run after that migration.

`gensym` follows dumped subr.el's native order: read the counter, inline 1+,
set it through ordinary variable handling, then call format and make-symbol
through the native trampoline protocol. Formatting retains byte kinds,
properties and delegated return values. Its mutable format/default-prefix
literals live in `native_lisp_literals`, which is rooted and included in
portable images. `format` keeps styled_format's direct-object returns for
unchanged literals and unpropertized two-character string conversions;
otherwise string arguments determine multibyte output, including unused
arguments. Regressions: `qa_gensym_semantics`, `qa_format_string_kind`, the
gensym counter-unintern CLI test and literal identity checks in portable_dump.

`read-symbol-shorthands` matches byte prefixes using the first valid string
pair, ignoring malformed entries and dotted/circular tails as GNU's safe
walk does. Reader punctuation-only names (`^*+-/<=>_|`) and nonempty `#_`/`#:`
tokens bypass expansion. Public interning APIs still expand punctuation and
empty names. A new transformed `intern` name is multibyte; reader-created
names retain the source kind. Source loads call the real
`hack-read-symbol-shorthands-function` in the temporary load buffer, with
the caller's input/output and eval-buffer-list, then bind its result for
evaluation. `.elc` filename suffixes inhibit inherited shorthands, including
buffer evaluation; a compiled header on an `.el` name does not. Regressions:
`qa_obarray_defaults`, `qa_symbol_shorthands`, `qa_load_shorthands`, and the
name-kind/name-mutation round trips in `tests/portable_dump.rs`.
For a transformed unibyte reader name, GNU's make_specified_string uses the
calculated character count as the byte length. A multibyte replacement prefix
can therefore truncate its raw bytes and discard the suffix; retain this
observable behavior. `qa_shorthand_characters` covers surrogate, Mule and
byte8 replacement prefixes in string/buffer/public-intern paths.

CLI `--eval` retains the argument's Lisp string kind, including arguments
inserted into command-line-args-left and inline `--eval=...` operands. Pass
the actual string to eval_first_object instead of flattening it to Rust text.
`--funcall` uses the normal native-call interning protocol, so dynamic obarrays,
symbol shorthands and enabled intern trampolines are observed. Regression:
`tests/cli_symbol_names.rs`.

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

Configuration parameter snapshots contain fresh associations for non-nil
`window-persistent-parameters` entries; values themselves remain shared.
Restore only saved keys into live leaf/minibuffer/internal window params,
leaving nonpersistent keys alone and avoiding new nil associations.
Tree slot 4 carries internal-window parameter snapshots. Cyclic persistence
specifications reset to nil as GNU does; dotted tails stop traversal.
`qa_window_configuration` pins these #284 snapshot and restore rules.

Window-state parameter slots are alists with fresh associations and shared
values. `window-state-get` keeps non-nil persistence flags, or exactly
`writable` flags for writable states; `clone-of` follows the same filter
and is synthesized only when absent. `window-state-put` resets existing
values to nil before merging saved associations into the internal plist,
including internal tree nodes. `qa_winstate` pins these #285 rules.

Window normal sizes live in `Window.normal_size` (height, width), separately
from rounded integer geometry and the pending `new_normal` slot. Splitting
computes the new fraction first and subtracts it from the old fraction;
unspecified sizes halve the intended fraction even for odd cell counts.
New internal parents inherit the replaced window's fractions. Deletion,
explicit resizing, configuration/state restoration and session dumps carry
the values; legacy dumps infer fractions from geometry. Configuration leaf
slot 20 and tree slot 5 hold normal sizes. `qa_winstate` covers #286.

Window use times advance `Interp.window_select_count`, independently of
other editor instances. `window-bump-use-time` only updates a nonselected
live window when the selected window carries that latest count: the target
gets the old count, the selected window gets the increment, and the old
count is returned. No-record selections and deletion of the newest window
do not lower the counter. Initial root selection records 1; other new
windows start at 0. Display-buffer bumps delegate to the public function.
Session images preserve the counter; legacy images infer the maximum saved
window use time. `qa_winstate` and session tests pin these #287 rules.

`window-old-point` reads `Window.old_point_marker`, independently of current
point. Buffer installation and splitting attach this marker with the buffer's
`window-point-insertion-type`; redisplay moves it to the point just displayed.
`old_point` is only the fallback cache for windows without an attached marker.
Configuration snapshots copy its current position into independent markers;
GC traces the live marker and session images preserve it. Legacy images attach
a new marker at the saved cache position. Split windows initialize point and
old point from the canonical window point, including selected buffers whose
buffer point has moved. `qa_winstate` and session tests pin these #288 rules.

`window-old-buffer` holds the previous notification snapshot, not the buffer
replaced by `set-window-buffer`. The pre-redisplay phase in `editor/changes.rs`
runs local change functions in the old/new buffers with a window argument,
then default functions once with the frame argument; local t sentinels are
ignored. History is recorded for all live frames after callbacks (also on
unwind). Ordinary, isearch and minibuffer loops enter this phase before drawing.
Frame epochs start at 1, new windows at 0. A non-nil old buffer with a stale
window epoch is exposed as t, including deleted windows. Shared epoch cells
outlive removed frames. Configuration record slot 7 saves the frame epoch;
leaf spec slot 21 saves old buffer, restored only within the same epoch.
Session images persist snapshots and rebind shared epoch cells; orphan windows
retain the owner's last epoch. GNU TTY golden fixtures and session tests pin
these #289 notification, deletion, reentrancy and restoration rules.

## Live backtraces (`crates/runtime/src/lisp/backtrace.rs`)

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

## Portable images (`crates/runtime/src/lisp/dump/`)

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

## Gap allocation (`crates/runtime/src/buffer/gapbuf.rs`)

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

