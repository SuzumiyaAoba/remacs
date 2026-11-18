# Runtime services and platform integrations

## Tree-sitter support (`crates/runtime/src/lisp/builtins/treesit.rs`)

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

## Subprocess pipe and PTY transports (`crates/runtime/src/lisp/process/child.rs`)

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

Process filter/sentinel dispatch binds `inhibit-quit` and `last-nonmenu-event`
to t and restores the interrupted current buffer if it survives, without
restoring point. The scope includes decoding hooks, default insertion/change
hooks, separate stderr and decoder tails. Match registers use GNU search.c's
lazy save on first search, including predicate/failed searches. Recursive
callbacks additionally unwind a marker-based copy after restoring the lazy
snapshot; a simple per-callback clone gives different observable registers.
The asynchronous flag and saved registers travel with each coroutine and
reset on image restore. GNU's manual search-state cleanup runs only on normal
return; throws still unwind dynamic bindings and the saved buffer. Regression:
`qa_process_callback_context` (41 GNU-verified checks). Zero-count buffer
searches validate bounds in GNU's backward direction and record an empty
match at point; invalid bounds fail before touching asynchronous match data.

Filter/sentinel function calls install GNU's internal error catch when
`debug-on-error` is nil on entry. Report caught errors through the current
`command-error-function` with the full condition, contextual prefix, and
private signaling-function value; register the catch before offering signals
so outer `handler-bind` observers do not intercept them. GNU's C `Qerror`
catch tag covers all signal conditions, including quits and non-error
conditions; reporter failures, throws and exits propagate. Successful reports clear
quit flags, re-inhibit quit and honor `process-error-pause-time` while pumping
timers/processes. Decoding and the fast default-insertion path stay outside
this function-call catch. Signaling-function metadata is coroutine state and
resets on image restore. Regressions: `qa_process_callback_errors` (33 GNU
checks), `tests/process_callback_errors.rs` (fresh normal/source CLI runs).
The default batch reporter exits through the real `kill-emacs` path. Batch
exit hooks use `safe_run_hooks`' internal catch, report/remove failed hooks,
and continue later hooks; interactive exit uses GNU's timed-query wrapper.
The default reporter validates context strings and prints minibuffer quits
without exiting, including GNU's batch stdout/bell behavior.

`Flow::Signal` owns a `Signal` packet containing the one Lisp condition
object and its cached error-conditions list. Preserve the object across
`handler-bind`, interpreted/byte-code `condition-case`, process reporters,
native pending errors, and thread error storage; never reconstruct a cons
from separate symbol/data fields. `(signal CONDITION)` retains CONDITION
itself. Handler and cleanup mutations to its car/cdr stay visible, while
clause selection uses the list captured at raise time. Replacing a symbol's
property does not replace that list; destructive edits to the list do affect
the ongoing search. Do not add an implicit match for the signal's symbol
when its error-conditions list omits that symbol. VM opcode failures offer
the packet before running unwind-protect cleanup, and native signaling uses
the same validated `signal` entry. Reader diagnostic enrichment updates an
unoffered packet's cdr in place. Regressions: `qa_signal_objects` (37 GNU
checks) and `qa_signal_native` (7 checks requiring actual native compilation).

Each signal also records its chosen condition-case scope and clause during
the initial handler walk, before cleanup. Later property/spec mutations do
not retarget that packet. Walk interpreted grouped clauses, VM clauses and
native catchers in dynamic order with handler-bind observers; an observer
temporarily hides both itself and all inner conditional scopes. Keep the
original interpreted clause objects, rather than rereading a caller-mutated
handler list. GNU interpreted evaluation retains the last :success clause;
the GNU byte compiler retains the first. New exits from cleanup choose a
fresh destination while the appropriate old catch scopes are still active.
VM unwind records return errors instead of discarding them, and unwinding
proceeds to each handler's saved depth before that handler is removed.
Binding restoration continues after a failing unlet watcher; later cleanup
failures take precedence. Track a varbind entry even if its let watcher
signals after the specbind was pushed, and clean up partial dynamic argument
bindings on setup failure. Regression coverage: `qa_vm_unwind` (57 GNU
checks) and `qa_signal_targets` (24 GNU checks), including interpreted/VM
controls, nested jumps, scope restoration, observers, GC and thread switches.

`signal-hook-function` runs for caught signals too, before error-name
validation, condition-list lookup, handler-bind observers and catcher
selection. Hook errors/throws replace the original signal; hook changes
to error-conditions affect that signal's selection. Keep the originally
selected name even if the shared condition's car changes in the hook.
`(signal nil CONDITION)` with a cons uses GNU's memory-full path, sharing
CONDITION and skipping this hook; other nil names become `error`.
Regression: `qa_signal_hook` (19 GNU checks).
Error callbacks reserve evaluation depth through GNU's max_ensure_room
protocol: 20 levels for signal-hook-function, 200 for handler-bind, capped
by lisp-eval-depth-reserve. These internal updates do not notify watchers.
Restore the old limit and return the callback's net limit changes to the
reserve on normal and nonlocal exits. Deliver a depth-limit signal before
decrementing the failing eval's depth. Regression: `qa_signal_room` (5 GNU
checks), including the former nonterminating caught-error hook.

Only cons forms consume eval depth; funcall/apply boundaries, native calls
and Bcall consume their own level. Host call frames participate too, so
fresh CLI budgets agree with GNU. Bcall reports GNU's generic nesting
error, while eval/funcall report excessive-lisp-nesting with the failing
depth. Autoload retry reuses its original call's level and displayed name,
including when compatibility loading returns a hidden definition.
Dedicated VM primitives call the canonical C subr without another depth
entry, function-cell lookup, advice or artificial call frame. Build this
registry from all subr sets, including editor/buffer primitives, and map
string=/string< to string-equal/string-lessp. Regressions: qa_call_depth
(12 GNU checks, run through fresh normal/source CLI with a watchdog) and
qa_native_call_depth (3 checks using actual native code).

Pending quits use the Lisp quit-flag value, including dynamic/alias bindings;
the host boolean is only an incoming request and is synchronized before use.
maybe_quit honors inhibit-quit, clears the flag without watcher notifications,
then gives kill-emacs priority over throw-on-input before signaling quit.
Check call/form entries before the depth guard, interpreted while after its
test, and native loop checkpoints. The VM checks every 255 backward branches,
sharing that counter for fast lexical-bytecode calls and reentering for
aliases/autoloads/dynamic calls; a caught exit resets it to 255.
Unbinding and unwind-protect temporarily hide pending quits. Preserve the
saved request unless a newer one replaces it; a failed cleanup abandons its
old saved request. Nonlocal exits retain this state across evaluator frames
and renew it at discarded handler boundaries. User cleanup runs with its own
normal binding scopes. Quit signals follow the ordinary observer/handler
protocol and abort minibuffers/macros like host cancellations.
Regressions: qa_quit_flag (48 GNU checks, normal/source boot), the pending
kill tests in exit_unwind, and native list/pending-connect quit tests.

Opcode 142 chooses a function action versus a legacy progn body when the
record is registered, using GNU FUNCTIONP without invoking Lisp advice or
autoloading. Preserve that choice if the function cell later changes.
Legacy nil/atom bodies do nothing and legacy dotted tails are ignored;
function actions still signal normal call/autoload errors. This includes
the empty unwind-protect generated by the compiler (#132), whose nil action
must not become a call to nil. Function predicates follow complete alias
chains rather than imposing a 64-hop cap. `eval_progn` follows Fprogn's
ordering: retain the form and next cdr before evaluating the form, while
reading that next cell's car when it is reached. Its internal atom/tail
tolerance does not bypass eval's proper-argument-list validation.
Regressions: `qa_unwind_legacy` (24 GNU checks), `qa_progn_traversal`
(21 checks), and the existing empty compiled unwind check in `qa_eval`.

An accepted `Flow::Exit` latches a shared interpreter exit status. Evaluation,
application and signal delivery thereafter propagate that same status without
executing Lisp cleanup, unlet watchers, sentinels or later forms. This is
global to cooperating threads, and is reset with other execution state on
image restore. Interactive `kill-emacs` also returns the uncatchable exit;
front ends observe `quit_editor`. Hook throws/restoration errors before
acceptance still abort shutdown and permit ordinary cleanup. `safe_run_hooks`
must propagate its binding-restoration failures rather than discard them.
Implicit batch termination uses the real kill path so a failed hook cannot
skip later hooks. Normal completion calls the public kill-emacs cell with t,
honoring advice and replacements; fatal/pending exits call the C primitive.
If a replacement returns, run normal-top-level's startup/term/window hooks
through the public run-hooks function, respecting inhibit-startup-hooks at
both GNU checkpoints, then enter the nonterminal batch event loop. Queued
commands and timers remain live without reading redirected stdin as commands.
The command-loop top-level catch then exits batch mode through the C primitive;
the startup top-level catch instead proceeds into the command loop.
Command errors reach command-error-function before post-command-hook; restore
the input/output and quit state before the next command.
Removing failed hooks uses normal variable setters and
propagates watcher failures before accepting shutdown. A default function
reached through a local `t` is removed from the default, preserving local
entries; alias bindings follow the same rule. Batch string exit arguments
do not print diagnostics. Regressions: `tests/exit_unwind.rs` (normal/source CLI,
GNU comparisons, actual native entry, worker exit and interactive host).

All hook runners share GNU's live cons traversal: read each cdr after its
callback, retain the original list across binding replacement, and accept
bare `(lambda ...)` functions and dotted list tails. A `t` fetches the
current default at that position; nested default-list `t` entries are
skipped. This also applies to safe hooks and before/after-change hooks.
Regression: `qa_hook_traversal` (16 GNU-verified checks).

Executable entry saves the pristine OS argv before parsing. `kill-emacs`'s
RESTART checks the original executable's existence (F_OK, not X_OK) before
shutdown hooks; resolve PATH and symlinks once, keeping the original argv[0]
for error messages. Only accepted shutdown records the process-wide request,
so an inner exit from a hook wins and an aborted restart cannot arm a later
normal exit. All CLI/front-end exits consume that request after terminal/UI
resources have been released. Unix restart uses execvp, retaining the PID,
OS environment and ignored signals, rather than the mutable Lisp argv or
process-environment. Failed exec reports errno and keeps the original exit
code. Quote validation errors through error_q without translating argv text.
Regression: `tests/restart.rs`, including disposable GNU copies and real PTYs.

Batch output uses the process C stdout buffer. Printing does not flush it;
flush-standard-output, terminal output and minibuffer prompts flush explicitly.
Successful exec discards unflushed piped output, while ordinary exit flushes it.
Batch minibuffer input shares C stdin's getchar cursor. Construct each line
like alloc.c make_string: all sequences must be canonical Emacs UTF-8 (including
surrogates/Mule) to select multibyte storage; any invalid sequence keeps the
entire original line unibyte. Locale coding and last-coding-system-used are
unaffected. Keep the Lisp string object through the minibuffer/read/completion
wrappers, including interning, so representation flags are not lost in a Rust
String conversion. Keyboard readers wait for queued events/timers rather than
consuming stdin; key sequences continue through active prefix maps. Interactive
c/k/K use this event path (K returns a vector), while e takes parameter events
from the supplied KEYS vector. read-key-sequence does not apply the character
readers' inhibit-interaction guard. Regressions: `tests/batch_input.rs` and the
existing CLI/character-output suites.

## Process input pause (`crates/runtime/src/lisp/process.rs`)

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

Nonpositive or NaN `sleep-for`/`sit-for` durations return without entering the
timer/event pump or yielding a worker. The boundary is applied after combining
seconds and optional milliseconds; positive totals retain the existing timer,
process and thread pumping. `qa_timer` pins zero/negative totals, millisecond
cancellation, positive controls, and a queued timer firing on the next wait
(#290).
Listener targets return without selecting; stopped connections retain
their select-timeout validation. Exit notifications bound only one turn.
Regression: `qa_process_timeout`, plus CLI catch/exit/notification checks.
The headless entry point propagates unexpected interpreter-worker panics
after worker cleanup, instead of silently reporting exit status zero.

## External libraries (`crates/runtime/src/lisp/dynlib.rs` + builtins)

Optional GNU libraries are dlopen'd at runtime (`dynlib::Library`):
`libgnutls` (crypto + TLS sessions, `crates/runtime/src/lisp/builtins/gnutls.rs` —
TLS I/O routed inside `crates/runtime/src/lisp/process.rs`), `liblcms2`
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

## NS/macOS GUI (`crates/runtime/src/lisp/builtins/nsgui.rs`)

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

## Native compilation (`crates/runtime/src/lisp/builtins/comp.rs`)

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
  under `~/.emacs.d/eln-cache/31.1-remacs-v2/`. `comp-el-to-eln-filename`
  uses the explicit target directory when supplied; otherwise it uses the
  first `native-comp-eln-load-path` directory and appends
  `comp-native-version-dir`. File compilation creates that versioned output
  directory after parsing the source. Before `normal-top-level` initializes
  the load path, internal compilation may use the private default cache;
  the public filename helper signals when its configured load path is empty.
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
- Loading unchanged shipped comp.el/comp-run.el initializes the GNU helper
  surface while retaining the live cc-backend compiler drivers before each
  subsequent top-level form and after-load callback. User load-path copies
  and edited sources remain authoritative. This prevents an explicit
  `(require 'comp)` from replacing the working backend with GNU's incompatible
  libgccjit driver. `qa_native_comp_load` covers compilation from an after-load
  callback, explicit reloads, comp-run initialization and a user comp.el override.
- Dynamic let/let* bindings and dynamic-function parameters use the real
  specbind stack through the native API. Evaluate parallel initializers before
  binding, unwind native scopes normally, and restore partially installed
  bindings after failed callbacks. Lexical-function arguments stay lexical
  even when their names are special; compiled inner dynamic lets shadow those
  arguments as GNU native code does. Use the real byte-optimize-letX rules,
  normalizing bare binders to one-element lists as cconv does before calling
  the optimizer. Honor byte-optimize and propagate compile-time callback exits.
  qa_native_letstar covers 37 GNU checks, including omitted initializers.
- Native value reads call the C getter without an extra funcall unless a
  symbol-value trampoline has been installed. Replacing/restoring that subr
  retains GNU's trampoline behavior; the enable variable and portable images
  preserve its state. New units export remacs_eln_init_v2 and use the v2 cache;
  the loader retains old init/API-prefix compatibility. qa_native_bindings
  covers 33 GNU checks; nativecomp tests load a real old-prefix C unit.
- `comp-libgccjit-version` → nil (no jit), same as a GNU
  `--without-native-compilation` build's surface.
- Known limits: max 256 native functions/units (trampoline pool),
  `comp--*` internals cover the stable entry points only
  (`comp--register-subr`, `comp--init/release-ctxt`,
  `comp--compile-ctxt-to-file0`, `comp--late-register-subr`,
  `comp--install-trampoline`), `comp.el`'s LIMPLE/optimizing passes
  are not run.

Regression suite: `tests/nativecomp.rs` and the `qa_native_*` fixtures.

## Subprocess environments (`crates/runtime/src/lisp/process/environment.rs`)

Subprocess environments are built by `crates/runtime/src/lisp/process/environment.rs`
from the current Lisp `process-environment` binding. Async children,
`call-process`, `call-process-region`, `process-lines` and shell command
helpers clear inherited OS variables and install this child-local copy;
they never mutate the editor's OS environment. Like GNU, the first entry
wins (bare names suppress later assignments), only the initial string
prefix of the list is consumed, and entries end at the first NUL.
Unibyte values retain their bytes. Program lookup uses Lisp `exec-path`
before applying the child's PATH. Regression: `qa_process_environment`.

`shell-file-name` initially uses the startup `SHELL` value, including an
empty string, and defaults to `/bin/sh` only when unset. `ESHELL` does not
override this default; later `setenv` calls do not reset it. Explicit Lisp
bindings still select the execution shell. The Rust `shell-command` and
`shell-command-to-string` use those bindings and `shell-command-switch`,
with the normal exec-path and process-directory resolution.
Regression: `shell_environment`
uses independent processes with GNU checks and both ELC modes (#295).

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

## Minibuffer reads (`crates/runtime/src/lisp/eval.rs` + `crates/runtime/src/editor/mod.rs` + `src/term`)

GNU structure kept: `read-file-name`/`completing-read`/`read-buffer`
are subrs that delegate to the user variables
(`read-file-name-function`, `completing-read-function`,
`read-buffer-function`) or their -default counterparts, exactly like
GNU's Lisp wrappers. `completing-read` passes all eight arguments to the
selected function before batch-input guards or prompt/initial-input
validation. A nil function signals `void-function`; the default implementation
owns physical input. Empty completion with nil DEF keeps the empty string;
a non-nil list DEF selects its car, including nil or non-string cars.
`completing-read-multiple` remains the real `crm.el` autoload so separator
regexps, prompt formats, properties and per-element completion share GNU's
implementation. Regressions: `qa_crm`, completion cases in `batch_input`,
and `tui_multiple_completion_completes_each_element`.
`f_completing_read_default` does
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

Scrolling primitives live in `crates/runtime/src/editor/mod.rs` (`window_scroll'):
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
clearing it as soon as point differs from the edit-tracking old-point marker (the
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

The GPUI view registers `EntityInputHandler` on its full-size canvas during
paint. Plain characters arrive through OS text commits; only modifier and
named command keys use `on_key_down`, preventing duplicate ASCII insertion.
`GuiEvent::Text` forwards every Unicode scalar in order through `ChanIo` and
the shared command loop, including supplementary and combining characters.
The input client owns only provisional composition text, with UTF-16 ranges
normalized at scalar boundaries; marked text is previewed and committed once.
Candidate bounds follow the grid cursor. `gui::` tests cover registration,
commands, composition ranges and a UTF-8 file saved through the logic thread
(#292).


