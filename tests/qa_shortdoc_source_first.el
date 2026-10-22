;;; qa_shortdoc_source_first.el --- Source entry can use the compiled data dependency -*- lexical-binding: t; -*-
(qa (load "shortdoc-doc.el" nil t) t)
(qa (list (featurep 'shortdoc) (featurep 'shortdoc-doc)
          (length shortdoc--groups)
          (autoloadp (symbol-function 'define-short-documentation-group)))
    '(t t 18 nil))
(qa (require 'shortdoc-doc) 'shortdoc-doc)
(qa (progn (load "shortdoc-doc.el" nil t) (length shortdoc--groups)) 18)
(princ (qa--done))
