;;; -*- lexical-binding: t -*-
;; Bound unmatched output while preserving prompts split across chunks (#3313).
(require 'dirtrack)

(defun qa-dirtrack--pending-result (chunks limit &optional regexp)
  (let ((original (symbol-function 'dirtrack--change-directory))
        seen
        (unchanged t))
    (unwind-protect
        (progn
          (fset 'dirtrack--change-directory
                (lambda (path _input) (push path seen)))
          (with-temp-buffer
            (insert "x")
            (setq-local dirtrack-mode t
                        dirtrack-list (list (or regexp "^P\\([^>]+\\)> ") 1)
                        dirtrack-max-prompt-length limit
                        dirtrack--pending-input "")
            (dolist (chunk chunks)
              (unless (eq (dirtrack chunk) chunk) (setq unchanged nil)))
            (list (nreverse seen) (length dirtrack--pending-input) unchanged)))
      (fset 'dirtrack--change-directory original))))

;; Include a single context character beyond the configurable prompt limit.
(qa (qa-dirtrack--pending-result (make-list 100 (make-string 1000 ?x)) 32)
    '(nil 33 t))
(qa (qa-dirtrack--pending-result (list (concat "P/a/> " (make-string 10000 ?x))) 32)
    '(("/a/") 33 t))
(qa (qa-dirtrack--pending-result
     (list (make-string 1000 ?x) "\nP/" "split/" "> ") 32)
    '(("/split/") 0 t))
(qa (qa-dirtrack--pending-result
     (list (make-string 1000 ?x) "P/a/> ") 32)
    '(("/a/") 0 t))
(qa (qa-dirtrack--pending-result '("P/a/> \nP/b/> \nP/c/> ") 32)
    '(("/a/" "/b/" "/c/") 0 t))
(qa (qa-dirtrack--pending-result
     (list (make-string 1000 ?x) "\nPREFIX\n/" "deep/> ")
     32 "^PREFIX\n\\([^>]+\\)> ")
    '(("/deep/") 0 t))

;; A complete prompt in one chunk is searched before retaining its suffix.
(qa (qa-dirtrack--pending-result
     (list (concat "P/" (make-string 100 ?a) "/> ")) 8)
    (list (list (concat "/" (make-string 100 ?a) "/")) 0 t))
(qa (qa-dirtrack--pending-result
     (list (make-string 1000 ?x) "\nP/a/> ") 0)
    '(("/a/") 0 t))
(qa (qa-dirtrack--pending-result '("P/" "split/> ") 0)
    '(nil 0 t))

;; Truncation must not turn a middle-of-line suffix into a fake ^ prompt.
(qa (qa-dirtrack--pending-result
     (list (concat (make-string 100 ?x) "PROMPT /x") "> ")
     8 "^PROMPT \\([^>]+\\)> ")
    '(nil 9 t))
(qa (let ((suffix "PROMPT /fake/> \nPROMPT /real/"))
      (qa-dirtrack--pending-result
       (list (concat (make-string 100 ?x) suffix) "> ")
       (1- (length suffix)) "^PROMPT \\([^>]+\\)> "))
    '(("/real/") 0 t))

(qa (with-temp-buffer
      (insert "x")
      (setq-local dirtrack-list '("UNMATCHED" 0)
                  dirtrack-max-prompt-length 8)
      (dirtrack-mode 1)
      (dirtrack (make-string 100 ?x))
      (dirtrack-mode -1)
      (let ((off (list dirtrack--pending-input dirtrack--pending-input-start)))
        (dirtrack-mode 1)
        (list off dirtrack--pending-input dirtrack--pending-input-start)))
    '((nil 0) "" 0))

(princ (qa--done))
