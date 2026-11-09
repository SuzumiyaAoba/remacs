;;; -*- lexical-binding: nil; -*-
;; A single wide path component must still fit the requested display width.
(require 'cl-lib)
(provide 'url-parse)
(cl-defstruct (qa-url (:constructor qa-url--create) (:conc-name url-))
  filename)
(require 'url-util)
(qa
 (let ((url "http://e/ああああ"))
   (cl-letf (((symbol-function 'url-generic-parse-url)
              (lambda (_url) (qa-url--create :filename "/ああああ")))
             ((symbol-function 'url-recreate-url)
              (lambda (_urlobj) url)))
     (string-width (url-truncate-url-for-viewing url 15))))
 15)
(princ (qa--done))
