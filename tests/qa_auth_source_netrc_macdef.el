;;; qa_auth_source_netrc_macdef.el --- auth-source macdef regression -*- lexical-binding: t -*-

(require 'auth-source)

(defconst qa-auth-source-netrc-macdef-contents
  (concat "machine host.example login alice password good\n"
          "macdef init\n"
          "machine fake.example login evil password bad\n"
          "\n"
          "macdef empty\n"
          "\n"
          "machine keep.example login bob password better\n"))

(if qa--remacs
    (qa (with-temp-buffer
          (insert qa-auth-source-netrc-macdef-contents)
          (goto-char (point-min))
          (let ((entries (auth-source-netrc-parse-entries
                          (lambda (_entry) t) 10)))
            (list (mapcar (lambda (entry)
                            (auth-source--aget entry "machine"))
                          entries)
                  (auth-source--aget (car entries) "login")
                  (auth-source--aget (car entries) "password"))))
        '(("host.example" "keep.example") "alice" "good"))
  ;; GNU 31.1 currently interprets tokens inside macdef bodies as credentials.
  (qa-xf (with-temp-buffer
           (insert qa-auth-source-netrc-macdef-contents)
           (goto-char (point-min))
           (let ((entries (auth-source-netrc-parse-entries
                           (lambda (_entry) t) 10)))
             (list (mapcar (lambda (entry)
                             (auth-source--aget entry "machine"))
                           entries)
                   (auth-source--aget (car entries) "login")
                   (auth-source--aget (car entries) "password"))))
         '(("host.example" "keep.example") "alice" "good")
         "auth-source ignores complete netrc macdef bodies"))

(princ (qa--done))
