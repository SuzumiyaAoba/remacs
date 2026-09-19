//! Core evaluator semantics + prelude macros.

mod common;
use common::{ev, ev_err, ev_out, ev_result};

#[test]
fn arith() {
    assert_eq!(ev("(+ 1 2 3)"), "6");
    assert_eq!(ev("(- 10 4)"), "6");
    assert_eq!(ev("(* 6 7)"), "42");
    assert_eq!(ev("(/ 7 2)"), "3");
    assert_eq!(ev("(/ 7.0 2)"), "3.5");
    assert_eq!(ev("(% 7 3)"), "1");
    assert_eq!(ev("(1+ 5)"), "6");
    assert_eq!(ev("(1- 5)"), "4");
}

#[test]
fn float_fns() {
    assert_eq!(ev("(frexp 8.0)"), "(0.5 . 4)");
    assert_eq!(ev("(ldexp 0.5 4)"), "8.0");
    assert_eq!(ev("(ffloor 3.7)"), "3.0");
    assert_eq!(ev("(fceiling 3.2)"), "4.0");
    assert_eq!(ev("(ftruncate -3.7)"), "-3.0");
    assert_eq!(ev("(fround 2.5)"), "2.0");
    assert_eq!(ev("(fround 3.5)"), "4.0");
    assert_eq!(ev("(copysign 3.0 -1.5)"), "-3.0");
    assert_eq!(ev_err("(copysign 3 -1)"), "wrong-type-argument");
    assert_eq!(ev("(logb 8)"), "3");
    assert_eq!(ev("(isnan (/ 0.0 0.0))"), "t");
    assert_eq!(ev("(logcount 13)"), "3");
    assert_eq!(ev("(logcount -13)"), "2");
}

#[test]
fn eval_forms() {
    assert_eq!(ev("(let ((a 1) (b 2)) (+ a b))"), "3");
    assert_eq!(ev("(let* ((a 1) (b a)) (+ a b))"), "2");
    assert_eq!(ev("(funcall (lambda (x) (* x x)) 9)"), "81");
    assert_eq!(ev("(progn (defun sq (x) (* x x)) (sq 4))"), "16");
    assert_eq!(ev("(cond (nil 1) (t 'yes))"), "yes");
    assert_eq!(ev("(and 1 2 3)"), "3");
    assert_eq!(ev("(or nil nil 5)"), "5");
    assert_eq!(ev("(if nil 1 2)"), "2");
    assert_eq!(ev("(progn 1 2 3)"), "3");
    assert_eq!(ev("(prog1 1 2 3)"), "1");
    assert_eq!(ev("(prog2 1 2 3)"), "2");
}

#[test]
fn catch_throw() {
    assert_eq!(ev("(catch 'tag (throw 'tag 42) 99)"), "42");
    assert_eq!(ev("(catch 'tag 1 2 3)"), "3");
}

#[test]
fn condition_case() {
    assert_eq!(
        ev("(condition-case e (car 5) (error 'caught))"),
        "caught"
    );
    assert_eq!(
        ev("(condition-case e (car 5) (wrong-type-argument (car e)))"),
        "wrong-type-argument"
    );
    assert_eq!(
        ev("(condition-case e (/ 1 0) (arith-error 'div))"),
        "div"
    );
    // Handler gets the error data.
    assert_eq!(
        ev("(condition-case e (car 5) (error (cdr e)))"),
        "(listp 5)"
    );
    // unwind-protect runs cleanup on signal.
    assert_eq!(
        ev("(let ((x 0))
             (condition-case nil
                 (unwind-protect (car 5) (setq x 99))
               (error nil))
             x)"),
        "99"
    );
}

#[test]
fn macros() {
    assert_eq!(
        ev("(progn (defmacro m (x) (list 'quote x)) (m hello))"),
        "hello"
    );
    assert_eq!(
        ev("(macroexpand '(when a b))"),
        "(if a (progn b))"
    );
}

#[test]
fn prelude_macros() {
    assert_eq!(ev("(when t 42)"), "42");
    assert_eq!(ev("(when nil 42)"), "nil");
    assert_eq!(ev("(unless t 1)"), "nil");
    assert_eq!(ev("(unless nil 1)"), "1");
    assert_eq!(
        ev_out("(dolist (x '(a b c)) (princ x))"),
        "abc"
    );
    assert_eq!(
        ev("(let ((r nil)) (dolist (x '(1 2 3) r) (push x r)))"),
        "(3 2 1)"
    );
    assert_eq!(ev("(dotimes (k 3 k))"), "3");
    assert_eq!(
        ev_out("(dotimes (k 3) (princ k))"),
        "012"
    );
    assert_eq!(
        ev("(with-temp-buffer (insert \"hi\") (buffer-string))"),
        "\"hi\""
    );
    assert_eq!(
        ev("(with-output-to-string (princ \"a\") (princ 1))"),
        "\"a1\""
    );
    assert_eq!(ev("(ignore-errors (car 5))"), "nil");
    assert_eq!(ev("(ignore-errors (+ 1 2))"), "3");
    // dolist result form.
    assert_eq!(
        ev("(dolist (x '(1 2 3) 'done))"),
        "done"
    );
    // dotimes var visible during loop.
    assert_eq!(
        ev("(let ((s 0)) (dotimes (k 4 s) (setq s (+ s k))))"),
        "6"
    );
}

#[test]
fn errors() {
    assert_eq!(ev_err("(car 5)"), "wrong-type-argument");
    assert_eq!(ev_err("(/ 1 0)"), "arith-error");
    assert_eq!(ev_err("(undefined-fn)"), "void-function");
    assert_eq!(ev_err("undefined-var"), "void-variable");
    assert_eq!(ev_err("(signal 'my-err '(1 2))"), "my-err");
    assert_eq!(ev_err("(throw 'notag 1)"), "no-catch");
    assert_eq!(ev_err("(setq t 1)"), "setting-constant");
    assert_eq!(ev_err("(make-vector -1 0)"), "args-out-of-range");
}

#[test]
fn dynamic_scope() {
    // defvar marks special → dynamic scope.
    assert_eq!(
        ev("(progn
             (defvar *dyn* 1)
             (defun get-dyn () *dyn*)
             (let ((*dyn* 2)) (get-dyn)))"),
        "2"
    );
    // let-bound non-special is also dynamically visible (Emacs dynamic default).
    assert_eq!(
        ev("(progn
             (defun get-x () x)
             (let ((x 7)) (get-x)))"),
        "7"
    );
}

#[test]
fn closures_lexical() {
    // With lexical-binding, closures capture.
    assert_eq!(
        ev("(progn
             (setq lexical-binding t)
             (funcall (let ((n 5)) (lambda () n))))"),
        "5"
    );
}

#[test]
fn string_ops() {
    assert_eq!(ev("(concat \"a\" \"b\" \"c\")"), "\"abc\"");
    assert_eq!(ev("(substring \"hello\" 1 3)"), "\"el\"");
    assert_eq!(ev("(length \"hello\")"), "5");
    assert_eq!(ev("(upcase \"abc\")"), "\"ABC\"");
    assert_eq!(ev("(downcase \"ABC\")"), "\"abc\"");
    assert_eq!(ev("(capitalize \"hello world\")"), "\"Hello World\"");
    assert_eq!(ev("(string= \"a\" \"a\")"), "t");
    assert_eq!(ev("(string< \"a\" \"b\")"), "t");
    assert_eq!(ev("(aref \"abc\" 1)"), "98");
    assert_eq!(ev("(elt \"abc\" 0)"), "97");
}

#[test]
fn list_ops() {
    assert_eq!(ev("(car '(1 2 3))"), "1");
    assert_eq!(ev("(cdr '(1 2 3))"), "(2 3)");
    assert_eq!(ev("(cadr '(1 2 3))"), "2");
    assert_eq!(ev("(caddr '(1 2 3))"), "3");
    assert_eq!(ev("(nth 1 '(a b c))"), "b");
    assert_eq!(ev("(nthcdr 2 '(a b c d))"), "(c d)");
    assert_eq!(ev("(length '(1 2 3))"), "3");
    assert_eq!(ev("(append '(1 2) '(3 4))"), "(1 2 3 4)");
    assert_eq!(ev("(reverse '(1 2 3))"), "(3 2 1)");
    assert_eq!(ev("(member 2 '(1 2 3))"), "(2 3)");
    assert_eq!(ev("(memq 'b '(a b c))"), "(b c)");
    assert_eq!(ev("(assq 'b '((a . 1) (b . 2)))"), "(b . 2)");
    assert_eq!(ev("(delq 'b '(a b c b))"), "(a c)");
    assert_eq!(ev("(mapcar #'1+ '(1 2 3))"), "(2 3 4)");
    assert_eq!(
        ev("(mapcan #'list '(1 2) '(3 4))"),
        "(1 3 2 4)"
    );
    assert_eq!(ev("(last '(1 2 3))"), "(3)");
    assert_eq!(ev("(butlast '(1 2 3))"), "(1 2)");
    assert_eq!(ev("(number-sequence 1 5)"), "(1 2 3 4 5)");
    assert_eq!(ev("(copy-sequence '(1 2))"), "(1 2)");
}

#[test]
fn vector_ops() {
    assert_eq!(ev("(make-vector 3 0)"), "[0 0 0]");
    assert_eq!(ev("(vector 1 2 3)"), "[1 2 3]");
    assert_eq!(ev("(length [1 2 3])"), "3");
    assert_eq!(ev("(aref [1 2 3] 1)"), "2");
    assert_eq!(
        ev("(let ((v (vector 1 2))) (aset v 0 9) v)"),
        "[9 2]"
    );
    assert_eq!(ev("(vconcat [1] [2 3])"), "[1 2 3]");
    assert_eq!(ev("(append [1 2] nil)"), "(1 2)");
}

#[test]
fn hash_tables() {
    assert_eq!(
        ev("(let ((h (make-hash-table)))
              (puthash 'a 1 h)
              (puthash 'b 2 h)
              (list (gethash 'a h) (gethash 'b h) (hash-table-count h)))"),
        "(1 2 2)"
    );
    assert_eq!(
        ev("(let ((h (make-hash-table :test 'equal)))
              (puthash \"k\" 7 h)
              (gethash \"k\" h))"),
        "7"
    );
    assert_eq!(
        ev("(let ((h (make-hash-table)))
              (puthash 'a 1 h)
              (remhash 'a h)
              (gethash 'a h 'none))"),
        "none"
    );
    assert_eq!(
        ev("(let ((h (make-hash-table)) (ks nil))
              (puthash 'a 1 h)
              (puthash 'b 2 h)
              (maphash (lambda (k v) (push k ks)) h)
              (sort (mapcar #'symbol-name ks) #'string<))"),
        "(\"a\" \"b\")"
    );
}

#[test]
fn read_print() {
    assert_eq!(ev("(prin1-to-string '(1 \"a\" b))"), "\"(1 \\\"a\\\" b)\"");
    assert_eq!(ev("(princ-to-string \"a\")"), "\"a\"");
    assert_eq!(ev("(read-from-string \"(1 2)\")"), "((1 2) . 5)");
    assert_eq!(ev("(car (read-from-string \"42\"))"), "42");
    assert_eq!(ev("(intern \"foo\")"), "foo");
    assert_eq!(ev("(intern-soft \"car\")"), "car");
    assert_eq!(ev("(symbol-name 'abc)"), "\"abc\"");
    // print-gensym defaults to nil in Emacs: no `#:' prefix.
    assert_eq!(ev("(make-symbol \"g\")"), "g");
    assert_eq!(ev("(intern \"#:g\")"), "#:g");
}

#[test]
fn type_predicates() {
    assert_eq!(ev("(listp '(1))"), "t");
    assert_eq!(ev("(listp nil)"), "t");
    assert_eq!(ev("(listp 5)"), "nil");
    assert_eq!(ev("(consp '(1))"), "t");
    assert_eq!(ev("(atom 5)"), "t");
    assert_eq!(ev("(atom '(1))"), "nil");
    assert_eq!(ev("(numberp 5)"), "t");
    assert_eq!(ev("(integerp 5)"), "t");
    assert_eq!(ev("(integerp 5.0)"), "nil");
    assert_eq!(ev("(floatp 5.0)"), "t");
    assert_eq!(ev("(stringp \"s\")"), "t");
    assert_eq!(ev("(symbolp 's)"), "t");
    assert_eq!(ev("(vectorp [1])"), "t");
    assert_eq!(ev("(functionp #'car)"), "t");
    assert_eq!(ev("(subrp #'car)"), "t");
    assert_eq!(ev("(bufferp (current-buffer))"), "t");
    assert_eq!(ev("(markerp (make-marker))"), "t");
    assert_eq!(ev("(wholenump 5)"), "t");
    assert_eq!(ev("(wholenump -5)"), "nil");
    assert_eq!(ev("(zerop 0)"), "t");
}

#[test]
fn equality() {
    assert_eq!(ev("(eq 'a 'a)"), "t");
    assert_eq!(ev("(eq \"a\" \"a\")"), "nil");
    assert_eq!(ev("(equal \"a\" \"a\")"), "t");
    assert_eq!(ev("(equal '(1 2) '(1 2))"), "t");
    assert_eq!(ev("(eql 1 1)"), "t");
    assert_eq!(ev("(= 1 1.0)"), "t");
    assert_eq!(ev("(eq 1 1.0)"), "nil");
}

#[test]
fn sequences() {
    assert_eq!(ev("(seqp '(1))"), "t");
    assert_eq!(ev("(seqp [1])"), "t");
    assert_eq!(ev("(seqp \"s\")"), "t");
    assert_eq!(ev("(seq-elt [1 2 3] 0)"), "1");
    assert_eq!(ev("(seq-length \"abc\")"), "3");
    assert_eq!(ev("(seq-take '(1 2 3 4) 2)"), "(1 2)");
    assert_eq!(ev("(seq-drop '(1 2 3 4) 2)"), "(3 4)");
    assert_eq!(
        ev("(seq-map #'1+ '(1 2 3))"),
        "(2 3 4)"
    );
    assert_eq!(
        ev("(seq-filter (lambda (n) (= 1 (% n 2))) '(1 2 3 4))"),
        "(1 3)"
    );
}

#[test]
fn apply_funcall() {
    assert_eq!(ev("(apply #'+ '(1 2 3))"), "6");
    assert_eq!(ev("(apply #'+ 1 '(2 3))"), "6");
    assert_eq!(ev("(funcall #'* 2 3)"), "6");
    assert_eq!(ev("(funcall-interactively #'car '(1))"), "1");
}

#[test]
fn format() {
    assert_eq!(ev("(format \"%s\" \"x\")"), "\"x\"");
    assert_eq!(ev("(format \"%S\" \"x\")"), "\"\\\"x\\\"\"");
    assert_eq!(ev("(format \"%d %d\" 1 2)"), "\"1 2\"");
    assert_eq!(ev("(format \"%05d\" 42)"), "\"00042\"");
    assert_eq!(ev("(format \"%c\" 65)"), "\"A\"");
    assert_eq!(ev("(format \"%o\" 8)"), "\"10\"");
    assert_eq!(ev("(format \"%x\" 255)"), "\"ff\"");
    assert_eq!(ev("(format \"%e\" 1000)"), "\"1.000000e+03\"");
    assert_eq!(ev("(format \"%%\")"), "\"%\"");
}

#[test]
fn save_excursion() {
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"abc\")
              (goto-char (point-min))
              (save-excursion (goto-char (point-max)))
              (point))"),
        "1"
    );
}

#[test]
fn markers() {
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"abc\")
              (let ((m (make-marker)))
                (set-marker m 2)
                (marker-position m)))"),
        "2"
    );
    // Markers track insertions.
    assert_eq!(
        ev("(with-temp-buffer
              (insert \"abc\")
              (let ((m (copy-marker 2)))
                (goto-char 1)
                (insert \"X\")
                (marker-position m)))"),
        "3"
    );
}

#[test]
fn undo_basic() {
    assert_eq!(
        ev_out("(with-temp-buffer
                  (insert \"abc\")
                  (undo)
                  (princ (buffer-string)))"),
        ""
    );
}

#[test]
fn buffer_local() {
    assert_eq!(
        ev("(with-temp-buffer
              (setq-local foo 42)
              foo)"),
        "42"
    );
}
