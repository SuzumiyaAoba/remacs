;;; -*- lexical-binding: nil; -*-
;; internal-delete-indirect-variable must preserve GNU's plist errors and
;; leave the malformed plist intact when clearing an alias.
(defvar qa-delete-indirect-base 42)

(let ((alias 'qa-delete-indirect-valid))
  (defvaralias alias 'qa-delete-indirect-base)
  (let ((plist (symbol-plist alias)))
    (internal-delete-indirect-variable alias)
    (qa (list (eq (indirect-variable alias) alias)
              (boundp alias)
              (eq plist (symbol-plist alias))
              (get alias 'variable-documentation))
        '(t nil t nil))))

(defun qa-delete-indirect-hide-doc (plist)
  ;; Mutate the plist in place so the C-level alias remains installed, and
  ;; locate the documentation key instead of assuming a fixed pair offset.
  (let ((cell plist))
    (while (consp cell)
      (when (eq (car cell) 'variable-documentation)
        (setcar cell 'qa-delete-indirect-other))
      (setq cell (cddr cell)))))

(let ((alias 'qa-delete-indirect-malformed))
  (defvaralias alias 'qa-delete-indirect-base)
  (let* ((plist (symbol-plist alias))
         (tail (last plist)))
    (qa-delete-indirect-hide-doc plist)
    (setcdr tail 'bar)
    (let ((err (condition-case err
                   (internal-delete-indirect-variable alias)
                 (error err))))
      (qa (list (car err) (cadr err) (eq (caddr err) plist)
                (eq (indirect-variable alias) alias)
                (eq (symbol-plist alias) plist)
                (boundp alias))
          '(wrong-type-argument plistp t t t t)))))

(let ((alias 'qa-delete-indirect-circular))
  (defvaralias alias 'qa-delete-indirect-base)
  (let* ((plist (symbol-plist alias))
         (tail (last plist)))
    (qa-delete-indirect-hide-doc plist)
    (setcdr tail plist)
    (unwind-protect
        (let ((err (condition-case err
                       (internal-delete-indirect-variable alias)
                     (error err))))
          (qa (list (car err) (eq (cadr err) plist)
                    (eq (indirect-variable alias) alias)
                    (eq (symbol-plist alias) plist)
                    (boundp alias))
              '(circular-list t t t t)))
      (setcdr tail nil))))

(princ (qa--done)) (terpri)
