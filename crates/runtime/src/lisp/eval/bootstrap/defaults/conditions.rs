//! GNU condition ancestry and default diagnostic text, in registration order.

pub(super) const CORE: &[(&str, &[&str])] = &[
    ("error", &["error"]),
    ("quit", &["quit"]),
    ("user-error", &["user-error", "error"]),
    ("arith-error", &["arith-error", "error"]),
    ("range-error", &["range-error", "arith-error", "error"]),
    ("domain-error", &["domain-error", "arith-error", "error"]),
    (
        "overflow-error",
        &["overflow-error", "range-error", "arith-error", "error"],
    ),
    (
        "underflow-error",
        &["underflow-error", "range-error", "arith-error", "error"],
    ),
    (
        "singularity-error",
        &["singularity-error", "domain-error", "arith-error", "error"],
    ),
    ("wrong-type-argument", &["wrong-type-argument", "error"]),
    (
        "wrong-number-of-arguments",
        &["wrong-number-of-arguments", "error"],
    ),
    ("args-out-of-range", &["args-out-of-range", "error"]),
    ("wrong-length-argument", &["wrong-length-argument", "error"]),
    ("void-function", &["void-function", "error"]),
    (
        "cyclic-function-indirection",
        &["cyclic-function-indirection", "error"],
    ),
    ("void-variable", &["void-variable", "error"]),
    ("setting-constant", &["setting-constant", "error"]),
    ("invalid-function", &["invalid-function", "error"]),
    ("invalid-read-syntax", &["invalid-read-syntax", "error"]),
    ("circular-list", &["circular-list", "error"]),
    ("beginning-of-buffer", &["beginning-of-buffer", "error"]),
    ("end-of-buffer", &["end-of-buffer", "error"]),
    ("buffer-read-only", &["buffer-read-only", "error"]),
    (
        "text-read-only",
        &["text-read-only", "buffer-read-only", "error"],
    ),
    ("mark-inactive", &["mark-inactive", "error"]),
    ("file-error", &["file-error", "error"]),
    ("file-missing", &["file-missing", "file-error", "error"]),
    ("end-of-file", &["end-of-file", "error"]),
    ("search-failed", &["search-failed", "error"]),
    ("no-catch", &["no-catch", "error"]),
    ("scan-error", &["scan-error", "error"]),
    ("invalid-regexp", &["invalid-regexp", "error"]),
    ("recursion-error", &["recursion-error", "error"]),
    (
        "excessive-lisp-nesting",
        &["excessive-lisp-nesting", "recursion-error", "error"],
    ),
    (
        "excessive-variable-binding",
        &["excessive-variable-binding", "recursion-error", "error"],
    ),
    ("unknown-image-type", &["unknown-image-type", "error"]),
    (
        "file-already-exists",
        &["file-already-exists", "file-error", "error"],
    ),
    (
        "file-supersession",
        &["file-supersession", "file-error", "error"],
    ),
    (
        "permission-denied",
        &["permission-denied", "file-error", "error"],
    ),
    // filelock.c
    ("file-locked", &["file-locked", "file-error", "error"]),
    // cl-macs.el `cl-assert'
    ("cl-assertion-failed", &["cl-assertion-failed", "error"]),
    ("coding-system-error", &["coding-system-error", "error"]),
    (
        "coding-conversion-error",
        &["coding-conversion-error", "error"],
    ),
    // JSON errors (json.c): a `json-error' parent under `error'.
    ("json-error", &["json-error", "error"]),
    (
        "json-parse-error",
        &["json-parse-error", "json-error", "error"],
    ),
    (
        "json-end-of-file",
        &[
            "json-end-of-file",
            "json-parse-error",
            "json-error",
            "error",
        ],
    ),
    (
        "json-escape-sequence-error",
        &[
            "json-escape-sequence-error",
            "json-parse-error",
            "json-error",
            "error",
        ],
    ),
    (
        "json-trailing-content",
        &[
            "json-trailing-content",
            "json-parse-error",
            "json-error",
            "error",
        ],
    ),
    (
        "json-invalid-surrogate-error",
        &["json-invalid-surrogate-error", "json-error", "error"],
    ),
    (
        "json-number-out-of-range-error",
        &["json-number-out-of-range-error", "json-error", "error"],
    ),
    (
        "json-object-too-deep",
        &["json-object-too-deep", "json-error", "error"],
    ),
    (
        "json-out-of-memory",
        &["json-out-of-memory", "json-error", "error"],
    ),
    (
        "json-utf8-decode-error",
        &["json-utf8-decode-error", "json-error", "error"],
    ),
    ("mark-set", &["mark-set"]),
    ("mark-active", &["mark-active"]),
];

pub(super) const ADDITIONAL: &[(&str, &[&str])] = &[
    (
        "cyclic-variable-indirection",
        &["cyclic-variable-indirection", "error"],
    ),
    (
        "file-date-error",
        &["file-date-error", "file-error", "error"],
    ),
    (
        "file-notify-error",
        &["file-notify-error", "file-error", "error"],
    ),
    (
        "remote-file-error",
        &["remote-file-error", "file-error", "error"],
    ),
    ("inhibited-interaction", &["inhibited-interaction", "error"]),
    ("invalid-arity", &["invalid-arity", "error"]),
    (
        "malformed-keyword-arg-list",
        &["malformed-keyword-arg-list", "error"],
    ),
    (
        "memory-buffer-too-small",
        &["memory-buffer-too-small", "error"],
    ),
    ("minibuffer-quit", &["minibuffer-quit", "quit"]),
    ("module-load-failed", &["module-load-failed", "error"]),
    (
        "missing-module-init-function",
        &[
            "missing-module-init-function",
            "module-load-failed",
            "error",
        ],
    ),
    (
        "module-init-failed",
        &["module-init-failed", "module-load-failed", "error"],
    ),
    (
        "module-not-gpl-compatible",
        &["module-not-gpl-compatible", "module-load-failed", "error"],
    ),
    (
        "module-open-failed",
        &["module-open-failed", "module-load-failed", "error"],
    ),
    ("module-out-of-memory", &["module-out-of-memory", "error"]),
    ("native-compiler-error", &["native-compiler-error", "error"]),
    (
        "native-ice",
        &["native-ice", "native-compiler-error", "error"],
    ),
    (
        "native-lisp-load-failed",
        &["native-lisp-load-failed", "error"],
    ),
    (
        "native-lisp-file-inconsistent",
        &[
            "native-lisp-file-inconsistent",
            "native-lisp-load-failed",
            "error",
        ],
    ),
    (
        "native-lisp-wrong-reloc",
        &[
            "native-lisp-wrong-reloc",
            "native-lisp-load-failed",
            "error",
        ],
    ),
    (
        "wrong-register-subr-call",
        &[
            "wrong-register-subr-call",
            "native-lisp-load-failed",
            "error",
        ],
    ),
    ("comp-sanitizer-error", &["comp-sanitizer-error", "error"]),
    ("protected-field", &["protected-field", "error"]),
    ("thread-buffer-killed", &["thread-buffer-killed", "error"]),
    ("trapping-constant", &["trapping-constant", "error"]),
    ("type-mismatch", &["type-mismatch", "error"]),
    (
        "user-search-failed",
        &["user-search-failed", "user-error", "search-failed", "error"],
    ),
    (
        "treesit-buffer_changed",
        &["treesit-buffer_changed", "treesit-error", "error"],
    ),
];

pub(super) const TREE_SITTER: &[(&str, &str)] = &[
    ("treesit-query-error", "Query pattern is malformed"),
    ("treesit-parse-error", "Parse failed"),
    (
        "treesit-range-invalid",
        "RANGES are invalid: they have to be ordered and should not overlap",
    ),
    ("treesit-buffer-too-large", "Buffer too large (> 4GiB)"),
    (
        "treesit-load-language-error",
        "Cannot load language definition",
    ),
    (
        "treesit-node-outdated",
        "This node is outdated, please retrieve a new one",
    ),
    (
        "treesit-buffer-changed",
        "Buffer content changed, please don't edit buffer in predicate function, etc",
    ),
    (
        "treesit-node-buffer-killed",
        "The buffer associated with this node is killed",
    ),
    (
        "treesit-parser-deleted",
        "This parser is deleted and cannot be used",
    ),
    (
        "treesit-invalid-predicate",
        "Invalid predicate, see `treesit-thing-settings' for valid forms for a predicate",
    ),
];

pub(super) const MESSAGES: &[(&str, &str)] = &[
    ("error", "error"),
    ("quit", "Quit"),
    ("user-error", ""),
    ("arith-error", "Arithmetic error"),
    ("range-error", "Arithmetic range error"),
    ("domain-error", "Arithmetic domain error"),
    ("overflow-error", "Arithmetic overflow error"),
    ("underflow-error", "Arithmetic underflow error"),
    ("wrong-type-argument", "Wrong type argument"),
    ("wrong-number-of-arguments", "Wrong number of arguments"),
    ("args-out-of-range", "Args out of range"),
    ("wrong-length-argument", "Wrong length argument"),
    ("void-function", "Symbol's function definition is void"),
    (
        "cyclic-function-indirection",
        "Symbol's chain of function indirections contains a loop",
    ),
    ("void-variable", "Symbol's value as variable is void"),
    ("setting-constant", "Attempt to set a constant symbol"),
    ("invalid-function", "Invalid function"),
    ("invalid-read-syntax", "Invalid read syntax"),
    ("circular-list", "List contains a loop"),
    ("beginning-of-buffer", "Beginning of buffer"),
    ("end-of-buffer", "End of buffer"),
    ("buffer-read-only", "Buffer is read-only"),
    ("text-read-only", "Text is read-only"),
    ("mark-inactive", "The mark is not active now"),
    ("file-error", "File error"),
    ("file-missing", "File is missing"),
    ("end-of-file", "End of file during parsing"),
    ("search-failed", "Search failed"),
    ("no-catch", "No catch for tag"),
    ("scan-error", "Scan error"),
    ("invalid-regexp", "Invalid regexp"),
    ("coding-system-error", "Invalid coding system"),
    ("coding-conversion-error", "Coding conversion error"),
    ("file-already-exists", "File already exists"),
    ("file-supersession", "File is already being edited"),
    ("permission-denied", "Cannot access file or directory"),
    ("file-locked", "File is locked"),
    ("cl-assertion-failed", "Assertion failed"),
    ("recursion-error", "Excessive recursive calling error"),
    (
        "excessive-lisp-nesting",
        "Lisp nesting exceeds `max-lisp-eval-depth'",
    ),
    (
        "excessive-variable-binding",
        "Variable binding depth exceeds max-specpdl-size",
    ),
    ("singularity-error", "Arithmetic singularity error"),
    ("unknown-image-type", "Cannot determine image type"),
    ("json-error", "generic JSON error"),
    ("json-parse-error", "could not parse JSON stream"),
    ("json-end-of-file", "end of JSON stream"),
    ("json-escape-sequence-error", "invalid escape sequence"),
    (
        "json-trailing-content",
        "trailing content after JSON stream",
    ),
    ("json-invalid-surrogate-error", "invalid surrogate pair"),
    ("json-number-out-of-range-error", "number out of range"),
    (
        "json-object-too-deep",
        "object cyclic or Lisp evaluation too deep",
    ),
    (
        "json-out-of-memory",
        "not enough memory for creating JSON object",
    ),
    ("json-utf8-decode-error", "invalid utf-8 encoding"),
    (
        "cyclic-variable-indirection",
        "Symbol's chain of variable indirections contains a loop",
    ),
    ("file-date-error", "Cannot set file date"),
    ("file-notify-error", "File notification error"),
    ("remote-file-error", "Remote file error"),
    ("inhibited-interaction", "User interaction while inhibited"),
    ("invalid-arity", "Invalid function arity"),
    (
        "malformed-keyword-arg-list",
        "Keyword lacks a corresponding value",
    ),
    ("memory-buffer-too-small", "Memory buffer too small"),
    ("minibuffer-quit", "Quit"),
    ("module-load-failed", "Module load failed"),
    (
        "missing-module-init-function",
        "Module does not export an initialization function",
    ),
    ("module-init-failed", "Module initialization failed"),
    ("module-not-gpl-compatible", "Module is not GPL compatible"),
    ("module-open-failed", "Module could not be opened"),
    ("module-out-of-memory", "Module out of memory"),
    ("native-compiler-error", "Native compiler error"),
    ("native-ice", "Internal native compiler error"),
    ("native-lisp-load-failed", "Native elisp load failed"),
    (
        "native-lisp-file-inconsistent",
        "eln file inconsistent with current runtime configuration, please recompile",
    ),
    (
        "native-lisp-wrong-reloc",
        "Primitive redefined or wrong relocation",
    ),
    (
        "wrong-register-subr-call",
        "comp--register-subr can only be called during native lisp load phase.",
    ),
    (
        "comp-sanitizer-error",
        "Native code sanitizer runtime error",
    ),
    ("protected-field", "Attempt to modify a protected field"),
    ("thread-buffer-killed", "Thread's current buffer killed"),
    (
        "trapping-constant",
        "Attempt to trap writes to a constant symbol",
    ),
    ("type-mismatch", "Types do not match"),
    ("user-search-failed", "Search failed"),
    ("sqlite-error", "Database error"),
    (
        "treesit-buffer_changed",
        "Buffer content changed, please don't edit buffer in predicate function, etc",
    ),
];
