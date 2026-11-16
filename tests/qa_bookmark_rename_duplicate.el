;;; qa_bookmark_rename_duplicate.el --- bookmark rename regression -*- lexical-binding: t -*-

(require 'bookmark)

(if qa--remacs
    (qa (let ((bookmark-save-flag nil)
              (bookmark-alist
               (list (list "a" (cons 'filename "/tmp/a"))
                     (list "b" (cons 'filename "/tmp/b")))))
          (condition-case nil
              (bookmark-rename "a" "b")
            (error nil))
          (list (bookmark-all-names) (bookmark-get-filename "b")))
        '(("a" "b") "/tmp/b"))
  ;; GNU 31.1 currently permits the duplicate and hides the original "b".
  (qa-xf (let ((bookmark-save-flag nil)
               (bookmark-alist
                (list (list "a" (cons 'filename "/tmp/a"))
                      (list "b" (cons 'filename "/tmp/b")))))
           (condition-case nil
               (bookmark-rename "a" "b")
             (error nil))
           (list (bookmark-all-names) (bookmark-get-filename "b")))
         '(("a" "b") "/tmp/b")
         "Remacs rejects a rename that would hide an existing bookmark"))

(princ (qa--done))
