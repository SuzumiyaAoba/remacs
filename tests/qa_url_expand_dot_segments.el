;;; Only complete dot segments receive a trailing slash.

;; Load the path normalizer without its unrelated URL dependencies.
(provide 'url-methods)
(provide 'url-util)
(provide 'url-parse)
(require 'url-expand)

(qa (url-expander-remove-relative-links "/a/...") "/a/...")
(qa (url-expander-remove-relative-links "/a/.../") "/a/.../")
(qa (url-expander-remove-relative-links "/a/.") "/a/")
(qa (url-expander-remove-relative-links "/a/..") "/")
(qa (url-expander-remove-relative-links "/a/b/../c") "/a/c")

(princ (qa--done))
