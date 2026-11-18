# Regression and verification notes

## Verification recipe

```sh
# byte-compare against GNU 31.1 source
rtk proxy sh -c 'GNU=/nix/store/7l70cwk87fk30viyjpj9iwha6a2wcs6v-emacs-unstable-31.1/share/emacs/31.1/lisp; gzip -dc "$GNU/<path>.el.gz" | cmp - lisp/<file>.el'

# parse-check all files without evaluating (see git history for script)
rtk proxy ./target/debug/remacs --batch --eval '(load "/tmp/parsecheck6.el")'
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

## Interpreter lifetime and byte accounting

Dropping a deep proper list or nested cons tree must not recurse on the Rust
stack. `Cons::drop` drains car/cdr children through a thread-local worklist;
cycles retain the existing Rc behavior. The 500,001-element
`qa_assoc_default_long_cycle` and `value::tests::dropping_deep_cons_nesting_does_not_recurse_on_the_rust_stack`
cover this path.

`memory-use-counts` counts string storage bytes, not Rust character count:
unibyte strings count one byte per character, multibyte strings count each
character's internal encoding width. `tests/qa_memory_statistics.el` checks
Japanese text plus unibyte and multibyte byte8 strings against GNU.

`Value::Debug` should render strings with quoted escapes, preserve non-Unicode
character codes as `\x{...}`, and avoid panicking while the string is borrowed.
`value::tests::debug_fmt_covers_every_variant` covers these cases. The QA
macros quote forms and call `eval`, so forms passed to `qa`/`qa-xf` cannot
capture a caller-only lexical variable; use a named global fixture when a
quoted form needs shared test data (`qa_auth_source_netrc_macdef.el`).

`qa_call_depth` runs in the real CLI entry context both with and without
embedded byte-code. In fast-dev, the source-only run loads `bytecomp` from
Lisp source and takes about 8 minutes; keep its per-child watchdog at 600
seconds and retain the outer process-group supervisor.

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

The prelude installs keyboard.c's core `special-event-map` bindings and
bindings.el's signal handlers through the real keymap constructors. Keep
frame focus/move/delete, visibility, save-session, thread, sleep and NS
working-text events in that map; switch-frame and select-window remain
outside it. This also creates their normal event parsing caches.
Regression: `qa_read_events`.

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
