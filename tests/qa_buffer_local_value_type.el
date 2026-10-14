;;; -*- lexical-binding: nil; -*-
;; Issue #69: BUFFER is mandatory, not an optional current-buffer default.
(dolist (value (list nil t 1 1.5 "*scratch*" [] (list 'a) (make-marker)))
  (qa-err-data (buffer-local-value 'fill-column value)
               'wrong-type-argument (list 'bufferp value)))
(qa-err-data (buffer-local-value 'fill-column (get-buffer "no-such-qa69-buffer"))
             'wrong-type-argument '(bufferp nil))
;; VARIABLE validation precedes BUFFER validation.
(dolist (variable '(1 1.5 "variable" [a] (a)))
  (qa-err-data (buffer-local-value variable nil)
               'wrong-type-argument (list 'symbolp variable)))
(dolist (variable '(nil t :keyword))
  (qa-err-data (buffer-local-value variable nil)
               'wrong-type-argument '(bufferp nil)))
(let ((one (generate-new-buffer " *qa69-one*"))
      (two (generate-new-buffer " *qa69-two*"))
      (variable (make-symbol "qa69-value"))
      (unbound (make-symbol "qa69-unbound")))
  (unwind-protect
      (progn
        (set variable 'default)
        (with-current-buffer one
          (set (make-local-variable variable) 'first))
        (with-current-buffer two
          (set (make-local-variable variable) 'second))
        (qa (buffer-local-value variable one) 'first)
        (qa (buffer-local-value variable two) 'second)
        (qa (buffer-local-value variable (current-buffer)) 'default)
        (qa (buffer-local-value nil one) nil)
        (qa (buffer-local-value t one) t)
        (qa (buffer-local-value :keyword one) :keyword)
        (qa-err-data (buffer-local-value unbound nil) 'wrong-type-argument '(bufferp nil))
        (qa-err-data (buffer-local-value unbound one) 'void-variable (list unbound))
        (let ((before (current-buffer)))
          (buffer-local-value variable two)
          (qa (eq before (current-buffer)) t)))
    (kill-buffer one)
    (kill-buffer two)))
;; CHECK_BUFFER accepts killed buffer objects; do not require liveness.
(let ((dead (generate-new-buffer " *qa69-dead*")))
  (kill-buffer dead)
  (qa (buffer-local-value t dead) t)
  (qa (buffer-local-value nil dead) nil))
(princ (qa--done))
