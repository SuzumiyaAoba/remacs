;;; -*- lexical-binding: nil; -*-
;; copy-keymap copies long sparse binding spines without a depth cutoff (#520).
(qa (let ((map (make-sparse-keymap)))
      (dotimes (n 300)
        (define-key map (vector (intern (format "key%d" n))) 'original))
      (let ((copy (copy-keymap map)))
        (define-key copy [key0] 'changed)
        (list (lookup-key copy [key0]) (lookup-key map [key0]))))
    '(changed original))
(princ (qa--done))
