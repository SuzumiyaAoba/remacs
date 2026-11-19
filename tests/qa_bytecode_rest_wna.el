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

;; A packed descriptor's upper field is authoritative even when it is
;; below the required count; GNU rejects this one-argument call as (1 . 0).
(qa (let* ((narrow (make-byte-code 1 (unibyte-string 135) [] 1))
           (rest (make-byte-code 129 (unibyte-string 135) [] 1))
           (fixed (make-byte-code 257 (unibyte-string 135) [] 1)))
      (list (func-arity narrow)
            (condition-case err
                (funcall narrow 'argument)
              (wrong-number-of-arguments
               (list (car err) (cadr err))))
            (funcall rest 'argument)
            (funcall fixed 'argument)))
    '((1 . 0) (wrong-number-of-arguments (1 . 0)) (argument) argument))

;; GNU shifts the full non-negative descriptor for its maximum argument
;; count, even when it exceeds the nominal seven-bit field.
(qa (let ((function (make-byte-code 32768 (unibyte-string 135) [] 256)))
      (list (func-arity function) (funcall function)))
    '((0 . 128) nil))

(princ (qa--done)) (terpri)
