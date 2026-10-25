;;; -*- lexical-binding: nil; -*-
;; Darwin random() preserves its distinct zero-seed initialization (#599).
(qa
 (let* ((expected '(1547800106193663275
                    337805899096409688
                    52
                    1209880335653950014))
        (sequence (lambda (seed)
                    (list (random seed) (random) (random 100) (random))))
        (empty-seed (funcall sequence ""))
        (xor-zero-seed
         (funcall sequence (apply #'unibyte-string (number-sequence 0 255))))
        (darwin-result (and (equal empty-seed expected)
                            (equal xor-zero-seed expected))))
   (if (eq system-type 'darwin)
       darwin-result
     (equal empty-seed xor-zero-seed)))
 t)
(princ (qa--done))
