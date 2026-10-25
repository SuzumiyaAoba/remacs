;;; -*- lexical-binding: nil; -*-
;; Only runtime-created records are positioned symbols (#579).
(qa
 (let* ((symbols-with-pos-enabled nil)
        (ordinary (record 'symbol-with-pos 'alpha 1))
        (positioned (position-symbol 'alpha 1)))
   (list (recordp ordinary)
         (symbol-with-pos-p ordinary)
         (condition-case err (bare-symbol ordinary) (error (car err)))
         (recordp positioned)
         (symbol-with-pos-p positioned)
         (bare-symbol positioned)))
 '(t nil wrong-type-argument nil t alpha))
(qa
 (let ((symbols-with-pos-enabled t)
       (ordinary (record 'symbol-with-pos 'beta 2))
       (positioned (position-symbol 'beta 2)))
   (list (recordp ordinary) (symbol-with-pos-p ordinary)
         (recordp positioned) (symbolp positioned)))
 '(t nil nil t))
(princ (qa--done))
