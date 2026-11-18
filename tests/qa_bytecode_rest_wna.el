;;; -*- lexical-binding: nil; -*-
;; WNA data reports the raw upper argument-count field, not the decoded
;; positional maximum: negative and positive rest descriptors differ.
(qa (func-arity (make-byte-code -1 (unibyte-string 192 135) [ok] 1))
    '(127 . many))

(qa (mapcar
     (lambda (descriptor)
       (let ((function (make-byte-code descriptor
                                       (unibyte-string 192 135) [ok] 1)))
         (condition-case err (funcall function)
           (wrong-number-of-arguments (cadr err)))))
     '(-1 1 129 257))
    '((127 . -1) (1 . 0) (1 . 0) (1 . 1)))

(princ (qa--done)) (terpri)
