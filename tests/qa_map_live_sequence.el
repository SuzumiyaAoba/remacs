;;; qa_map_live_sequence.el --- Mapping reads current elements between callbacks -*- lexical-binding: t; -*-

(defun qa337-map (operation kind action)
  (let* ((sequence (cond ((eq kind 'list) (list 1 2 3))
                         ((eq kind 'vector) (vector 1 2 3))
                         ((eq kind 'unicode) (copy-sequence "日ab"))
                         ((eq kind 'unibyte) (unibyte-string 255 128 65))
                         ((eq kind 'multibyte-byte) (string-as-multibyte (unibyte-string 255 65 66)))
                         ((eq kind 'bool) (bool-vector t nil t))
                         (t (copy-sequence "abc"))))
         (seen nil)
         (calls 0)
         (callback
          (lambda (value)
            (push value seen)
            (setq calls (1+ calls))
            (cond ((eq action 'future)
                   (cond ((eq kind 'list) (setcar (cdr sequence) 9))
                         ((eq kind 'vector) (aset sequence 1 9))
                         ((eq kind 'bool) (aset sequence 1 t))
                         (t (aset sequence 1 88))))
                  ((and (= calls 1) (eq action 'truncate)) (setcdr sequence nil))
                  ((and (= calls 1) (eq action 'append)) (setcdr (last sequence) (list 4 5)))
                  ((and (= calls 1) (eq action 'cycle)) (setcdr sequence sequence)))
            (cond ((eq operation 'mapconcat) (prin1-to-string value))
                  ((eq operation 'mapcan) (list value))
                  (t value))))
         (result (if (eq operation 'mapconcat)
                     (funcall operation callback sequence ",")
                   (funcall operation callback sequence))))
    (list (if (eq operation 'mapc) (eq result sequence) result)
          (nreverse seen)
          (if (eq action 'cycle) (eq (cdr sequence) sequence)
            (mapcar #'identity sequence)))))
(defun qa337-invalid (operation)
  (let ((calls 0))
    (list (condition-case error
              (if (eq operation 'mapconcat)
                  (funcall operation (lambda (_) (setq calls (1+ calls)) "x") '(1 . 2) ",")
                (funcall operation (lambda (_) (setq calls (1+ calls)) nil) '(1 . 2)))
            (error (car error))) calls)))

(defun qa337-byte-cursor (clear)
  (let ((sequence (if clear (copy-sequence "日ab") (unibyte-string 255 65 66)))
        (calls 0) (seen nil))
    (let ((result
           (mapcar (lambda (value)
                     (push value seen)
                     (setq calls (1+ calls))
                     (when (= calls 1)
                       (if clear
                           (progn (clear-string sequence) (aset sequence 3 88))
                         (aset sequence 0 0) (aset sequence 1 255)))
                     value)
                   sequence)))
      (list result (nreverse seen) (string-to-list sequence)))))

;; mapcar reads changed future list
(qa (qa337-map #'mapcar 'list 'future)
    '((1 9 3) (1 9 3) (1 9 3)))

;; mapcar reads changed future vector
(qa (qa337-map #'mapcar 'vector 'future)
    '((1 9 3) (1 9 3) (1 9 3)))

;; mapcar reads changed future ascii
(qa (qa337-map #'mapcar 'ascii 'future)
    '((97 88 99) (97 88 99) (97 88 99)))

;; mapcar reads changed future unicode
(qa (qa337-map #'mapcar 'unicode 'future)
    '((26085 88 98) (26085 88 98) (26085 88 98)))

;; mapcar reads changed future unibyte
(qa (qa337-map #'mapcar 'unibyte 'future)
    '((255 88 65) (255 88 65) (255 88 65)))

;; mapcar reads changed future bool
(qa (qa337-map #'mapcar 'bool 'future)
    '((t t t) (t t t) (t t t)))

;; mapcar list truncate
(qa (qa337-map #'mapcar 'list 'truncate)
    '((1) (1) (1)))

;; mapcar list append
(qa (qa337-map #'mapcar 'list 'append)
    '((1 2 3) (1 2 3) (1 2 3 4 5)))

;; mapcar list cycle
(qa (qa337-map #'mapcar 'list 'cycle)
    '((1 1 1) (1 1 1) t))

;; mapcar validates initial list before callbacks
(qa (qa337-invalid #'mapcar)
    '(wrong-type-argument 0))

;; mapc reads changed future list
(qa (qa337-map #'mapc 'list 'future)
    '(t (1 9 3) (1 9 3)))

;; mapc reads changed future vector
(qa (qa337-map #'mapc 'vector 'future)
    '(t (1 9 3) (1 9 3)))

;; mapc reads changed future ascii
(qa (qa337-map #'mapc 'ascii 'future)
    '(t (97 88 99) (97 88 99)))

;; mapc reads changed future unicode
(qa (qa337-map #'mapc 'unicode 'future)
    '(t (26085 88 98) (26085 88 98)))

;; mapc reads changed future unibyte
(qa (qa337-map #'mapc 'unibyte 'future)
    '(t (255 88 65) (255 88 65)))

;; mapc reads changed future bool
(qa (qa337-map #'mapc 'bool 'future)
    '(t (t t t) (t t t)))

;; mapc list truncate
(qa (qa337-map #'mapc 'list 'truncate)
    '(t (1) (1)))

;; mapc list append
(qa (qa337-map #'mapc 'list 'append)
    '(t (1 2 3) (1 2 3 4 5)))

;; mapc list cycle
(qa (qa337-map #'mapc 'list 'cycle)
    '(t (1 1 1) t))

;; mapc validates initial list before callbacks
(qa (qa337-invalid #'mapc)
    '(wrong-type-argument 0))

;; mapconcat reads changed future list
(qa (qa337-map #'mapconcat 'list 'future)
    '("1,9,3" (1 9 3) (1 9 3)))

;; mapconcat reads changed future vector
(qa (qa337-map #'mapconcat 'vector 'future)
    '("1,9,3" (1 9 3) (1 9 3)))

;; mapconcat reads changed future ascii
(qa (qa337-map #'mapconcat 'ascii 'future)
    '("97,88,99" (97 88 99) (97 88 99)))

;; mapconcat reads changed future unicode
(qa (qa337-map #'mapconcat 'unicode 'future)
    '("26085,88,98" (26085 88 98) (26085 88 98)))

;; mapconcat reads changed future unibyte
(qa (qa337-map #'mapconcat 'unibyte 'future)
    '("255,88,65" (255 88 65) (255 88 65)))

;; mapconcat reads changed future bool
(qa (qa337-map #'mapconcat 'bool 'future)
    '("t,t,t" (t t t) (t t t)))

;; mapconcat list append
(qa (qa337-map #'mapconcat 'list 'append)
    '("1,2,3" (1 2 3) (1 2 3 4 5)))

;; mapconcat list cycle
(qa (qa337-map #'mapconcat 'list 'cycle)
    '("1,1,1" (1 1 1) t))

;; mapconcat validates initial list before callbacks
(qa (qa337-invalid #'mapconcat)
    '(wrong-type-argument 0))

;; mapcan reads changed future list
(qa (qa337-map #'mapcan 'list 'future)
    '((1 9 3) (1 9 3) (1 9 3)))

;; mapcan reads changed future vector
(qa (qa337-map #'mapcan 'vector 'future)
    '((1 9 3) (1 9 3) (1 9 3)))

;; mapcan reads changed future ascii
(qa (qa337-map #'mapcan 'ascii 'future)
    '((97 88 99) (97 88 99) (97 88 99)))

;; mapcan reads changed future unicode
(qa (qa337-map #'mapcan 'unicode 'future)
    '((26085 88 98) (26085 88 98) (26085 88 98)))

;; mapcan reads changed future unibyte
(qa (qa337-map #'mapcan 'unibyte 'future)
    '((255 88 65) (255 88 65) (255 88 65)))

;; mapcan reads changed future bool
(qa (qa337-map #'mapcan 'bool 'future)
    '((t t t) (t t t) (t t t)))

;; mapcan list truncate
(qa (qa337-map #'mapcan 'list 'truncate)
    '((1) (1) (1)))

;; mapcan list append
(qa (qa337-map #'mapcan 'list 'append)
    '((1 2 3) (1 2 3) (1 2 3 4 5)))

;; mapcan list cycle
(qa (qa337-map #'mapcan 'list 'cycle)
    '((1 1 1) (1 1 1) t))

;; mapcan validates initial list before callbacks
(qa (qa337-invalid #'mapcan)
    '(wrong-type-argument 0))

;; mapcan core arity rejects multiple sequences
(qa (condition-case error (mapcan #'list '(1) '(2)) (error (car error)))
    'wrong-number-of-arguments)

;; mapcan core arity is fixed
(qa (func-arity #'mapcan)
    '(2 . 2))

;; mapcar reads changed future multibyte raw byte string
(qa (qa337-map #'mapcar 'multibyte-byte 'future)
    '((4194303 88 66) (4194303 88 66) (4194303 88 66)))

;; mapc reads changed future multibyte raw byte string
(qa (qa337-map #'mapc 'multibyte-byte 'future)
    '(t (4194303 88 66) (4194303 88 66)))

;; mapconcat reads changed future multibyte raw byte string
(qa (qa337-map #'mapconcat 'multibyte-byte 'future)
    '("4194303,88,66" (4194303 88 66) (4194303 88 66)))

;; mapcan reads changed future multibyte raw byte string
(qa (qa337-map #'mapcan 'multibyte-byte 'future)
    '((4194303 88 66) (4194303 88 66) (4194303 88 66)))

;; unibyte storage proxies may change Rust widths
(qa (qa337-byte-cursor nil)
    '((255 255 66) (255 255 66) (0 255 66)))

;; native byte cursor survives clear-string mode change
(qa (qa337-byte-cursor t)
    '((26085 88 0) (26085 88 0) (0 0 0 88 0)))

(princ (qa--done))
