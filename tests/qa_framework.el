;;; -*- lexical-binding: t -*-
;; Shared QA framework — load before every tests/qa_*.el file.
;; Provides `qa', `qa-err', `qa-same', `qa-xf', `qa--done'.
;;
;; `qa' asserts EQUAL between the form's value and EXPECTED.
;; `qa-err' asserts the form signals a specific error symbol.
;; `qa-same' asserts EQ.
;; `qa-xf' is expected-fail: silently notes divergence.
;; `qa--done' returns a summary string or signals 'qa-failures.

(defvar qa--count 0)
(defvar qa--fails nil)
(defvar qa--xfails nil)
(defvar qa--xpass nil)
;; t when running under remacs — detected via invocation-name.
(defvar qa--remacs
  (and (boundp 'invocation-name)
       (stringp invocation-name)
       (string-match-p "remacs" invocation-name)))

(defun qa--eval (form)
  (condition-case e (eval form) (error (list 'qa--err (car e) (cdr e)))))

(defun qa--eval-lex (form)
  (condition-case e (eval form t) (error (list 'qa--err (car e) (cdr e)))))

(defmacro qa-lex (form expected)
  "Like `qa' but evaluates FORM under lexical binding."
  (declare (indent 1))
  `(let ((qa--v (qa--eval-lex ',form))
         (qa--x ,expected))
     (setq qa--count (1+ qa--count))
     (unless (equal qa--v qa--x)
       (push (list ',form qa--x qa--v) qa--fails))))

(defmacro qa (form expected)
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form))
         (qa--x ,expected))
     (setq qa--count (1+ qa--count))
     (unless (equal qa--v qa--x)
       (push (list ',form qa--x qa--v) qa--fails))))

(defmacro qa-same (form expected)
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form))
         (qa--x ,expected))
     (setq qa--count (1+ qa--count))
     (unless (eq qa--v qa--x)
       (push (list ',form qa--x qa--v) qa--fails))))

(defmacro qa-err (form errsym)
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form)))
     (setq qa--count (1+ qa--count))
     (unless (and (consp qa--v)
                  (eq (car qa--v) 'qa--err)
                  (eq (car (cdr qa--v)) ,errsym))
       (push (list ',form ,errsym qa--v) qa--fails))))

(defmacro qa-xf (form expected &optional note)
  "Expected-fail: runs the check but treats failure as an xfail note."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form))
         (qa--x ,expected))
     (setq qa--count (1+ qa--count))
     (if (equal qa--v qa--x)
         (push (list ',form qa--x ,note) qa--xpass)
       (push (list ',form qa--x qa--v ,note) qa--xfails))))

;; GNU Fsignal rejects symbols with no `error-conditions' property
;; ("Invalid error symbol"), so give our internal signal a real one.
(put 'qa-failures 'error-conditions '(qa-failures error))

(defun qa--done ()
  (cond (qa--fails
         (let ((tail (reverse qa--fails)))
           (while tail
             (let ((f (car tail)))
               (princ (format "FAIL %S\n" f))
               (setq tail (cdr tail)))))
         (signal 'qa-failures (list qa--fails)))
        (t
         (format "%d checks passed%s%s"
                 qa--count
                 (if qa--xfails
                     (format " (%d xfail)" (length qa--xfails))
                   "")
                 (if qa--xpass
                     (format " [%d xpass]" (length qa--xpass))
                   "")))))

(defmacro qa-prin1 (form expected)
  "Assert (prin1-to-string (eval FORM)) equals EXPECTED string."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form))
         (qa--x ,expected))
     (setq qa--count (1+ qa--count))
     (unless (and (stringp qa--x)
                  (equal (condition-case e (prin1-to-string qa--v)
                           (error 'qa--prin1-err))
                         qa--x))
       (push (list ',form qa--x qa--v) qa--fails))))

(defmacro qa-xf-prin1 (form expected &optional note)
  "Expected-fail variant of `qa-prin1'."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form))
         (qa--x ,expected))
     (setq qa--count (1+ qa--count))
     (if (and (stringp qa--x)
              (equal (condition-case e (prin1-to-string qa--v)
                       (error 'qa--prin1-err))
                     qa--x))
         (push (list ',form qa--x ,note) qa--xpass)
       (push (list ',form qa--x qa--v ,note) qa--xfails))))

(defmacro qa-err-data (form errsym errdata)
  "Assert FORM signals error ERRSYM whose data equals ERRDATA."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form)))
     (setq qa--count (1+ qa--count))
     (unless (and (consp qa--v)
                  (eq (car qa--v) 'qa--err)
                  (eq (car (cdr qa--v)) ,errsym)
                  (equal (car (cdr (cdr qa--v))) ,errdata))
       (push (list ',form (list ,errsym ,errdata) qa--v) qa--fails))))

(defmacro qa-xf-err (form errsym &optional note)
  "Expected-fail variant of `qa-err': FORM should signal ERRSYM."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form)))
     (setq qa--count (1+ qa--count))
     (if (and (consp qa--v)
              (eq (car qa--v) 'qa--err)
              (eq (car (cdr qa--v)) ,errsym))
         (push (list ',form ,errsym ,note) qa--xpass)
       (push (list ',form ,errsym qa--v ,note) qa--xfails))))

(defmacro qa-xf-err-data (form errsym errdata &optional note)
  "Expected-fail variant of `qa-err-data'."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form)))
     (setq qa--count (1+ qa--count))
     (if (and (consp qa--v)
              (eq (car qa--v) 'qa--err)
              (eq (car (cdr qa--v)) ,errsym)
              (equal (car (cdr (cdr qa--v))) ,errdata))
         (push (list ',form (list ,errsym ,errdata) ,note) qa--xpass)
       (push (list ',form (list ,errsym ,errdata) qa--v ,note)
             qa--xfails))))

(defmacro qa-xf-same (form expected &optional note)
  "Expected-fail variant of `qa-same' (eq comparison)."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form))
         (qa--x ,expected))
     (setq qa--count (1+ qa--count))
     (if (eq qa--v qa--x)
         (push (list ',form qa--x ,note) qa--xpass)
       (push (list ',form qa--x qa--v ,note) qa--xfails))))

(defmacro qa-skip (form expected &optional note)
  "Soft check: failure records an xfail note instead of failing.
Use for features remacs is not expected to implement yet; GNU
still runs the check and reports divergence as xfail/xpass."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form))
         (qa--x ,expected))
     (setq qa--count (1+ qa--count))
     (if (equal qa--v qa--x)
         (push (list ',form qa--x ,note) qa--xpass)
       (push (list ',form qa--x qa--v ,note) qa--xfails))))

(defmacro qa-skip-err (form errsym &optional note)
  "Soft expected-error check: failure records an xfail note."
  (declare (indent 1))
  `(let ((qa--v (qa--eval ',form)))
     (setq qa--count (1+ qa--count))
     (if (and (consp qa--v)
              (eq (car qa--v) 'qa--err)
              (eq (car (cdr qa--v)) ,errsym))
         (push (list ',form ,errsym ,note) qa--xpass)
       (push (list ',form ,errsym qa--v ,note) qa--xfails))))

(provide 'qa-framework)
