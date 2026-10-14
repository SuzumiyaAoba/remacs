;;; -*- lexical-binding: nil -*-
;; Issue #41: `i' is a valid ignored-argument control code, not parser leniency.
(defun qa41-direct (spec)
 (call-interactively (eval (list 'lambda '(&rest args) (list 'interactive spec) 'args) t)))
(dolist (prefix '(nil 7 -3 (4) -))
 (let ((current-prefix-arg prefix))
  (dolist (spec '("i" "iP" "ii" "i~" "iIgnored prompt" "i😀" "iPp" "-i"))
   (qa (advice-eval-interactive-spec spec) '(nil))
   (qa (qa41-direct spec) '(nil)))
  (qa (advice-eval-interactive-spec "i\nP") (list nil prefix))
  (qa (qa41-direct "i\nP") (list nil prefix))
  (qa (advice-eval-interactive-spec "i\np\nP") (list nil (prefix-numeric-value prefix) prefix))
  (qa (qa41-direct "i\ni") '(nil nil))))
;; Prompt formatting still happens even though the argument ignores its prompt.
(qa-err-data (advice-eval-interactive-spec "i%s") 'error '("Not enough arguments for format string"))
(qa-err-data (qa41-direct "i%s") 'error '("Not enough arguments for format string"))
(qa (let ((current-prefix-arg 8)) (advice-eval-interactive-spec "i\nP%S")) '(nil 8))
(qa (qa41-direct "i\ni%S") '(nil nil))
;; Genuine unknown letters still fail at the start of a new spec line.
(dolist (spec '("~" "I" "?" "i\n~" "i\nI"))
 (qa (condition-case e (advice-eval-interactive-spec spec) (error (car e))) 'error)
 (qa (condition-case e (qa41-direct spec) (error (car e))) 'error))
(qa-err-data (advice-eval-interactive-spec "~") 'error
 '("Invalid control letter ‘~’ (#o176, #x007e) in interactive calling string"))
(qa-err-data (advice-eval-interactive-spec "i\nI") 'error
 '("Invalid control letter ‘I’ (#o111, #x0049) in interactive calling string"))
;; Ordinary prefixes and non-string specs keep their existing semantics.
(qa (with-temp-buffer (qa41-direct "*i")) '(nil))
(qa (with-temp-buffer (setq buffer-read-only t) (qa41-direct "i")) '(nil))
(qa-err (with-temp-buffer (setq buffer-read-only t) (qa41-direct "*i")) 'buffer-read-only)
(qa (advice-eval-interactive-spec '(list 1 2)) '(1 2))
(qa (advice-eval-interactive-spec nil) nil)
(defun qa41-command (ignored) (interactive "iP") (list ignored))
(defun qa41-around (fn &rest args) (apply fn args))
(unwind-protect
 (progn (advice-add 'qa41-command :around #'qa41-around)
        (qa (let ((current-prefix-arg 9)) (call-interactively #'qa41-command)) '(nil)))
 (advice-remove 'qa41-command #'qa41-around))
(princ (qa--done)) (terpri)
