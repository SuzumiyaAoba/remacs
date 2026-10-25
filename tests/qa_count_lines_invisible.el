;;; -*- lexical-binding: nil; -*-
;; count-lines ignores lines hidden by text properties or overlays (#560).
(qa (with-temp-buffer
      (insert "a\nb\nc\n")
      (put-text-property 3 5 'invisible t)
      (let ((buffer-invisibility-spec t))
        (list (count-lines 1 (point-max))
              (count-lines 1 (point-max) t))))
    '(3 2))

(qa (with-temp-buffer
      (insert "a\nb\nc\n")
      (let ((buffer-invisibility-spec t)
            (overlay (make-overlay 3 5)))
        (overlay-put overlay 'invisible t)
        (count-lines 1 (point-max) t)))
    2)

(qa (with-temp-buffer
      (insert "a\nb\nc\n")
      (put-text-property 3 5 'invisible 'folded)
      (let ((buffer-invisibility-spec '(folded)))
        (count-lines 1 (point-max) t)))
    2)

(qa (with-temp-buffer
      (insert "a\nb\nc\n")
      (put-text-property 3 5 'invisible t)
      (let ((buffer-invisibility-spec nil))
        (count-lines 1 (point-max) t)))
    3)
(princ (qa--done))
