//! Regression tests for native compilation (src/lisp/builtins/comp.rs):
//! cc-emitted .eln shared objects, the em_obj handle API, trampoline
//! dispatch, .eln naming, and the comp-* primitive surface.
//! Tests skip when no C compiler is available (native-comp-available-p
//! nil — same contract as a --without-native-compilation GNU build).

mod common;
use common::{ev, ev_err};

fn avail() -> bool {
    ev("(native-comp-available-p)") == "t"
}

/// Write a scratch .el file and return its path.
fn scratch(name: &str, body: &str) -> String {
    let dir = std::env::temp_dir().join("remacs-nctest");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------
// availability / feature / version surface
// ---------------------------------------------------------------

#[test]
fn availability_and_feature() {
    assert_eq!(
        ev("(list (featurep 'native-compile) (featurep 'native-comp))"),
        "(t nil)"
    );
    if !avail() {
        return;
    }
    // remacs emits C via cc — no libgccjit, so nil like a
    // --without-native-compilation check on the jit version.
    assert_eq!(ev("(comp-libgccjit-version)"), "nil");
    assert_eq!(ev("(comp-native-compiler-options-effective-p)"), "t");
    assert_eq!(ev("(comp-native-driver-options-effective-p)"), "t");
    // Entry points are Lisp wrappers (defun), like GNU's comp.el.
    assert_eq!(ev("(fboundp 'native-compile)"), "t");
    assert_eq!(ev("(fboundp 'native-compile-async)"), "t");
    assert_eq!(ev("(fboundp 'native-compile-directory)"), "t");
    assert_eq!(ev("(fboundp 'batch-native-compile)"), "t");
}

#[test]
fn old_native_api_prefix_remains_loadable() {
    if !avail() { return; }
    let source = scratch("legacy-api-prefix.c", r#"
typedef unsigned long long em_obj;
typedef unsigned long em_env;
typedef em_obj (*entry_fn)(const void *, em_env, const em_obj *, long);
struct old_api {
  em_obj (*nil)(em_env);
  em_obj (*t)(em_env);
  em_obj (*integer)(em_env, long long);
  void *unused_prefix[14];
  long (*defsubr)(const void *, em_env, long, const void *);
};
struct old_desc {
  const char *name; unsigned long name_len;
  unsigned int min, max; entry_fn entry;
  const char *doc; unsigned long doc_len;
  const char *arglist; unsigned long arglist_len;
  const char *iform; unsigned long iform_len;
  const char *sexp; unsigned long sexp_len;
};
static em_obj legacy_entry(const void *api, em_env env, const em_obj *args, long count) {
  return ((const struct old_api *)api)->integer(env, 37);
}
static const struct old_desc desc = {
  "qa-legacy-native", 16, 0, 0, legacy_entry, 0, 0, "nil", 3, 0, 0, 0, 0
};
int remacs_eln_init(const void *api, em_env env, long unit) {
  return ((const struct old_api *)api)->defsubr(api, env, unit, &desc);
}
"#);
    let output = std::path::Path::new(&source).with_extension("eln");
    let compiler = std::env::var("REMACS_CC").unwrap_or_else(|_| "cc".into());
    let status = std::process::Command::new(compiler)
        .args(["-shared", "-fPIC", "-x", "c", &source, "-o"])
        .arg(&output).status().unwrap();
    assert!(status.success());
    assert_eq!(ev(&format!("(progn (native-elisp-load {:?}) (list (qa-legacy-native) \
        (native-comp-function-p (symbol-function 'qa-legacy-native))))", output.to_str().unwrap())),
        "(37 t)");
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(output);
}

#[test]
fn interleaved_threads_preserve_native_handles_and_constant_contexts() {
    if !avail() {
        return;
    }
    let path = scratch(
        "threads196.el",
        r#";;; -*- lexical-binding: t; -*-
(defun nc196-a (x)
  (let ((saved (vector x "alpha")))
    (thread-yield)
    (list (aref saved 0) (eq (aref saved 1) "alpha"))))
(defun nc196-b (x)
  (let ((saved (vector x "beta")))
    (thread-yield)
    (list (aref saved 0) (eq (aref saved 1) "beta"))))
"#,
    );
    assert_eq!(
        ev(&format!(
            "(progn (native-elisp-load (native-compile {path:?})) \
          (let ((a (make-thread (lambda () (nc196-a 11)))) \
                (b (make-thread (lambda () (nc196-b 22))))) \
            (thread-yield) (thread-yield) \
            (list (thread-join a) (thread-join b) \
              (native-comp-function-p (symbol-function 'nc196-a)) \
              (native-comp-function-p (symbol-function 'nc196-b)))))"
        )),
        "((11 t) (22 t) t t)"
    );
}

#[test]
fn dropping_a_suspended_native_worker_unwinds_across_the_c_boundary() {
    if !avail() {
        return;
    }
    let path = scratch(
        "drop-thread196.el",
        "(defun nc196-drop () (thread-yield) 7)\n",
    );
    // ev drops its interpreter before returning. The worker is still
    // suspended inside generated C, so its stack must be unwound safely.
    assert_eq!(
        ev(&format!(
            "(progn (native-elisp-load (native-compile {path:?})) \
          (make-thread (lambda () (nc196-drop))) \
          (thread-yield) 'paused)"
        )),
        "paused"
    );
}

#[test]
fn native_feature_name_does_not_depend_on_runtime_compiler() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("remacs-no-compiler-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_remacs"))
        .env("PATH", &root)
        .env_remove("REMACS_CC")
        .env_remove("EMACSLOADPATH")
        .env_remove("REMACS_NO_ELC")
        .args(["-Q", "--batch", "--eval",
            "(prin1 (list (featurep 'native-compile) (featurep 'native-comp) (native-comp-available-p)))"])
        .output().unwrap();
    let _ = std::fs::remove_dir_all(root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "(t nil nil)");
}

// ---------------------------------------------------------------
// .eln naming (GNU's <base>-<md5(truename)>-<md5(contents)>.eln)
// ---------------------------------------------------------------

#[test]
fn eln_filename_shape() {
    if !avail() {
        return;
    }
    let p = scratch("shape1.el", "(defun shape1 (x) x)\n");
    // <base>-8hex-8hex.eln
    assert_eq!(
        ev(&format!(
            "(string-match \"-[0-9a-f]\\\\{{8\\\\}}-[0-9a-f]\\\\{{8\\\\}}\\\\.eln\\\\'\" \
                            (comp-el-to-eln-rel-filename {p:?}))"
        )),
        "6"
    );
    // Deterministic for same file.
    assert_eq!(
        ev(&format!(
            "(equal (comp-el-to-eln-rel-filename {p:?}) \
                    (comp-el-to-eln-rel-filename {p:?}))",
        )),
        "t"
    );
    // Content change → different content hash.
    let p2 = scratch("shape2.el", "(defun shape2 (x) x)\n");
    assert_eq!(
        ev(&format!(
            "(equal (comp-el-to-eln-rel-filename {p:?}) \
                    (comp-el-to-eln-rel-filename {p2:?}))"
        )),
        "nil"
    );
}

// ---------------------------------------------------------------
// compile a file → load → run real native code
// ---------------------------------------------------------------

#[test]
fn compile_load_run_file() {
    if !avail() {
        return;
    }
    let src = concat!(
        "(defun nc-add (a b) (+ a b))\n",
        "(defun nc-fib (n) (if (< n 2) n (+ (nc-fib (- n 1)) (nc-fib (- n 2)))))\n",
        "(defun nc-loop (n) (let ((acc 0)) (while (> n 0) (setq acc (+ acc n)) (setq n (1- n))) acc))\n",
    );
    let p = scratch("run1.el", src);
    let r = ev(&format!(
        "(progn (let ((eln (native-compile {p:?}))) \
                   (native-elisp-load eln)) \
                (list (nc-add 3 4) (nc-fib 10) (nc-loop 10) \
                      (native-comp-function-p (symbol-function 'nc-add)) \
                      (subrp (symbol-function 'nc-add))))"
    ));
    assert_eq!(r, "(7 55 55 t t)");
}

#[test]
fn obsolete_macro_aliases_compile_with_keywords() {
    if !avail() {
        return;
    }
    let source = r#";;; -*- lexical-binding: t; -*-
(defun nc-pushnew-alias ()
  (let ((xs '("a" "b")) (ys '((1 . a) (2 . b))))
    (pushnew (copy-sequence "a") xs :test 'equal)
    (pushnew '(1 . c) ys :key 'car)
    (list xs ys)))
"#;
    let path = scratch("legacy-macro-alias.el", source);
    assert_eq!(
        ev(&format!(
            "(progn (require 'cl) \
               (native-elisp-load (native-compile {path:?})) \
               (list (nc-pushnew-alias) \
                     (native-comp-function-p (symbol-function 'nc-pushnew-alias))))"
        )),
        "(((\"a\" \"b\") ((1 . a) (2 . b))) t)"
    );
}

#[test]
fn optional_rest_letstar() {
    if !avail() {
        return;
    }
    let src = concat!(
        "(defun nc-opt (a &optional b (c 10) &rest r) (list a b c r))\n",
        "(defun nc-star (a) (let* ((b (+ a 1)) (c (* b 2))) (list a b c)))\n",
    );
    let p = scratch("opt1.el", src);
    assert_eq!(
        ev(&format!(
            "(progn (native-elisp-load (native-compile {p:?})) \
                    (list (nc-opt 1) (nc-opt 1 2) (nc-opt 1 2 3 4 5) (nc-star 3)))"
        )),
        "((1 nil 10 nil) (1 2 10 nil) (1 2 3 (4 5)) (3 4 8))"
    );
}

#[test]
fn compile_lambda_object() {
    if !avail() {
        return;
    }
    assert_eq!(
        ev(
            "(let ((f (native-compile (lambda (x y) (list x y (+ x y)))))) \
             (list (funcall f 2 5) (native-comp-function-p f) (subrp f) (cl-type-of f)))"
        ),
        "((2 5 7) t t native-comp-function)"
    );
    // By symbol: replaces the function cell with the native subr.
    assert_eq!(
        ev("(progn (defun nc-sym (n) (* n n)) \
                    (native-compile 'nc-sym) \
                    (list (nc-sym 9) (native-comp-function-p \
                          (symbol-function 'nc-sym))))"),
        "(81 t)"
    );
}

// ---------------------------------------------------------------
// eval-trampoline fallback (unwind forms) + defmacro
// ---------------------------------------------------------------

#[test]
fn fallback_and_macro() {
    if !avail() {
        return;
    }
    let src = concat!(
        "(defun nc-err (x) (condition-case e (/ x 0) (arith-error 'caught)))\n",
        "(defmacro nc-mx (x) `(1+ ,x))\n",
        "(defun nc-use-mx (y) (nc-mx y))\n",
    );
    let p = scratch("fb1.el", src);
    assert_eq!(
        ev(&format!(
            "(progn (native-elisp-load (native-compile {p:?})) \
                    (list (nc-err 5) (macrop 'nc-mx) (nc-use-mx 10) (nc-mx 42)))"
        )),
        "(caught t 11 43)"
    );
}

// ---------------------------------------------------------------
// metadata: unit, lambda list, signature, documentation
// ---------------------------------------------------------------

#[test]
fn metadata_surface() {
    if !avail() {
        return;
    }
    let src = concat!(
        "(defun nc-doc (a b) \"Doc string.\" (+ a b))\n",
        "(defun nc-cmd (n) (interactive \"p\") (* n n))\n",
    );
    let p = scratch("meta1.el", src);
    let r = ev(&format!(
        "(progn (native-elisp-load (native-compile {p:?})) \
                (let ((f (symbol-function 'nc-doc))) \
                  (list (subr-native-lambda-list f) \
                        (native-comp-unit-file (subr-native-comp-unit f)) \
                        (documentation 'nc-doc) \
                        (interactive-form 'nc-cmd) \
                        (commandp 'nc-cmd))))"
    ));
    assert!(r.contains("(a b)"), "arglist: {r}");
    assert!(r.contains(".eln"), "unit file: {r}");
    assert!(r.contains("Doc string."), "doc: {r}");
    assert!(r.contains("(interactive \"p\")"), "iform: {r}");
    assert!(r.ends_with("t)"), "commandp: {r}");
    // Built-in subr → t (not a lambda list).
    assert_eq!(ev("(subr-native-lambda-list (symbol-function 'car))"), "t");
    // comp--subr-signature takes the subr object and matches GNU:
    // "NAME(MIN . MAX)" (a symbol signals `(wrong-type-argument subrp)').
    assert_eq!(
        ev("(comp--subr-signature (symbol-function 'car))"),
        "\"car(1 . 1)\""
    );
    assert_eq!(
        ev("(comp--subr-signature (symbol-function 'apply))"),
        "\"apply(1 . many)\""
    );
}

// ---------------------------------------------------------------
// error paths
// ---------------------------------------------------------------

#[test]
fn error_paths() {
    if !avail() {
        return;
    }
    // Missing source file: the first pass's `insert-file-contents' error
    // with the input appended (GNU-verified).
    assert_eq!(
        ev("(condition-case e (native-compile \"/nonexistent-dir-zz/nope.el\") (error e))"),
        "(file-missing \"Opening input file\" \"No such file or directory\" \
\"/nonexistent-dir-zz/nope.el\" \"/nonexistent-dir-zz/nope.el\")"
    );
    // Neither a function nor a file name.
    assert_eq!(
        ev("(condition-case e (native-compile 42) (error e))"),
        "(native-compiler-error \"Not a function symbol or file\" 42)"
    );
    // Loading a non-.eln file errors cleanly.
    let p = scratch("noteln.txt", "hello");
    assert_eq!(
        ev_err(&format!("(native-elisp-load {p:?})")),
        "native-lisp-load-failed"
    );
    // native-comp-function-p of ordinary objects.
    assert_eq!(ev("(native-comp-function-p 'car)"), "nil");
    assert_eq!(ev("(native-comp-function-p (lambda (x) x))"), "nil");
    // comp--register-subr on a bad c-name errors, doesn't crash.
    let src = "(defun nc-z (x) x)\n";
    let p2 = scratch("z1.el", src);
    assert_eq!(
        ev(&format!(
            "(progn (native-elisp-load (native-compile {p2:?})) \
                    (condition-case e \
                        (comp--register-subr 'zz \"no_such_sym\" 0 0 nil nil \
                          (subr-native-comp-unit (symbol-function 'nc-z))) \
                      (error 'boom)))"
        )),
        "boom"
    );
}

/// Remacs-specific handle callbacks also have to stop at their first error.
/// GNU does not serialize quoted markers or rebuild fallback closures this way.
#[test]
fn failed_read_and_fallback_eval_abort() {
    if !avail() {
        return;
    }
    assert_eq!(
        ev(r#"
          (defvar nc-error-side nil)
          (let* ((dir (make-temp-file "remacs-native-callback-errors-" t))
                 (user-emacs-directory dir)
                 (native-comp-eln-load-path (list dir)))
            (unwind-protect
                (let* ((read-fn
                        (native-compile
                         (eval (list 'lambda nil (list 'quote (make-marker))
                                     '(setq nc-error-side 'after-read)) t)
                         (expand-file-name "read.eln" dir)))
                       (eval-fn
                        (native-compile
                         '(lambda () (unwind-protect
                                         (setq nc-error-side 'fallback-body) nil))
                         (expand-file-name "eval.eln" dir)))
                       (old (symbol-function 'make-interpreted-closure))
                       (read-error (condition-case err (funcall read-fn)
                                     (error (car err))))
                       eval-error)
                  (unwind-protect
                      (progn
                        (fset 'make-interpreted-closure
                              (lambda (&rest ignored) (error "native-closure-stop")))
                        (setq eval-error (condition-case err (funcall eval-fn)
                                           (error err))))
                    (fset 'make-interpreted-closure old))
                  (list (native-comp-function-p read-fn)
                        (native-comp-function-p eval-fn)
                        read-error eval-error nc-error-side
                        (funcall (native-compile '(lambda () (identity nil))
                                                 (expand-file-name "nil.eln" dir)))))
              (delete-directory dir t)))
          "#),
        "(t t invalid-read-syntax (error \"native-closure-stop\") nil nil)"
    );
}

/// Remacs's synchronous async entry point shares the file compiler. Empty
/// files remain skippable, but initializer-only files must no longer vanish.
#[test]
fn async_file_toplevel() {
    if !avail() {
        return;
    }
    let source = scratch("async-toplevel.el", "(setq nc-async-loaded 42)\n");
    let empty = scratch("async-empty.el", ";;; Empty compilation input.\n");
    assert_eq!(
        ev(&format!(
            "(progn (defvar nc-async-loaded nil) (let* ((nc-async-loaded nil) \
                    (outputs (native-compile-async (list {source:?} {empty:?}) nil nil)) \
                    (before nc-async-loaded)) \
                (native-elisp-load (car outputs)) \
                (let ((loaded nc-async-loaded)) \
                  (setq nc-async-loaded nil) \
                  (native-compile-async {source:?} nil t) \
                  (list (length outputs) before loaded nc-async-loaded))))"
        )),
        "(1 nil 42 42)"
    );
}

/// The synchronous async/directory wrappers must not accept or load a valid
/// prefix of malformed source. Their return/error contract is remacs-specific.
#[test]
fn async_file_parse_errors() {
    if !avail() {
        return;
    }
    assert_eq!(
        ev(r#"
          (let* ((dir (make-temp-file "remacs-native-parse-errors-" t))
                 (source (expand-file-name "broken.el" dir))
                 output)
            (unwind-protect
                (progn
                  (with-temp-file source
                    (insert ";;; -*- lexical-binding: t; -*-\n"
                            "(defun nc-parse-prefix () 7)\n(\n"))
                  (setq output (comp-el-to-eln-filename source))
                  (let ((async-error
                         (condition-case err (native-compile-async source)
                           (error (car err))))
                        (load-error
                         (condition-case err (native-compile-async source nil t)
                           (error (car err))))
                        (directory-error
                         (condition-case err (native-compile-directory dir)
                           (error (car err))))
                        (batch-error
                         (let ((command-line-args-left (list source)))
                           (condition-case err (batch-native-compile)
                             (error (car err))))))
                    (list async-error load-error directory-error batch-error
                          (file-exists-p output) (fboundp 'nc-parse-prefix)
                          (progn
                            (with-temp-file source
                              (insert "(defun nc-parse-prefix () 23)\n; clean EOF"))
                            (native-compile-async source nil t)
                            (nc-parse-prefix)))))
              (when (and output (file-exists-p output)) (delete-file output))
              (when (file-exists-p source)
                (let ((recovered (comp-el-to-eln-filename source)))
                  (when (file-exists-p recovered) (delete-file recovered))))
              (delete-directory dir t)))
          "#),
        "(end-of-file end-of-file end-of-file end-of-file nil nil 23)"
    );
}

// This pins Remacs source-value preservation, not GNU native output parity:
// GNU 31.1's native serializer itself normalizes explicitly multibyte ASCII
// and raw-eight-bit constants to unibyte. Identical raw C bytes must still
// retain each source object's kind in Remacs's direct constant cache.
#[test]
fn direct_string_constants_preserve_source_kinds() {
    if !avail() {
        return;
    }
    assert_eq!(
        ev(
            r#"(let ((directory (make-temp-file "remacs-native-kinds-" t))
                    (results nil))
                (unwind-protect
                    (dolist (reverse-order '(nil t))
                      (let* ((values (list "" (string-to-multibyte "")
                                           "ascii" (string-to-multibyte "ascii")
                                           "\200\377" (string-to-multibyte "\200\377")))
                             (values (if reverse-order (reverse values) values))
                             (shape (lambda (s) (list (multibyte-string-p s)
                                                       (string-to-list s)
                                                       (string-bytes s))))
                             (wanted (mapcar shape values))
                             (fn (native-compile
                                  (eval (list 'lambda nil (cons 'list values)) t)
                                  (expand-file-name (if reverse-order "reverse.eln" "forward.eln")
                                                    directory)))
                             (first (funcall fn))
                             (second (funcall fn))
                             (stable t))
                        (dotimes (index 6)
                          (unless (eq (nth index first) (nth index second))
                            (setq stable nil)))
                        (push (list (native-comp-function-p fn)
                                    (equal (mapcar shape first) wanted)
                                    (equal (mapcar shape second) wanted)
                                    (not (eq first second)) stable)
                              results)))
                  (delete-directory directory t))
                results)"#
        ),
        "((t t t t t) (t t t t t))"
    );
}

#[test]
fn arbitrary_precision_arithmetic_survives_native_calls() {
    if !avail() {
        return;
    }
    assert_eq!(
        ev("(let* ((n (expt 2 130)) \
                   (f (native-compile (lambda (n) (list (1+ n) (- n) (* n n)))))) \
             (list (native-comp-function-p f) (mapcar #'number-to-string (funcall f n))))"),
        "(t (\"1361129467683753853853498429727072845825\" \"-1361129467683753853853498429727072845824\" \"1852673427797059126777135760139006525652319754650249024631321344126610074238976\"))"
    );
}
