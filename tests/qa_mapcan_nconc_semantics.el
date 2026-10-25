;;; -*- lexical-binding: nil; -*-
;; mapcan shares nconc's circular-list error and mutation order (#536).
(qa (let ((cycle (list 'a)))
      (setcdr cycle cycle)
      (condition-case e (mapcan #'identity (list cycle nil))
        (error (car e))))
    'circular-list)

(qa (let ((prefix nil)
          (seen nil))
      (condition-case nil
          (mapcan (lambda (n)
                    (push n seen)
                    (pcase n
                      (1 (setq prefix (list 'before)) prefix)
                      (2 42)
                      (_ '(later))))
                  '(1 2 3))
        (error (list prefix (nreverse seen)))))
    '((before . 42) (1 2 3)))

;; The final mapcan result is nconc's unvalidated tail.
(qa (mapcan (lambda (n) (if (= n 1) '(a) 'tail)) '(1 2))
    '(a . tail))
(princ (qa--done))
