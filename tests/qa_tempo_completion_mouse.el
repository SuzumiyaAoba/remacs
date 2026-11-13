;;; qa_tempo_completion_mouse.el --- Tempo completion mouse regression -*- lexical-binding:t; -*-

(require 'tempo)

(qa
 (let ((tempo-tags nil)
       (tempo-show-completion-buffer t)
       (tempo-leave-completion-buffer t))
   (with-temp-buffer
     (tempo-define-template "qa2133-first" '("FIRST") "qaztempoaa")
     (tempo-define-template "qa2133-second" '("SECOND") "qaztempoab")
     (insert "qaztempoa")
     (tempo-complete-tag)
     (let* ((source (current-buffer))
            (completion-buffer (get-buffer "*Completions*"))
            (base-position
             (with-current-buffer completion-buffer
               completion-base-position))
            (insert-function
             (with-current-buffer completion-buffer
               completion-list-insert-choice-function))
            (reference-buffer
             (with-current-buffer completion-buffer
               completion-reference-buffer))
            (completion-window
             (or (get-buffer-window completion-buffer t)
                 (selected-window)))
            (candidate-position
             (with-current-buffer completion-buffer
               (save-excursion
                 (goto-char (point-min))
                 (search-forward "qaztempoab")
                 (match-beginning 0))))
            (base-matches-source
             (with-current-buffer source
               (equal base-position (list (point-min) (point-max))))))
       (set-window-buffer completion-window completion-buffer)
       (with-current-buffer completion-buffer
         (choose-completion
          (list 'mouse-2
                (list completion-window candidate-position '(0 . 0) 0))
          t t))
       (with-current-buffer source
         (list (eq reference-buffer source)
               base-matches-source
               (eq insert-function #'tempo-completion-list-insert-choice)
               (buffer-string))))))
 '(t t t "SECOND"))

(princ (qa--done))
