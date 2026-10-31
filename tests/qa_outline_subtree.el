;;; -*- lexical-binding: t -*-
;; Optional subtree movement counts default to one (#3363).
(require 'outline)

(defconst qa-outline--a "* A\nA body\n** A child\n")
(defconst qa-outline--b "* B\nB body\n** B child\n")
(defconst qa-outline--c "* C\nC body\n** C child\n")

(defun qa-outline--move (function heading &rest args)
  (with-temp-buffer
    (insert qa-outline--a qa-outline--b qa-outline--c)
    (outline-mode)
    (goto-char (point-min))
    (re-search-forward (concat "^\\* " heading "$"))
    (beginning-of-line)
    (apply function args)
    (buffer-string)))

(qa (qa-outline--move #'outline-move-subtree-up "B")
    (concat qa-outline--b qa-outline--a qa-outline--c))
(qa (qa-outline--move #'outline-move-subtree-down "A")
    (concat qa-outline--b qa-outline--a qa-outline--c))
(qa (qa-outline--move #'outline-move-subtree-up "B" nil)
    (concat qa-outline--b qa-outline--a qa-outline--c))
(qa (qa-outline--move #'outline-move-subtree-down "A" nil)
    (concat qa-outline--b qa-outline--a qa-outline--c))

;; Explicit counts retain their direction and carry the entire subtree.
(qa (qa-outline--move #'outline-move-subtree-up "B" 1)
    (concat qa-outline--b qa-outline--a qa-outline--c))
(qa (qa-outline--move #'outline-move-subtree-down "A" 1)
    (concat qa-outline--b qa-outline--a qa-outline--c))
(qa (qa-outline--move #'outline-move-subtree-up "C" 2)
    (concat qa-outline--c qa-outline--a qa-outline--b))
(qa (qa-outline--move #'outline-move-subtree-down "A" 2)
    (concat qa-outline--b qa-outline--c qa-outline--a))
(qa (qa-outline--move #'outline-move-subtree-up "A" -1)
    (concat qa-outline--b qa-outline--a qa-outline--c))
(qa (qa-outline--move #'outline-move-subtree-down "B" -1)
    (concat qa-outline--b qa-outline--a qa-outline--c))

;; Zero remains a no-op, even inside a body in a read-only buffer.
(defun qa-outline--zero (function)
  (with-temp-buffer
    (insert qa-outline--a)
    (outline-mode)
    (goto-char 6)
    (set-buffer-modified-p nil)
    (let ((buffer-read-only t))
      (funcall function 0))
    (list (point) (buffer-modified-p) (buffer-string))))
(qa (qa-outline--zero #'outline-move-subtree-up) (list 6 nil qa-outline--a))
(qa (qa-outline--zero #'outline-move-subtree-down) (list 6 nil qa-outline--a))

(qa-err (qa-outline--move #'outline-move-subtree-up "A") 'user-error)
(qa-err (qa-outline--move #'outline-move-subtree-down "C") 'user-error)
(qa-err (qa-outline--move #'outline-move-subtree-up "B" 'bad)
        'wrong-type-argument)
(qa-err (qa-outline--move #'outline-move-subtree-down "A" 'bad)
        'wrong-type-argument)

(princ (qa--done))
