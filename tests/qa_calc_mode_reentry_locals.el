;;; -*- lexical-binding: t; -*-
;; Reentering Calc mode preserves local settings without changing defaults (#1978).

(let* ((lisp-directory
        (expand-file-name "../lisp" (file-name-directory load-file-name)))
       (load-path (cons lisp-directory load-path)))
  (load (expand-file-name "calc.el" lisp-directory) nil nil t))

(qa
 (let ((calc-settings-file nil)
       (saved-angle-mode (default-toplevel-value 'calc-angle-mode)))
   (unwind-protect
       (progn
         (set-default-toplevel-value 'calc-angle-mode 'deg)
         (with-temp-buffer
           (calc-mode)
           (setq-local calc-angle-mode 'rad)
           (calc-mode)
           (let ((current-state
                  (list (default-value 'calc-angle-mode) calc-angle-mode)))
             (with-temp-buffer
               (calc-mode)
               (list current-state
                     (list (default-value 'calc-angle-mode) calc-angle-mode))))))
     (set-default-toplevel-value 'calc-angle-mode saved-angle-mode)))
 '((deg rad) (deg deg)))

(princ (qa--done))
