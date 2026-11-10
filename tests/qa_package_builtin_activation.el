;;; qa_package_builtin_activation.el --- Built-in package activation regression -*- lexical-binding:t -*-

(require 'package-activate)

(qa
 (let ((package-activated-list nil))
   (let ((result (package-activate 'seq)))
     (list (and result t)
           (and (memq 'seq package-activated-list) t))))
 '(t t))

(princ (qa--done))
