;;; -*- lexical-binding: t -*-
;; Repeated MANPATH entries retain their first occurrence (#3362).
(require 'woman)

(defun qa-woman--path-order (level order)
  (let ((root (make-temp-file "remacs-woman-path-" t)))
    (unwind-protect
        (progn
          (dolist (name '("A" "B" "C"))
            (let ((directory (expand-file-name name root)))
              (make-directory directory)
              (with-temp-file (expand-file-name "foo.1" directory)
                (insert ".TH foo 1\n"))))
          (let* ((woman-cache-level level)
                 (woman-expanded-directory-path
                  (mapcar (lambda (name) (expand-file-name name root)) order))
                 (woman-topic-all-completions
                  (woman-topic-all-completions woman-expanded-directory-path)))
            (mapcar
             (lambda (entry)
               (file-name-nondirectory
                (directory-file-name (file-name-directory (car entry)))))
             (woman-file-name-all-completions "foo"))))
      (delete-directory root t))))

;; Level 1 scans the path at lookup time; levels 2 and 3 cache path indices.
(defun qa-woman--all-cache-levels (order)
  (mapcar (lambda (level) (qa-woman--path-order level order)) '(1 2 3)))

(qa (qa-woman--all-cache-levels '("A" "B"))
    '(("A" "B") ("A" "B") ("A" "B")))
(qa (qa-woman--all-cache-levels '("A" "B" "A"))
    '(("A" "B") ("A" "B") ("A" "B")))
(qa (qa-woman--all-cache-levels '("A" "B" "B"))
    '(("A" "B") ("A" "B") ("A" "B")))
(qa (qa-woman--all-cache-levels '("B" "A" "B"))
    '(("B" "A") ("B" "A") ("B" "A")))
(qa (qa-woman--all-cache-levels '("A" "B" "A" "C" "B" "A"))
    '(("A" "B" "C") ("A" "B" "C") ("A" "B" "C")))
(qa (qa-woman--all-cache-levels '("A" "A"))
    '(("A") ("A") ("A")))
(qa (qa-woman--all-cache-levels nil) '(nil nil nil))

(princ (qa--done))
