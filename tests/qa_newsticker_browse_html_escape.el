;;; qa_newsticker_browse_html_escape.el --- newsticker HTML escaping regression -*- lexical-binding: t -*-

(require 'cl-lib)
(require 'newst-reader)

(defun qa-newsticker-browse-html-heading ()
  (let ((feed "<script>feed()</script>")
        (title "<script>title()</script>")
        temp-file
        html)
    (unwind-protect
        (progn
          (cl-letf (((symbol-function 'newsticker--title)
                     (lambda (_item) title))
                    ((symbol-function 'newsticker--time)
                     (lambda (_item) (current-time)))
                    ((symbol-function 'newsticker--desc)
                     (lambda (_item) nil))
                    ((symbol-function 'newsticker--enclosure)
                     (lambda (_item) nil))
                    ((symbol-function 'newsticker--extra)
                     (lambda (_item) nil))
                    ((symbol-function 'browse-url)
                     (lambda (file)
                       (setq temp-file file)
                       (with-temp-buffer
                         (insert-file-contents file)
                         (buffer-string)))))
            (setq html (newsticker-browse-url-item feed 'dummy)))
          (let ((expected-heading
                 (concat "<h1>" (xml-escape-string feed) ": "
                         (xml-escape-string title) "</h1>")))
            (list (and (string-match-p (regexp-quote expected-heading) html) t)
                  (not (string-match-p (regexp-quote "<h1><script>") html)))))
      (when (and temp-file (file-exists-p temp-file))
        (delete-file temp-file)))))

(if qa--remacs
    (qa (qa-newsticker-browse-html-heading) '(t t))
  ;; GNU 31.1 inserts feed and item titles into the XHTML heading unescaped.
  (qa-xf (qa-newsticker-browse-html-heading) '(t t)
         "newsticker-browse-url-item escapes feed and item titles"))

(princ (qa--done))
