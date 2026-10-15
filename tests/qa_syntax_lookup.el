;;; -*- lexical-binding: nil; -*-
;; Syntax lookup uses full Emacs character codes and the active table.
(with-syntax-table (standard-syntax-table)
  (dolist (c '(55296 57343 1114111 2097151 4194176 4194303))
    (qa (char-syntax c) ?w)
    (qa (matching-paren c) nil))
  (qa (char-syntax 1114112) ?_))
(dolist (c '(?x 233 55296 1114112 2097151 4194176 4194303))
  (let ((st (copy-syntax-table)))
    (modify-syntax-entry c "(y" st)
    (with-syntax-table st
      (qa (char-syntax c) ?\()
      (qa (matching-paren c) ?y))
    (modify-syntax-entry c ")x" st)
    (with-syntax-table st
      (qa (char-syntax c) ?\))
      (qa (matching-paren c) ?x))
    (modify-syntax-entry c "wz" st)
    (with-syntax-table st
      (qa (char-syntax c) ?w)
      (qa (matching-paren c) nil))))
;; The literal character does not override its current syntax entry.
(let ((st (copy-syntax-table)))
  (modify-syntax-entry ?\( "." st)
  (modify-syntax-entry ?\) "(x" st)
  (modify-syntax-entry ?\[ "(" st)
  (with-syntax-table st
    (qa (matching-paren ?\() nil)
    (qa (matching-paren ?\)) ?x)
    (qa (matching-paren ?\[) nil)))
;; Parent entries, nil children, defaults, and absent entries.
(let* ((parent (make-syntax-table)) (child (make-syntax-table parent)))
  (set-char-table-range parent 4194303 '(4 . 1114112))
  (with-syntax-table child
    (qa (char-syntax 4194303) ?\()
    (qa (matching-paren 4194303) 1114112))
  (set-char-table-range child 4194303 '(5 . 55296))
  (with-syntax-table child (qa (matching-paren 4194303) 55296))
  (set-char-table-range child 4194303 nil)
  (with-syntax-table child (qa (matching-paren 4194303) 1114112))
  (set-char-table-parent child nil)
  (with-syntax-table child
    (qa (char-syntax 4194303) ?\s)
    (qa (matching-paren 4194303) nil))
  (set-char-table-range child t '(5 . 97))
  (with-syntax-table child
    (qa (char-syntax 4194303) ?\))
    (qa (matching-paren 4194303) 97)))
;; High flag bits do not change the class; matching cdr is returned as-is.
(dolist (entry '((4 . 4194303) (260 . 94) (65540 . 1114112) (131077 . 55296)
                (4) (5 . arbitrary) (4 . (a b))))
  (let ((st (make-syntax-table)))
    (set-char-table-range st ?x entry)
    (with-syntax-table st
      (qa (char-syntax ?x) (if (= (logand (car entry) 255) 4) ?\( ?\)))
      (qa (matching-paren ?x) (cdr entry)))))
(let ((st (make-syntax-table)))
  (set-char-table-range st ?x t)
  (with-syntax-table st
    (qa (char-syntax ?x) ?\s)
    (qa (matching-paren ?x) nil))
  (set-char-table-range st ?x '(20 . 94))
  (with-syntax-table st (qa (matching-paren ?x) nil))
  (set-char-table-range st ?x '(13 . 94))
  (with-syntax-table st
    (qa (char-syntax ?x) ?@)
    (qa (matching-paren ?x) nil)))
;; Unibyte char-syntax maps high bytes to raw-byte character codes.
;; matching-paren intentionally uses the supplied code without conversion.
(with-temp-buffer
  (set-buffer-multibyte nil)
  (let ((st (make-syntax-table)))
    (dolist (c '(128 233 255))
      (modify-syntax-entry c "(x" st)
      (modify-syntax-entry (+ c #x3fff00) ")y" st)
      (with-syntax-table st
        (qa (char-syntax c) ?\))
        (qa (matching-paren c) ?x)))
    (modify-syntax-entry ?a "(z" st)
    (with-syntax-table st
      (qa (char-syntax ?a) ?\()
      (qa (matching-paren ?a) ?z))))
;; Text properties are deliberately irrelevant to these character APIs.
(with-temp-buffer
  (insert "x")
  (put-text-property 1 2 'syntax-table '(4 . 121))
  (let ((parse-sexp-lookup-properties t))
    (qa (char-syntax ?x) ?w)
    (qa (matching-paren ?x) nil)))
(dolist (bad (list -1 4194304 1.5 nil t "x" '(1) [1] (make-marker)
                  (expt 2 80)))
  (qa-err-data (char-syntax bad) 'wrong-type-argument (list 'characterp bad))
  (qa-err-data (matching-paren bad) 'wrong-type-argument (list 'characterp bad)))
(princ (qa--done)) (terpri)
