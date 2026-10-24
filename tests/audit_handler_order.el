;;; -*- lexical-binding: t; -*-
(load (expand-file-name "audit_helpers.el" (file-name-directory load-file-name)) nil t)
(audit-check "handler-order:outer-steals-caught-error" '(catch 'out (handler-bind ((error (lambda (_) (throw 'out 'stolen))))
  (condition-case nil (error "audit") (error 'caught)))))
(audit-check "handler-order:inner-observer" '(let ((seen nil)) (list (condition-case nil
  (handler-bind ((error (lambda (_) (push 'observed seen)))) (error "audit"))
  (error 'caught)) seen)))
(audit-check "handler-order:unmatched-inner-condition" '(let ((seen nil)) (list (condition-case nil
  (handler-bind ((error (lambda (_) (push 'observed seen))))
    (condition-case nil (error "audit") (arith-error 'wrong-handler)))
  (error 'caught)) seen)))
(audit-check "handler-order:resignal" '(let ((seen nil)) (list (condition-case nil
  (handler-bind ((error (lambda (_) (push 'observed seen))))
    (condition-case err (error "audit") (error (signal (car err) (cdr err)))))
  (error 'caught)) seen)))
(princ "AUDIT-DONE\n")
