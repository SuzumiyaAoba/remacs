;;; qa_newsticker_extra_html_escape.el --- newsticker extra fields escaping regression -*- lexical-binding: t -*-

(require 'cl-lib)
(require 'newst-reader)

(defun qa-newsticker-extra-fragment (htmlish)
  (cl-letf (((symbol-function 'newsticker--extra)
             (lambda (_item)
               '((author nil "<img src=\"https://tracker.example/pixel\">")))))
    (with-temp-buffer
      (newsticker--print-extra-elements 'dummy nil htmlish)
      (buffer-string))))

(if qa--remacs
    (progn
      (qa (qa-newsticker-extra-fragment t)
          "<ul><li>author: &lt;img src=&quot;https://tracker.example/pixel&quot;&gt;</li></ul>")
      (qa (qa-newsticker-extra-fragment nil)
          "author: <img src=\"https://tracker.example/pixel\">\n"))
  ;; GNU 31.1 inserts the additional XML text as an HTML fragment.
  (qa-xf (qa-newsticker-extra-fragment t)
         "<ul><li>author: &lt;img src=&quot;https://tracker.example/pixel&quot;&gt;</li></ul>"
         "Newsticker extra XML text is escaped for HTML rendering"))

(princ (qa--done))
