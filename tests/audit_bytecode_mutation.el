;;; audit_bytecode_mutation.el --- byte-code slot mutation and caching -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)

(audit-check "bytecode:constants-in-place-control"
 '(let* ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1))
         (before (funcall f)))
    (aset (aref f 2) 0 33)
    (list before (funcall f))))

(audit-check "bytecode:constants-slot-control"
 '(let* ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1))
         (before (funcall f)))
    (let ((result (condition-case err (progn (aset f 2 (vector 33)) 'stored)
                    (error (list (car err) (cadr err))))))
      (list before result (funcall f)))))

(audit-check "bytecode:code-slot-control"
 '(let* ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1))
         (before (funcall f)))
    (let ((result (condition-case err (progn (aset f 1 (unibyte-string 193 135)) 'stored)
                    (error (list (car err) (cadr err))))))
      (list before result (funcall f)))))

(audit-check "bytecode:code-in-place"
 '(let* ((f (make-byte-code 0 (unibyte-string 192 135) (vector 11 22) 1))
         (before (funcall f)))
    (aset (aref f 1) 0 193)
    (list before (funcall f))))

(princ "AUDIT-DONE\n")
