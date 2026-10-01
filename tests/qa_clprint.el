;; cl-print.el — GNU 31.1 pins (verified).
;;
;; GNU truths pinned:
;;   - cl-print-object prints a Lisp object to a stream/buffer.
;;   - cl-print-object returns the object itself.
;;   - cl-print-object-contents / cl-print-expand-ellipsis /
;;     cl-print-to-string-with-limit are real fns.
;;   - cl-print-to-string is NOT a fn.

(require 'cl-print)

;; == surface ==
(qa (fboundp 'cl-print-object) t)
(qa (fboundp 'cl-print-to-string-with-limit) t)
(qa (fboundp 'cl-print-object-contents) t)
(qa (fboundp 'cl-print-expand-ellipsis) t)
;; NOT fns
(qa (fboundp 'cl-print-to-string) nil)
(qa (fboundp 'cl-print-compiled) nil)
(qa (fboundp 'cl-print-compiled-button) nil)
(qa (fboundp 'cl-print-struct) nil)
(qa (fboundp 'cl-print-vector) nil)
(qa (fboundp 'cl-print-object-tag) nil)
(qa (fboundp 'cl-print-find-generic-method) nil)

;; == cl-print-object — prints to stream/buffer ==
(let ((v (with-temp-buffer
           (cl-print-object '(a b) (current-buffer))
           (buffer-string))))
  (qa v "(a b)"))

(let ((v (with-temp-buffer
           (cl-print-object "hi" (current-buffer))
           (buffer-string))))
  (qa v "\"hi\""))

(let ((v (with-temp-buffer
           (cl-print-object [1 2 3] (current-buffer))
           (buffer-string))))
  (qa v "[1 2 3]"))

;; == cl-print-object returns the object ==
(let ((result (with-temp-buffer
                (cl-print-object 42 (current-buffer)))))
  (qa result 42))

;; == on a list ==
(let ((v (with-temp-buffer
           (cl-print-object '(1 (2 3) 4) (current-buffer))
           (buffer-string))))
  (qa v "(1 (2 3) 4)"))

;; == on a hash-table — printed form ==
(let ((h (make-hash-table :test 'equal)))
  (puthash 'k 'v h)
  (let ((out (with-temp-buffer
               (cl-print-object h (current-buffer))
               (buffer-string))))
    ;; #<hash-table equal 1/6 0xADDR ...>
    (qa (string-match-p "hash-table" out) 2)))

;; == via standard-output ==
(let ((out (with-output-to-string
             (cl-print-object '(x y) standard-output))))
  (qa out "(x y)"))

;; == on a record ==
(let ((r (make-record 'tag 3 nil))
      (out nil))
  (setq out (with-temp-buffer
              (cl-print-object r (current-buffer))
              (buffer-string)))
  (qa (stringp out) t))

;; == on a cons ==
(let ((v (with-temp-buffer
           (cl-print-object '(a . b) (current-buffer))
           (buffer-string))))
  (qa v "(a . b)"))

;; == symbol prints as name ==
(let ((v (with-temp-buffer
           (cl-print-object 'sym (current-buffer))
           (buffer-string))))
  (qa v "sym"))

;; == nil prints as nil ==
(let ((v (with-temp-buffer
           (cl-print-object nil (current-buffer))
           (buffer-string))))
  (qa v "nil"))

;; == t prints as t ==
(let ((v (with-temp-buffer
           (cl-print-object t (current-buffer))
           (buffer-string))))
  (qa v "t"))

;; == deep nesting ==
(let ((v (with-temp-buffer
           (cl-print-object '((a b) ((c d))) (current-buffer))
           (buffer-string))))
  (qa v "((a b) ((c d)))"))

;; == keywords ==
(let ((v (with-temp-buffer
           (cl-print-object :kw (current-buffer))
           (buffer-string))))
  (qa v ":kw"))

;; == cl-print-to-string-with-limit — returns a string ==
(let ((s (cl-print-to-string-with-limit #'cl-print-object '(a b) 100)))
  (qa s "(a b)"))

;; == ellipsis with a small limit ==
(let ((s (cl-print-to-string-with-limit #'cl-print-object '(1 2 3 4 5) 3)))
  ;; gets truncated
  (qa (stringp s) t))

(princ (qa--done))
