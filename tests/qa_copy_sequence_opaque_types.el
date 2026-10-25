;;; -*- lexical-binding: nil; -*-
;; copy-sequence rejects opaque font-specs/obarrays but copies ordinary records (#478).
(dolist (object (list (font-spec) (obarray-make)))
  (qa (condition-case err (copy-sequence object)
        (error (list (car err) (cadr err))))
      '(wrong-type-argument sequencep)))
(let* ((record (record 'tag 1)) (copy (copy-sequence record)))
  (qa (list (recordp copy) (eq copy record) (equal copy record)) '(t nil t)))
(princ (qa--done))
