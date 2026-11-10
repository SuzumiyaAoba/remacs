;;; qa_elisp_scope_nested_loop.el --- Nested cl-loop binding regression -*- lexical-binding:t -*-

(require 'elisp-scope)

(defvar qa-elisp-scope-loop-bindings nil)

(defun qa-elisp-scope-loop-callback (role _pos symbol _id definition)
  (when (memq symbol '(a b c))
    (push (list role symbol (and definition t))
          qa-elisp-scope-loop-bindings)))

(qa
 (let ((qa-elisp-scope-loop-bindings nil))
   (with-temp-buffer
     (insert "(cl-loop for (a (b . c)) in '((1 (2 . 3))) collect (+ a b c))")
     (goto-char (point-min))
     (elisp-scope-analyze-form #'qa-elisp-scope-loop-callback))
   (and (member '(binding-variable a t) qa-elisp-scope-loop-bindings)
        (member '(binding-variable b t) qa-elisp-scope-loop-bindings)
        (member '(binding-variable c t) qa-elisp-scope-loop-bindings)
        (member '(bound-variable a t) qa-elisp-scope-loop-bindings)
        (member '(bound-variable b t) qa-elisp-scope-loop-bindings)
        (member '(bound-variable c t) qa-elisp-scope-loop-bindings)
        t))
 t)

(princ (qa--done))
