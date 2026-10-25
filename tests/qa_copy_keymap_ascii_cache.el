;;; -*- lexical-binding: nil; -*-
;; Range updates keep copied ASCII cache and trie bindings coherent (#518).
(qa (let* ((map (make-keymap))
           (prefix (make-sparse-keymap)))
      (define-key prefix "x" 'original)
      (define-key map "p" prefix)
      (let ((copy (copy-keymap map)))
        (define-key copy "py" 'extra)
        (define-key copy (vector (cons 65 90)) 'upper)
        (list (lookup-key copy "py")
              (lookup-key map "py"))))
    '(extra nil))
(princ (qa--done))
