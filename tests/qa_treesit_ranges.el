;;; -*- lexical-binding: t; -*-
(defvar treesit-extra-load-path nil)
(setq treesit-extra-load-path
      (cons (expand-file-name "~/.emacs.d/tree-sitter") treesit-extra-load-path))

(defun qa216-result (ranges &optional narrow)
  (with-temp-buffer
    (insert "[1,2]")
    (when narrow (narrow-to-region (car narrow) (cdr narrow)))
    (let ((parser (treesit-parser-create 'json)))
      (unwind-protect
          (condition-case err
              (list 'ok (treesit-parser-set-included-ranges parser ranges))
            (error err))
        (treesit-parser-delete parser)))))

(defun qa216-identity (ranges)
  (let ((err (qa216-result ranges)))
    (list (car err) (cadr err) (eq (nth 2 err) ranges) (length err))))

(defun qa216-types ()
  (with-temp-buffer
    (insert "[1,2]")
    (let ((parser (treesit-parser-create 'json)))
      (unwind-protect
          (mapcar
           (lambda (value)
             (mapcar
              (lambda (range)
                (condition-case err
                    (list 'accepted (treesit-parser-set-included-ranges parser (list range)))
                  (error (list (car err) (cadr err) (equal (nth 2 err) value)))))
              (list (cons value 3) (cons 2 value))))
           (list nil t 'x 1.5 "1" [1] '(1) (point-marker) (expt 2 80)))
        (treesit-parser-delete parser)))))

(if (and (fboundp 'treesit-language-available-p) (treesit-language-available-p 'json))
    (progn
      (qa (qa216-result '((0 . 99)))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((0 . 99))))
      (qa (qa216-result '((0 . 3)))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((0 . 3))))
      (qa (qa216-result '((2 . 7)))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((2 . 7))))
      (qa (qa216-result '((4 . 2)))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((4 . 2))))
      (qa (qa216-result '((2 . 4) (3 . 5)))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((2 . 4) (3 . 5))))
      (qa (qa216-result '((4 . 5) (2 . 3) (5 . 6)))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((4 . 5) (2 . 3) (5 . 6))))
      (qa (qa216-identity (list (cons 0 99)))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" t 3))
      (qa (qa216-identity (list (cons 2 4) (cons 3 5) (cons 5 6)))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" t 3))
      (qa (qa216-result '((1 . 3)) '(2 . 5))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((1 . 3))))
      (qa (qa216-result '((2 . 6)) '(2 . 5))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((2 . 6))))
      ;; Validate each range before moving to its tail. Dotted-list
      ;; failures carry the original list, rather than its final atom.
      (qa (qa216-result '((2 . 3) . 7)) '(wrong-type-argument listp ((2 . 3) . 7)))
      (qa (qa216-result '((0 . 99) . 7))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((0 . 99) . 7)))
      (qa (qa216-result '((0 . 99) 42))
          '(treesit-range-invalid "RANGE is either overlapping, out-of-order or out-of-range" ((0 . 99) 42)))
      (qa (qa216-result '((2 . 3) 42)) '(wrong-type-argument consp 42))
      (qa (qa216-result 42) '(wrong-type-argument consp 42))
      (qa (qa216-result '(42)) '(wrong-type-argument consp 42))
      (qa (qa216-result '((2 3))) '(wrong-type-argument fixnump (3)))
      (qa (qa216-types)
          '(((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))
            ((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))
            ((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))
            ((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))
            ((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))
            ((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))
            ((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))
            ((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))
            ((wrong-type-argument fixnump t) (wrong-type-argument fixnump t))))
      (qa (qa216-result nil) '(ok nil))
      (qa (qa216-result '((2 . 3) (3 . 4))) '(ok nil))
      (qa (qa216-result '((2 . 2))) '(ok nil))
      (qa (qa216-result '((2 . 2) (2 . 2))) '(ok nil))
      (qa (qa216-result '((2 . 5)) '(2 . 5)) '(ok nil))
      (qa (with-temp-buffer
            (insert "[1,2]")
            (let* ((parser (treesit-parser-create 'json)) (ranges (list (cons 2 2))))
              (setcdr ranges ranges)
              (unwind-protect
                  (condition-case err (treesit-parser-set-included-ranges parser ranges)
                    (error (car err)))
                (treesit-parser-delete parser))))
          'circular-list)
      (qa (with-temp-buffer
            (insert "[1,2]")
            (let* ((parser (treesit-parser-create 'json)) (ranges (list (cons 2 3))))
              (unwind-protect
                  (progn
                    (treesit-parser-set-included-ranges parser ranges)
                    (let* ((old (treesit-parser-root-node parser))
                           (err (condition-case err
                                    (treesit-parser-set-included-ranges parser '((0 . 99)))
                                  (error (car err)))))
                      (list err (eq ranges (treesit-parser-included-ranges parser))
                            (treesit-node-check old 'outdated)
                            (treesit-node-eq old (treesit-parser-root-node parser)))))
                (treesit-parser-delete parser))))
          '(treesit-range-invalid t nil t)))
  (princ "SKIP qa_treesit_ranges: JSON grammar unavailable\n"))
(princ (qa--done)) (terpri)
