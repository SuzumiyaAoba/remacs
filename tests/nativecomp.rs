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
    if !avail() {
        return;
    }
    assert_eq!(ev("(featurep 'native-comp)"), "t");
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
             (list (funcall f 2 5) (native-comp-function-p f) (subrp f)))"
        ),
        "((2 5 7) t t)"
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
    // comp--subr-signature matches GNU: (NAME MIN . MAX).
    assert_eq!(ev("(comp--subr-signature 'car)"), "(car 1 . 1)");
    assert_eq!(ev("(comp--subr-signature 'apply)"), "(apply 1 . many)");
}

// ---------------------------------------------------------------
// error paths
// ---------------------------------------------------------------

#[test]
fn error_paths() {
    if !avail() {
        return;
    }
    // Missing source file.
    assert_eq!(
        ev_err("(native-compile \"/nonexistent-dir-zz/nope.el\")"),
        "error"
    );
    // Loading a non-.eln file errors cleanly.
    let p = scratch("noteln.txt", "hello");
    assert_eq!(ev_err(&format!("(native-elisp-load {p:?})")), "error");
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
