;;; qa_m2_empty_type_section.el --- Empty Modula-2 section indentation regression -*- lexical-binding:t -*-

(require 'modula2)

(defun qa-m2-line-indentation (line)
  (goto-char (point-min))
  (forward-line (1- line))
  (current-indentation))

(qa
 (with-temp-buffer
   (m2-mode)
   (insert "MODULE test;\nTYPE\nVAR\nx : INTEGER;\nBEGIN\nEND test.\n")
   (indent-region (point-min) (point-max))
   (list (qa-m2-line-indentation 2)
         (qa-m2-line-indentation 3)
         (qa-m2-line-indentation 4)))
 '(0 0 5))

(princ (qa--done))
