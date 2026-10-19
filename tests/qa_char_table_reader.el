;;; -*- lexical-binding: nil; -*-
;; Issue #15: reject malformed trie literals before printing or use.
(defun qa-sub-table-source (depth minimum count)
  (concat "#^^[" (prin1-to-string depth) " " (prin1-to-string minimum)
          (apply #'concat (make-list count " nil")) "]"))

(qa-err-data (read-from-string "#^^[]") 'error
             '("Invalid size of sub-char-table"))
(qa-err-data (read-from-string "#^^[a]") 'error
             '("Invalid size of sub-char-table"))
(dolist (depth '(0 4 nil a 1.0))
  (qa-err-data (read (qa-sub-table-source depth 0 0)) 'error
               '("Invalid depth in sub-char-table")))
(dolist (spec '((1 16) (2 32) (3 128)))
  (let ((depth (car spec)) (count (cadr spec)))
    (dolist (length (list 0 (1- count) (1+ count)))
      (qa-err-data (read (qa-sub-table-source depth 0 length)) 'error
                   '("Invalid size in sub-char-table")))
    (dolist (minimum '(-1 4194304 nil a 1.0))
      (qa-err-data (read (qa-sub-table-source depth minimum count)) 'error
                   '("Invalid minimum character in sub-char-table")))
    (dolist (minimum '(0 1 128 4194303))
      (let ((source (qa-sub-table-source depth minimum count)))
        (qa (prin1-to-string (read source)) source)
        (qa (cdr (read-from-string source)) (length source))))))

;; Error ordering: depth, then the slot count, then minimum character.
(qa-err-data (read "#^^[4 nil]") 'error
             '("Invalid depth in sub-char-table"))
(qa-err-data (read "#^^[3 nil]") 'error
             '("Invalid size in sub-char-table"))
(dolist (source '("#^[]" "#^[a]" "#^[nil nil nil]"))
  (qa-err-data (read source) 'invalid-read-syntax
               '("Invalid size char-table")))
(let ((source (concat "#^[nil nil nil"
                      (apply #'concat (make-list 64 " nil")) "]")))
  (qa-err-data (read source) 'invalid-read-syntax
               '("Invalid size char-table")))
;; Valid tables, extras and nested sub-tables still round-trip.
(dolist (extras '(0 1 10))
  (let* ((source (concat "#^[nil nil nil"
                         (apply #'concat (make-list (+ 65 extras) " nil")) "]"))
         (table (read source)))
    (qa (char-table-p table) t)
    (qa (aref table 1000) nil)
    (qa (prin1-to-string table) source)))
(let ((table (make-char-table nil)))
  (aset table 300 42)
  (aset table 10000 99)
  (let ((copy (read (prin1-to-string table))))
    (qa (mapcar (lambda (c) (aref copy c)) '(0 299 300 301 10000))
        '(nil nil 42 nil 99))))
;; User-created short records cannot reach the printer's trie slice.
(qa (prin1-to-string (record 'sub-char-table)) "#s(sub-char-table)")
(qa (prin1-to-string (record 'sub-char-table 1)) "#s(sub-char-table 1)")
(princ (qa--done))
