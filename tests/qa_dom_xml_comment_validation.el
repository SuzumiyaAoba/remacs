;;; qa_dom_xml_comment_validation.el --- reject malformed XML comments -*- lexical-binding: t; -*-

(require 'dom)

(defun qa-dom--xml-comment-errors-p (text)
  (condition-case nil
      (with-temp-buffer
        (dom-print (list 'comment nil text) nil t)
        nil)
    (error t)))

(qa
 (list
  (qa-dom--xml-comment-errors-p "bad--comment")
  (qa-dom--xml-comment-errors-p "trailing-")
  (qa-dom--xml-comment-errors-p (concat "bad" (string 1)))
  (with-temp-buffer
    (dom-print '(comment nil "safe-comment") nil t)
    (buffer-string))
  (with-temp-buffer
    (dom-print '(comment nil "bad--comment") nil nil)
    (buffer-string)))
 '(t t t "<!--safe-comment-->" "<!--bad--comment-->"))

(princ (qa--done))
