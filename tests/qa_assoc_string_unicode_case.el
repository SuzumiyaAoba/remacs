;;; -*- lexical-binding: nil; -*-
;; assoc-string uses the live Unicode case table and stored string forms (#530).
(qa (assoc-string "Σ" (list (cons "ς" 'value)) t) '("ς" . value))
(qa (assoc-string 'Σ (list (cons 'ς 'symbol-value)) t)
    '(ς . symbol-value))
(qa (assoc-string "Σ" (list (cons "ς" 'value)) nil) nil)
(qa (let ((table (copy-case-table (standard-case-table))))
      (set-case-syntax-pair ?A ?b table)
      (with-temp-buffer
        (set-case-table table)
        (assoc-string "A" (list (cons "b" 'custom)) t)))
    '("b" . custom))
(qa (assoc-string (unibyte-string 233)
                  (list (cons (string #x3fffe9) 'byte8)) nil)
    nil)
(princ (qa--done))
