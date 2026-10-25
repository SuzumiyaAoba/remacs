;;; -*- lexical-binding: nil; -*-
;; recordp excludes bool-vectors and char-tables (#469).
(qa (mapcar #'recordp (list (bool-vector t)
                            (make-char-table 'test)
                            (record 'test)))
    '(nil nil t))
(princ (qa--done))
