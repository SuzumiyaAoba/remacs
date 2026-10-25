;;; -*- lexical-binding: nil; -*-
;; Recursive copy-tree calls use the live function cell (#534).
(let ((seen nil)
      (probe (lambda (tree &optional _vecp) (push tree seen))))
  (advice-add 'copy-tree :before probe '((name . remacs-copy-tree-probe)))
  (unwind-protect
      (progn
        (copy-tree '((a) (b)))
        (qa seen '((b) (a) ((a) (b)))))
    (advice-remove 'copy-tree probe)))

;; A callback's cdr mutation is observed before the next spine step.
(let* ((tree (list '(a) '(b)))
       (replacement '(c))
       (mutator (lambda (part &optional _vecp)
                  (when (eq part (car tree))
                    (setcdr tree (list replacement)))))
       copied)
  (advice-add 'copy-tree :before mutator '((name . remacs-copy-tree-mutator)))
  (unwind-protect
      (progn
        (setq copied (copy-tree tree))
        (qa (list copied tree) '(((a) (c)) ((a) (c)))))
    (advice-remove 'copy-tree mutator)))
(princ (qa--done))
